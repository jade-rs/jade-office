# jade-office

The native desktop app for jade: one gpui app for Word (`.docx`), Excel (`.xlsx`) and PowerPoint (`.pptx`) files, with native print / PDF export on macOS.

The window follows jade-ide (the browser IDE in jlsi/jade): a title bar, an activity bar, a folder explorer, editor tabs and a status bar, in the IDE's zinc/indigo palette (dark and light, ⌥⌘T). Each file opens as a tab in the view that suits it — paged editing with the ribbon for documents; an editable grid for workbooks (name box and formula bar, typing with IME into cells, formulas that recalculate, Delete, copy/cut/paste as tab-separated text, undo/redo, save, and Excel's average/count/sum for a selection); thumbnails, notes and a full-screen slide show for decks. A tab with unsaved edits shows a dot.

The engine (readers, writers, layout, painting, native renderers) is [jade-rs/jade-core](https://github.com/jade-rs/jade-core), pulled in as the `jade-core` submodule.

## Build

```sh
git clone --recurse-submodules https://github.com/jade-rs/jade-office.git
cd jade-office
cargo run --manifest-path crates/jade-gpui-app/Cargo.toml -- path/to/file.docx path/to/book.xlsx path/to/deck.pptx
```

With no arguments the app opens an empty workspace: File ▸ Open… (⌘O), Open Folder… (⇧⌘O), drag and drop, and recently opened files. A folder passed on the command line becomes the explorer's root.

Zoom is the workspace's: ⌘= / ⌘- step through Office's zoom stops, ⌘0 is 100%, ⌘9 fits, and the status bar has the same controls. Workbooks and slides repaint at the new zoom and at the display's pixel density rather than scaling a bitmap.

Scenario tests drive the real views headlessly through gpui's test support (`cargo test` in `crates/jade-gpui-app`): typing into cells, undo/redo, opening every format as a tab, zoom.

The shell speaks Japanese and English (`crates/jade-gpui-app/i18n/{ja,en}.tsv`), picked from the system's preferred language; `JADE_LANG=en` or `JADE_LANG=ja` overrides it. The document ribbon's labels are still Japanese only.

`crates/jade-pptx-native-render` draws slides through Skia (the counterpart of jade-core's `jade-docx-native-render` / `jade-xlsx-native-render`); `cargo run --bin pptx_to_png -- deck.pptx out/` from its directory writes each slide as a PNG.

Use `git submodule update --init` if you cloned without `--recurse-submodules`. The app reads its bundled fonts from `jade-core/fonts/Noto_Sans_JP`.

## License

PolyForm Noncommercial 1.0.0. Free for personal, research, educational, and noncommercial use. Commercial use requires a separate license — contact the author.

Third-party dependency licenses: see THIRD_PARTY_LICENSES.html (regenerate with `cargo about generate about.hbs --all-features -m crates/jade-gpui-app/Cargo.toml`).

The full license text is in [LICENSE](LICENSE).
