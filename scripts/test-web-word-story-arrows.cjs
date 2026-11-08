#!/usr/bin/env node
// Requires the built web shell, Playwright via NODE_PATH, and python3.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const {chromium} = require('playwright');
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jade-story-arrows-'));
execFileSync('python3', ['-c', `
import sys,zipfile
w='http://schemas.openxmlformats.org/wordprocessingml/2006/main'
p=lambda text:'<w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>'+text+'</w:t></w:r></w:p>'
position='position:absolute;margin-left:108pt;margin-top:72pt;mso-position-horizontal-relative:page;mso-position-vertical-relative:page;' if sys.argv[2]=='1' else ''
for kind in ['footnote','endnote','textbox']:
 with zipfile.ZipFile(sys.argv[1]+'/'+kind+'.docx','w') as z:
  overrides='<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>'
  overrides+='<Override PartName="/word/settings.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.settings+xml"/>'
  rels='<Relationship Id="settings" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/settings" Target="settings.xml"/>'
  z.writestr('word/settings.xml','<w:settings xmlns:w="'+w+'"><w:compat><w:compatSetting w:name="compatibilityMode" w:uri="http://schemas.microsoft.com/office/word" w:val="15"/></w:compat></w:settings>')
  if kind!='textbox': overrides+='<Override PartName="/word/'+kind+'s.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.'+kind+'s+xml"/>'
  z.writestr('[Content_Types].xml','<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>'+overrides+'</Types>')
  z.writestr('_rels/.rels','<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
  body=p('Body before')
  if kind=='textbox':
   body+='<w:p><w:r><w:pict><v:shape id="Box1" style="'+position+'width:200pt;height:100pt"><v:textbox><w:txbxContent>'+p('Story first')+p('Story middle')+p('Story last')+'</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>'
   body+='<w:p><w:r><w:pict><v:shape id="Box2" style="width:200pt;height:100pt"><v:textbox><w:txbxContent>'+p('Other story')+'</w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p>'
  else:
   body+='<w:p><w:r><w:t>References</w:t></w:r><w:r><w:'+kind+'Reference w:id="1"/></w:r><w:r><w:'+kind+'Reference w:id="2"/></w:r></w:p>'
   rels+='<Relationship Id="notes" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/'+kind+'s" Target="'+kind+'s.xml"/>'
   z.writestr('word/'+kind+'s.xml','<w:'+kind+'s xmlns:w="'+w+'"><w:'+kind+' w:id="1">'+p('Story first')+p('Story middle')+p('Story last')+'</w:'+kind+'><w:'+kind+' w:id="2">'+p('Other story')+'</w:'+kind+'></w:'+kind+'s>')
  z.writestr('word/_rels/document.xml.rels','<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">'+rels+'</Relationships>')
  body+=p('Body after')
  z.writestr('word/document.xml','<w:document xmlns:w="'+w+'" xmlns:v="urn:schemas-microsoft-com:vml"><w:body>'+body+'<w:sectPr><w:pgSz w:w="12240" w:h="8000"/><w:pgMar w:top="720" w:right="1440" w:bottom="720" w:left="1440"/></w:sectPr></w:body></w:document>')
`, temporary, process.env.FLOATING_BOX||'0']);
async function saved(page) {
  const downloading = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const file = await (await downloading).path();
  return JSON.parse(execFileSync('python3', ['-c', `
import sys,zipfile,json,xml.etree.ElementTree as E
ns={'w':'http://schemas.openxmlformats.org/wordprocessingml/2006/main','v':'urn:schemas-microsoft-com:vml','wp':'http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing'}
def position(p):
 anchor=p.find('.//wp:anchor',ns)
 if anchor is not None:
  return [round(int(anchor.find('wp:position'+axis+'/wp:posOffset',ns).text)/9525,3) for axis in ['H','V']]
 shape=p.find('.//v:shape',ns)
 if shape is None: return None
 style=dict(item.split(':',1) for item in shape.get('style').split(';') if ':' in item)
 if style.get('position')!='absolute': return None
 return [round(float(style[axis].removesuffix('pt'))*4/3,3) for axis in ['margin-left','margin-top']]
def geometry(p):
 extent=p.find('.//wp:extent',ns)
 if extent is not None: return [round(int(extent.get(axis))/9525,3) for axis in ['cx','cy']]
 shape=p.find('.//v:shape',ns)
 style=dict(item.split(':',1) for item in shape.get('style').split(';') if ':' in item)
 return [round(float(style[axis].removesuffix('pt'))*4/3,3) for axis in ['width','height']]
with zipfile.ZipFile(sys.argv[1]) as z:
 result={}
 for name in ['document','footnotes','endnotes']:
  part='word/'+name+'.xml'
  if part not in z.namelist(): continue
  root=E.fromstring(z.read(part))
  paths={'document':'./w:body/w:p','footnotes':'.//w:p','endnotes':'.//w:p'}
  result[name]=[''.join(t.text or '' for t in p.findall('./w:r/w:t',ns)) for p in root.findall(paths[name],ns)]
  if name=='document': result['position']=[position(p) for p in root.findall('./w:body/w:p',ns) if p.find('.//w:txbxContent',ns) is not None]
  if name=='document': result['geometry']=[geometry(p) for p in root.findall('./w:body/w:p',ns) if p.find('.//w:txbxContent',ns) is not None]
  if name=='document': result['textbox']=[''.join(t.text or '' for t in p.findall('.//w:t',ns)) for p in root.findall('.//w:txbxContent/w:p',ns)]
 print(json.dumps(result))
`,file], {encoding:'utf8'}));
}
(async()=>{
 const browser=await chromium.launch({headless:process.env.HEADLESS==='1',executablePath:process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,args:['--enable-unsafe-webgpu']});
 try {
  for(const kind of (process.env.STORY_KINDS||'footnote,endnote,textbox').split(',')) {
   const context=await browser.newContext({viewport:{width:1280,height:1000}});
   const page=await context.newPage();
   const errors=[];
   page.on('pageerror',error=>{errors.push(error.message);console.error(error.message);});
   page.on('console',message=>{if(message.type()==='error') console.error(message.text());});
   await page.route(`**/samples/${kind}.docx`,route=>route.fulfill({body:fs.readFileSync(path.join(temporary,`${kind}.docx`))}));
   await page.goto(new URL(`?open=${kind}.docx`,process.argv[2]||'http://localhost:8765/').href);
   await page.waitForFunction(()=>!document.getElementById('loading'),null,{timeout:90000});
   await page.waitForTimeout(3000);
   const firstY={footnote:667,endnote:388,textbox:337}[kind];
   const story=kind==='textbox'?'textbox':`${kind}s`;
   await page.mouse.click(600,firstY);
   const initial=await saved(page);
   assert.deepEqual(initial.document,kind==='textbox'?['Body before','','','Body after']:['Body before','References','Body after']);
   assert.deepEqual(initial[story],['Story first','Story middle','Story last','Other story']);
   const expected=(texts)=>({...initial,[story]:texts});
   async function undo() {
    await page.keyboard.press('Control+z');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),initial,`${kind}: undo must restore every story`);
   }
   if(kind==='textbox' && process.env.OBJECT_CLIPBOARD==='1') {
    await context.grantPermissions(['clipboard-read','clipboard-write']);
    await page.mouse.click(600,firstY+20);
    await page.keyboard.press('Home');
    await page.keyboard.press('Shift+ArrowDown');
    await page.mouse.click(565,410);
    await page.evaluate(()=>navigator.clipboard.writeText('OBJECT COPY SENTINEL'));
    await page.keyboard.press('Control+c');
    await page.waitForFunction(async()=>await navigator.clipboard.readText()==='\ufffc');
    assert.deepEqual(await saved(page),initial,'frame copy must preserve the remembered text range');
    await page.keyboard.press('Meta+v');
    await page.waitForTimeout(500);
    const duplicated={...initial,textbox:[...initial.textbox.slice(0,3),...initial.textbox]};
    assert.deepEqual(await saved(page),duplicated,'paste on a selected frame must insert a complete box without replacing remembered text');
    await undo();
    for(const shortcut of ['Meta+Alt+Shift+v','Control+Meta+v']) {
     await page.mouse.click(565,410);
     await page.keyboard.press(shortcut);
     await page.waitForTimeout(1000);
     if(shortcut==='Control+Meta+v') {
      await page.keyboard.press('Enter');
      await page.waitForTimeout(500);
     }
     assert.deepEqual(await saved(page),duplicated,`${shortcut}: async paste must keep the object and every text range`);
     await undo();
    }
    await page.mouse.click(600,firstY+20);
    await page.keyboard.press('Home');
    await page.keyboard.press('Shift+ArrowDown');
    await page.mouse.click(565,410);
    await page.evaluate(()=>navigator.clipboard.writeText('OBJECT CUT SENTINEL'));
    await page.keyboard.press('Control+x');
    await page.waitForFunction(async()=>await navigator.clipboard.readText()==='\ufffc');
    const deleted={...initial,textbox:['Other story'],geometry:[initial.geometry[1]],position:[initial.position[1]]};
    assert.deepEqual(await saved(page),deleted,'cut must remove the selected frame and retain every text range outside it');
    await page.keyboard.press('Meta+v');
    await page.waitForTimeout(500);
    assert.deepEqual(await saved(page),initial,'a cut frame must paste with all its paragraphs and geometry');
    await page.keyboard.press('Control+z');
    assert.deepEqual(await saved(page),deleted,'undo paste must leave the cut in place');
    await undo();
    await page.mouse.click(565,410);
    await page.evaluate(()=>navigator.clipboard.writeText('EXTERNAL'));
    await page.keyboard.press('Meta+v');
    await page.waitForTimeout(400);
    assert.deepEqual(await saved(page),initial,'external text must not reuse a stale object copy or replace remembered text');
    await page.keyboard.press('Control+c');
    await page.waitForFunction(async()=>await navigator.clipboard.readText()==='\ufffc');
    await page.mouse.click(600,312);
    await page.keyboard.press('Home');
    await page.keyboard.press('Shift+End');
    await page.keyboard.press('Meta+v');
    await page.waitForTimeout(500);
    const replaced={...duplicated,document:['','','','Body after'],geometry:[initial.geometry[0],...initial.geometry],position:[initial.position[0],...initial.position]};
    assert.deepEqual(await saved(page),replaced,'object paste must replace selected body text while retaining every existing box');
    await undo();
    await page.keyboard.press('Control+Shift+z');
    assert.deepEqual(await saved(page),replaced,'redo must restore text replacement and the pasted object together');
    await undo();
    assert.deepEqual(errors,[]);
    console.log('textbox: frame copy/cut/paste, remembered range isolation, geometry, clipboard identity, text replacement and undo/redo passed');
    await context.close();
    continue;
   }
   if(kind==='textbox' && process.env.FLOATING_BOX==='1') {
    assert.deepEqual(initial.position,[[144,96],null]);
    await page.mouse.click(878,440);
    let position=[...initial.position[0]];
    for(const [key,dx,dy] of [['ArrowRight',7.5,0],['ArrowDown',0,7.5],['Alt+ArrowLeft',-1,0],['Alt+ArrowUp',0,-1]]) {
     await page.keyboard.press(key);
     await page.waitForTimeout(250);
     position=[position[0]+dx,position[1]+dy];
     const moved={...initial,position:[position,initial.position[1]]};
     assert.deepEqual(await saved(page),moved,`${key} must move only the selected frame and retain its text`);
    }
    for(let i=0;i<4;i++) await page.keyboard.press('Control+z');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),initial,'undo must restore the original frame position');
    for(let i=0;i<4;i++) await page.keyboard.press('Control+y');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),{...initial,position:[position,initial.position[1]]},'redo must restore all four nudges');
    for(let i=0;i<4;i++) await page.keyboard.press('Control+z');
    await page.keyboard.press('ArrowRight');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),{...initial,position:[[151.5,96],initial.position[1]]},'movement after undo must start at the restored frame');
    await page.mouse.click(485,95);
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),initial,'ribbon Undo must restore the original position');
    await page.mouse.click(511,95);
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),{...initial,position:[[151.5,96],initial.position[1]]},'ribbon Redo must restore the movement');
    await page.keyboard.press('ArrowDown');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),{...initial,position:[[151.5,103.5],initial.position[1]]},'movement after ribbon history must use the current frame');
    console.log('textbox: floating-frame arrows, Alt fine movement, saved text/geometry isolation and keyboard/ribbon undo/redo passed');
    assert.deepEqual(errors,[]);
    await context.close();
    continue;
   }
   for(const shift of ['', 'Shift+']) {
    await page.mouse.click(600,firstY);
    await page.keyboard.press('Home');
    await page.keyboard.press(`${shift}ArrowUp`);
    await page.keyboard.insertText('|TOP|');
    await page.waitForTimeout(250);
    const top=[...initial[story]];top[0]='|TOP|'+top[0];
    assert.deepEqual(await saved(page),expected(top),`${kind}: ${shift}Up must stay at the first line`);
    await undo();
    await page.mouse.click(600,firstY+40);
    await page.keyboard.press('End');
    await page.keyboard.press(`${shift}ArrowDown`);
    await page.keyboard.insertText('|BOTTOM|');
    await page.waitForTimeout(250);
    const bottom=[...initial[story]];bottom[2]+='|BOTTOM|';
    assert.deepEqual(await saved(page),expected(bottom),`${kind}: ${shift}Down must stay in the current story`);
    await undo();
   }
   await page.mouse.click(600,firstY+20);
   await page.keyboard.press('Home');
   await page.keyboard.press('ArrowDown');
   await page.keyboard.press('ArrowUp');
   await page.keyboard.insertText('|ROUND|');
   await page.waitForTimeout(250);
   const round=[...initial[story]];round[1]='|ROUND|'+round[1];
   assert.deepEqual(await saved(page),expected(round),`${kind}: arrows must move between internal paragraphs`);
   await undo();
   await page.mouse.click(600,firstY+20);
   await page.keyboard.press('Home');
   await page.keyboard.press('Shift+ArrowDown');
   await page.keyboard.insertText('|SELECT|');
   await page.waitForTimeout(250);
   const selected=[...initial[story]];selected.splice(1,2,'|SELECT|Story last');
   assert.deepEqual(await saved(page),expected(selected),`${kind}: Shift+Down must select to the next internal paragraph`);
   await undo();
   if(kind==='textbox') {
    await page.mouse.click(565,410);
    await page.keyboard.insertText('|BORDER|');
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),initial,'typing with the box selected must preserve the previous text selection');
    const input=await context.newCDPSession(page);
    await input.send('Input.imeSetComposition',{text:'日本語',selectionStart:3,selectionEnd:3});
    await input.send('Input.insertText',{text:'日本語'});
    await input.detach();
    await page.waitForTimeout(250);
    assert.deepEqual(await saved(page),initial,'IME with the box selected must preserve text');
    for(const html of [false,true]) {
     await page.evaluate(html=>{
      const data=new DataTransfer();
      data.setData('text/plain','|PASTE|');
      if(html) data.setData('text/html','<p><b>|PASTE|</b></p>');
      document.activeElement.dispatchEvent(new ClipboardEvent('paste',{clipboardData:data,bubbles:true,cancelable:true}));
     },html);
     await page.waitForTimeout(250);
     assert.deepEqual(await saved(page),initial,'plain and rich paste with the box selected must preserve text');
    }
    // Clicking the text returns keyboard input to the box content.
    await page.mouse.click(600,firstY+20);
    await page.keyboard.press('Home');
    await page.keyboard.insertText('|RETURN|');
    const returned=[...initial[story]];returned[1]='|RETURN|'+returned[1];
    assert.deepEqual(await saved(page),expected(returned),'clicking content must resume text editing');
    await undo();
    for(const key of ['Delete','Backspace']) {
     // Leave a range selected before selecting the frame.
     await page.mouse.click(600,firstY+20);
     await page.keyboard.press('Home');
     await page.keyboard.press('Shift+ArrowDown');
     await page.mouse.click(565,410);
     await page.keyboard.press(key);
     await page.waitForTimeout(250);
     const deleted={...initial,textbox:['Other story'],geometry:[initial.geometry[1]],position:[initial.position[1]]};
     assert.deepEqual(await saved(page),deleted,`${key} must delete only the selected box`);
     await undo();
     await page.keyboard.press('Control+y');
     await page.waitForTimeout(250);
     assert.deepEqual(await saved(page),deleted,`${key} redo must delete the box again`);
     await undo();
    }
    await page.mouse.click(600,firstY+20);
    await page.keyboard.press('Home');
    await page.mouse.click(565,410);
    await page.keyboard.press('Escape');
    await page.keyboard.insertText('|ESC|');
    const escaped=[...initial[story]];escaped[1]='|ESC|'+escaped[1];
    assert.deepEqual(await saved(page),expected(escaped),'Escape must release frame selection back to text');
    await undo();
    await page.mouse.move(829,454);
    await page.mouse.down();
    await page.mouse.move(849,464,{steps:5});
    await page.mouse.up();
    await page.waitForTimeout(250);
    const resized=await saved(page);
    assert.ok(Math.abs(resized.geometry[0][0]-initial.geometry[0][0]-20)<0.1,'resize must persist the new width');
    assert.ok(Math.abs(resized.geometry[0][1]-initial.geometry[0][1]-10)<0.1,'resize must persist the new height');
    assert.deepEqual(resized.geometry[1],initial.geometry[1],'resize must preserve the other box frame');
    assert.deepEqual({...resized,geometry:initial.geometry},initial,'resize must preserve each text box exactly once');
    await undo();
   }
   console.log(`${kind}: Up/Down and Shift boundaries, internal movement/selection, saved DOCX isolation and undo passed${kind==='textbox'?'; frame typing/IME/paste preserve text, content click resumes editing, Delete/Backspace and history isolate the selected box, Escape returns to text, resize persists one box and undo restores its frame':''}`);
   assert.deepEqual(errors,[]);
   await context.close();
  }
 }finally{await browser.close();fs.rmSync(temporary,{recursive:true,force:true});}
})().catch(error=>{console.error(error);process.exitCode=1;});
