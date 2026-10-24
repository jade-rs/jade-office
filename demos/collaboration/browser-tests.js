// Browser composition-event tests, using the real canvas Word editor. Native
// pointer/keyboard coverage lives in the collaboration Playwright CI suite.
const output=document.getElementById('result');
const pause=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const assert=(value,message)=>{if(!value)throw new Error(message);};
async function until(predicate,message){for(let i=0;i<800;i++){if(predicate())return;await pause(25);}throw new Error(message);}
const model=frame=>frame.contentWindow.jadeDemo?.state();
const editor=frame=>frame.contentWindow.jadeDemo?.editor();
const synced=frame=>frame.contentDocument?.getElementById('status')?.textContent==='● 同期済み';
const input=frame=>frame.contentDocument.getElementById('ime');
function event(frame,type,options){const Event=type.startsWith('composition')?frame.contentWindow.CompositionEvent:frame.contentWindow.InputEvent;input(frame).dispatchEvent(new Event(type,{bubbles:true,cancelable:true,...options}));}
function text(frame,value){event(frame,'beforeinput',{inputType:'insertText',data:value});}
function key(frame,value){input(frame).dispatchEvent(new frame.contentWindow.KeyboardEvent('keydown',{key:value,bubbles:true,cancelable:true}));}
function focus(frame){input(frame).focus();}

