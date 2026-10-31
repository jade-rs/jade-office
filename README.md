# jade-office

The native desktop app for jade: one gpui app for Word (`.docx`), Excel (`.xlsx`) and PowerPoint (`.pptx`) files, with native print / PDF export on macOS.

The window follows jade-ide (the browser IDE in jlsi/jade): a title bar, an activity bar, a folder explorer, editor tabs and a status bar, in the IDE's zinc/indigo palette (dark and light, ⌥⌘T). Each file opens as a tab in the view that suits it — paged editing with the ribbon for documents; an editable grid for workbooks worked as Excel is (ranges by drag, Shift and the headers, ⌘-arrows, Enter and Tab round a range; typing with IME into cells, F2, formulas built by pointing at cells, F4; the fill handle, ⌘D/⌘R/⌘Enter; copies that move formulas and cuts that move cells; bold and number-format shortcuts, Merge & Center, sort; rows, columns and sheets inserted, deleted, resized and renamed; Find, AutoSum, notes on hover, a right-click menu; undo/redo, save, and Excel's average/count/sum for a selection — see docs/COMPATIBILITY.md); thumbnails, notes and a full-screen slide show for decks, where a shape can be selected (a click, or Tab), moved (drag or the arrow keys, ⌥ for one px), resized from its corners, deleted and typed into, with undo/redo and save. A tab with unsaved edits shows a dot.

