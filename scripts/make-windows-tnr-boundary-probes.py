"""Generate Windows Word Latin/CJK boundary probes, with and without balancing."""
from pathlib import Path
from zipfile import ZipFile, ZipInfo, ZIP_DEFLATED

ROOT = Path(__file__).resolve().parents[1]
NS = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
TEXT = '漢a漢漢s漢漢d漢漢Y漢'
paragraphs = []
for style in ['', '<w:b/>', '<w:i/>', '<w:b/><w:i/>']:
    for size in [18, 21, 24, 40, 80]:
        for row in range(2):
            properties = (f'<w:rFonts w:ascii="Times New Roman" w:hAnsi="MS Mincho" '
                          f'w:eastAsia="MS Mincho"/><w:sz w:val="{size}"/>' + style)
            page = '<w:pageBreakBefore/>' if row == 0 and paragraphs else ''
            paragraphs.append(f'<w:p><w:pPr>{page}<w:jc w:val="left"/>'
                              f'<w:rPr>{properties}</w:rPr><w:spacing w:before="0" w:after="0" '
                              'w:line="240" w:lineRule="auto"/><w:snapToGrid w:val="0"/></w:pPr>'
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
        + '<w:sectPr><w:pgSz w:w="15840" w:h="12240"/>'
        '<w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>',
}
for balanced in [False, True]:
    capture_parts = dict(parts)
    if balanced:
        capture_parts['word/settings.xml'] = capture_parts['word/settings.xml'].replace(
            '<w:useFELayout/>', '<w:useFELayout/><w:balanceSingleByteDoubleByteWidth/>')
    name = 'balanced' if balanced else 'unbalanced'
    target = ROOT / f'jade-core/public/fixtures/word-windows-tnr-{name}-sweep.docx'
    with ZipFile(target, 'w') as archive:
        for name, content in capture_parts.items():
            entry = ZipInfo(name, date_time=(2026, 11, 3, 0, 0, 0))
            entry.compress_type = ZIP_DEFLATED
            archive.writestr(entry, content.encode('utf-8'))
    print(target)