document.getElementById('run').onclick=async()=>{
  document.getElementById('run').disabled=true;output.dataset.status='running';output.textContent='準備中…';
  try{
    const name=`browser-canvas-${Date.now()}.docx`;
    const seed=await fetch('/documents/demo.docx').then(r=>r.arrayBuffer());
    assert((await fetch(`/documents/${name}`,{method:'PUT',body:seed})).status===201,'isolated document');
    const frames=['IME author','Peer'].map(actor=>{
      const frame=document.createElement('iframe');frame.title=actor;
      frame.src=`/?name=${encodeURIComponent(actor)}&document=${encodeURIComponent(name)}`;
      document.getElementById('frames').append(frame);return frame;
    });
    const [author,peer]=frames;
    await until(()=>frames.every(f=>synced(f)&&f.contentWindow.jadeDemo?.ready),'both canvas editors must become ready');
    assert(frames.every(f=>!f.contentDocument.querySelector('textarea')),'no textarea editor is allowed');
    await until(()=>frames.every(f=>f.contentDocument.querySelector('.peer-caret')),'both participants must appear on the page');
    focus(author);key(author,'Home');
    input(author).dispatchEvent(new author.contentWindow.KeyboardEvent('keydown',{key:'ArrowRight',shiftKey:true,bubbles:true,cancelable:true}));
    input(author).dispatchEvent(new author.contentWindow.KeyboardEvent('keydown',{key:'ArrowRight',shiftKey:true,bubbles:true,cancelable:true}));
    const range=editor(author).selection;
    await until(()=>peer.contentDocument.querySelector('.peer-selection')?.dataset.focus===JSON.stringify(range.focus),'the final selected range must reach the peer');
    assert(peer.contentDocument.querySelector('.peer-name').textContent.includes('IME author'),'selection must be labelled with its participant');
    const checks=[];
    // DocumentStart positions both clients through the real Word navigator.
    for(const frame of frames){focus(frame);input(frame).dispatchEvent(new frame.contentWindow.KeyboardEvent('keydown',{key:'Home',ctrlKey:true,bubbles:true,cancelable:true}));}
    for(const finalInputBeforeEnd of [false,true]){
      focus(author);
      const before=editor(author),p=before.selection.focus[0],base=before.paragraphs[p];
      const draft=`IME${checks.length}日本語😀`,remote=`PEER${checks.length}`;
      event(author,'compositionstart',{data:''});
      event(author,'compositionupdate',{data:'に'});
      event(author,'compositionupdate',{data:draft});
      const draftState=editor(author);
      assert(draftState.composing&&draftState.paragraphs[p].includes(draft),'engine must render IME draft');
      text(peer,remote);
      await until(()=>synced(peer)&&editor(peer).paragraphs[p].includes(remote),'peer typing must be acknowledged');
      await pause(150);
      author.contentDocument.getElementById('connection').click();
      assert(JSON.stringify(editor(author).selection)===JSON.stringify(draftState.selection),'disconnect must preserve canvas IME caret');
      assert(editor(author).paragraphs[p]===draftState.paragraphs[p],'disconnect must preserve draft');
      author.contentDocument.getElementById('connection').click();await pause(300);
      assert(editor(author).paragraphs[p]===draftState.paragraphs[p],'resume waits for composition');
      const final=()=>event(author,'beforeinput',{inputType:'insertFromComposition',data:draft});
      if(finalInputBeforeEnd)final();event(author,'compositionend',{data:draft});if(!finalInputBeforeEnd)final();
      await until(()=>frames.every(synced)&&JSON.stringify(editor(author).paragraphs)===JSON.stringify(editor(peer).paragraphs),'canvas editor text must converge');
      const result=editor(author).paragraphs[p];
      assert(result.split(draft).length===2&&result.split(remote).length===2,'each insertion appears exactly once');
      assert(result.length===base.length+draft.length+remote.length,'no lost text');
      assert(frames.every(f=>!f.contentDocument.getElementById('notice').textContent),'no editor/protocol errors');
      checks.push(finalInputBeforeEnd?'final-input-before-compositionend':'final-input-after-compositionend');
    }
    // Cancellation over a selected range must restore the original text.
    focus(author);key(author,'Home');
    input(author).dispatchEvent(new author.contentWindow.KeyboardEvent('keydown',{key:'ArrowRight',shiftKey:true,bubbles:true,cancelable:true}));
    const beforeCancel=editor(author);event(author,'compositionstart',{data:''});event(author,'compositionupdate',{data:'仮'});event(author,'compositionend',{data:''});
    assert(JSON.stringify(editor(author).paragraphs)===JSON.stringify(beforeCancel.paragraphs),'cancelled IME must restore selection contents');
    assert(JSON.stringify(editor(author).selection)===JSON.stringify(beforeCancel.selection),'cancelled IME selection');
    assert(model(author).pending===0,'cancelled composition submits nothing');
    // Composition may create and remove pages before it commits. Its DOM
    // input must stay attached and focused throughout native repagination.
    focus(author);input(author).dispatchEvent(new author.contentWindow.KeyboardEvent('keydown',{key:'Home',ctrlKey:true,bubbles:true,cancelable:true}));
    const beforePages=editor(author);
    event(author,'compositionstart',{data:''});event(author,'compositionupdate',{data:'ページ\n'.repeat(90)});
    assert(editor(author).pages>beforePages.pages,'draft must create additional native pages');
    assert(author.contentDocument.activeElement===input(author),'repagination must keep the OS IME input focused');
    event(author,'compositionupdate',{data:'短い変換'});
    assert(author.contentDocument.activeElement===input(author),'shrinking the draft must retain IME focus');
    event(author,'compositionend',{data:''});
    assert(JSON.stringify(editor(author).paragraphs)===JSON.stringify(beforePages.paragraphs),'cancelled multi-page composition restores text');
    author.contentDocument.getElementById('connection').click();
    await until(()=>!peer.contentDocument.querySelector('.peer-caret'),'disconnected participant must disappear');
    author.contentDocument.getElementById('connection').click();
    await until(()=>frames.every(synced)&&peer.contentDocument.querySelector('.peer-caret'),'resuming must republish the retained cursor without a new edit');
    output.dataset.status='passed';output.textContent=JSON.stringify({status:'passed',document:name,checks:['canvas-editor-without-textarea','named-peer-caret-and-selection','peer-left-and-resume','composition-text-and-caret-survive-disconnect','peer-commit-during-composition','resume-after-queued-frames','cancel-selected-composition','ime-focus-survives-repagination',...checks]},null,2);
  }catch(error){output.dataset.status='failed';output.textContent=String(error.stack??error);}
};
