import test from 'node:test';
import assert from 'node:assert/strict';
import { replacement, anchor, position, renderInput } from './editing.mjs';

test('minimal replacement never divides an emoji surrogate pair', () => {
  for (const [before, after] of [['A😀B','A😁B'],['😀😀','😀'],['x','x😀'],['😀x','x'],['あい','あう']]) {
    const r = replacement(before, after);
    assert.equal(before.slice(0,r.start)+r.text+before.slice(r.end),after);
    assert.equal(r.start > 0 && /[\uDC00-\uDFFF]/.test(before[r.start] ?? ''), false);
    assert.equal(/[\uDC00-\uDFFF]/.test(before[r.end] ?? ''), false);
  }
});

test('stable cursor survives a peer insertion, deletion and emoji', () => {
  const chars = [{id:'a',units:1},{id:'b',units:2},{id:'c',units:1}];
  const point = anchor(chars,3);
  assert.equal(position([{id:'z',units:1},...chars],point),4);
  assert.equal(position([chars[0],chars[2]],point),1);
  assert.equal(position([],point),0);
});

test('reconnect repaint must not write an active IME draft or move its caret', () => {
  const chars = [{id:'a',units:1},{id:'b',units:1}];
  const confirmed = {text:'ab',chars};
  const input = {
    value:'ab日本語😀',selectionStart:7,selectionEnd:7,
    setSelectionRange(start,end) { this.selectionStart=start;this.selectionEnd=end; },
  };
  renderInput(input,confirmed,confirmed,true,true,null);
  assert.equal(input.value,'ab日本語😀');
  assert.equal(input.selectionStart,7);
  assert.equal(input.selectionEnd,7);
});

test('normal remote repaint rebases a selection; a local edit keeps its new caret', () => {
  const before = {text:'ab',chars:[{id:'a',units:1},{id:'b',units:1}]};
  const after = {text:'😀ab',chars:[{id:'peer',units:2},...before.chars]};
  const input = {value:'ab',selectionStart:1,selectionEnd:2,setSelectionRange(start,end){this.selectionStart=start;this.selectionEnd=end;}};
  renderInput(input,before,after,false,true,null);
  assert.equal(input.value,'😀ab');
  assert.deepEqual([input.selectionStart,input.selectionEnd],[3,4]);
  renderInput(input,after,after,false,true,{start:1,end:1});
  assert.deepEqual([input.selectionStart,input.selectionEnd],[1,1]);
});
