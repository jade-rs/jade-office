import test from 'node:test';
import assert from 'node:assert/strict';
import { replacement, anchor, position } from './editing.mjs';

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
