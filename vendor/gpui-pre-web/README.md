# gpui-pre-web 0.3.9 local patch

Source: crates.io `gpui-pre-web` 0.3.9, Zed revision
`93d745668805029476e6d7fbb19652116fed51de`. Apache-2.0; see LICENSE-APACHE.

This copy fixes browser IME candidate positioning. Upstream leaves
`PlatformWindow::update_ime_position` empty and the hidden textarea at (0, 0).

Changes are confined to `src/ime_mirror.rs` and `src/window.rs`:

- Position the textarea at the input handler's candidate bounds in CSS pixels.
- Measure and scroll its native caret to that position despite mirrored context.
- Refresh after drawing, including scrolling, zooming and active composition.
- Preserve the textarea's value, selection and focus while positioning it.

The normalized registry Cargo.toml is included to build outside Zed's workspace.
Remove the patch in crates/jade-gpui-web/Cargo.toml when an upstream release
provides this behavior.