The engine (readers, writers, layout, painting, native renderers) is [jade-rs/jade-core](https://github.com/jade-rs/jade-core), pulled in as the `jade-core` submodule.

## Build

```sh
git clone --recurse-submodules https://github.com/jade-rs/jade-office.git
cd jade-office
cargo run --manifest-path crates/jade-gpui-app/Cargo.toml -- path/to/file.docx path/to/book.xlsx path/to/deck.pptx
```

With no arguments the app opens an empty workspace: File ▸ Open… (⌘O), Open Folder… (⇧⌘O), drag and drop, and recently opened files. A folder passed on the command line becomes the explorer's root.

Zoom is the workspace's: ⌘= / ⌘- step through Office's zoom stops, ⌘0 is 100%, ⌘9 fits, and the status bar has the same controls. Workbooks and slides repaint at the new zoom and at the display's pixel density rather than scaling a bitmap.

Scenario tests drive the real views headlessly through gpui's test support (`cargo test` in `crates/jade-gpui-app`): typing into cells, moving, typing into and deleting slide shapes, undo/redo, opening every format as a tab, zoom.

The shell speaks Japanese and English (`crates/jade-gpui-app/i18n/{ja,en}.tsv`), picked from the system's preferred language; `JADE_LANG=en` or `JADE_LANG=ja` overrides it. That covers the document ribbon and its tooltips too; font family names stay as the fonts are called.

`crates/jade-pptx-native-render` draws slides through Skia (the counterpart of jade-core's `jade-docx-native-render` / `jade-xlsx-native-render`); `cargo run --bin pptx_to_png -- deck.pptx out/` from its directory writes each slide as a PNG.

Use `git submodule update --init` if you cloned without `--recurse-submodules`. The app reads its bundled fonts from `jade-core/fonts/Noto_Sans_JP`.

## Shared editing models

`crates/jade-office-model` holds the editing models both shells use: a Word ribbon's commands (`word.rs`: character and paragraph formatting, the style gallery, lists, and the table commands, ported from jade-ide's style menu and table tools) over any document that is a `WordSession` — the native app's `NativeDocument` and the browser's tiny-skia document; a workbook over jade-core's `HeadlessWorkbook` (typed text, recalculation, clear, paste, copy text, the selection summary, undo/redo, save) and a deck over the pptx `EditSession` (shape hit-testing and Tab order, text, moves, resizes, deletes, undo/redo, save). They are UI-free and generic over a painter `Backend`: `skia` for the native app, `tiny-skia` for the browser build. So typing into a cell or a slide runs the same code natively and in wasm; the shells only draw the chrome round it.

## In the browser (prototype)

The engine's renderers — documents, slides and sheets — also run as WebAssembly, on a pure-Rust painter instead of Skia:

- `crates/jade-tiny-skia-painter` implements `jade_painter::Painter` with tiny-skia (paths, fills, strokes, dashes, clips), ttf-parser (text drawn as glyph outlines, measured by advances like the Skia backend) and `image` (pictures). No C++, so it builds for `wasm32-unknown-unknown` as it is. Shadows, blur and SVG pictures are not drawn yet.
- `crates/jade-web` puts the readers, layout and renderers of all three formats on it, natively and as a wasm-bindgen API. Word documents go through `jade-docx-editor`'s renderer with a tiny-skia render host (`src/document.rs`, the counterpart of `jade-docx-native-render`'s Skia host), measuring with the same shaping measurer. `cargo run --release --bin tiny_vs_skia` compares its slides and pages with the Skia backend's PNGs: within 0.04–2.4% of pixels on the fixture decks and 0.5–5% on documents. `cargo run --release --example typing_profile -- [file.docx] [scale] [paragraph]` types into a paragraph and prints, per keystroke, the time it took and the rows of each page it left to re-read — what the browser shell uploads to the GPU.
- `crates/jade-gpui-web` is a gpui shell for the browser on gpui-pre (a crates.io snapshot of Zed's gpui with a WebGPU/WebGL2 web platform), single-threaded, showing documents, slides and sheets from `jade-web` — and editing sheets and slides through the same cores the native app uses (`HeadlessWorkbook`, the pptx `EditSession`): work a sheet as the native app does (the same selection, keys, fills, clipboard, formats, rows, columns and sheets), ⌘C/⌘X/⌘V in Excel's clipboard text, select a shape (click or Tab) and type into it, move it with the arrow keys, delete it, ⌘Z/⇧⌘Z, and Save, which downloads the edited file. It pins a newer stable toolchain of its own; the native app stays on gpui 0.2.

```sh
./scripts/build-web.sh
python3 -m http.server 8765 --directory crates/jade-web/www
```

Then open http://localhost:8765/ — the gpui shell, laid out like the native app (the same title bar, explorer, tabs, slide and page lists, formula bar and status bar, in the same palette, words and icons: it compiles in `jade-gpui-app`'s i18n tables and icon set). The explorer lists the bundled samples; open your own with the + button, ⌘O or by dropping files on the page; `?open=contract.docx` opens one at start. Word documents edit as in the native app: click to place the caret, drag or Shift-click to select, the arrows, Home and End (Shift extends), ⌘A, type over the selection (IMEs included), Backspace, Delete and Enter, ⌘C/⌘X/⌘V (pasted lines become paragraphs), undo and redo, and save the edited .docx. Over the pages sits jade-ide's ribbon, ported from its style menu and table tools (`crates/jade-gpui-web/src/ribbon.rs`, the native app's `crates/jade-gpui-app/src/ribbon.rs`, both over `jade-office-model`'s `word.rs`): font family and size, grow and shrink, bold, italic, underline (⌘B/⌘I/⌘U), strikethrough, sub- and superscript, colour, highlight and clear formatting — on the selection, or on the word around a bare caret, as in Word; bullets and numbering, indent, alignment, line spacing, shading and borders; the style gallery (標準 and 見出し 1–3, then the document's own paragraph styles); a table picked on a 10 × 8 grid; and, with the caret in a table, rows and columns inserted and deleted, the table deleted or split, cells merged and unmerged, the engine's table-style presets, and a cell-format list (table and cell alignment, shading, borders, even rows and columns, split cells, repeated header row). http://localhost:8765/canvas.html is the plain canvas page over the wasm-bindgen API.

What a keystroke, an open, a cell edit or a paint costs in the browser build is measured by `crates/jade-office-perf`: the same renderers on the tiny-skia painter at 2x, natively, under callgrind. `.github/workflows/perf.yml` runs it on a pull request's base and head and comments the instructions each scenario took, and the functions that moved (`scripts/perf/`, after jade-core's). Run one scenario plainly with `cargo run --profile perf-ci --manifest-path crates/jade-office-perf/Cargo.toml -- docx-keystroke contract.docx`.

## License

PolyForm Noncommercial 1.0.0. Free for personal, research, educational, and noncommercial use. Commercial use requires a separate license — contact the author.

Third-party dependency licenses: see THIRD_PARTY_LICENSES.html (regenerate with `cargo about generate about.hbs --all-features -m crates/jade-gpui-app/Cargo.toml`).

The full license text is in [LICENSE](LICENSE).
