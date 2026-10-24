#!/usr/bin/env python3
"""Deterministic, original pivot/cache regression package (no Office oracle claim)."""
from pathlib import Path
from datetime import datetime
from xml.sax.saxutils import escape
import zipfile
MAIN='http://schemas.openxmlformats.org/spreadsheetml/2006/main'
REL='http://schemas.openxmlformats.org/officeDocument/2006/relationships'
PKG='http://schemas.openxmlformats.org/package/2006/relationships'
ROOT=Path(__file__).resolve().parents[2]
def relationships(items):
    body=''.join(f'<Relationship Id="{id}" Type="{REL}/{kind}" Target="{target}"/>' for id,kind,target in items)
    return f'<Relationships xmlns="{PKG}">{body}</Relationships>'
def cell(ref,value):
    if isinstance(value,str):return f'<c r="{ref}" t="inlineStr"><is><t>{escape(value)}</t></is></c>'
    return f'<c r="{ref}"><v>{value}</v></c>'
def sheet(rows,extra=''):
    body=''.join(f'<row r="{i}">'+''.join(cell(ref,v) for ref,v in row)+'</row>' for i,row in rows)
    return f'<worksheet xmlns="{MAIN}" xmlns:r="{REL}"><sheetData>{body}</sheetData>{extra}</worksheet>'
def parts():
    cache=f'''<pivotCacheDefinition xmlns="{MAIN}" xmlns:r="{REL}" r:id="records" recordCount="4" refreshOnLoad="0"><cacheSource type="worksheet"><worksheetSource sheet="Data" ref="A1:C5"/></cacheSource><cacheFields count="3"><cacheField name="Region"><sharedItems count="3"><s v="East"/><s v="Historical" u="1"/><s v="West"/></sharedItems></cacheField><cacheField name="Amount" numFmtId="164"><sharedItems count="0"/></cacheField><cacheField name="When"><sharedItems count="2"><d v="2026-10-24T12:30:00"/><m/></sharedItems></cacheField></cacheFields></pivotCacheDefinition>'''
    records=f'''<pivotCacheRecords xmlns="{MAIN}" count="4"><r><x v="0"/><n v="10"/><x v="0"/></r><r><x v="2"/><n v="20"/><x v="1"/></r><r><x v="0"/><n v="5"/><x v="0"/></r><r><x v="2"/><n v="7"/><x v="1"/></r></pivotCacheRecords>'''
    table=f'''<pivotTableDefinition xmlns="{MAIN}" name="RegionalSales" cacheId="7" dataCaption="Values" rowGrandTotals="1" colGrandTotals="1"><location ref="E3:F6" firstHeaderRow="0" firstDataRow="1" firstDataCol="1"/><pivotFields count="3"><pivotField axis="axisRow" name="Territory"><items count="3"><item x="0"/><item x="2"/><item t="default"/></items></pivotField><pivotField dataField="1"/><pivotField axis="axisPage"><items count="2"><item x="0"/><item x="1"/></items></pivotField></pivotFields><rowFields count="1"><field x="0"/></rowFields><rowItems count="3"><i><x v="0"/></i><i><x v="1"/></i><i t="grand"><x/></i></rowItems><colFields count="1"><field x="-2"/></colFields><colItems count="1"><i i="0"><x v="0"/></i></colItems><pageFields count="1"><pageField fld="2"/></pageFields><dataFields count="1"><dataField name="Total Amount" fld="1" subtotal="sum" numFmtId="164" baseField="0" baseItem="1"/></dataFields><pivotTableStyleInfo name="PivotStyleMedium9" showRowHeaders="1" showColStripes="1"/></pivotTableDefinition>'''
    data=[(1,[('A1','Region'),('B1','Amount'),('C1','When')])]
    serial=(datetime(2026,10,24,12,30)-datetime(1899,12,30)).total_seconds()/86400
    for i,(region,amount,day) in enumerate([('East',10,serial),('West',20,None),('East',5,serial),('West',7,None)],2):
        data.append((i,[(f'A{i}',region),(f'B{i}',amount)]+([(f'C{i}',day)] if day is not None else [])))
    report=[(3,[('E3','Territory'),('F3','Total Amount')]),(4,[('E4','East'),('F4',15)]),(5,[('E5','West'),('F5',27)]),(6,[('E6','Grand Total'),('F6',42)])]
    result={
        '_rels/.rels':relationships([('book','officeDocument','xl/workbook.xml')]),
        'xl/workbook.xml':f'<workbook xmlns="{MAIN}" xmlns:r="{REL}"><sheets><sheet name="Data" sheetId="1" r:id="source"/><sheet name="Report" sheetId="2" r:id="report"/></sheets><pivotCaches><pivotCache cacheId="7" r:id="sharedCache"/></pivotCaches></workbook>',
        'xl/_rels/workbook.xml.rels':relationships([('source','worksheet','worksheets/sheet1.xml'),('report','worksheet','worksheets/sheet2.xml'),('style','styles','styles.xml'),('sharedCache','pivotCacheDefinition','pivotCache/cache.xml')]),
        'xl/worksheets/sheet1.xml':sheet(data),
        'xl/worksheets/sheet2.xml':sheet(report,'<pivotTableParts count="1"><pivotTablePart r:id="pivot"/></pivotTableParts>'),
        'xl/worksheets/_rels/sheet2.xml.rels':relationships([('pivot','pivotTable','../pivotTables/report.xml')]),
        'xl/pivotTables/report.xml':table,
        'xl/pivotCache/cache.xml':cache,
        'xl/pivotCache/records.xml':records,
        'xl/pivotCache/_rels/cache.xml.rels':relationships([('records','pivotCacheRecords','records.xml')]),
        'xl/styles.xml':f'<styleSheet xmlns="{MAIN}"><numFmts count="1"><numFmt numFmtId="164" formatCode="0.00"/></numFmts><fonts count="1"><font><sz val="11"/><name val="Calibri"/></font></fonts><fills count="2"><fill><patternFill patternType="none"/></fill><fill><patternFill patternType="gray125"/></fill></fills><borders count="1"><border/></borders><cellStyleXfs count="1"><xf/></cellStyleXfs><cellXfs count="1"><xf numFmtId="0" fontId="0" fillId="0" borderId="0" xfId="0"/></cellXfs><cellStyles count="1"><cellStyle name="Normal" xfId="0" builtinId="0"/></cellStyles></styleSheet>'
    }
    kinds={'xl/workbook.xml':'sheet.main','xl/styles.xml':'styles','xl/worksheets/sheet1.xml':'worksheet','xl/worksheets/sheet2.xml':'worksheet','xl/pivotTables/report.xml':'pivotTable','xl/pivotCache/cache.xml':'pivotCacheDefinition','xl/pivotCache/records.xml':'pivotCacheRecords'}
    overrides=''.join(f'<Override PartName="/{part}" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.{kind}+xml"/>' for part,kind in kinds.items())
    result['[Content_Types].xml']=f'<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>{overrides}</Types>'
    return result
if __name__=='__main__':
    path=ROOT/'jade-core/public/fixtures/pivot-cache-compat.xlsx'
    with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as archive:
        for name,value in sorted(parts().items(),key=lambda pair:(pair[0]!='[Content_Types].xml',pair[0])):
            entry=zipfile.ZipInfo(name,date_time=(2026,10,24,0,0,0));entry.compress_type=zipfile.ZIP_DEFLATED;entry.external_attr=0o100644<<16
            archive.writestr(entry,value.encode('utf-8'),compresslevel=9)
    print(path)
