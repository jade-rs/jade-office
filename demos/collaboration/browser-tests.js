// Browser-side regression harness, separate from the demo's product screen.
const output = document.getElementById('result');
const pause = (ms) => new Promise(resolve => setTimeout(resolve,ms));
const assert = (value,message) => { if (!value) throw new Error(message); };
async function until(predicate,message) {
  for (let i=0;i<400;i++) { if (predicate()) return; await pause(25); }
  throw new Error(message);
}
function append(frame,paragraph,text) {
  const field = frame.contentDocument.getElementById(`paragraph-${paragraph}`);
  field.focus(); field.value += text;
  field.dispatchEvent(new frame.contentWindow.InputEvent('input',{bubbles:true,inputType:'insertText',data:text}));
}
function synced(frame) { return frame.contentDocument?.getElementById('status')?.textContent === '● 同期済み'; }
function value(frame,paragraph) { return frame.contentDocument.getElementById(`paragraph-${paragraph}`).value; }

document.getElementById('run').onclick = async () => {
  document.getElementById('run').disabled = true;
  output.dataset.status = 'running'; output.textContent = '準備中…';
  try {
    const name = `browser-ime-${Date.now()}.docx`;
    const seed = await fetch('/documents/demo.docx').then(r=>{if(!r.ok)throw new Error('seed download failed');return r.arrayBuffer();});
    const uploaded = await fetch(`/documents/${name}`,{method:'PUT',body:seed});
    assert(uploaded.status===201,'isolated test document must be created');
    const frames = ['IME author','Peer'].map(actor => {
      const frame = document.createElement('iframe'); frame.title = actor;
      frame.src = `/?name=${encodeURIComponent(actor)}&document=${encodeURIComponent(name)}`;
      document.getElementById('frames').append(frame); return frame;
    });
    const [author,peer] = frames;
    await until(()=>frames.every(f=>synced(f)&&f.contentDocument.querySelector('#preview canvas')),'both real editors and previews must become ready');
    const checks = [];
    for (const finalInputBeforeEnd of [false,true]) {
      const field = author.contentDocument.getElementById('paragraph-1');
      const base = field.value, draft = ` IME${checks.length}日本語😀`, remote = ` PEER${checks.length}`;
      field.focus(); field.setSelectionRange(base.length,base.length);
      field.dispatchEvent(new author.contentWindow.CompositionEvent('compositionstart',{bubbles:true,data:''}));
      field.value = base + draft; field.setSelectionRange(field.value.length,field.value.length);
      field.dispatchEvent(new author.contentWindow.InputEvent('input',{bubbles:true,inputType:'insertCompositionText',data:draft,isComposing:true}));
      append(peer,1,remote);
      await until(()=>synced(peer)&&value(peer,1).includes(remote),'peer typing must be acknowledged during composition');
      // Allow that committed frame to reach the composing author, where it
      // must remain deferred. A focus-preserving click emulates a transport
      // loss while the browser still owns an active composition selection.
      await pause(150);
      author.contentDocument.getElementById('connection').click();
      assert(field.value===base+draft,'disconnect must preserve the uncommitted composition text');
      assert(field.selectionStart===field.value.length&&field.selectionEnd===field.value.length,'disconnect must preserve the composition caret');
      author.contentDocument.getElementById('connection').click();
      await pause(300);
      assert(field.value===base+draft,'resumption must wait for composition to end');
      const input = () => field.dispatchEvent(new author.contentWindow.InputEvent('input',{bubbles:true,inputType:'insertFromComposition',data:draft}));
      if (finalInputBeforeEnd) input();
      field.dispatchEvent(new author.contentWindow.CompositionEvent('compositionend',{bubbles:true,data:draft}));
      if (!finalInputBeforeEnd) input();
      await until(()=>frames.every(synced)&&value(author,1)===value(peer,1),'composition and remote edit must converge after resumption');
      const result = value(author,1);
      assert(result.split(draft).length===2&&result.split(remote).length===2,'each author insertion must appear exactly once');
      assert(frames.every(f=>!f.contentDocument.getElementById('notice').textContent),'neither editor may report a protocol or edit error');
      checks.push(finalInputBeforeEnd?'final-input-before-compositionend':'final-input-after-compositionend');
    }
    await until(()=>frames.every(f=>f.contentDocument.querySelector('#preview canvas')),'both native previews must render');
    output.dataset.status = 'passed';
    output.textContent = JSON.stringify({status:'passed',document:name,checks:['composition-text-and-caret-survive-disconnect','peer-commit-during-composition','resume-after-queued-frames',...checks]},null,2);
  } catch (error) {
    output.dataset.status = 'failed'; output.textContent = String(error.stack ?? error);
  }
};
