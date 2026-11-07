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
report = []
for pdf in sorted(root.glob('*.pdf')):
    native = pdf.with_suffix('.json')
    if not native.exists():
        continue
    actual = json.loads(native.read_text(encoding='utf-8'))
    expected = []
    doc = pymupdf.open(pdf)
    for page_no, page in enumerate(doc, 1):
        for block in page.get_text('rawdict')['blocks']:
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
    row = {'fixture': pdf.stem, 'word_pages': len(doc), 'native_pages': actual['page_count'],
           'word_glyphs': len(expected), 'native_glyphs': len(observed), 'matched': len(pairs),
           'within_0_375pt': sum(x <= .375 and y <= .375 for x,y,_,_ in errors),
           'x_p50': percentile([v[0] for v in errors], .5), 'x_p95': percentile([v[0] for v in errors], .95),
           'y_p50': percentile([v[1] for v in errors], .5), 'y_p95': percentile([v[1] for v in errors], .95),
           'first_errors': [{'word':e,'native':o,'dx':o[2]-e[2],'dy':o[3]-e[3]} for x,y,e,o in errors if x > .375 or y > .375][:20],
           'pdf_sha256': hashlib.sha256(pdf.read_bytes()).hexdigest()}
    report.append(row)
(root/'comparison.json').write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding='utf-8')
for r in report:
    print(json.dumps({k:v for k,v in r.items() if k not in ('first_errors','pdf_sha256')}, ensure_ascii=True))
