#!/usr/bin/env node
// Build/serve the gpui web shell first. Requires Playwright via NODE_PATH
// and python3 (standard-library ZIP support). HEADLESS=1 is optional.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');

const base = process.argv[2] || 'http://localhost:8765/';
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jade-shortcuts-'));
const fixture = path.join(temporary, 'shortcuts.docx');
execFileSync('python3', ['-c', `
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as z:
 z.writestr('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
 z.writestr('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
 z.writestr('word/document.xml', '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Paragraph</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="5000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>')
`, fixture]);

const pause = page => page.waitForTimeout(250);
async function saved(page, modifier) {
  const downloading = page.waitForEvent('download');
  await page.keyboard.press(`${modifier}+s`);
  const download = await downloading;
  const file = await download.path();
  page.lastSavedDocx = fs.readFileSync(file);
  return JSON.parse(execFileSync('python3', ['-c', `
import sys, zipfile, json, xml.etree.ElementTree as E
ns = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
attr = '{' + ns['w'] + '}'
with zipfile.ZipFile(sys.argv[1]) as z:
 root = E.fromstring(z.read('word/document.xml'))
 settings = E.fromstring(z.read('word/settings.xml')) if 'word/settings.xml' in z.namelist() else None
 tracking = settings.find('w:trackRevisions', ns) if settings is not None else None
 p = root.find('w:body/w:p', ns)
 def value(tag, name='val'):
  node = p.find('w:pPr/w:' + tag, ns)
  return node.get(attr + name) if node is not None else None
 print(json.dumps({'tracking': tracking is not None and tracking.get(attr+'val') not in ['0', 'false', 'off'], 'paragraphs': [''.join(t.text or '' for t in p.findall('.//w:t', ns)) for p in root.findall('.//w:p', ns)], 'alignment': value('jc'), 'indent': value('ind', 'leftChars') or value('ind', 'startChars') or value('ind', 'left') or value('ind', 'start'), 'list': p.find('w:pPr/w:numPr', ns) is not None, 'breaks': [n.get(attr+'type') for n in root.findall('.//w:br', ns)], 'rows': len(root.findall('.//w:tr', ns)), 'trackedText': [''.join(n.itertext()) for n in root.findall('.//w:ins', ns)]}))
`, file], { encoding: 'utf8' }));
}

