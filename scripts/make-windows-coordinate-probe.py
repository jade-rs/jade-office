"""Create a deterministic fixture; Word is only used to render, never resave it."""
from pathlib import Path
from zipfile import ZipFile, ZIP_DEFLATED
from xml.sax.saxutils import escape

root = Path(__file__).resolve().parents[1]
out = root / 'tmp/word-windows'
out.mkdir(parents=True, exist_ok=True)
for fe, grid in ((False, False), (True, False), (False, True), (True, True)):
    paragraphs = []
    for family, text in [('Arial', 'ABCDE abcde 01234'), ('Times New Roman', 'ABCDE abcde 01234'),
                         ('MS Mincho', 'あいうえおかきくけこ'), ('Yu Mincho', 'あいうえおかきくけこ')]:
        for size in (21, 24, 40):
            for row in range(3):
                page = '<w:pageBreakBefore/>' if row == 0 and paragraphs else ''
                paragraphs.append(f'''<w:p><w:pPr>{page}<w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:snapToGrid w:val="0"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="{family}" w:hAnsi="{family}" w:eastAsia="{family}"/><w:sz w:val="{size}"/></w:rPr><w:t>{escape(text)}</w:t></w:r></w:p>''')
    document = f'''<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>{''.join(paragraphs)}<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440" w:header="720" w:footer="720" w:gutter="0"/></w:sectPr></w:body></w:document>'''
    settings = f'''<w:settings xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:compat>{'<w:useFELayout/>' if fe else ''}<w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>'''
    styles = '''<w:styles xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial" w:eastAsia="MS Mincho"/><w:sz w:val="21"/><w:lang w:val="en-US" w:eastAsia="ja-JP"/></w:rPr></w:rPrDefault></w:docDefaults></w:styles>'''
    if grid:
        document = document.replace('<w:snapToGrid w:val="0"/>', '').replace('</w:sectPr>', '<w:docGrid w:type="lines" w:linePitch="360"/></w:sectPr>')
    suffix = '-grid' if grid else ''
    with ZipFile(root/'jade-core/public/fixtures/contract.docx') as source, ZipFile(out/f'controlled-fe-{int(fe)}{suffix}.docx', 'w', ZIP_DEFLATED) as target:
        replace = {'word/document.xml': document, 'word/settings.xml': settings, 'word/styles.xml': styles}
        for item in source.infolist():
            target.writestr(item, replace.get(item.filename, source.read(item.filename)))
