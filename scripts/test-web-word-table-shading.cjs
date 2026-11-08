#!/usr/bin/env node
// Requires the built web shell, Playwright/pngjs via NODE_PATH, and python3.
// Set JADE_TABLE_WRAP=1 with btLrV to check short-column wrapping and final ink.
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { execFileSync } = require('node:child_process');
const { chromium } = require('playwright');
const { PNG } = require('pngjs');
const temporary = fs.mkdtempSync(path.join(os.tmpdir(), 'jade-table-shading-'));
const file = path.join(temporary, 'table-shading.docx');
const direction = process.argv[3] || '';
const wrapFixture = process.env.JADE_TABLE_WRAP === '1';
const uprightGraphemeFixture = process.env.JADE_TABLE_UPRIGHT_GRAPHEMES === '1';
const uprightAlternateFixture = process.env.JADE_TABLE_UPRIGHT_ALTERNATES === '1' || uprightGraphemeFixture;
const uprightFixture = process.env.JADE_TABLE_UPRIGHT === '1' || uprightAlternateFixture;
if (uprightFixture) assert(['tbRlV', 'tbLrV'].includes(direction));
assert(['', 'lrTbV', 'tbRl', 'tbRlV', 'btLr', 'btLrV', 'tbLrV'].includes(direction));
execFileSync('python3', ['-c', `
import sys, zipfile
w = 'http://schemas.openxmlformats.org/wordprocessingml/2006/main'
with zipfile.ZipFile(sys.argv[1], 'w') as z:
 z.writestr('[Content_Types].xml', '<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
 z.writestr('_rels/.rels', '<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>')
 document = '<w:document xmlns:w="'+w+'"><w:body><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="5000"/></w:tblGrid><w:tr>'+('<w:trPr><w:trHeight w:val="3600"/></w:trPr>' if sys.argv[2] else '')+'<w:tc><w:tcPr><w:tcW w:w="5000" w:type="dxa"/>'+('<w:textDirection w:val="'+sys.argv[2]+'"/>' if sys.argv[2] else '')+'</w:tcPr><w:p><w:pPr><w:shd w:fill="000080"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/><w:color w:val="auto"/></w:rPr><w:t>Automatic</w:t></w:r><w:r><w:rPr><w:sz w:val="48"/><w:color w:val="000000"/></w:rPr><w:t>Fixed</w:t></w:r></w:p><w:p><w:pPr><w:shd w:fill="FFFF00"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/></w:rPr><w:t>Automatic</w:t></w:r></w:p><w:p><w:pPr><w:shd w:fill="000080"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/><w:highlight w:val="cyan"/><w:shd w:fill="FF0000"/></w:rPr><w:t>Highlight</w:t></w:r></w:p><w:p><w:pPr><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/><w:highlight w:val="none"/><w:shd w:fill="008000"/></w:rPr><w:t>Shading</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>'
 if sys.argv[3] == '1':
  document = '<w:document xmlns:w="'+w+'"><w:body><w:tbl><w:tblPr><w:tblW w:w="5000" w:type="dxa"/></w:tblPr><w:tblGrid><w:gridCol w:w="5000"/></w:tblGrid><w:tr><w:trPr><w:trHeight w:val="900"/></w:trPr><w:tc><w:tcPr><w:tcW w:w="5000" w:type="dxa"/><w:textDirection w:val="'+sys.argv[2]+'"/></w:tcPr><w:p><w:pPr><w:shd w:fill="000080"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/></w:rPr><w:t>ABCDEFG</w:t></w:r><w:r><w:rPr><w:sz w:val="48"/><w:color w:val="FF0000"/></w:rPr><w:t>H</w:t></w:r></w:p></w:tc></w:tr></w:tbl><w:p/><w:sectPr><w:pgSz w:w="11906" w:h="16838"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>'
 if sys.argv[4] == '1':
  start = document.index('<w:p>')
  end = document.index('</w:tc>')
  document = document[:start] + '<w:p><w:pPr><w:shd w:fill="000080"/><w:spacing w:after="0"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/><w:sz w:val="48"/><w:color w:val="FFFFFF"/></w:rPr><w:t>llll</w:t></w:r></w:p>' + document[end:]
 if sys.argv[5] == '1':
  document = document.replace('Arial', 'Noto Sans JP').replace('llll', 'か\\u3099か\\u3099' if sys.argv[6] == '1' else '（（')
 z.writestr('word/document.xml', document)
`, file, direction, wrapFixture ? '1' : '0', uprightFixture ? '1' : '0', uprightAlternateFixture ? '1' : '0', uprightGraphemeFixture ? '1' : '0']);

