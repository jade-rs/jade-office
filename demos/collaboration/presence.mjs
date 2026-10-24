import { replacement } from './editing.mjs';

const colors=['#2563eb','#c0265b','#059669','#9333ea','#b45309','#0891b2'];
export function peerColor(id){let hash=0;for(const ch of String(id))hash=(Math.imul(hash,31)+ch.charCodeAt(0))>>>0;return colors[hash%colors.length];}
export function validPresence(value){
  return !!value&&typeof value.name==='string'&&value.name.length<=120&&
    !!value.selection&&['anchor','focus'].every(key=>typeof value.selection[key]==='string'&&value.selection[key].length<=3500);
}

// Leading + trailing publication: fast drags must not lose their last point.
export class PresencePublisher {
  constructor(read,send,interval=100){Object.assign(this,{read,send,interval,last:-Infinity,timer:null});}
  update(){
    clearTimeout(this.timer);this.timer=null;
    const wait=this.interval-(performance.now()-this.last);
    if(wait<=0)this.flush();else this.timer=setTimeout(()=>this.flush(),wait);
  }
  flush(){this.timer=null;const state=this.read();if(state){this.last=performance.now();this.send(state);}}
  cancel(){clearTimeout(this.timer);this.timer=null;this.last=-Infinity;}
  dispose(){this.cancel();}
}

// IME draft text is local only. Map confirmed peer endpoints into its current
// native layout rather than moving or committing the local composition.
export function draftSelection(selection,paragraphs,native){
  const map=([p,offset])=>{
    const text=native.paragraphs[p];if(typeof text!=='string'||!paragraphs[p])return null;
    const r=replacement(paragraphs[p].text,text);
    const at=offset<=r.start?offset:offset>=r.end?offset+r.text.length-(r.end-r.start):r.start;
    return [p,Math.min(text.length,Math.max(0,at))];
  };
  const anchor=map(selection.anchor),focus=map(selection.focus);
  return anchor&&focus?{anchor,focus}:null;
}
