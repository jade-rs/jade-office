// Canvas/input shell for jade-web::editing::EditableDocument. All positions,
// selections, edits and IME drafts belong to that Word editor in WASM.
export class CanvasEditor {
  constructor(root, input, document, hooks) {
    Object.assign(this,{root,input,document,hooks,dragging:false,composing:false,readonly:false});
    this.pages=[];
    root.addEventListener('pointerdown',e=>this.pointerDown(e));
    root.addEventListener('pointermove',e=>this.pointerMove(e));
    root.addEventListener('pointerup',e=>{this.dragging=false;if(root.hasPointerCapture(e.pointerId))root.releasePointerCapture(e.pointerId);});
    root.addEventListener('pointercancel',()=>{this.dragging=false;});
    input.addEventListener('keydown',e=>this.key(e));
    input.addEventListener('beforeinput',e=>this.beforeInput(e));
    input.addEventListener('compositionstart',()=>this.startComposition());
    input.addEventListener('compositionupdate',e=>this.updateComposition(e.data));
    input.addEventListener('compositionend',e=>this.endComposition(e.data));
    input.addEventListener('paste',e=>{e.preventDefault();this.edit(()=>document.insert(e.clipboardData.getData('text/plain').replace(/\r\n?/g,'\n')));});
    input.addEventListener('copy',e=>{e.preventDefault();e.clipboardData.setData('text/plain',document.selected_text());});
    input.addEventListener('cut',e=>{e.preventDefault();e.clipboardData.setData('text/plain',document.selected_text());this.edit(()=>document.insert(''));});
    input.addEventListener('focus',()=>root.classList.add('focused'));
    input.addEventListener('blur',()=>root.classList.remove('focused'));
    this.resizeObserver=new ResizeObserver(()=>this.resize());this.resizeObserver.observe(root);
    this.paint();
  }
  state(){return JSON.parse(this.document.state());}
  focus(){this.input.focus({preventScroll:true});}
  resetInput(){
    this.input.textContent='\u200b';
    if(globalThis.document.activeElement===this.input){const s=getSelection();s.removeAllRanges();const r=globalThis.document.createRange();r.selectNodeContents(this.input);r.collapse(false);s.addRange(r);}
  }
  point(event){
    // A captured drag's target is the root, so locate the page by geometry.
    for(let page=0;page<this.pages.length;page++){
      const rect=this.pages[page].getBoundingClientRect();
      if(event.clientY<rect.top||event.clientY>rect.bottom)continue;
      const x=(event.clientX-rect.left)*this.pages[page].offsetWidth/rect.width;
      const y=(event.clientY-rect.top)*this.pages[page].offsetHeight/rect.height;
      return JSON.parse(this.document.hit_test(page,x,y));
    }
    return null;
  }
  pointerDown(event){
    if(event.button!==0||this.composing)return;
    const point=this.point(event);if(!point)return;
    event.preventDefault();
    const state=this.state();
    this.dragAnchor=event.shiftKey?state.selection.anchor:point;
    this.document.select(...this.dragAnchor,...point);
    if(event.detail===2){this.document.select_word(...point);this.dragAnchor=this.state().selection.anchor;}
    if(event.detail>=3){const text=state.paragraphs[point[0]];this.document.select(point[0],0,point[0],text.length);}
    this.dragging=true;this.root.setPointerCapture(event.pointerId);
    this.focus();this.resetInput();this.overlay();this.hooks.selection?.();
  }
  pointerMove(event){
    if(!this.dragging||this.composing)return;
    const point=this.point(event);if(!point)return;
    this.document.select(...this.dragAnchor,...point);this.overlay();this.hooks.selection?.();
  }
  editable(){
    if(this.readonly)return false;
    const s=this.state().selection;
    if(!s||s.anchor[0]!==s.focus[0]){this.hooks.error('このデモではひとつの段落・セル内の範囲を編集できます。');return false;}
    return true;
  }
  edit(action){
    if(this.composing||!this.editable())return;
    const before=this.state();
    try {action();this.hooks.change(before,this.state());this.paint();this.resetInput();this.reveal();}
    catch(error){this.hooks.error(error);}
  }
  remove(backwards){
    const s=this.state();if(!this.editable())return;
    const {anchor:a,focus:f}=s.selection;
    if(a[0]===f[0]&&a[1]===f[1]&&(backwards?f[1]===0:f[1]===s.paragraphs[f[0]].length))return;
    this.edit(()=>this.document.delete(backwards));
  }
  key(event){
    if(event.isComposing||this.composing||event.keyCode===229)return;
    const mod=event.metaKey||event.ctrlKey;
    if(mod&&['c','v','x'].includes(event.key.toLowerCase()))return;
    if(mod&&event.key.toLowerCase()==='a'){
      event.preventDefault();const s=this.state(),p=s.selection.focus[0];
      this.document.select(p,0,p,s.paragraphs[p].length);this.overlay();return;
    }
    if(mod&&['z','y','b','i','u'].includes(event.key.toLowerCase())){event.preventDefault();this.hooks.error('書式変更と共同編集のUndoはこのデモの対象外です。');return;}
    const motion={ArrowLeft:'left',ArrowRight:'right',ArrowUp:'up',ArrowDown:'down',Home:'home',End:'end'}[event.key];
    if(motion){
      event.preventDefault();let target=motion;
      if((event.altKey||event.ctrlKey)&&['left','right'].includes(motion))target=motion==='left'?'wordLeft':'wordRight';
      if(event.metaKey&&['left','right'].includes(motion))target=motion==='left'?'home':'end';
      if(mod&&['Home','End'].includes(event.key))target=event.key==='Home'?'start':'finish';
      try{this.document.navigate(target,event.shiftKey);this.overlay();this.reveal();this.hooks.selection?.();}catch(error){this.hooks.error(error);}
      return;
    }
    if(event.key==='Backspace'||event.key==='Delete'){event.preventDefault();this.remove(event.key==='Backspace');}
    if(event.key==='Enter'){event.preventDefault();this.edit(()=>this.document.insert('\n'));}
    if(event.key==='Tab'){event.preventDefault();this.edit(()=>this.document.insert('\t'));}
  }
  beforeInput(event){
    if(this.composing||event.isComposing||event.inputType==='insertCompositionText')return;
    event.preventDefault();
    if(event.inputType==='insertFromComposition')return;
    if(event.inputType==='insertText'&&this.finalInput&&event.data===this.finalInput.text&&performance.now()-this.finalInput.at<100){this.finalInput=null;return;}
    this.finalInput=null;
    if(event.inputType==='insertText'||event.inputType==='insertReplacementText')this.edit(()=>this.document.insert(event.data??''));
    else if(['insertParagraph','insertLineBreak'].includes(event.inputType))this.edit(()=>this.document.insert('\n'));
    else if(event.inputType==='deleteContentBackward')this.remove(true);
    else if(event.inputType==='deleteContentForward')this.remove(false);
  }
  startComposition(){
    if(!this.editable())return;
    this.composing=true;this.compositionBefore=this.state();this.hooks.compositionStart();
  }
  updateComposition(text){
    if(!this.composing)return;
    try{this.document.compose(text);this.paint();}catch(error){this.hooks.error(error);}
  }
  endComposition(text){
    if(!this.composing)return;
    try{
      // An empty final string means cancellation: restore the pre-composition
      // selection/text through the caller, rather than deleting selected text.
      this.document.commit(text);
      this.composing=false;
      this.hooks.change(this.compositionBefore,this.state(),text==='');
      this.finalInput={text,at:performance.now()};
      this.resetInput();this.paint();
    }catch(error){this.composing=false;this.hooks.error(error);}
    this.hooks.compositionEnd();
  }
  resize(){
    const available=this.root.clientWidth-48;
    for(const page of this.pages)page.style.zoom=Math.min(1,available/page.offsetWidth);
  }
  paint(){
    const count=this.document.page_count();
    while(this.pages.length>count)this.pages.pop().remove();
    for(let i=0;i<count;i++){
      let page=this.pages[i];
      if(!page){page=globalThis.document.createElement('div');page.className='page';page.dataset.page=i;const canvas=globalThis.document.createElement('canvas');canvas.setAttribute('aria-label',`文書 ${i+1} ページ`);page.append(canvas);this.root.append(page);this.pages.push(page);}
      const pixels=this.document.render_page(i),canvas=page.querySelector('canvas');
      canvas.width=this.document.width();canvas.height=this.document.height();
      page.style.width=`${canvas.width}px`;page.style.height=`${canvas.height}px`;
      canvas.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(pixels),canvas.width,canvas.height),0,0);
    }
    this.resize();this.overlay();
  }
  overlay(){
    const s=this.state();
    for(const page of this.pages)for(const item of page.querySelectorAll('.caret,.selection'))item.remove();
    const box=(rect,kind)=>{
      const page=this.pages[rect.page];if(!page)return;
      const item=globalThis.document.createElement('div');item.className=kind;
      Object.assign(item.style,{left:`${rect.x}px`,top:`${rect.y}px`,width:`${Math.max(rect.width,1)}px`,height:`${rect.height}px`});page.append(item);
    };
    for(const r of s.highlights)box(r,'selection');
    if(s.caret){
      box(s.caret,'caret');
      const page=this.pages[s.caret.page];if(page){if(this.input.parentElement!==page)page.append(this.input);Object.assign(this.input.style,{left:`${s.caret.x}px`,top:`${s.caret.y}px`,height:`${s.caret.height}px`});}
    }
    this.root.dataset.composing=String(this.composing);
  }
  reveal(){const caret=this.root.querySelector('.caret');caret?.scrollIntoView({block:'nearest',inline:'nearest'});}
}
