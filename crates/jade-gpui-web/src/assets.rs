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
            "align-center",
            "align-left",
            "align-right",
            "borders",
            "chevron-down",
            "chevron-right",
            "clock",
            "download",
            "file",
            "file-plus",
            "file-text",
            "files",
            "filter",
            "folder",
            "folder-open",
            "folder-plus",
            "merge",
            "moon",
            "paint-bucket",
            "panel-right",
            "play",
            "presentation",
            "printer",
            "redo",
            "sheet",
            "sun",
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
