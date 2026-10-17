# jade-office

The native desktop app for jade: a gpui window that opens, paints and edits `.docx` and `.xlsx`, with native print / PDF export on macOS.

The engine (readers, writers, layout, painting, native renderers) is [jade-rs/jade-core](https://github.com/jade-rs/jade-core), pulled in as the `jade-core` submodule.

## Build

```sh
git clone --recurse-submodules https://github.com/jade-rs/jade-office.git
cd jade-office
cargo run --manifest-path crates/jade-gpui-app/Cargo.toml -- path/to/document.docx
```

Use `git submodule update --init` if you cloned without `--recurse-submodules`. The app reads its bundled fonts from `jade-core/fonts/Noto_Sans_JP`.

## License

PolyForm Noncommercial 1.0.0. Free for personal, research, educational, and noncommercial use. Commercial use requires a separate license — contact the author.

Third-party dependency licenses: see THIRD_PARTY_LICENSES.html (regenerate with `cargo about generate about.hbs --all-features -m crates/jade-gpui-app/Cargo.toml`).

The full license text is in [LICENSE](LICENSE).
