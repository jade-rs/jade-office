"""Compare PDF glyph origins with word_coordinates output, without alignment.

Usage: compare-word-coordinates.py capture-directory
Requires PyMuPDF. Reports unmatched text/page counts as well as coordinate errors.
"""
import difflib
import hashlib
import json
import pathlib
import sys
import pymupdf

root = pathlib.Path(sys.argv[1])
visible = '--visible' in sys.argv[2:]
painted = '--native-pdf' in sys.argv[2:]
reference_path = root/('word-visible-reference.json' if visible or painted else 'word-reference.json')
reference = json.loads(reference_path.read_text(encoding='utf-8-sig')) if reference_path.exists() else {}
projection = reference.get('table_glyph_projection', 'visible')
if projection not in ('visible', 'unclipped'):
    raise ValueError('Unknown table glyph projection')
flags = pymupdf.TEXTFLAGS_RAWDICT if projection == 'visible' else pymupdf.TEXTFLAGS_RAWDICT & ~pymupdf.TEXT_CLIP
if projection == 'unclipped' and reference.get('pdf_text_flags') != flags:
    raise ValueError('PDF extraction flags differ from the recorded projection')
report = []
for pdf in sorted(root.glob('*.pdf')):
    native = pdf.with_suffix('.visible.json' if visible else '.json')
    native_pdf = pdf.with_name(pdf.stem+'.native.pdf')
    if painted:
        if not native_pdf.exists():
            continue
        if native_pdf.read_bytes() == pdf.read_bytes():
            raise ValueError('Independent native PDF required; refusing a Word self-comparison')
        native_doc = pymupdf.open(native_pdf)
        actual_glyphs = []
        for number, page in enumerate(native_doc, 1):
            for block in page.get_text('rawdict', flags=pymupdf.TEXTFLAGS_RAWDICT)['blocks']:
                for line in block.get('lines', []):
                    for span in line['spans']:
                        actual_glyphs.extend([number, char['c'], *char['origin']]
                                             for char in span['chars'] if not char['c'].isspace())
        actual = dict(page_count=len(native_doc), glyphs=actual_glyphs, table_glyph_projection='visible')
    else:
        if not native.exists():
            continue
        actual = json.loads(native.read_text(encoding='utf-8'))
    if actual.get('table_glyph_projection', 'visible') != projection:
        raise ValueError('Native table glyph projection differs from Word reference')
    expected = []
    doc = pymupdf.open(pdf)
    for page_no, page in enumerate(doc, 1):
        for block in page.get_text('rawdict', flags=flags)['blocks']:
            for line in block.get('lines', []):
                for span in line['spans']:
                    for char in span['chars']:
                        if not char['c'].isspace():
                            expected.append([page_no, char['c'], *char['origin']])
    observed = [g for g in actual['glyphs'] if not g[1].isspace()]
    # Match within corresponding physical lines. Global text-only matching can
    # align repeated list bodies with a different paragraph when debug glyphs
    # omit separately painted numbering labels. No coordinate shift is applied.
    def keys(glyphs):
        glyphs.sort(key=lambda g: (g[0], g[3], g[2]))
        result, page, baseline, line = [], None, None, -1
        for g in glyphs:
            if page != g[0]:
                page, baseline, line = g[0], g[3], 0
            elif abs(g[3] - baseline) > .25:
                baseline, line = g[3], line + 1
            result.append((page, line, g[1]))
        return result
    a, b = keys(expected), keys(observed)
    matcher = difflib.SequenceMatcher(a=a, b=b, autojunk=False)
    pairs = [(expected[i+k], observed[j+k]) for i,j,n in matcher.get_matching_blocks() for k in range(n)]
    errors = [(abs(e[2]-o[2]), abs(e[3]-o[3]), e, o) for e,o in pairs]
    percentile = lambda values, p: sorted(values)[min(len(values)-1, int((len(values)-1)*p))] if values else None
    row = {'fixture': pdf.stem, 'table_glyph_projection': projection, 'word_pages': len(doc), 'native_pages': actual['page_count'],
           'word_glyphs': len(expected), 'native_glyphs': len(observed), 'matched': len(pairs),
           'within_0_375pt': sum(x <= .375 and y <= .375 for x,y,_,_ in errors),
           'x_p50': percentile([v[0] for v in errors], .5), 'x_p95': percentile([v[0] for v in errors], .95),
           'y_p50': percentile([v[1] for v in errors], .5), 'y_p95': percentile([v[1] for v in errors], .95),
           'first_errors': [{'word':e,'native':o,'dx':o[2]-e[2],'dy':o[3]-e[3]} for x,y,e,o in errors if x > .375 or y > .375][:20],
           'pdf_sha256': hashlib.sha256(pdf.read_bytes()).hexdigest()}
    if painted:
        row.update(coordinate_source='Native vector PDF', native_pdf_sha256=hashlib.sha256(native_pdf.read_bytes()).hexdigest())
    report.append(row)
(root/('painted-comparison.json' if painted else 'visible-comparison.json' if visible else 'comparison.json')).write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
for r in report:
    print(json.dumps({k:v for k,v in r.items() if k not in ('first_errors','pdf_sha256')}, ensure_ascii=True))
