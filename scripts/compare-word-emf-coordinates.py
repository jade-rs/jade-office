"""Compare raw Word drawing origins with native origins, without fitting."""
import argparse
import hashlib
import json
from pathlib import Path


def spatial_order(glyphs):
    glyphs = sorted(glyphs, key=lambda g: (g[0], g[3]))
    start = 0
    while start < len(glyphs):
        end = start + 1
        while end < len(glyphs) and glyphs[end][0] == glyphs[start][0] and abs(glyphs[end][3] - glyphs[start][3]) <= 0.25:
            end += 1
        glyphs[start:end] = sorted(glyphs[start:end], key=lambda g: g[2])
        start = end
    return glyphs


def compare(directory):
    directory = Path(directory)
    reference = json.loads((directory/'word-emf-reference.json').read_text(encoding='utf-8'))
    if reference['tolerance_pt'] != 0.375:
        raise ValueError('Unexpected drawing-origin tolerance')
    report = []
    for fixture in reference['fixtures']:
        actual = json.loads((directory/(fixture['name']+'.json')).read_text(encoding='utf-8'))
        expected = spatial_order(fixture['glyphs'])
        observed = spatial_order([g for g in actual['glyphs'] if not g[1].isspace()])
        pairs = list(zip(expected, observed))
        same_text = len(expected) == len(observed) and all(e[:2] == o[:2] for e, o in pairs)
        errors = [(abs(e[2]-o[2]), abs(e[3]-o[3]), e, o) for e, o in pairs]
        percentile = lambda values, p: sorted(values)[min(len(values)-1, int((len(values)-1)*p))] if values else None
        row = dict(fixture=fixture['name'], coordinate_source=reference['coordinate_source'],
                   word_pages=fixture['page_count'], native_pages=actual['page_count'],
                   word_glyphs=len(expected), native_glyphs=len(observed),
                   matched=len(expected) if same_text else 0,
                   within_0_375pt=sum(e[:2] == o[:2] and x <= .375 and y <= .375 for x, y, e, o in errors),
                   x_p50=percentile([e[0] for e in errors], .5), x_p95=percentile([e[0] for e in errors], .95),
                   y_p50=percentile([e[1] for e in errors], .5), y_p95=percentile([e[1] for e in errors], .95),
                   first_errors=[dict(word=e, native=o, dx=o[2]-e[2], dy=o[3]-e[3])
                                 for x, y, e, o in errors if e[:2] != o[:2] or x > .375 or y > .375][:20],
                   pdf_sha256=hashlib.sha256((directory/fixture['pdf_file']).read_bytes()).hexdigest(),
                   metafile_sha256=[hashlib.sha256((directory/p['file']).read_bytes()).hexdigest() for p in fixture['metafiles']])
        report.append(row)
    return report


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    target = args.directory/'emf-comparison.json'
    if target.exists():
        raise FileExistsError(f'Refusing to overwrite comparison: {target}')
    report = compare(args.directory)
    target.write_bytes((json.dumps(report, ensure_ascii=False, indent=2)+'\n').encode('utf-8'))
    for row in report:
        print(json.dumps({k: v for k, v in row.items() if k not in ('first_errors', 'metafile_sha256', 'pdf_sha256')}))
