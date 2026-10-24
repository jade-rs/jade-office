import test from 'node:test';
import assert from 'node:assert/strict';
import { replacement, anchor, position, captureSelection, restoreSelection } from './editing.mjs';

test('minimal replacement never divides an emoji surrogate pair', () => {
  for (const [before,after] of [['A😀B','A😁B'],['😀😀','😀'],['x','x😀'],['😀x','x'],['あい','あう'],['a\nb','a\nあb']]) {
    const r=replacement(before,after);
    assert.equal(before.slice(0,r.start)+r.text+before.slice(r.end),after);
    assert.equal(r.start>0&&/[\uDC00-\uDFFF]/.test(before[r.start]??''),false);
    assert.equal(/[\uDC00-\uDFFF]/.test(before[r.end]??''),false);
  }
});
const chars = text => [...text].map((ch,i)=>({id:`id-${i}`,units:ch.length}));

test('a selection follows stable character IDs when peers edit around it', () => {
  const old=[{index:0,key:'body',text:'a😀bc',chars:chars('a😀bc')}];
  const saved=captureSelection(old,{anchor:[0,1],focus:[0,4]});
  const next=[{...old[0],text:'あa😀bc',chars:[{id:'peer',units:1},...old[0].chars]}];
  assert.deepEqual(restoreSelection(next,saved),{anchor:[0,2],focus:[0,5]});
});

test('deleting both immediate neighbours uses the nearest surviving character', () => {
  const old=chars('abcdef');
  const saved=anchor(old,3);
  assert.equal(position([old[0],old[1],old[4],old[5]],saved),2);
  assert.equal(position([],saved),0);
});

test('selection direction and table paragraph identity survive remote edits', () => {
  const body={index:0,key:'body',text:'a',chars:chars('a')};
  const cell={index:1,key:'cell',text:'abc',chars:chars('abc')};
  const saved=captureSelection([body,cell],{anchor:[1,3],focus:[1,1]});
  assert.deepEqual(restoreSelection([{...cell,index:0}, {...body,index:1}],saved),{anchor:[0,3],focus:[0,1]});
});

test('stateful cursor projection stays on Unicode boundaries across random deliveries', () => {
  let random=20261024;
  const next=()=>{random=(Math.imul(random,1664525)+1013904223)>>>0;return random;};
  for(let trial=0;trial<128;trial++) {
    let current=chars('あ😀abcdef'), serial=10;
    let offset=0;
    for(let step=0;step<100;step++) {
      const saved=anchor(current,offset), old=[...current];
      if(next()%2&&current.length)current.splice(next()%current.length,1);
      else current.splice(next()%(current.length+1),0,{id:`new-${serial++}`,units:next()%2?1:2});
      offset=position(current,saved);
      const boundaries=new Set([0]);let end=0;for(const c of current){end+=c.units;boundaries.add(end);}
      assert.ok(boundaries.has(offset));
      if(JSON.stringify(current)===JSON.stringify(old))assert.equal(position(old,saved),offset);
    }
  }
});
