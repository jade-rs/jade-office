"""Archive Word-version evidence; explicitly promote only fully matching fixtures."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess


def archive(manifest, evidence_root, strict):
    manifest, evidence_root = Path(manifest), Path(evidence_root)
    capture = json.loads(manifest.read_text(encoding='utf-8-sig'))
    if capture.get('status') != 'complete' or capture.get('schema') != 2:
        raise ValueError('Capture must be complete')
    version, capture_id = capture['word']['product_version'], capture['capture_id']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('Complete Word product version required')
    if not re.fullmatch(r'[0-9]{8}T[0-9]{9}Z-[a-f0-9]{8}', capture_id):
        raise ValueError('Invalid capture ID')
    source = manifest.parent
    reference = json.loads((source/'word-reference.json').read_text(encoding='utf-8'))
    if reference['word_product_version'] != version or reference['capture_id'] != capture_id:
        raise ValueError('Word reference belongs to a different version/capture')
    comparison = json.loads((source/'comparison.json').read_text(encoding='utf-8'))
    by_name = {r['fixture']: r for r in comparison}
    if len(by_name) != len(comparison) or set(by_name) != {item['name'] for item in capture['documents']}:
        raise ValueError('Incomplete or duplicate native comparison')
    for item in capture['documents']:
        if by_name[item['name']]['pdf_sha256'] != item['pdf_sha256']:
            raise ValueError('Comparison belongs to a different Word PDF')
    passed = {name for name, r in by_name.items() if r['word_pages'] == r['native_pages']
              and r['word_glyphs'] == r['native_glyphs'] == r['matched'] == r['within_0_375pt']}
    if not set(strict) <= passed:
        raise ValueError(f'Refusing to promote mismatching fixtures: {set(strict)-passed}')
    for item in capture['documents']:
        pdf_name = item['pdf']
        if '/' in pdf_name or '\\' in pdf_name:
            raise ValueError('PDF must belong to the capture directory')
        if hashlib.sha256((source/pdf_name).read_bytes()).hexdigest() != item['pdf_sha256']:
            raise ValueError(f'Captured PDF changed: {pdf_name}')
        if hashlib.sha256(Path(item['source']).read_bytes()).hexdigest() != item['sha256']:
            raise ValueError(f'Source DOCX changed: {item["name"]}')
    relative = Path(f'word-{version}')/capture_id
    target = evidence_root/relative
    if target.exists():
        raise FileExistsError(f'Refusing to overwrite archived evidence: {target}')
    target.mkdir(parents=True)
    for name in ['capture.json', 'word-reference.json', 'comparison.json']:
        shutil.copyfile(source/name, target/name)
    for item in capture['documents']:
        shutil.copyfile(source/item['pdf'], target/item['pdf'])
    core = Path(__file__).resolve().parents[1]/'jade-core'
    engine = subprocess.check_output(['git', '-C', str(core), 'rev-parse', 'HEAD'], text=True).strip()
    engine_dirty = bool(subprocess.check_output(['git', '-C', str(core), 'status', '--porcelain', '--', 'crates'], text=True).strip())
    verification = dict(engine_commit=engine, word_product_version=version, capture_id=capture_id,
                        engine_source_worktree_dirty=engine_dirty,
                        strict_fixtures=list(strict), matching_fixtures=sorted(passed),
                        pending_fixtures=sorted(set(by_name)-passed))
    (target/'verification.json').write_text(json.dumps(verification, indent=2)+'\n', encoding='utf-8')
    if strict:
        selected = dict(reference, fixtures=[f for f in reference['fixtures'] if f['name'] in strict])
        if len(selected['fixtures']) != len(set(strict)):
            raise ValueError('Missing or duplicate strict fixture')
        (target/'strict.json').write_text(json.dumps(selected, ensure_ascii=False, indent=1)+'\n', encoding='utf-8')
        index_path = evidence_root/'index.json'
        index = json.loads(index_path.read_text(encoding='utf-8')) if index_path.exists() else dict(schema=1, references=[])
        index['references'].append(dict(word_product_version=version, directory=relative.as_posix(), reference='strict.json',
                                        fixture_count=len(selected['fixtures']),
                                        glyph_count=sum(len(f['glyphs']) for f in selected['fixtures']),
                                        baseline_line_count=sum(36 for f in selected['fixtures'] if f['name'].startswith('word-windows-controlled-'))))
        temp = index_path.with_suffix('.json.tmp')
        temp.write_text(json.dumps(index, indent=2)+'\n', encoding='utf-8')
        temp.replace(index_path)
    return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--evidence-root', type=Path, default=Path('jade-core/tests/word-oracle/windows'))
    parser.add_argument('--strict', nargs='*', default=[])
    args = parser.parse_args()
    print(archive(args.manifest, args.evidence_root, args.strict))
