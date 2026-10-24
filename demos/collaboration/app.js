import initReplica, { BrowserDocument } from '/collab-pkg/jade_collab_browser.js';
import initOffice, { PreviewFonts, OfficeEditor } from '/office-pkg/jade_office_collab_preview.js';
import { replacement, projectDocument } from './editing.mjs';
import { CanvasEditor } from './canvas-editor.mjs';
import { PresencePublisher, peerColor, draftSelection, validPresence, activePeers } from './presence.mjs';

const $=id=>document.getElementById(id);
const params=new URLSearchParams(location.search);
const name=(params.get('name')||'Alice').slice(0,60);
const documentName=params.get('document')||'demo.docx';
const documentPath=`/documents/${encodeURIComponent(documentName)}`;
$('document-name').textContent=documentName;
document.querySelector('.download').href=documentPath;
document.querySelector('.download').download=documentName;
let replica,state,socket,ready=false,paused=false,stopped=false,canvas,fonts,opening;
let composing=false,queued=[],retryTimer,retry=250;
const peers=new Map();
const notice=message=>{$('notice').textContent=String(message);};
const publisher=new PresencePublisher(()=>{
  if(!ready||!canvas||stopped||socket?.readyState!==WebSocket.OPEN)return null;
  const native=canvas.state(),selection=composing?canvas.compositionBefore?.selection:native.selection;
  if(!selection)return null;
  const point=JSON.parse(replica.capture_selection(...selection.anchor,...selection.focus));
  return {name,selection:point,focused:document.activeElement===$('ime')&&document.hasFocus()&&!document.hidden,composing};
},value=>socket.send(JSON.stringify({type:'presence',state:value})));
function remoteSelections(native){
  if(!replica)return [];
  const result=[];
  for(const [id,peer] of activePeers(peers)){
    try{
      const selection=JSON.parse(replica.resolve_selection(JSON.stringify(peer.selection)));
      if(!selection)continue; // its edit may still be in flight
      const local=draftSelection(selection,state.paragraphs,native);
      if(local)result.push({...peer,id,color:peerColor(id),selection:local});
    }catch{/* Ignore an invalid ephemeral cursor; never stop document editing. */}
  }
  return result;
}


function render(localSelection) {
  const previous=state;
  state=JSON.parse(replica.state());
  $('identity').textContent=`${name} · #${state.replica}`;
  $('identity').style.color=peerColor(state.replica);
  $('version').textContent=`版 ${state.seq}`;
  $('connection').disabled=stopped;
  $('connection').textContent=paused?'再接続する':'オフラインにする';
  $('status').textContent=stopped?'編集を停止しました':paused?`オフライン · 未確定 ${state.pending}`:!ready?'再接続中…':state.pending?`同期中 · ${state.pending}`:'● 同期済み';
  if(canvas){
    canvas.readonly=stopped;
    if(!composing&&!stopped){
      const changed=projectDocument(canvas.document,previous?.paragraphs??state.paragraphs,state.paragraphs,localSelection);
      if(changed)canvas.paint();else canvas.overlay();
    }
  }
}

function publishEdit(before,after,cancelled=false) {
  try{
    if(cancelled){projectDocument(canvas.document,state.paragraphs,state.paragraphs,before.selection);return;}
    if(after.paragraphs.length!==state.paragraphs.length)throw new Error('このデモでは段落構造を変更できません');
    const changed=after.paragraphs.flatMap((text,index)=>text!==state.paragraphs[index].text?[index]:[]);
    if(changed.length>1)throw new Error('ひとつの段落・セル内の範囲を編集してください');
    if(changed.length){
      const p=changed[0],r=replacement(state.paragraphs[p].text,after.paragraphs[p]);
      const outgoing=JSON.parse(replica.replace(p,r.start,r.end,r.text));
      // Each inner string remains raw: parsing u64 IDs in JavaScript rounds them.
      if(ready&&socket.readyState===WebSocket.OPEN)for(const wire of outgoing)socket.send(wire);
    }
    render(after.selection);if(!composing)presence();
  }catch(error){
    projectDocument(canvas.document,state.paragraphs,state.paragraphs,before.selection);
    notice(error);render(before.selection);
  }
}

async function openEditor(){
  if(canvas||!fonts||!replica||opening)return;
  opening=true;
  let editor;
  try{
    // The file supplies the original formatting/package. It can be newer
    // than our welcome during fetch; project its text to our replica view.
    const response=await fetch(documentPath,{cache:'no-store'});
    if(!response.ok)throw new Error(`DOCX取得: ${response.status}`);
    editor=new OfficeEditor(new Uint8Array(await response.arrayBuffer()),fonts);
    projectDocument(editor,state.paragraphs,state.paragraphs,{anchor:[0,0],focus:[0,0]});
    $('loading').remove();
    canvas=new CanvasEditor($('editor'),$('ime'),editor,{
      change:publishEdit,error:notice,selection:presence,peers:remoteSelections,
      compositionStart(){composing=true;presence();},
      compositionEnd(){composing=false;const frames=queued;queued=[];for(const wire of frames)accept(wire);presence();},
    });
    render();canvas.resetInput();presence();
    $('editor').dataset.ready='true';
  }catch(error){editor?.free();fail(error);}
  finally{opening=false;}
}

