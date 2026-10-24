import test from 'node:test';
import assert from 'node:assert/strict';
import { PresencePublisher, peerColor, draftSelection, validPresence } from './presence.mjs';

test('presence publishes the final cursor of a rapid gesture',async()=>{
  let value=0;const sent=[];
  const publisher=new PresencePublisher(()=>({value}),state=>sent.push(state.value),20);
  publisher.update();value=1;publisher.update();value=2;publisher.update();
  await new Promise(resolve=>setTimeout(resolve,40));
  assert.deepEqual(sent,[0,2]);publisher.dispose();
});
test('disconnect cancels a queued presence, and resume publishes the current position',async()=>{
  let value=0;const sent=[];
  const publisher=new PresencePublisher(()=>({value}),state=>sent.push(state.value),20);
  publisher.update();value=1;publisher.update();publisher.cancel();
  await new Promise(resolve=>setTimeout(resolve,30));assert.deepEqual(sent,[0]);
  value=2;publisher.update();assert.deepEqual(sent,[0,2]);publisher.dispose();
});
test('peer coordinates follow local IME drafts without changing model anchors',()=>{
  const selection={anchor:[1,2],focus:[1,5]};
  const native={paragraphs:['title','ab日本😀c']};
  const model=[{text:'title'},{text:'abcde'}];
  assert.deepEqual(draftSelection(selection,model,native),{anchor:[1,2],focus:[1,7]});
  assert.deepEqual(selection,{anchor:[1,2],focus:[1,5]});
});
test('peer color is deterministic; malformed selections cannot become DOM geometry',()=>{
  assert.equal(peerColor('9007199254740993'),peerColor('9007199254740993'));
  assert.notEqual(peerColor('1'),peerColor('2'));
  assert.equal(validPresence({name:'Bob',selection:{anchor:'{}',focus:'{}'},focused:true}),true);
  for(const bad of [null,{}, {name:'x',selection:{anchor:1,focus:'{}'}},{name:'x',selection:{anchor:'x'.repeat(8193),focus:'{}'}}])assert.equal(validPresence(bad),false);
});
