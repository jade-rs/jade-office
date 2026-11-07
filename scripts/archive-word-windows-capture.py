"""Archive Word-version evidence; explicitly promote only fully matching fixtures."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import subprocess


def extract_emf_capture(manifest):
    script = Path(__file__).resolve().parents[1]/'jade-core/scripts/word-oracle/extract_windows_emf.py'
    spec = importlib.util.spec_from_file_location('word_emf_capture', script)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.extract(manifest)


def archive(manifest, evidence_root, strict, reference_kind='pdf'):
    if reference_kind not in ('pdf', 'emf'):
        raise ValueError('Unknown Word reference kind')
    reference_name = 'word-reference.json' if reference_kind == 'pdf' else 'word-emf-reference.json'
    comparison_name = 'comparison.json' if reference_kind == 'pdf' else 'emf-comparison.json'
    strict_name = 'strict.json' if reference_kind == 'pdf' else 'strict-emf.json'
    # Resolve before appending long fixture names: Win32 still caps relative
    # paths containing parent segments even when their absolute path is valid.
    manifest, evidence_root = Path(manifest).resolve(), Path(evidence_root).resolve()
    capture = json.loads(manifest.read_text(encoding='utf-8-sig'))
    if capture.get('status') != 'complete' or capture.get('schema') != 2:
        raise ValueError('Capture must be complete')
    version, capture_id = capture['word']['product_version'], capture['capture_id']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('Complete Word product version required')
    if not re.fullmatch(r'[0-9]{8}T[0-9]{9}Z-[a-f0-9]{8}', capture_id):
        raise ValueError('Invalid capture ID')
    source = manifest.parent
    reference = json.loads((source/reference_name).read_text(encoding='utf-8'))
    if reference['word_product_version'] != version or reference['capture_id'] != capture_id:
        raise ValueError('Word reference belongs to a different version/capture')
    comparison = json.loads((source/comparison_name).read_text(encoding='utf-8'))
    by_name = {r['fixture']: r for r in comparison}
    if len(by_name) != len(comparison) or set(by_name) != {item['name'] for item in capture['documents']}:
        raise ValueError('Incomplete or duplicate native comparison')
    for item in capture['documents']:
        if by_name[item['name']]['pdf_sha256'] != item['pdf_sha256']:
            raise ValueError('Comparison belongs to a different Word PDF')
        if reference_kind == 'emf' and by_name[item['name']].get('metafile_sha256') != [p['sha256'] for p in item.get('metafiles', [])]:
            raise ValueError('Comparison belongs to different Word metafiles')
    passed = {name for name, r in by_name.items() if r['word_pages'] == r['native_pages']
              and r['word_glyphs'] == r['native_glyphs'] == r['matched'] == r['within_0_375pt']}
    if not set(strict) <= passed:
        raise ValueError(f'Refusing to promote mismatching fixtures: {set(strict)-passed}')
    for item in capture['documents']:
        name = item['name']
        if name in ('', '.', '..') or any(ch in name for ch in '/\\:'):
            raise ValueError('Fixture name must be a plain filename stem')
        pdf_name = item['pdf']
        if '/' in pdf_name or '\\' in pdf_name:
            raise ValueError('PDF must belong to the capture directory')
        if hashlib.sha256((source/pdf_name).read_bytes()).hexdigest() != item['pdf_sha256']:
            raise ValueError(f'Captured PDF changed: {pdf_name}')
        if hashlib.sha256(Path(item['source']).read_bytes()).hexdigest() != item['sha256']:
            raise ValueError(f'Source DOCX changed: {item["name"]}')
        for page in item.get('metafiles', []):
            name = page['file']
            if '/' in name or '\\' in name or name in ('', '.', '..'):
                raise ValueError('Metafile must belong to the capture directory')
            if hashlib.sha256((source/name).read_bytes()).hexdigest() != page['sha256']:
                raise ValueError(f'Captured metafile changed: {name}')
    if reference_kind == 'emf' and extract_emf_capture(manifest) != reference:
        raise ValueError('Drawing reference differs from raw Word metafiles')
    relative = Path(f'word-{version}')/capture_id
    target = evidence_root/relative
    if target.exists():
        raise FileExistsError(f'Refusing to overwrite archived evidence: {target}')
    target.mkdir(parents=True)
    for name in ['capture.json', reference_name, comparison_name]:
        shutil.copyfile(source/name, target/name)
    for name in ['word-reference.json', 'comparison.json', 'word-emf-reference.json', 'emf-comparison.json']:
        if name not in (reference_name, comparison_name) and (source/name).exists():
            shutil.copyfile(source/name, target/name)
    for item in capture['documents']:
        shutil.copyfile(source/item['pdf'], target/item['pdf'])
        for page in item.get('metafiles', []):
            shutil.copyfile(source/page['file'], target/page['file'])
    # A failed probe may not yet belong to the public fixture corpus. Keep its
    # actual input bytes with this versioned capture so later cleanup or edits
    # to the working source cannot make the original coordinates irreproducible.
    input_root = target/'inputs'
    input_root.mkdir()
    inputs = []
    for item in capture['documents']:
        filename = item['name']+'.docx'
        destination = input_root/filename
        shutil.copyfile(Path(item['source']), destination)
        if hashlib.sha256(destination.read_bytes()).hexdigest() != item['sha256']:
            raise ValueError(f'Source DOCX changed while archiving: {item["name"]}')
        inputs.append(dict(name=item['name'], file='inputs/'+filename, sha256=item['sha256']))
    (target/'inputs.json').write_bytes((json.dumps(dict(schema=1, documents=inputs), indent=2)+'\n').encode('utf-8'))
    core = Path(__file__).resolve().parents[1]/'jade-core'
    engine = subprocess.check_output(['git', '-C', str(core), 'rev-parse', 'HEAD'], text=True).strip()
    engine_dirty = bool(subprocess.check_output(['git', '-C', str(core), 'status', '--porcelain', '--', 'crates'], text=True).strip())
    verification = dict(engine_commit=engine, word_product_version=version, capture_id=capture_id,
                        reference_kind=reference_kind,
                        engine_source_worktree_dirty=engine_dirty,
                        strict_fixtures=list(strict), matching_fixtures=sorted(passed),
                        pending_fixtures=sorted(set(by_name)-passed))
    (target/'verification.json').write_bytes((json.dumps(verification, indent=2)+'\n').encode('utf-8'))
    if strict:
        selected = dict(reference, fixtures=[f for f in reference['fixtures'] if f['name'] in strict])
        if len(selected['fixtures']) != len(set(strict)):
            raise ValueError('Missing or duplicate strict fixture')
        (target/strict_name).write_bytes((json.dumps(selected, ensure_ascii=False, indent=1)+'\n').encode('utf-8'))
        index_path = evidence_root/'index.json'
        index = json.loads(index_path.read_text(encoding='utf-8')) if index_path.exists() else dict(schema=1, references=[])
        index['references'].append(dict(word_product_version=version, directory=relative.as_posix(), reference=strict_name,
                                        fixture_count=len(selected['fixtures']),
                                        glyph_count=sum(len(f['glyphs']) for f in selected['fixtures']),
                                        baseline_line_count=sum(36 for f in selected['fixtures'] if f['name'].startswith('word-windows-controlled-'))))
        temp = index_path.with_suffix('.json.tmp')
        temp.write_bytes((json.dumps(index, indent=2)+'\n').encode('utf-8'))
        temp.replace(index_path)
    return target


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--evidence-root', type=Path, default=Path('jade-core/tests/word-oracle/windows'))
    parser.add_argument('--strict', nargs='*', default=[])
    parser.add_argument('--reference-kind', choices=['pdf', 'emf'], default='pdf')
    args = parser.parse_args()
    print(archive(args.manifest, args.evidence_root, args.strict, args.reference_kind))