const referenceFile = path.join(temporary, 'vertical-reference.docx');
if (uprightAlternateFixture) {
  execFileSync('python3', ['-c', `
import sys, zipfile
with zipfile.ZipFile(sys.argv[1]) as source, zipfile.ZipFile(sys.argv[2], 'w') as target:
 for name in source.namelist():
  data = source.read(name)
  if name == 'word/document.xml': data = data.decode().replace('か\\u3099', 'が').replace('（', '︵').encode()
  target.writestr(name, data)
`, file, referenceFile]);
}

function uprightInk(png) {
  // Select the main paragraph band, excluding the page thumbnail's navy ink.
  const seen = new Uint8Array(png.width * png.height);
  let largest = { count: 0 };
  const navy = index => png.data[index * 4] === 0 &&
    png.data[index * 4 + 1] === 0 && png.data[index * 4 + 2] === 128;
  for (let index = 0; index < seen.length; index++) {
    if (seen[index] || !navy(index)) continue;
    const band = { count: 0, left: png.width, top: png.height, right: -1, bottom: -1 };
    const stack = [index];
    seen[index] = 1;
    while (stack.length) {
      const current = stack.pop();
      const x = current % png.width, y = Math.floor(current / png.width);
      band.count++;
      band.left = Math.min(band.left, x); band.right = Math.max(band.right, x);
      band.top = Math.min(band.top, y); band.bottom = Math.max(band.bottom, y);
      const neighbors = [];
      if (x > 0) neighbors.push(current - 1);
      if (x + 1 < png.width) neighbors.push(current + 1);
      if (y > 0) neighbors.push(current - png.width);
      if (y + 1 < png.height) neighbors.push(current + png.width);
      for (const next of neighbors) if (!seen[next] && navy(next)) {
        seen[next] = 1;
        stack.push(next);
      }
    }
    if (band.count > largest.count) largest = band;
  }
  const { left, top, right, bottom } = largest;
  assert(right > left && bottom > top, 'missing paragraph background');
  const ink = [];
  for (let y = top; y <= bottom; y++) for (let x = left; x <= right; x++) {
    const at = (y * png.width + x) * 4;
    if (png.data[at] > 220 && png.data[at + 1] > 220 && png.data[at + 2] > 220) ink.push([x, y]);
  }
  assert(ink.length > 10, 'missing upright text ink');
  return { bounds: [left, top, right, bottom], ink };
}

// Count text pixels enclosed by each solid paragraph band. Page-white pixels
// outside the band cannot satisfy the automatic white-text assertion.
function bandPixels(png, rgb, minimumRowPixels = direction ? 5 : 100) {
  let background = 0, white = 0, black = 0;
  const is = (x, y, color) => {
    const offset = (y * png.width + x) * 4;
    return color.every((v, channel) => png.data[offset + channel] === v);
  };
  for (let y = 0; y < png.height; y++) {
    let left = png.width, right = -1, count = 0;
    for (let x = 0; x < png.width; x++) {
      if (is(x, y, rgb)) { left = Math.min(left, x); right = x; count++; }
    }
    if (count < minimumRowPixels) continue;
    background += count;
    for (let x = left; x <= right; x++) {
      if (is(x, y, [255, 255, 255])) white++;
      if (is(x, y, [0, 0, 0])) black++;
    }
  }
  return { background, white, black };
}

