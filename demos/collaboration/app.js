import initReplica, { BrowserDocument } from '/collab-pkg/jade_collab_browser.js';
import initOffice, { PreviewFonts, OfficePreview } from '/office-pkg/jade_office_collab_preview.js';
import { replacement, renderInput } from './editing.mjs';

const $ = (id) => document.getElementById(id);
const name = (new URLSearchParams(location.search).get('name') || 'Alice').slice(0,60);
const documentName = new URLSearchParams(location.search).get('document') || 'demo.docx';
const documentPath = `/documents/${encodeURIComponent(documentName)}`;
$('document-name').textContent = documentName;
document.querySelector('.download').href = documentPath;
document.querySelector('.download').download = documentName;
const entries = new Map(), peers = new Map();
let replica, state, socket, ready = false, paused = false, stopped = false;
let composing = null, queued = [], fonts, previewTimer, retryTimer, retry = 250;
let previewSerial = 0;
const notice = (message) => { $('notice').textContent = String(message); };

function render(localCaret) {
  state = JSON.parse(replica.state());
  $('identity').textContent = `${name} · #${state.replica}`;
  $('version').textContent = `版 ${state.seq}`;
  $('connection').disabled = stopped;
  $('connection').textContent = paused ? '再接続する' : 'オフラインにする';
  $('status').textContent = stopped ? '編集を停止しました' : paused ? `オフライン · 未確定 ${state.pending}` : !ready ? '再接続中…' : state.pending ? `同期中 · ${state.pending}` : '● 同期済み';
  const active = document.activeElement;
  const keys = new Set();
  for (const p of state.paragraphs) {
    keys.add(p.key);
    let entry = entries.get(p.key);
    if (!entry) {
      const box = document.createElement('div'); box.className = 'paragraph';
      const label = document.createElement('label');
      const input = document.createElement('textarea');
      input.id = `paragraph-${p.index}`; label.htmlFor = input.id;
      input.spellcheck = false;
      entry = { box, label, input, paragraph:p };
      entries.set(p.key,entry); box.append(label,input);
      input.addEventListener('beforeinput', (e) => {
        if (['insertParagraph','insertLineBreak'].includes(e.inputType)) { e.preventDefault(); notice('このデモでは既存段落内の文字を編集できます。'); }
      });
      input.addEventListener('input', () => { if (composing !== entry) edit(entry); });
      input.addEventListener('compositionstart', () => { composing = entry; });
      input.addEventListener('compositionend', () => {
        composing = null; edit(entry);
        const incoming = queued; queued = [];
        for (const wire of incoming) accept(wire);
      });
      input.addEventListener('select', () => presence(entry));
      input.addEventListener('focus', () => presence(entry));
    }
    renderInput(entry.input,entry.paragraph,p,composing===entry,active===entry.input,localCaret?.key===p.key ? localCaret : null);
    entry.input.disabled = stopped;
    entry.label.textContent = p.story.startsWith('Body') ? `段落 ${p.index + 1}` : `表のセル · 段落 ${p.index + 1}`;
    entry.paragraph = p;
    // Reorder only when necessary: moving the focused textarea breaks IME.
    const at = $('editor').children[p.index];
    if (at !== entry.box) $('editor').insertBefore(entry.box, at ?? null);
  }
  for (const [key,entry] of entries) if (!keys.has(key)) { entry.box.remove(); entries.delete(key); }
}

function edit(entry) {
  const after = entry.input.value, before = entry.paragraph.text;
  if (after === before) return;
  try {
    if (/[\r\n]/.test(after)) throw new Error('改行の追加はこのデモの対象外です。段落ごとに編集してください。');
    const r = replacement(before,after);
    const caret = {key:entry.paragraph.key,start:entry.input.selectionStart,end:entry.input.selectionEnd};
    const outgoing = JSON.parse(replica.replace(entry.paragraph.index,r.start,r.end,r.text));
    if (ready && socket.readyState === WebSocket.OPEN) for (const message of outgoing) socket.send(message);
    render(caret); presence(entry);
  } catch (error) { notice(error); render(); }
}

let lastPresence = 0;
function presence(entry) {
  if (!ready || performance.now() - lastPresence < 100) return;
  lastPresence = performance.now();
  socket.send(JSON.stringify({type:'presence', state:{name,paragraph:entry.paragraph.index}}));
}
function showPeers() {
  $('peers').textContent = [...peers.values()].map((p) => `${p.name} · 段落${(p.paragraph ?? 0)+1}`).join(' / ');
}

