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
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jade-clipboard-'));
const fixture = path.join(temporary, 'clipboard.docx');
execFileSync('python3', ['-c', `
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as z:
 z.writestr('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
 z.writestr('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
 z.writestr('word/document.xml', '<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Paragraph</w:t></w:r></w:p><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="5000"/></w:tblGrid><w:tr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="dxa"/></w:tcPr><w:p><w:r><w:t>Cell</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>')
`, fixture]);

const pause = page => page.waitForTimeout(250);
async function saved(page) {
  const downloading = page.waitForEvent('download');
  await page.keyboard.press('Meta+s');
  const file = await (await downloading).path();
  return JSON.parse(execFileSync('python3', ['-c', `
import sys, zipfile, json, xml.etree.ElementTree as E
ns = {'w': 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'}
a = '{' + ns['w'] + '}'
with zipfile.ZipFile(sys.argv[1]) as z:
 root = E.fromstring(z.read('word/document.xml'))
 runs = []
 for r in root.findall('.//w:r', ns):
  text = ''.join(t.text or '' for t in r.findall('w:t', ns))
  def prop(tag):
   n = r.find('w:rPr/w:' + tag, ns)
   if tag in ['b', 'i']: return n is not None and n.get(a+'val', 'true') not in ['0', 'false', 'off']
   return n.get(a+'val', 'true') if n is not None else None
  runs.append({'text':text, 'bold':prop('b'), 'italic':prop('i'), 'underline':prop('u') not in [None, 'none', 'false', '0'], 'color':prop('color'), 'size':prop('sz')})
 paragraphs = [''.join(t.text or '' for t in p.findall('.//w:t', ns)) for p in root.findall('.//w:p', ns)]
 print(json.dumps({'text': ''.join(r['text'] for r in runs), 'runs':runs, 'paragraphs':paragraphs, 'softBreaks':[len(p.findall('.//w:br', ns)) for p in root.findall('.//w:p', ns)]}))
`, file], { encoding: 'utf8' }));
}
async function paste(page, formats, keyboard = false) {
  if (keyboard) {
    await page.context().grantPermissions(['clipboard-read', 'clipboard-write']);
    await page.evaluate(async formats => {
      await navigator.clipboard.write([new ClipboardItem(Object.fromEntries(
        Object.entries(formats).map(([type, value]) => [type, new Blob([value], {type})]),
      ))]);
    }, formats);
    await page.keyboard.press('Meta+v');
    await pause(page);
    return;
  }
  // Deliver the browser's ordinary paste event, including both MIME types.
  // The app consumes it through the focused GPUI input handler.
  await page.evaluate(formats => {
    const data = new DataTransfer();
    for (const [type, value] of Object.entries(formats)) data.setData(type, value);
    document.activeElement.dispatchEvent(new ClipboardEvent('paste', {
      clipboardData: data, bubbles: true, cancelable: true,
    }));
  }, formats);
  await pause(page);
}
async function copyToOtherApp(page) {
  await page.keyboard.press('Meta+Home');
  for (let i = 0; i < 'Rich Paste'.length; i++) await page.keyboard.press('Shift+ArrowRight');
  await page.evaluate(() => navigator.clipboard.writeText('copy sentinel'));
  await page.keyboard.press('Meta+c');
  await page.waitForFunction(async () => await navigator.clipboard.readText() === 'Rich Paste');
  const types = await page.evaluate(async () => (await navigator.clipboard.read())[0].types);
  assert.ok(types.includes('text/html'), `copy must publish HTML: ${types}`);
  const receiver = await page.context().newPage();
  try {
    await receiver.route('**/clipboard-receiver', route => route.fulfill({
      contentType:'text/html', body:'<!doctype html><div id="editor" contenteditable="true"></div>',
    }));
    await receiver.goto(new URL('clipboard-receiver', base).href);
    await receiver.locator('#editor').focus();
    await receiver.keyboard.press('Meta+v');
    await pause(receiver);
    const received = await receiver.locator('#editor').evaluate(editor => ({
      text:editor.innerText,
      runs:[...editor.querySelectorAll('*')].map(node => {
        const style = getComputedStyle(node);
        return {text:node.textContent, bold:Number(style.fontWeight) >= 700,
          italic:style.fontStyle, color:style.color, size:style.fontSize};
      }),
    }));
    assert.equal(received.text.trim(), 'Rich Paste');
    assert.ok(received.runs.some(r => r.text.includes('Rich') && r.bold), JSON.stringify(received));
    assert.ok(received.runs.some(r => r.text.includes('Paste') && r.italic === 'italic' &&
      r.color === 'rgb(18, 52, 86)' && r.size === '24px'), JSON.stringify(received));
  } finally { await receiver.close(); }
  await page.bringToFront();
  // Both unsupported constructors and a rejected rich write retain plain text.
  for (const failure of ['constructor', 'rejection']) {
    await page.evaluate(failure => {
      window.savedClipboardItem = window.ClipboardItem;
      window.savedClipboardWrite = navigator.clipboard.write;
      if (failure === 'constructor') window.ClipboardItem = undefined;
      else navigator.clipboard.write = () => Promise.reject(new DOMException('HTML unavailable', 'NotSupportedError'));
      return navigator.clipboard.writeText('fallback sentinel');
    }, failure);
    await page.keyboard.press('Meta+c');
    await page.waitForFunction(async () => await navigator.clipboard.readText() === 'Rich Paste');
    await page.evaluate(() => {
      window.ClipboardItem = window.savedClipboardItem;
      navigator.clipboard.write = window.savedClipboardWrite;
    });
  }
  // Restore a genuine rich copy before testing same-document paste defaults.
  await page.keyboard.press('Meta+c');
  await page.waitForFunction(async () => (await navigator.clipboard.read())[0].types.includes('text/html'));
  const html = await page.evaluate(async () =>
    (await (await navigator.clipboard.read())[0].getType('text/html')).text());
  await page.keyboard.press('ArrowRight');
  return html;
}
(async () => {
  const browser = await chromium.launch({
    headless: process.env.HEADLESS === '1',
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    args: ['--enable-unsafe-webgpu'],
  });
  try {
    const context = await browser.newContext({viewport: {width:1280, height:1000}});
    await context.addInitScript(() => localStorage.setItem(
      'jade-office.settings.options.txt', 'paste_same=merge\npaste_between=keep\npaste_other=keep\n',
    ));
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.route('**/samples/clipboard.docx', route => route.fulfill({body:fs.readFileSync(fixture)}));
    await page.goto(new URL('?open=clipboard.docx', base).href);
    await page.waitForFunction(() => !document.getElementById('loading'), null, {timeout:90000});
    await page.waitForTimeout(3000);
    await page.mouse.click(800,330);
    await page.keyboard.press('Meta+Home');
    const initial = await saved(page);
    await paste(page, {
      'text/plain':'Rich Paste',
      'text/html':'<html><body><!--StartFragment--><p><strong>Rich</strong> <span style="font-style:italic;color:#123456;font-size:18pt">Paste</span></p><!--EndFragment--></body></html>',
    }, true);
    const rich = await saved(page);
    assert.ok(rich.text.includes('Rich Paste'), JSON.stringify(rich));
    assert.ok(rich.runs.some(r => r.text.includes('Rich') && r.bold), JSON.stringify(rich));
    assert.ok(rich.runs.some(r => r.text.includes('Paste') && r.italic && r.color?.toUpperCase() === '123456' && r.size === '36'), JSON.stringify(rich));
    const copiedHTML = await copyToOtherApp(page);
    await page.keyboard.insertText('After');
    await pause(page);
    assert.ok((await saved(page)).text.includes('Rich PasteAfter'), 'caret must follow pasted text');
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), initial, 'one undo must remove the complete rich paste');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), rich, 'redo must restore text and formatting');
    await page.keyboard.press('Meta+z');
    await pause(page);
    await paste(page, {'text/html':'<p><b>HTML only</b></p>'});
    const only = await saved(page);
    assert.ok(only.runs.some(r => r.text.includes('HTML only') && r.bold), JSON.stringify(only));
    await page.keyboard.press('Meta+z');
    await pause(page);
    await paste(page, {'text/plain':'Plain fallback', 'text/html':'<!-- no fragment content -->'});
    assert.ok((await saved(page)).text.includes('Plain fallback'));
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.keyboard.press('Meta+Home');
    await paste(page, {
      'text/plain':'\nFirst\n\nSecond\n',
      'text/html':'<div><p></p><p>First</p><p> </p><p>Second</p><p><br></p></div>',
    }, true);
    const blankParagraphs = await saved(page);
    assert.deepEqual(blankParagraphs.paragraphs.slice(0, 5), ['', 'First', '', 'Second', 'Paragraph'],
      'HTML paste must retain leading, middle and trailing empty paragraphs');
    await page.keyboard.type('After');
    await pause(page);
    assert.equal((await saved(page)).paragraphs[4], 'AfterParagraph', 'caret follows the trailing empty paragraph');
    const typedAfterPaste = await saved(page);
    await page.waitForTimeout(3100);
    await page.keyboard.type('Z');
    await pause(page);
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), typedAfterPaste, 'a pause starts a separate typing action');
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), blankParagraphs, 'consecutive key events undo as one typing action');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), typedAfterPaste, 'redo restores the complete typing run');
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), initial, 'empty paragraph paste is one undo step');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), blankParagraphs, 'redo restores all empty paragraphs');
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.keyboard.press('Meta+Home');
    await paste(page, {
      'text/plain':'First\nSecond\n\nLast',
      'text/html':'<p>First<br><b>Second</b><br><br>Last</p>',
    }, true);
    const softLines = await saved(page);
    assert.equal(softLines.paragraphs.length, initial.paragraphs.length, 'br must not add paragraphs');
    assert.equal(softLines.paragraphs[0], 'FirstSecondLastParagraph');
    assert.equal(softLines.softBreaks[0], 3, 'HTML br must save as paragraph-internal DOCX breaks');
    assert.ok(softLines.runs.some(run => run.text === 'Second' && run.bold), 'formatting survives across a soft break');
    await page.keyboard.insertText('X');
    await pause(page);
    assert.equal((await saved(page)).paragraphs[0], 'FirstSecondLastXParagraph', 'caret follows the last soft line');
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), softLines);
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), initial, 'soft line paste is one undo step');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), softLines, 'redo restores soft breaks and formatting');
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.keyboard.press('Meta+Home');
    await paste(page, {
      'text/plain':'plain色',
      'text/html':`<p title="a > b style='font-style:italic'">plain<b title='x > y'>色</b></p>`,
    }, true);
    const quotedAttributes = await saved(page);
    assert.equal(quotedAttributes.paragraphs[0], 'plain色Paragraph', 'attribute contents must not become text');
    assert.ok(quotedAttributes.runs.some(run => run.text === '色' && run.bold));
    assert.ok(!quotedAttributes.runs.some(run => run.text.includes('plain') && run.italic), 'title text must not supply character formatting');
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), initial, 'attribute-rich paste is one undo step');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), quotedAttributes);
    await page.keyboard.press('Meta+z');
    await pause(page);
    const spacedText = '10\u202f000\u00a0円　　end\u2009';
    await page.keyboard.press('Meta+Home');
    await page.evaluate(async text => {
      await navigator.clipboard.write([new ClipboardItem({
        'text/plain': new Blob([text], {type:'text/plain'}),
        'text/html': new Blob(['<p><b>10&#x202f;000&nbsp;円</b>　　end&#x2009;</p>'], {type:'text/html'}),
      })]);
    }, spacedText);
    await page.keyboard.press('Meta+v');
    await pause(page);
    const spaced = await saved(page);
    assert.equal(spaced.paragraphs[0], spacedText + 'Paragraph', 'Unicode spaces must survive HTML paste and DOCX save');
    assert.ok(spaced.runs.some(run => run.text === '10\u202f000\u00a0円' && run.bold));
    await page.keyboard.insertText('X');
    await pause(page);
    assert.equal((await saved(page)).paragraphs[0], spacedText + 'XParagraph', 'caret accounts for multibyte spaces');
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), spaced);
    await page.keyboard.press('Meta+z');
    await pause(page);
    assert.deepEqual(await saved(page), initial, 'Unicode space paste is one undo step');
    await page.keyboard.press('Meta+Shift+z');
    await pause(page);
    assert.deepEqual(await saved(page), spaced);
    await page.keyboard.press('Meta+z');
    await pause(page);
    await page.evaluate(async html => {
      await navigator.clipboard.write([new ClipboardItem({
        'text/plain': new Blob(['Rich Paste'], {type:'text/plain'}),
        'text/html': new Blob([html], {type:'text/html'}),
      })]);
    }, copiedHTML);
    for (const [shortcut, marker] of [
      ['Meta+v', 'DefaultAfter'],
      ['Meta+Alt+Shift+v', 'MergedAfter'],
      ['Control+Meta+v', 'SpecialAfter'],
    ]) {
      await page.keyboard.press('Meta+Home');
      await page.keyboard.press(shortcut);
      await page.waitForTimeout(1000);
      if (shortcut === 'Control+Meta+v') {
        await page.keyboard.press('Enter');
        await page.waitForTimeout(500);
      }
      const pasted = await saved(page);
      assert.ok(pasted.text.startsWith('Rich Paste'), JSON.stringify(pasted));
      assert.ok(pasted.runs.some(r => r.text.includes('Rich') && r.bold), JSON.stringify(pasted));
      const italic = pasted.runs.find(r => r.text.includes('Paste') && r.italic);
      assert.ok(italic, JSON.stringify(pasted));
      if (shortcut === 'Control+Meta+v') {
        assert.equal(italic.color?.toUpperCase(), '123456');
        assert.equal(italic.size, '36');
      } else {
        assert.notEqual(italic.color?.toUpperCase(), '123456', `${shortcut}: merge color`);
        assert.notEqual(italic.size, '36', `${shortcut}: merge size`);
      }
      await page.keyboard.insertText(marker);
      await pause(page);
      assert.ok((await saved(page)).text.startsWith('Rich Paste' + marker),
        `${shortcut}: caret must follow pasted text`);
      await page.keyboard.press('Meta+z');
      await pause(page);
      await page.keyboard.press('Meta+z');
      await pause(page);
      assert.deepEqual(await saved(page), initial, `${shortcut}: atomic paste undo`);
      await page.keyboard.press('Meta+Shift+z');
      await pause(page);
      assert.deepEqual(await saved(page), pasted, `${shortcut}: paste redo`);
      await page.keyboard.press('Meta+z');
      await pause(page);
    }
    // A copy made elsewhere can contain the same text as the cached app copy.
    // Its actual formats must win over that stale in-process copy.
    for (const variant of ['plain', 'html', 'edited']) {
      for (const shortcut of ['Meta+v', 'Meta+Alt+Shift+v']) {
        await page.keyboard.press('Meta+Home');
        await page.evaluate(async ({variant, copiedHTML}) => {
          const data = {'text/plain': new Blob(['Rich Paste'], {type:'text/plain'})};
          let html = '<p><u>Rich Paste</u></p>';
          if (variant === 'edited') {
            const document = new DOMParser().parseFromString(copiedHTML, 'text/html');
            const paragraph = document.querySelector('[data-jade-clipboard]');
            if (!paragraph) throw new Error('app copy must carry an identity marker');
            paragraph.innerHTML = '<u>Rich Paste</u>';
            html = document.documentElement.outerHTML;
          }
          if (variant !== 'plain') data['text/html'] = new Blob([html], {type:'text/html'});
          await navigator.clipboard.write([new ClipboardItem(data)]);
        }, {variant, copiedHTML});
        await page.keyboard.press(shortcut);
        await page.waitForTimeout(1000);
        const actual = await saved(page);
        assert.ok(actual.text.startsWith('Rich Paste'), JSON.stringify(actual));
        assert.ok(actual.runs.filter(r => r.text.includes('Rich') || r.text.includes('Paste'))
          .every(r => !r.bold && !r.italic), `${shortcut}: external copy must replace stale formatting`);
        if (variant !== 'plain') assert.ok(actual.runs.some(r => r.text.includes('Rich Paste') && r.underline),
          `${shortcut}: external underline must survive`);
        await page.keyboard.press('Meta+z');
        await pause(page);
        assert.deepEqual(await saved(page), initial);
      }
    }
    assert.deepEqual(errors, []);
    for (const mode of ['text', 'merge']) {
      const externalContext = await browser.newContext({viewport: {width:1280, height:1000}});
      try {
        await externalContext.addInitScript(mode => localStorage.setItem(
          'jade-office.settings.options.txt',
          `paste_same=keep\npaste_between=keep\npaste_other=${mode}\n`,
        ), mode);
        const external = await externalContext.newPage();
        external.on('pageerror', error => errors.push(error.message));
        await external.route('**/samples/clipboard.docx', route => route.fulfill({body:fs.readFileSync(fixture)}));
        await external.goto(new URL('?open=clipboard.docx', base).href);
        await external.waitForFunction(() => !document.getElementById('loading'), null, {timeout:90000});
        await external.waitForTimeout(3000);
        await external.mouse.click(800,330);
        await external.keyboard.press('Meta+Home');
        const before = await saved(external);
        await paste(external, {
          'text/plain':'External',
          'text/html':'<p><b><i><span style="color:#123456;font-size:18pt">External</span></i></b></p>',
        }, true);
        const pasted = await saved(external);
        const run = pasted.runs.find(r => r.text.includes('External'));
        assert.ok(run, JSON.stringify(pasted));
        assert.equal(run.bold, mode === 'merge', `${mode}: external paste bold`);
        assert.equal(run.italic, mode === 'merge', `${mode}: external paste italic`);
        assert.notEqual(run.color?.toUpperCase(), '123456', `${mode}: destination color`);
        assert.notEqual(run.size, '36', `${mode}: destination size`);
        await external.keyboard.insertText('After');
        await pause(external);
        assert.ok((await saved(external)).text.startsWith('ExternalAfter'), `${mode}: external paste caret`);
        await external.keyboard.press('Meta+z');
        await pause(external);
        await external.keyboard.press('Meta+z');
        await pause(external);
        assert.deepEqual(await saved(external), before, `${mode}: external paste undo`);
        await external.keyboard.press('Meta+Shift+z');
        await pause(external);
        assert.deepEqual(await saved(external), pasted, `${mode}: external paste redo`);
      } finally { await externalContext.close(); }
    }
    assert.deepEqual(errors, []);
    console.log('PASS: rich clipboard import/export, typing undo groups, empty paragraphs, soft line breaks, Unicode spaces, HTML attribute boundaries, copy identity, write fallbacks, paste modes and Paste Special, external paste preferences, caret and atomic undo/redo');
  } finally {
    await browser.close();
    fs.rmSync(temporary, {recursive:true, force:true});
  }
})().catch(error => {console.error(error);process.exitCode=1;});
