#!/usr/bin/env node
// npm install --prefix /tmp/jade-store-test fake-indexeddb
// NODE_PATH=/tmp/jade-store-test/node_modules node scripts/test-web-storage.cjs
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { indexedDB, IDBDatabase } = require('fake-indexeddb');
const source = fs.readFileSync(path.join(__dirname, '../crates/jade-web/www/index.html'), 'utf8');
const script = source.match(/<script>([\s\S]*?)<\/script>/)[1];
const listeners = new Map();
let clock = 1000;
const context = vm.createContext({
  indexedDB,
  Date: { now: () => clock },
  addEventListener: (type, callback) => {
    if (!listeners.has(type)) listeners.set(type, new Set());
    listeners.get(type).add(callback);
  },
  removeEventListener: (type, callback) => listeners.get(type)?.delete(callback),
});
vm.runInContext(script, context);
const store = context.jadeStore;

(async () => {
  const bytes = new Uint8Array([80, 75, 3, 4]);
  // Upgrade the deployed v1 database without losing recovery/templates.
  await new Promise((resolve, reject) => {
    const request = indexedDB.open('jade-office', 1);
    request.onupgradeneeded = () => {
      const recovery = request.result.createObjectStore('recovery');
      recovery.put({ name: 'legacy.docx', time: 1, bytes }, 'legacy');
      const templates = request.result.createObjectStore('templates');
      templates.put({ bytes }, 'legacy-normal');
    };
    request.onsuccess = () => { request.result.close(); resolve(); };
    request.onerror = () => reject(request.error);
  });
  assert.deepEqual((await store.get('recovery', 'legacy')).bytes, bytes);
  assert.deepEqual((await store.get('templates', 'legacy-normal')).bytes, bytes);
  await store.remove('recovery', 'legacy');

  // Concurrent opens are serialized; the bounded list retains the newest.
  await Promise.all(Array.from({ length: 10 }, (_, index) =>
    store.remember(`document-${index}.docx`, new Uint8Array([index]))));
  let recent = await store.all('recent');
  assert.equal(recent.length, 8);
  assert.equal(await store.get('recent', 'document-0.docx'), undefined);
  assert.equal(await store.get('recent', 'document-1.docx'), undefined);
  assert.equal(new Set(recent.map(entry => entry.time)).size, 8);
  const previousTime = Math.max(...recent.map(entry => entry.time));
  clock = 1; // Clock rollback must not reorder the recent list.
  await store.remember('document-2.docx', bytes);
  assert.deepEqual((await store.get('recent', 'document-2.docx')).bytes, bytes);
  assert.ok((await store.get('recent', 'document-2.docx')).time > previousTime);
  const beforeAbort = Array.from(await store.all('recent'), entry => entry.key).sort();

  await store.put('recovery', 'draft', { name: 'draft.docx', time: 123, bytes });
  assert.deepEqual((await store.get('recovery', 'draft')).bytes, bytes);
  assert.equal((await store.all('recovery'))[0].key, 'draft');

  // Abort *after* the put request succeeds. Its promise must reject, and
  // the previous recovery copy must survive the rolled-back replacement.
  const original = IDBDatabase.prototype.transaction;
  IDBDatabase.prototype.transaction = function (...args) {
    const transaction = original.apply(this, args);
    if (args[1] === 'readwrite') {
      const objectStore = transaction.objectStore(args[0]);
      const put = objectStore.put;
      objectStore.put = function (...values) {
        const request = put.apply(this, values);
        request.addEventListener('success', () => transaction.abort());
        return request;
      };
    }
    return transaction;
  };
  try {
    await assert.rejects(store.put('recovery', 'draft', { name: 'replacement.docx' }));
    await assert.rejects(store.remember('aborted.docx', bytes));
  } finally {
    IDBDatabase.prototype.transaction = original;
  }
  assert.equal((await store.get('recovery', 'draft')).name, 'draft.docx');
  assert.deepEqual(Array.from(await store.all('recent'), entry => entry.key).sort(), beforeAbort);
  assert.equal(await store.get('recent', 'aborted.docx'), undefined);

  // A new page context sees the last saved bytes and does not add a duplicate.
  const reloaded = vm.createContext({ indexedDB, addEventListener() {}, removeEventListener() {} });
  vm.runInContext(script, reloaded);
  assert.deepEqual((await reloaded.jadeStore.get('recent', 'document-2.docx')).bytes, bytes);
  await reloaded.jadeStore.remember('document-2.docx', new Uint8Array([42]));
  assert.equal((await store.all('recent')).length, 8);
  assert.deepEqual((await store.get('recent', 'document-2.docx')).bytes, new Uint8Array([42]));
  await reloaded.jadeStore.remember('document-10.docx', bytes);
  assert.equal(await store.get('recent', 'document-3.docx'), undefined);
  assert.deepEqual((await store.get('recent', 'document-2.docx')).bytes, new Uint8Array([42]));
  assert.equal((await store.all('recent')).length, 8);
  // Even an exhausted cursor is not a completed read transaction.
  IDBDatabase.prototype.transaction = function (...args) {
    const transaction = original.apply(this, args);
    const objectStore = transaction.objectStore(args[0]);
    const openCursor = objectStore.openCursor;
    objectStore.openCursor = function (...values) {
      const request = openCursor.apply(this, values);
      request.addEventListener('success', () => {
        if (!request.result) transaction.abort();
      });
      return request;
    };
    return transaction;
  };
  try {
    await assert.rejects(store.all('recovery'));
  } finally {
    IDBDatabase.prototype.transaction = original;
  }
  await store.remove('recovery', 'draft');
  assert.equal((await store.all('recovery')).length, 0);

  context.jadeSetUnsavedChanges(true);
  context.jadeSetUnsavedChanges(true);
  assert.equal(listeners.get('beforeunload').size, 1);
  let prevented = false;
  const event = { preventDefault: () => { prevented = true; } };
  for (const callback of listeners.get('beforeunload')) callback(event);
  assert.equal(prevented, true);
  assert.equal(event.returnValue, '');
  context.jadeSetUnsavedChanges(false);
  assert.equal(listeners.get('beforeunload').size, 0);

  // An initial database-open failure must not poison all subsequent calls.
  let attempts = 0;
  const retryContext = vm.createContext({
    addEventListener() {}, removeEventListener() {},
    indexedDB: {
      open(...args) {
        ++attempts;
        if (attempts === 1) throw new Error('storage temporarily denied');
        if (attempts > 2) return indexedDB.open(...args);
        const request = { error: new Error('temporarily unavailable') };
        queueMicrotask(() => request.onerror());
        return request;
      },
    },
  });
  vm.runInContext(script, retryContext);
  await assert.rejects(retryContext.jadeStore.get('templates', 'normal'));
  await assert.rejects(retryContext.jadeStore.get('templates', 'normal'));
  await retryContext.jadeStore.put('templates', 'normal', { bytes });
  assert.deepEqual((await retryContext.jadeStore.get('templates', 'normal')).bytes, bytes);
  console.log('Browser storage migration/recent/commit/abort/retry and unsaved navigation checks passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