function fail(error) {
  stopped = true; ready = false;
  notice(`${error}\n未確定の変更を保持しています。このタブを閉じずに原因を確認してください。`);
  socket?.close(); if (replica) render();
}

function accept(wire) {
  if (stopped) return;
  try {
    const message = JSON.parse(wire);
    if (['welcome','resumed','committed','rejected'].includes(message.type) && composing) { queued.push(wire); return; }
    switch(message.type) {
      case 'welcome':
        if (replica) throw new Error('既存の編集状態を置き換える接続応答を拒否しました');
        replica = new BrowserDocument(wire); ready = true; retry = 250; render(); break;
      case 'resumed': {
        const reasons = JSON.parse(replica.receive(wire));
        if (reasons.length) notice(reasons.join('\n'));
        for (const submission of JSON.parse(replica.pending())) socket.send(submission);
        ready = true; retry = 250; render(); break;
      }
      case 'committed': case 'rejected': {
        const reasons = JSON.parse(replica.receive(wire));
        if (reasons.length) notice(reasons.join('\n'));
        render(); break;
      }
      case 'presence': peers.set(String(message.replica),message.state); showPeers(); return;
      case 'presence_left': peers.delete(String(message.replica)); showPeers(); return;
      case 'presence_rejected': return;
      case 'overloaded': socket.close(); return;
      case 'rejoin': case 'revoked': case 'error': throw new Error(message.message);
      default: throw new Error('未知のサーバー応答を受信しました');
    }
    if (ready) {
      if (message.type === 'welcome' || message.type === 'resumed') socket.send(JSON.stringify({type:'presence',state:{name}}));
      if (!state.pending) refreshPreview();
    }
  } catch (error) { fail(error); }
}

function connect() {
  if (paused || stopped) return;
  clearTimeout(retryTimer);
  // Drain any frames buffered for IME before choosing the resume watermark.
  if (composing) { retryTimer = setTimeout(connect,250); return; }
  peers.clear(); showPeers(); ready = false;
  socket = new WebSocket(`${location.protocol === 'https:' ? 'wss' : 'ws'}://${location.host}${documentPath}/ws`);
  const current = socket;
  current.onopen = () => { if (current === socket) current.send(replica ? replica.resume_message() : '{"type":"join"}'); };
  current.onmessage = (event) => { if (current === socket) accept(event.data); };
  current.onclose = () => {
    if (current !== socket) return;
    ready = false; peers.clear(); showPeers(); if (replica) render();
    if (!paused && !stopped) { retryTimer = setTimeout(connect,retry); retry = Math.min(retry*2,5000); }
  };
}

function refreshPreview() {
  clearTimeout(previewTimer);
  previewTimer = setTimeout(paintPreview,250);
}
async function paintPreview() {
  if (!fonts || !ready || state.pending) return;
  const serial = ++previewSerial;
  let preview;
  try {
    const response = await fetch(documentPath,{cache:'no-store'});
    if (!response.ok) throw new Error(`DOCX取得: ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    if (serial !== previewSerial) return;
    preview = new OfficePreview(bytes,fonts);
    const pages = [];
    for (let i=0;i<preview.page_count();i++) {
      const pixels = preview.render_page(i), canvas = document.createElement('canvas');
      canvas.width = preview.width(); canvas.height = preview.height();
      canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),canvas.width,canvas.height),0,0);
      pages.push(canvas);
    }
    $('preview').replaceChildren(...pages);
    $('preview-status').textContent = `確定済みの文書 · ${pages.length} ページ`;
  } catch (error) { $('preview-status').textContent = `プレビュー: ${error}`; }
  finally { preview?.free(); }
}

$('second').onclick = () => {
  const url = new URL(location.href); url.searchParams.set('name',name === 'Alice' ? 'Bob' : 'Alice');
  window.open(url,'_blank','noopener');
};
$('connection').onclick = () => {
  paused = !paused; clearTimeout(retryTimer);
  if (paused) { ready = false; socket.close(); } else connect();
  render();
};
addEventListener('beforeunload',(event) => { if (state?.pending) { event.preventDefault(); event.returnValue = ''; } });

try {
  await initReplica(); connect();
  await initOffice();
  const data = await Promise.all(['NotoSansJP-Regular.ttf','NotoSansJP-Bold.ttf'].map(async (name) => {
    const response = await fetch(`/fonts/${name}`);
    if (!response.ok) throw new Error(`フォント取得: ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }));
  fonts = new PreviewFonts(...data); refreshPreview();
} catch (error) { notice(`起動: ${error}`); }
