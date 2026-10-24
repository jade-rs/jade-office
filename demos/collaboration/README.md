# Collaborative Word canvas demo

The document page itself is editable. Click body text or a table cell and type;
drag or use Shift + arrows to select, then type or delete. All hit testing,
caret motion, selection geometry, text edits, IME drafts, layout and pixels come
from `jade-web::editing::EditableDocument`, the Word editing core used by the
Office app. `canvas-editor.mjs` supplies browser pointer/keyboard/clipboard
input and paints the editor's caret/selection over its pages. There are no
paragraph textareas or separate read-only preview.

`jade-collaboration`'s production Rust replica owns the collaborative state.
Edits become stable-character operations, not file replacements. Incoming
changes patch only differing ranges in the native editor, preserving its
paragraph/table layout and rebasing selections by character IDs. Disconnects
and queued server frames do not interrupt an active IME composition. A
cancelled composition restores the original selection and text without sending
an edit.

From the sibling `jade-collaboration` repository:

```sh
./demos/run-office.sh
```

Open <http://127.0.0.1:8787/?name=Alice> and click **別の編集者を開く**.
Both clients connect to the same WebSocket document owner. No sticky sessions
are required with this single host. **DOCXを保存** downloads the native server
export. Offline edits are visible in the page immediately and resume with the
same replica when reconnecting; unacknowledged state lives in this tab's memory.

This dedicated canvas shell uses the real Office Word editing engine; the
full GPUI ribbon/workspace is not connected. Editing is confined to existing
paragraphs and table cells. Enter inserts a Word line break within a paragraph;
paste supports line breaks and tabs. Paragraph creation/merging, table
structure, formatting controls, collaborative undo, XLSX/PPTX and remote
cursor graphics are outside this demo's scope. Cmd/Ctrl+A selects the current
paragraph. Cross-paragraph text replacement is refused before mutation.

The `renderer` WASM bridge exposes `OfficeEditor` and the bundled fonts. The
launcher builds committed Office/engine sources in an isolated generated tree,
then overlays only this demo, preserving unrelated work in the checkout.

```sh
node --test demos/collaboration/editing.test.mjs
cargo +stable fmt --manifest-path demos/collaboration/renderer/Cargo.toml -- --check
```

`/browser-tests.html` drives two real canvas editors on a separate document.
It tests IME drafts/carets during peer edits and reconnect, both final-input
orders and cancellation over a selection. These are composition-event tests,
not an automated OS IME driver. The collaboration repository additionally
checks actual pointer/keyboard input in Chromium CI, real-socket convergence,
range projection into the native Word editor and native DOCX export/reopen.