(async () => {
  const browser = await chromium.launch({
    headless: true,
    executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE,
    args: ['--enable-unsafe-webgpu'],
  });
  try {
    // Keep enough viewport space to detect misplaced later paragraphs too.
    const page = await browser.newPage({ viewport: { width: 1280, height: direction && !wrapFixture ? 2400 : 1000 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    let sourceFile = file;
    await page.route('**/samples/table-shading.docx', route => route.fulfill({ body: fs.readFileSync(sourceFile) }));
    await page.goto(new URL('?open=table-shading.docx', process.argv[2] || 'http://localhost:8765/').href);
    await page.waitForFunction(() => !document.getElementById('loading'), null, { timeout: 90000 });
    await page.waitForTimeout(3000);
    const screenshot = await page.screenshot();
    if (process.env.JADE_TABLE_SHADING_SCREENSHOT) fs.writeFileSync(process.env.JADE_TABLE_SHADING_SCREENSHOT, screenshot);
    const png = PNG.sync.read(screenshot);
    if (uprightAlternateFixture) {
      const actual = uprightInk(png);
      sourceFile = referenceFile;
      await page.reload();
      await page.waitForFunction(() => !document.getElementById('loading'), null, { timeout: 90000 });
      await page.waitForTimeout(3000);
      const reference = uprightInk(PNG.sync.read(await page.screenshot()));
      assert.deepEqual(actual, reference, 'upright text must match the equivalent vertical reference');
      assert.deepEqual(errors, []);
      console.log('upright text matches vertical reference', { direction, grapheme: uprightGraphemeFixture, pixels: actual.ink.length });
      return;
    }
    if (uprightFixture) {
      const inkRows = [];
      for (let y = 0; y < png.height; y++) {
        let left = png.width, right = -1;
        for (let x = 0; x < png.width; x++) {
          const at = (y * png.width + x) * 4;
          if (png.data[at] === 0 && png.data[at + 1] === 0 && png.data[at + 2] === 128) {
            left = Math.min(left, x); right = x;
          }
        }
        let ink = 0;
        for (let x = left; x <= right; x++) {
          const at = (y * png.width + x) * 4;
          if (png.data[at] > 220 && png.data[at + 1] > 220 && png.data[at + 2] > 220) ink++;
        }
        if (ink >= 2) inkRows.push(y);
      }
      const starts = inkRows.filter((y, index) => index === 0 || y > inkRows[index - 1] + 1);
      assert.equal(starts.length, 4, `upright letters must have four separate ink bands: ${starts}`);
      for (let index = 1; index < starts.length; index++) {
        assert(starts[index] - starts[index - 1] >= 30,
          `24pt upright letters need an em pitch: ${starts}`);
      }
      assert.deepEqual(errors, []);
      console.log('upright Latin: four separate glyphs at an em pitch', { direction, starts });
      return;
    }
    if (wrapFixture) {
      let red = 0, enclosedRed = 0;
      const inkColumns = new Set();
      for (let y = 0; y < png.height; y++) {
        let left = png.width, right = -1;
        for (let x = 0; x < png.width; x++) {
          const at = (y * png.width + x) * 4;
          if (png.data[at] === 0 && png.data[at + 1] === 0 && png.data[at + 2] === 128) {
            left = Math.min(left, x); right = x;
          }
        }
        for (let x = 0; x < png.width; x++) {
          const at = (y * png.width + x) * 4;
          if (png.data[at] > 220 && png.data[at + 1] < 30 && png.data[at + 2] < 30) {
            red++;
            if (x >= left && x <= right) enclosedRed++;
          }
          if (x >= left && x <= right && png.data[at] === 255 &&
              png.data[at + 1] === 255 && png.data[at + 2] === 255) inkColumns.add(x);
        }
      }
      assert(red > 50, `last glyph H must survive wrapping: ${red} red pixels`);
      assert.equal(enclosedRed, red, 'last glyph must remain inside the column background');
      assert(inkColumns.size > 40, `text must occupy multiple columns: ${inkColumns.size}`);
      assert.deepEqual(errors, []);
      console.log('rotated table wrap: final glyph visible inside column bounds', { direction, red, inkColumns: inkColumns.size });
      return;
    }
    const navy = bandPixels(png, [0, 0, 128]);
    const yellow = bandPixels(png, [255, 255, 0]);
    assert(navy.background > 1000, `navy paragraph band missing: ${JSON.stringify(navy)}`);
    assert(navy.white > 20, `automatic white text missing: ${JSON.stringify(navy)}`);
    assert(navy.black > 10, `explicit black text missing: ${JSON.stringify(navy)}`);
    assert(yellow.background > 1000 && yellow.black > 20, `yellow paragraph/black text missing: ${JSON.stringify(yellow)}`);
    // Run fills end at the text edge and are narrower than paragraph bands.
    const cyan = bandPixels(png, [0, 255, 255], direction ? 5 : 30);
    const green = bandPixels(png, [0, 128, 0], direction ? 5 : 30);
    assert(cyan.background > 1000 && cyan.black > 20,
      `run highlight must override character/paragraph shading: ${JSON.stringify(cyan)}`);
    assert(green.background > 1000 && green.white > 20,
      `character shading/automatic white text missing: ${JSON.stringify(green)}`);
    if (direction && direction !== 'lrTbV') {
      const firstRow = color => {
        for (let y = 0; y < png.height; y++) {
          let count = 0;
          for (let x = 0; x < png.width; x++) {
            const offset = (y * png.width + x) * 4;
            if (color.every((value, channel) => png.data[offset + channel] === value)) count++;
          }
          if (count >= 5) return y;
        }
        return -1;
      };
      const navyTop = firstRow([0, 0, 128]);
      const yellowTop = firstRow([255, 255, 0]);
      assert(navyTop >= 0 && Math.abs(navyTop - yellowTop) <= 1,
        `vertical paragraphs must share their top edge: navy=${navyTop}, yellow=${yellowTop}`);
    }
    assert.deepEqual(errors, []);
    console.log('table backgrounds: paragraph fills, fixed colors, run highlight precedence, and character shading passed', { direction: direction || 'horizontal', navy, yellow, cyan, green });
  } finally {
    await browser.close();
    fs.rmSync(temporary, { recursive: true, force: true });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
