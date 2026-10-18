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
            "chevron-down",
            "chevron-right",
            "clock",
            "download",
            "file",
            "file-plus",
            "file-text",
            "files",
            "folder",
            "folder-open",
            "folder-plus",
            "moon",
            "panel-right",
            "play",
            "presentation",
            "redo",
            "printer",
            "sheet",
            "sun",
            "undo",
            "x",
            "zap",
        );
        Ok(bytes.map(Cow::Borrowed))
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(Vec::new())
    }
}
