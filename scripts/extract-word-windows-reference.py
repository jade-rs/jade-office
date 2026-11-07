"""Extract independent Word PDF origins from one immutable, versioned capture."""
import argparse
import hashlib
import json
import re
from pathlib import Path
import pymupdf


def extract(manifest_path, projection=None):
    manifest_path = Path(manifest_path)
    capture = json.loads(manifest_path.read_text(encoding='utf-8-sig'))
    if capture.get('schema') != 2 or capture.get('status') != 'complete' or capture.get('platform') != 'windows':
        raise ValueError('Only complete versioned Word captures can become evidence')
    version = capture['word']['product_version']
    if not re.fullmatch(r'[0-9]+\.[0-9]+\.[0-9]+\.[0-9]+', version):
        raise ValueError('Word product version must include the complete build')
    projection = projection or capture.get('table_glyph_projection', 'visible')
    if projection not in ('visible', 'unclipped'):
        raise ValueError('Unknown table glyph projection')
    flags = pymupdf.TEXTFLAGS_RAWDICT
    if projection == 'unclipped':
        flags &= ~pymupdf.TEXT_CLIP
    fixtures = []
    for item in capture['documents']:
        pdf = manifest_path.parent / item['pdf']
        if hashlib.sha256(pdf.read_bytes()).hexdigest() != item['pdf_sha256']:
            raise ValueError(f'PDF changed after capture: {pdf}')
        doc = pymupdf.open(pdf)
        if len(doc) != item['pages']:
            raise ValueError(f'Word/PDF page-count mismatch: {pdf}')
        glyphs = []
        for number, page in enumerate(doc, 1):
            for block in page.get_text('rawdict', flags=flags)['blocks']:
                for line in block.get('lines', []):
                    for span in line['spans']:
                        glyphs.extend([number, char['c'], *char['origin']]
                                      for char in span['chars'] if not char['c'].isspace())
        fixtures.append(dict(name=item['name'], docx_sha256=item['sha256'],
                             pdf_sha256=item['pdf_sha256'], pdf_file=item['pdf'],
                             page_count=len(doc), glyphs=glyphs))
    result = dict(schema=2, platform='windows', word_product_version=version,
                word=capture['word'], capture_id=capture['capture_id'],
                captured_at_utc=capture['captured_at_utc'], extractor=f'PyMuPDF {pymupdf.VersionBind}',
                tolerance_pt=0.375, fixtures=fixtures)
    if projection == 'unclipped' or capture.get('table_glyph_projection') == 'unclipped':
        result.update(table_glyph_projection=projection, pdf_text_flags=flags)
    return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('manifest', type=Path)
    parser.add_argument('--visible', action='store_true')
    args = parser.parse_args()
    target = args.manifest.parent / ('word-visible-reference.json' if args.visible else 'word-reference.json')
    if target.exists():
        raise FileExistsError(f'Refusing to overwrite evidence: {target}')
    target.write_text(json.dumps(extract(args.manifest, 'visible' if args.visible else None), ensure_ascii=False, indent=1)+'\n', encoding='utf-8')
    print(target)
