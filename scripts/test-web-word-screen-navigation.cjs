#!/usr/bin/env node
// Requires the built gpui web shell, Playwright via NODE_PATH and python3.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jade-screen-'));
const fixture = path.join(temporary, 'screen.docx');
execFileSync('python3', ['-c', `
import sys,zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as z:
 z.writestr('[Content_Types].xml','<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
 z.writestr('_rels/.rels','<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
 paragraphs=''.join('<w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:t>Line%03d abcdefghij</w:t></w:r></w:p>'%i for i in range(120))
 z.writestr('word/document.xml','<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>'+paragraphs+'<w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>')
`, fixture]);
const pause = page => page.waitForTimeout(300);
async function saved(page) {
  const downloading = page.waitForEvent('download');
  await page.keyboard.press('Control+s');
  const file = await (await downloading).path();
  return JSON.parse(execFileSync('python3', ['-c', `
import sys,zipfile,json,xml.etree.ElementTree as E
with zipfile.ZipFile(sys.argv[1]) as z:
 root=E.fromstring(z.read('word/document.xml'))
 ns={'w':'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
 print(json.dumps([''.join(t.text or '' for t in p.findall('.//w:t',ns)) for p in root.findall('.//w:body/w:p',ns)]))
`, file], { encoding: 'utf8' }));
}
(async () => {
  const browser = await chromium.launch({headless: process.env.HEADLESS === '1', executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE, args:['--enable-unsafe-webgpu']});
  try {
    const distances = [];
    for (const height of [600, 1000]) {
      const context = await browser.newContext({viewport:{width:1280,height}});
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.route('**/samples/screen.docx', route => route.fulfill({body:fs.readFileSync(fixture)}));
      await page.goto(new URL('?open=screen.docx', process.argv[2] || 'http://localhost:8765/').href);
      await page.waitForFunction(() => !document.getElementById('loading'), null, {timeout:90000});
      await page.waitForTimeout(3000);
      await page.mouse.click(800,330);
      await page.keyboard.press('Control+Home');
      const initial = await saved(page);
      assert.equal(initial.length,120);
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('Meta+Shift+ArrowDown');
      await page.keyboard.insertText('|PARAGRAPH|');
      await pause(page);
      assert.deepEqual(await saved(page), ['Li|PARAGRAPH|', ...initial.slice(1)], 'Cmd+Shift+Down must retain the paragraph break');
      await page.keyboard.press('Control+z');
      await pause(page);
      assert.deepEqual(await saved(page), initial);
      for (const modifier of ['Control','Meta']) {
        await page.keyboard.press('Control+Home');
        await page.keyboard.press(`${modifier}+PageDown`);
        await page.keyboard.insertText('|PHYSICAL|');
        await pause(page);
        const physical = await saved(page);
        const physicalIndex = physical.findIndex(text => text.includes('|PHYSICAL|'));
        assert.ok(physicalIndex > 38 && physicalIndex < 65, `physical page navigation: ${modifier}, paragraph=${physicalIndex}`);
        assert.ok(physical[physicalIndex].startsWith('|PHYSICAL|'), 'physical navigation must reach the first column');
        await page.keyboard.press('Control+z');
        await page.keyboard.press(`${modifier}+PageUp`);
        await page.keyboard.insertText('|FIRST|');
        await pause(page);
        assert.ok((await saved(page))[0].startsWith('|FIRST|'));
        await page.keyboard.press('Control+z');
        await pause(page);
        assert.deepEqual(await saved(page),initial);
        await page.keyboard.press('Control+Home');
        await page.keyboard.press(`${modifier}+Shift+PageDown`);
        await page.keyboard.insertText('|PAGESELECT|');
        await pause(page);
        const pageSelected = await saved(page);
        assert.equal(pageSelected.length, initial.length - physicalIndex);
        assert.equal(pageSelected[0], '|PAGESELECT|' + initial[physicalIndex]);
        await page.keyboard.press('Control+z');
        await pause(page);
        assert.deepEqual(await saved(page), initial);
      }
      await page.keyboard.press('Control+Home');
      await page.keyboard.press('PageDown');
      await page.keyboard.insertText('|PAGE|');
      await pause(page);
      const down = await saved(page);
      const index = down.findIndex(text => text.includes('|PAGE|'));
      assert.ok(index > 3 && index < 60, `PageDown must move by a screen: height=${height}, paragraph=${index}`);
      distances.push(index);
      await page.keyboard.press('Control+z');
      await pause(page);
      assert.deepEqual(await saved(page),initial);
      await page.keyboard.press('PageUp');
      await page.keyboard.insertText('|UP|');
      await pause(page);
      assert.ok((await saved(page)).findIndex(text => text.includes('|UP|')) <= 2, 'PageUp must return near the first line');
      await page.keyboard.press('Control+z');
      await page.keyboard.press('Control+Home');
      await pause(page);
      await page.keyboard.press('Shift+PageDown');
      await page.keyboard.insertText('|SELECT|');
      await pause(page);
      const selected = await saved(page);
      assert.ok(selected.length < initial.length - 3, 'Shift+PageDown must select to the screen edge');
      assert.ok(selected[0].startsWith('|SELECT|'));
      assert.ok(selected.at(-1).includes('Line119'));
      await page.keyboard.press('Control+z');
      await pause(page);
      assert.deepEqual(await saved(page),initial);
      await page.keyboard.press('Control+End');
      await pause(page);
      await page.keyboard.press('Shift+PageUp');
      await page.keyboard.insertText('|SELECTUP|');
      await pause(page);
      const selectedUp = await saved(page);
      assert.ok(selectedUp.length < initial.length - 3, 'Shift+PageUp must select to the screen edge');
      assert.equal(selectedUp[0],initial[0]);
      assert.ok(selectedUp.at(-1).endsWith('|SELECTUP|'));
      await page.keyboard.press('Control+z');
      await pause(page);
      assert.deepEqual(await saved(page),initial);
      assert.deepEqual(errors,[]);
      console.log(`height=${height}: PageDown moved ${index} paragraphs; Ctrl/Meta physical pages, page selection, PageUp, screen selection and undo passed`);
      await context.close();
    }
    assert.ok(distances[1] > distances[0] + 3, `navigation must follow viewport height: ${distances}`);
  } finally {
    await browser.close();
    fs.rmSync(temporary,{recursive:true,force:true});
  }
})().catch(error => { console.error(error); process.exitCode=1; });
