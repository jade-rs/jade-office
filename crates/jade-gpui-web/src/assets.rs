//! The native app's icons (`jade-gpui-app/assets/icons`), compiled in.

use std::borrow::Cow;

use gpui::SharedString;

pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        macro_rules! icons {
            ($($name:literal),* $(,)?) => {
                match path {
                    $(concat!("icons/", $name, ".svg") => Some(include_bytes!(concat!(
                        "../../jade-gpui-app/assets/icons/", $name, ".svg"
                    )).as_slice()),)*
                    _ => None,
                }
            };
        }
        let bytes = icons!(
            "a-arrow-down",
            "a-arrow-up",
            "align-center",
            "align-justify",
            "align-left",
            "align-right",
            "arrow-down-to-line",
            "arrow-left-to-line",
            "arrow-right-to-line",
            "arrow-up-to-line",
            "baseline",
            "bold",
            "borders",
            "chart-column",
            "check",
            "chevron-down",
            "chevron-right",
            "chevrons-up-down",
            "clock",
            "download",
            "eye-off",
            "eraser",
            "file",
            "file-plus",
            "file-text",
            "files",
            "filter",
            "folder",
            "folder-open",
            "folder-plus",
            "highlighter",
            "image",
            "indent-decrease",
            "indent-increase",
            "italic",
            "list",
            "list-ordered",
            "merge",
            "moon",
            "paint-bucket",
            "paintbrush",
            "panels-top-left",
            "panel-right",
            "play",
            "presentation",
            "printer",
            "redo",
            "settings-2",
            "shapes",
            "sheet",
            "split",
            "square",
            "strikethrough",
            "subscript",
            "sun",
            "superscript",
            "table",
            "trash-2",
            "type",
            "underline",
            "undo",
            "wrap-text",
            "x",
            "zap",
        );
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
