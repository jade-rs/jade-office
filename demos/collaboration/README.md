# Collaborative DOCX demo

This dedicated demo shell combines jade-office's DOCX renderer with the
production Rust replica from the sibling `jade-collaboration` checkout.
It supports simultaneous text editing in existing paragraphs and table cells,
Japanese IME, offline/resume, peer presence, native DOCX preview and download.

From `jade-collaboration`, run:

```sh
./demos/run-office.sh
```

Open <http://127.0.0.1:8787/?name=Alice> and click **別の編集者を開く**.
Both clients connect to the same WebSocket document owner. No sticky sessions
are required with this single host.

Prerequisites, options and limits are documented in
[`jade-collaboration/demos/README.md`](https://github.com/jade-rs/jade-collaboration/blob/main/demos/README.md).
This is a dedicated demo screen; the normal GPUI editor is not connected yet.
XLSX/PPTX, formatting, table structure controls and collaborative undo are outside
this screen's scope. Pending offline edits live only in the tab's memory.

The small `renderer` crate exposes `jade-web::document::Document` and the same
fonts/painter as the normal browser Office build. The launch script builds clean
committed Office/engine sources in an isolated generated tree, preserving any
unrelated local work. `Cargo.lock` is generated against those clean sources.

```sh
node --test demos/collaboration/editing.test.mjs
cargo +stable fmt --manifest-path demos/collaboration/renderer/Cargo.toml -- --check
```

While the demo host is running, `/browser-tests.html` exercises two real
editor frames on a separate document. It verifies that composition text and
the caret survive a disconnect, a peer commit is reconciled after resuming,
and both final-input/compositionend event orders produce exactly one edit.
The collaboration repository also runs this screen in Chromium CI. These
are browser composition-event tests, rather than an automated OS IME driver.
