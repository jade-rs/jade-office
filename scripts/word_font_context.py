"""Identify captured PDF font resources without deriving drawing expectations."""
import hashlib
from io import BytesIO
import json
from pathlib import Path
import re

import pymupdf
from fontTools.ttLib import TTFont, TTLibError


def digest(data):
    return hashlib.sha256(data).hexdigest()


def capture_font_context(manifest):
    manifest = Path(manifest)
    raw = manifest.read_bytes()
    capture = json.loads(raw.decode('utf-8-sig'))
    if (capture.get('schema') != 2 or capture.get('status') != 'complete'
            or capture.get('platform') != 'windows'
            or not re.fullmatch(r'\d+\.\d+\.\d+\.\d+', capture['word']['product_version'])):
        raise ValueError('Font context requires a complete versioned Windows capture')
    documents = []
    for item in capture['documents']:
        name = item['pdf']
        if name in ('', '.', '..') or '/' in name or '\\' in name or ':' in name:
            raise ValueError('PDF must belong to the capture directory')
        data = (manifest.parent / name).read_bytes()
        if digest(data) != item['pdf_sha256']:
            raise ValueError('Captured PDF changed before font identification')
        if digest(Path(item['source']).read_bytes()) != item['sha256']:
            raise ValueError('Source DOCX changed before font identification')
        resources, text_fonts = [], set()
        with pymupdf.open(stream=data, filetype='pdf') as pdf:
            if len(pdf) != item['pages']:
                raise ValueError('Font context page count differs from capture')
            xrefs = sorted({f[0] for page in pdf for f in page.get_fonts(full=True)})
            for xref in xrefs:
                font_name, extension, kind, program = pdf.extract_font(xref)
                fact = dict(pdf_font_name=font_name, font_kind=kind, extension=extension,
                            embedded=bool(program), program_sha256=digest(program) if program else None,
                            program_bytes=len(program), family=None, style=None, version=None,
                            metadata_status='not-embedded' if not program else 'unsupported-font-format')
                if program and extension in ('ttf', 'otf'):
                    try:
                        with TTFont(BytesIO(program)) as font:
                            fact.update(family=font['name'].getDebugName(1), style=font['name'].getDebugName(2),
                                        version=font['name'].getDebugName(5), metadata_status='sfnt-name-table')
                    except (TTLibError, KeyError):
                        fact['metadata_status'] = 'unreadable-name-table'
                resources.append(fact)
            for page in pdf:
                for block in page.get_text('dict', flags=pymupdf.TEXTFLAGS_DICT)['blocks']:
                    for line in block.get('lines', []):
                        text_fonts.update(span['font'] for span in line['spans'] if span.get('text'))
        documents.append(dict(name=item['name'], source_sha256=item['sha256'], pdf=name,
                              pdf_sha256=item['pdf_sha256'], pages=item['pages'],
                              font_resources=resources, extracted_text_font_names=sorted(text_fonts)))
    result = dict(schema=1, platform='windows', word=capture['word'], capture_id=capture['capture_id'],
                  capture_sha256=digest(raw), documents=documents)
    result['context_sha256'] = digest(json.dumps(result, sort_keys=True, separators=(',', ':'),
                                               ensure_ascii=False).encode('utf-8'))
    return result
