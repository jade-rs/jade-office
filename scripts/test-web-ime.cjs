#!/usr/bin/env node
// Build with scripts/build-web.sh and serve crates/jade-web/www first.
// Requires Playwright (install outside the repo and expose via NODE_PATH).
// Usage: node scripts/test-web-ime.cjs http://localhost:8765/
// Optional: PLAYWRIGHT_CHROMIUM_EXECUTABLE=/path/to/chromium
const assert = require('node:assert/strict');
const { chromium } = require('playwright');

const base = new URL(process.argv[2] || 'http://localhost:8765/');
const pause = page => page.waitForTimeout(350);
const state = page => page.locator('textarea').evaluate(el => {
  const rect = el.getBoundingClientRect();
  return {
    x: rect.x, y: rect.y, height: rect.height,
    value: el.value, start: el.selectionStart, end: el.selectionEnd,
    scrollLeft: el.scrollLeft, scrollTop: el.scrollTop,
    focused: document.activeElement === el,
  };
});
const sameInput = (actual, expected) => {
  assert.equal(actual.value, expected.value, 'positioning changed the IME value');
  assert.equal(actual.start, expected.start, 'positioning changed selection start');
  assert.equal(actual.end, expected.end, 'positioning changed selection end');
  assert.ok(actual.focused, 'positioning lost input focus');
};

async function word(page) {
  await page.goto(new URL('?open=contract.docx', base).href);
  await page.waitForFunction(() => !document.getElementById('loading'), undefined, { timeout: 90000 });
  await page.waitForTimeout(3500);
  await page.mouse.click(800, 470);
  await page.waitForFunction(() => document.querySelector('textarea')?.value.length > 0);
  await pause(page);
  const clicked = await state(page);
  // The fixture's second paragraph is at this point in the 1280×1000 viewport.
  // The old platform implementation keeps this at (0, 0), regardless of DPR.
  assert.ok(Math.abs(clicked.x - 800) < 35, `Word caret x: ${clicked.x}`);
  assert.ok(Math.abs(clicked.y - 470) < 25, `Word caret y: ${clicked.y}`);
  assert.ok(clicked.scrollLeft > 0, 'mirrored context leaves the native caret outside its box');

  await page.keyboard.press('ArrowRight');
  await pause(page);
  const moved = await state(page);
  assert.ok(moved.x > clicked.x + 5, 'IME anchor did not follow the arrow key');

  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.imeSetComposition', { text: 'にほん', selectionStart: 3, selectionEnd: 3 });
  await pause(page);
  const composing = await state(page);
  assert.ok(composing.value.includes('にほん'), 'composition did not reach the mirror');
  await page.mouse.move(900, 600);
  await page.mouse.wheel(0, 150);
  await pause(page);
  const scrolled = await state(page);
  sameInput(scrolled, composing);
  assert.ok(scrolled.y < composing.y - 50, 'IME anchor did not follow document scrolling');

  await cdp.send('Input.insertText', { text: '日本語' });
  await pause(page);
  const committed = await state(page);
  assert.ok(committed.value.includes('日本語'), 'Japanese composition did not commit');
  assert.ok(!committed.value.includes('にほん'), 'committed preedit remained in the document');
  assert.ok(committed.focused);

  // Commit before clicking a toolbar control, which can end native composition.
  // The Word status bar's zoom-in button changes layout without editing text.
  await page.mouse.click(1184, 989);
  await pause(page);
  // Status controls take GPUI focus; return it to the document.
  await page.mouse.click(854, 312);
  await pause(page);
  const zoomed = await state(page);
  assert.ok(zoomed.value.length > 0, 'zoomed document has no input context');
  assert.ok(Math.abs(zoomed.x - 854) < 35 && Math.abs(zoomed.y - 312) < 25,
    `zoomed caret is far from the click: ${zoomed.x}, ${zoomed.y}`);
  assert.ok(zoomed.focused);
  assert.ok(zoomed.height > scrolled.height,
    `IME anchor did not follow document zoom: ${scrolled.height} -> ${zoomed.height}`);
  await cdp.detach();
}

async function sheet(page) {
  await page.goto(new URL('?open=grid-operations-workbook.xlsx', base).href);
  await page.waitForFunction(() => !document.getElementById('loading'), undefined, { timeout: 90000 });
  await page.waitForTimeout(3500);
  // B10 is blank in the fixture; edit away from the grid's origin.
  await page.mouse.dblclick(388, 352);
  const cdp = await page.context().newCDPSession(page);
  await cdp.send('Input.insertText', { text: '先頭😀' });
  await pause(page);
  const first = await state(page);
  assert.ok(first.x > 390 && first.x < 450, `Excel caret x: ${first.x}`);
  assert.ok(first.y > 340 && first.y < 365, `Excel caret y: ${first.y}`);

  await cdp.send('Input.insertText', { text: '\n次の行' });
  await pause(page);
  const multiline = await state(page);
  assert.equal(multiline.value, '先頭😀\n次の行');
  assert.equal(multiline.start, multiline.value.length, 'UTF-16 caret offset');
  assert.ok(multiline.y > first.y + 10, 'IME anchor did not follow the second line');
  assert.ok(multiline.scrollTop >= 10 && multiline.scrollTop < 40,
    `native caret was not scrolled to the current line: ${multiline.scrollTop}`);

  await cdp.send('Input.imeSetComposition', { text: 'にほん', selectionStart: 3, selectionEnd: 3 });
  await pause(page);
  const composing = await state(page);
  assert.ok(composing.x > multiline.x + 25, 'Excel anchor did not follow composition');
  assert.ok(Math.abs(composing.y - multiline.y) < 2);
  await cdp.send('Input.insertText', { text: '日本語' });
  await pause(page);
  const committed = await state(page);
  assert.equal(committed.value, '先頭😀\n次の行日本語');
  assert.equal(committed.start, committed.value.length);
  assert.ok(committed.focused);
  await cdp.detach();
}

(async () => {
  for (const dpr of [1, 2]) {
    // deviceScaleFactor alone does not emulate ResizeObserver's device-pixel
    // content box. GPUI uses that box for its canvas backing size.
    const browser = await chromium.launch({
      headless: true,
      executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
      args: ['--enable-unsafe-webgpu', `--force-device-scale-factor=${dpr}`],
    });
    try {
      const context = await browser.newContext({
        viewport: { width: 1280, height: 1000 }, deviceScaleFactor: dpr, locale: 'ja-JP',
      });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await word(page);
      await sheet(page);
      assert.deepEqual(errors, [], 'browser application errors');
      console.log(`IME positioning: Word, Excel multiline, composition, scroll and zoom at DPR ${dpr}: passed`);
      await context.close();
    } finally {
      await browser.close();
    }
  }
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
