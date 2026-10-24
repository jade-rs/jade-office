#!/usr/bin/env python3
"""Original deterministic styled pivot, using standard worksheet ownership."""
from pathlib import Path
import importlib.util,zipfile
root=Path(__file__).resolve().parents[2]
spec=importlib.util.spec_from_file_location('pivot_fixture',Path(__file__).with_name('generate_pivot_fixture.py'))
module=importlib.util.module_from_spec(spec);spec.loader.exec_module(module)
parts=module.parts()
parts['xl/worksheets/sheet2.xml']=parts['xl/worksheets/sheet2.xml'].replace('<pivotTableParts count="1"><pivotTablePart r:id="pivot"/></pivotTableParts>','').replace('<c r="F4">','<c r="F4" s="1">').replace('<c r="F5"><v>27</v></c>','')
parts['xl/pivotTables/report.xml']=parts['xl/pivotTables/report.xml'].replace('PivotStyleMedium9','OwnedPivot').replace('showRowHeaders="1"','showRowHeaders="1" showColHeaders="1"')
s=parts['xl/styles.xml'].replace('<fills count="2">','<fills count="3">').replace('</fills>','<fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></fills>')
s=s.replace('<cellXfs count="1">','<cellXfs count="2">').replace('</cellXfs>','<xf numFmtId="0" fontId="0" fillId="2" borderId="0" applyFont="1" applyFill="1"/></cellXfs>')
dxfs='<dxf><fill><patternFill patternType="solid"><fgColor rgb="FFFF0000"/></patternFill></fill></dxf><dxf><font><b/><color rgb="FFFFFFFF"/></font><fill><patternFill patternType="solid"><fgColor rgb="FF0000FF"/></patternFill></fill></dxf><dxf><font><b/></font><fill><patternFill patternType="solid"><fgColor rgb="FF00FF00"/></patternFill></fill></dxf><dxf><fill><patternFill patternType="solid"><fgColor rgb="FFFFFF00"/></patternFill></fill></dxf>'
parts['xl/styles.xml']=s.replace('</styleSheet>',f'<dxfs count="4">{dxfs}</dxfs><tableStyles count="1"><tableStyle name="OwnedPivot" table="0" pivot="1"><tableStyleElement type="wholeTable" dxfId="0"/><tableStyleElement type="headerRow" dxfId="1"/><tableStyleElement type="totalRow" dxfId="2"/></tableStyle></tableStyles></styleSheet>')
path=root/'jade-core/public/fixtures/pivot-style-compat.xlsx'
with zipfile.ZipFile(path,'w',compression=zipfile.ZIP_DEFLATED,compresslevel=9) as z:
 for name,value in sorted(parts.items(),key=lambda p:(p[0]!='[Content_Types].xml',p[0])):
  info=zipfile.ZipInfo(name,date_time=(2026,10,24,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.external_attr=0o100644<<16
  z.writestr(info,value.encode(),compresslevel=9)
print(path)
