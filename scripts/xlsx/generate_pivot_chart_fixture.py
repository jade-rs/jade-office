#!/usr/bin/env python3
"""Original deterministic, relationship-owned PivotChart regression workbook."""
from pathlib import Path
import importlib.util,zipfile
root=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('pivot_fixture',Path(__file__).with_name('generate_pivot_fixture.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
p=module.parts()
p['xl/pivotTables/_rels/report.xml.rels']=module.relationships([('cache','pivotCacheDefinition','../pivotCache/cache.xml')])
# A sorted report inserts North between existing East/West, exercising point identity.
p['xl/pivotTables/report.xml']=p['xl/pivotTables/report.xml'].replace('axis="axisRow" name="Territory"','axis="axisRow" name="Territory" sortType="ascending"')
p['xl/worksheets/sheet2.xml']=p['xl/worksheets/sheet2.xml'].replace('<pivotTableParts count="1"><pivotTablePart r:id="pivot"/></pivotTableParts>','<drawing r:id="reportDrawing"/>')
p['xl/worksheets/_rels/sheet2.xml.rels']=p['xl/worksheets/_rels/sheet2.xml.rels'].replace('</Relationships>',f'<Relationship Id="reportDrawing" Type="{module.REL}/drawing" Target="../drawings/pivot.xml"/></Relationships>')
p['xl/drawings/pivot.xml']='''<xdr:wsDr xmlns:xdr="http://schemas.openxmlformats.org/drawingml/2006/spreadsheetDrawing" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships"><xdr:twoCellAnchor><xdr:from><xdr:col>7</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>1</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:from><xdr:to><xdr:col>15</xdr:col><xdr:colOff>0</xdr:colOff><xdr:row>17</xdr:row><xdr:rowOff>0</xdr:rowOff></xdr:to><xdr:graphicFrame macro=""><xdr:nvGraphicFramePr><xdr:cNvPr id="2" name="Regional PivotChart"/><xdr:cNvGraphicFramePr/></xdr:nvGraphicFramePr><xdr:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/></xdr:xfrm><a:graphic><a:graphicData uri="http://schemas.openxmlformats.org/drawingml/2006/chart"><c:chart r:id="chart"/></a:graphicData></a:graphic></xdr:graphicFrame><xdr:clientData/></xdr:twoCellAnchor></xdr:wsDr>'''
p['xl/drawings/_rels/pivot.xml.rels']=module.relationships([('chart','chart','../charts/pivot.xml')])
def ref(kind,formula,values,fmt=''):
 cache='numCache' if kind=='numRef' else 'strCache'
 return f'<c:{kind}><c:f>{formula}</c:f><c:{cache}>{fmt}<c:ptCount val="{len(values)}"/>'+''.join(f'<c:pt idx="{i}"><c:v>{v}</c:v></c:pt>' for i,v in enumerate(values))+f'</c:{cache}></c:{kind}>'
name=ref('strRef',"'Report'!$F$3",['Total Amount'])
cat=ref('strRef',"'Report'!$E$4:$E$5",['East','West'])
val=ref('numRef',"'Report'!$F$4:$F$5",['15','27'],'<c:formatCode>0.00</c:formatCode>')
p['xl/charts/pivot.xml']=f'''<c:chartSpace xmlns:c="http://schemas.openxmlformats.org/drawingml/2006/chart" xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:j="urn:jade:test"><c:pivotSource><c:name>[pivot-chart-compat.xlsx]Report!RegionalSales</c:name><c:fmtId val="0"/></c:pivotSource><c:chart><c:title><c:tx><c:rich><a:bodyPr/><a:lstStyle/><a:p><a:r><a:t>Regional Amount</a:t></a:r></a:p></c:rich></c:tx></c:title><c:plotArea><c:layout/><c:barChart><c:barDir val="col"/><c:grouping val="clustered"/><c:ser><c:idx val="0"/><c:order val="0"/><c:tx>{name}</c:tx><c:spPr><a:solidFill><a:srgbClr val="336699"/></a:solidFill></c:spPr><c:dPt><c:idx val="1"/><c:spPr><a:solidFill><a:srgbClr val="FF9900"/></a:solidFill></c:spPr></c:dPt><c:cat>{cat}</c:cat><c:val>{val}</c:val></c:ser><c:dLbls><c:showVal val="1"/></c:dLbls><c:gapWidth val="150"/><c:axId val="1"/><c:axId val="2"/></c:barChart><c:catAx><c:axId val="1"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="b"/><c:crossAx val="2"/></c:catAx><c:valAx><c:axId val="2"/><c:scaling><c:orientation val="minMax"/></c:scaling><c:axPos val="l"/><c:crossAx val="1"/></c:valAx></c:plotArea><c:legend><c:legendPos val="r"/></c:legend><c:plotVisOnly val="1"/></c:chart><c:extLst><c:ext uri="urn:jade:keep"><j:payload j:caption="preserved"/></c:ext></c:extLst></c:chartSpace>'''
p['[Content_Types].xml']=p['[Content_Types].xml'].replace('</Types>','<Override PartName="/xl/drawings/pivot.xml" ContentType="application/vnd.openxmlformats-officedocument.drawing+xml"/><Override PartName="/xl/charts/pivot.xml" ContentType="application/vnd.openxmlformats-officedocument.drawingml.chart+xml"/></Types>')
path=root/'jade-core/public/fixtures/pivot-chart-compat.xlsx'
with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 for name,value in sorted(p.items(),key=lambda p:(p[0]!='[Content_Types].xml',p[0])):
  info=zipfile.ZipInfo(name,date_time=(2026,10,24,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16
  z.writestr(info,value.encode(),compresslevel=9)
print(path)
