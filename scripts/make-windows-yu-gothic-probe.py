"""Generate the immutable Windows Yu Gothic line-box probe without Word."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
NS = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
TEXT = '天地玄黄宇宙洪荒日月盈昃辰宿列張寒来暑往秋収冬蔵閏余成歳律呂'
paragraphs = []
for family, bold in [('Yu Gothic', False), ('Yu Gothic Light', False),
                     ('Yu Gothic Medium', False), ('Yu Gothic', True)]:
    for size in [18, 20, 21, 22, 24, 40]:
        for row in range(3):
            page = '<w:pageBreakBefore/>' if row == 0 and paragraphs else ''
            properties = (f'<w:rFonts w:ascii="{family}" w:hAnsi="{family}" '
                          f'w:eastAsia="{family}"/><w:sz w:val="{size}"/>'
                          + ('<w:b/>' if bold else ''))
            paragraphs.append(f'<w:p><w:pPr>{page}<w:rPr>{properties}</w:rPr>'
                              '<w:spacing w:before="0" w:after="0" w:line="240" '
                              'w:lineRule="auto"/><w:snapToGrid w:val="0"/></w:pPr>'
                              f'<w:r><w:rPr>{properties}</w:rPr><w:t>{TEXT}</w:t></w:r></w:p>')
parts = {
    '[Content_Types].xml': '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
        '<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/></Types>',
    '_rels/.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>',
    'word/_rels/document.xml.rels': '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'
        '<Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/></Relationships>',
    'word/settings.xml': f'<w:settings xmlns:w="{NS}"><w:compat><w:useFELayout/>'
        '<w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>',
    'word/document.xml': f'<w:document xmlns:w="{NS}"><w:body>' + ''.join(paragraphs)
        + '<w:sectPr><w:pgSz w:w="12240" w:h="15840"/>'
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>',
}
target = ROOT / 'jade-core/public/fixtures/word-windows-yu-gothic-lines.docx'
with ZipFile(target, 'w') as archive:
    for name, content in parts.items():
        entry = ZipInfo(name, date_time=(2026, 11, 3, 0, 0, 0))
        entry.compress_type = ZIP_DEFLATED
        archive.writestr(entry, content.encode('utf-8'))
print(target)