function presence(){publisher.update();}
function showPeers(){
  $('peers').replaceChildren();
  for(const [id,peer] of activePeers(peers)){
    const button=document.createElement('button');button.type='button';button.className='peer-chip';
    button.style.setProperty('--peer-color',peerColor(id));button.dataset.peer=id;
    button.textContent=`${peer.name} · ${peer.composing?'変換中':'編集中'}`;
    button.title=`${peer.name} の位置へ移動`;
    button.onclick=()=>canvas?.revealPeer(id);$('peers').append(button);
  }
  canvas?.overlay();
}
function fail(error){
  stopped=true;ready=false;
  notice(`${error}\n未確定の変更を保持しています。このタブを閉じずに原因を確認してください。`);
  publisher.cancel();peers.clear();showPeers();socket?.close();if(replica)render();
}
function accept(wire){
  if(stopped)return;
  try{
    const message=JSON.parse(wire);
    if(['welcome','resumed','committed','rejected'].includes(message.type)&&composing){queued.push(wire);return;}
    switch(message.type){
      case 'welcome':
        if(replica)throw new Error('既存の編集状態を置き換える接続応答を拒否しました');
        replica=new BrowserDocument(wire);ready=true;retry=250;render();openEditor();break;
      case 'resumed':{
        const reasons=JSON.parse(replica.receive(wire));if(reasons.length)notice(reasons.join('\n'));
        const connected=!paused&&socket.readyState===WebSocket.OPEN;
        if(connected)for(const submission of JSON.parse(replica.pending()))socket.send(submission);
        ready=connected;retry=250;render();break;
      }
      case 'committed':case 'rejected':{
        const reasons=JSON.parse(replica.receive(wire));if(reasons.length)notice(reasons.join('\n'));
        render();break;
      }
      case 'presence':{
        if(validPresence(message.state)){
          const id=replica.presence_id(wire);
          if(id!==state.replica)peers.set(id,{...message.state,name:(message.name??message.state.name).slice(0,60),seen:performance.now()});
          showPeers();
        }
        return;
      }
      case 'presence_left':peers.delete(replica.presence_id(wire));showPeers();return;
      case 'presence_rejected':return;
      case 'overloaded':socket.close();return;
      case 'rejoin':case 'revoked':case 'error':throw new Error(message.message);
      default:throw new Error('未知のサーバー応答を受信しました');
    }
    if(ready&&['welcome','resumed'].includes(message.type))presence();
  }catch(error){fail(error);}
}
function connect(){
  if(paused||stopped)return;
  clearTimeout(retryTimer);
  if(composing){retryTimer=setTimeout(connect,250);return;}
  publisher.cancel();peers.clear();showPeers();ready=false;
  socket=new WebSocket(`${location.protocol==='https:'?'wss':'ws'}://${location.host}${documentPath}/ws`);
  const current=socket;
  current.onopen=()=>{if(current===socket)current.send(replica?replica.resume_message():'{"type":"join"}');};
  current.onmessage=event=>{if(current===socket)accept(event.data);};
  current.onclose=()=>{
    if(current!==socket)return;
    ready=false;publisher.cancel();peers.clear();showPeers();if(replica)render();
    if(!paused&&!stopped){retryTimer=setTimeout(connect,retry);retry=Math.min(retry*2,5000);}
  };
}
$('second').onclick=()=>{const url=new URL(location.href);url.searchParams.set('name',name==='Alice'?'Bob':'Alice');window.open(url,'_blank','noopener');};
$('connection').onclick=()=>{paused=!paused;clearTimeout(retryTimer);if(paused){ready=false;publisher.cancel();peers.clear();showPeers();socket.close();}else connect();render();};
addEventListener('focus',presence);addEventListener('blur',presence);
document.addEventListener('visibilitychange',presence);
setInterval(()=>{presence();let changed=false;for(const [id,peer] of peers)if(performance.now()-peer.seen>30000){peers.delete(id);changed=true;}if(changed)showPeers();},10000);
addEventListener('beforeunload',event=>{if(state?.pending||composing){event.preventDefault();event.returnValue='';}});

// Read-only diagnostics support browser regression tests and make a failed
// demo inspectable. Editing still enters through real pointer/key/IME events.
globalThis.jadeDemo={state:()=>state,editor:()=>canvas?.state(),get ready(){return !!canvas&&!stopped;}};
try{
  await initReplica();connect();await initOffice();
  const data=await Promise.all(['Regular','Bold'].map(async weight=>{
    const response=await fetch(`/fonts/NotoSansJP-${weight}.ttf`);if(!response.ok)throw new Error(`フォント取得: ${response.status}`);
    return new Uint8Array(await response.arrayBuffer());
  }));
  fonts=new PreviewFonts(...data);openEditor();
}catch(error){fail(`起動: ${error}`);}
