// DOM offsets are UTF-16; the Rust adapter validates and converts them.
export function replacement(before, after) {
  let start = 0;
  while (start < before.length && start < after.length && before[start] === after[start]) start++;
  if (start > 0 && /[\uDC00-\uDFFF]/.test(before[start] ?? after[start] ?? "")) start--;
  let end = before.length, tail = after.length;
  while (end > start && tail > start && before[end - 1] === after[tail - 1]) { end--; tail--; }
  if (end < before.length && /[\uDC00-\uDFFF]/.test(before[end])) { end++; tail++; }
  return { start, end, text: after.slice(start, tail) };
}

// Keep the surrounding IDs as well: both immediate neighbours can be deleted
// by a concurrent range replacement. Prefer the nearest surviving right ID.
export function anchor(chars, offset) {
  let units=0, split=0;
  for (const ch of chars) {
    if (units>=offset) break;
    units+=ch.units; split++;
  }
  return {left:chars.slice(0,split).map(c=>c.id).reverse(),right:chars.slice(split).map(c=>c.id)};
}

export function position(chars, point) {
  const starts=new Map(),ends=new Map();let units=0;
  for(const ch of chars){starts.set(ch.id,units);units+=ch.units;ends.set(ch.id,units);}
  for(const id of point.right)if(starts.has(id))return starts.get(id);
  for(const id of point.left)if(ends.has(id))return ends.get(id);
  return point.left.length ? units : 0;
}

export function captureSelection(paragraphs, selection) {
  if(!selection)return null;
  const capture=([p,offset])=>({key:paragraphs[p].key,point:anchor(paragraphs[p].chars,offset)});
  return {anchor:capture(selection.anchor),focus:capture(selection.focus)};
}

export function restoreSelection(paragraphs, saved) {
  if(!saved||!paragraphs.length)return null;
  const restore=({key,point})=>{
    const p=paragraphs.find(p=>p.key===key)??paragraphs[0];
    return [p.index,position(p.chars,point)];
  };
  return {anchor:restore(saved.anchor),focus:restore(saved.focus)};
}

// Apply only changed ranges to the real Word editor. This preserves native
// paragraph/run/table layout and reanchors the caret against character IDs.
export function projectDocument(editor, previous, next, selection) {
  const actual=JSON.parse(editor.state());
  if(actual.paragraphs.length!==next.length)throw new Error('段落構造が変更されました。文書を保存してから開き直してください。');
  const saved=selection===undefined?captureSelection(previous,actual.selection):null;
  let changed=false;
  for(const p of next){
    const before=actual.paragraphs[p.index];
    if(before===p.text)continue;
    const r=replacement(before,p.text);
    editor.select(p.index,r.start,p.index,r.end);editor.insert(r.text);changed=true;
  }
  const target=selection??restoreSelection(next,saved);
  if(target)editor.select(...target.anchor,...target.focus);
  const result=JSON.parse(editor.state());
  if(result.paragraphs.some((text,index)=>text!==next[index].text))throw new Error('エディタと共同編集モデルの内容が一致しません');
  return changed;
}