(async () => {
  const browser = await chromium.launch({
    headless: process.env.HEADLESS === '1',
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    args: ['--enable-unsafe-webgpu'],
  });
  try {
    for (const modifier of ['Meta', 'Control']) {
      const context = await browser.newContext({ viewport: { width: 1280, height: 1000 } });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.route('**/samples/shortcuts.docx', route => route.fulfill({ body: page.lastSavedDocx || fs.readFileSync(fixture) }));
      await page.goto(new URL('?open=shortcuts.docx', base).href);
      await page.waitForFunction(() => !document.getElementById('loading'), null, { timeout: 90000 });
      await page.waitForTimeout(3000);
      await page.mouse.click(800, 330);
      await page.keyboard.press(`${modifier}+Home`);
      await pause(page);
      const initial = await saved(page, modifier);
      assert.equal(initial.rows, 1);

      if (modifier === 'Meta') {
        await page.keyboard.press('ArrowRight');
        await page.keyboard.press('ArrowRight');
        await page.keyboard.press('Meta+Shift+ArrowDown');
        await page.keyboard.insertText('Selected');
        await pause(page);
        assert.deepEqual((await saved(page, modifier)).paragraphs, ['PaSelected', 'Cell', ''], 'Cmd+Shift+Down must stop at the current paragraph text end');
        await page.keyboard.press('Meta+z');
        await pause(page);
        assert.deepEqual(await saved(page, modifier), initial);
        await page.keyboard.press('Meta+Home');
      }

      await page.keyboard.press(`${modifier}+ArrowDown`);
      await page.keyboard.insertText('Next');
      await pause(page);
      assert.deepEqual((await saved(page, modifier)).paragraphs, ['Paragraph', 'NextCell', ''], `${modifier}+Down must move one paragraph`);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      assert.deepEqual(await saved(page, modifier), initial);
      await page.keyboard.press(`${modifier}+Home`);
      await page.keyboard.press(`${modifier}+ArrowDown`);
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press('ArrowRight');
      await page.keyboard.press(`${modifier}+ArrowUp`);
      await page.keyboard.insertText('Start');
      await pause(page);
      assert.deepEqual((await saved(page, modifier)).paragraphs, ['Paragraph', 'StartCell', ''], `${modifier}+Up must reach the current paragraph start`);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      await page.keyboard.press(`${modifier}+Home`);
      assert.deepEqual(await saved(page, modifier), initial);

      for (const [key, alignment] of [['e', 'center'], ['r', 'right'], ['j', 'both'], ['l', 'left']]) {
        await page.keyboard.press(`${modifier}+${key}`);
        await pause(page);
        assert.equal((await saved(page, modifier)).alignment, alignment);
      }
      await page.keyboard.press(`${modifier}+]`);
      await pause(page);
      assert.ok(Number((await saved(page, modifier)).indent) > 0);
      await page.keyboard.press(`${modifier}+[`);
      await pause(page);
      assert.equal(Number((await saved(page, modifier)).indent), 0);
      await page.keyboard.press(`${modifier}+Shift+l`);
      await pause(page);
      assert.ok((await saved(page, modifier)).list);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      assert.ok(!(await saved(page, modifier)).list, 'undo must remove the bullets');
      await page.keyboard.press(`${modifier}+Shift+e`);
      await pause(page);
      assert.ok((await saved(page, modifier)).tracking, 'recording switch must be saved in settings.xml');
      await page.keyboard.insertText('Tracked');
      await pause(page);
      assert.ok((await saved(page, modifier)).trackedText.join('').includes('Tracked'));
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      await page.keyboard.press(`${modifier}+Shift+e`);
      await pause(page);
      assert.ok(!(await saved(page, modifier)).tracking, 'stopping recording must clear the document switch');
      await page.keyboard.insertText('Plain');
      await pause(page);
      assert.ok(!(await saved(page, modifier)).trackedText.join('').includes('Plain'));
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);

      await page.keyboard.press(`${modifier}+Alt+Shift+Enter`);
      await pause(page);
      assert.deepEqual((await saved(page, modifier)).breaks, [], 'unassigned modifiers must not insert a page break');
      await page.keyboard.press(`${modifier}+Shift+Enter`);
      await pause(page);
      assert.deepEqual((await saved(page, modifier)).breaks, ['column']);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      await page.keyboard.press(`${modifier}+Enter`);
      await pause(page);
      assert.deepEqual((await saved(page, modifier)).breaks, ['page']);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);

      // Move from the first paragraph into the table's only cell.
      await page.keyboard.press(`${modifier}+Home`);
      await page.keyboard.press('ArrowDown');
      await page.keyboard.press(`${modifier}+Alt+Enter`);
      await pause(page);
      assert.equal((await saved(page, modifier)).rows, 2);
      await page.keyboard.press(`${modifier}+Shift+Backspace`);
      await pause(page);
      assert.equal((await saved(page, modifier)).rows, 1);
      await page.keyboard.press(`${modifier}+z`);
      await pause(page);
      assert.equal((await saved(page, modifier)).rows, 2, 'undo must restore the deleted row');
      await page.keyboard.press(`${modifier}+Shift+e`);
      await pause(page);
      assert.ok((await saved(page, modifier)).tracking);
      await page.reload();
      await page.waitForFunction(() => !document.getElementById('loading'), null, { timeout: 90000 });
      await page.waitForTimeout(3000);
      await page.mouse.click(800, 330);
      await page.keyboard.press(`${modifier}+Home`);
      await page.keyboard.insertText('Reopened');
      await pause(page);
      assert.ok((await saved(page, modifier)).trackedText.join('').includes('Reopened'), 'reopened DOCX must keep recording edits');
      assert.deepEqual(errors, []);
      console.log(`${modifier}: paragraph navigation, formatting, history, breaks, table rows and recording persistence passed`);
      await context.close();
    }
  } finally {
    await browser.close();
    fs.rmSync(temporary, { recursive: true, force: true });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
