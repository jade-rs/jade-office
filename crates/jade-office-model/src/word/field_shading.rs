//! Word 環境設定 › 表示 › フィールドの網かけ: whether fields' text is drawn
//! on Word's grey — never, always, or while the selection is in the field
//! (Word's default). A view setting: the document and its PDF are left
//! alone.

use std::rc::Rc;

use jade_docx_editor::canvas_renderer::FieldShading;

use super::WordSession;

/// The pop-up's three choices.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FieldShadingMode {
    /// 表示しない.
    Never,
    /// 常に表示する.
    Always,
    /// 選択時に表示する.
    #[default]
    WhenSelected,
}

impl FieldShadingMode {
    /// In the order Word's pop-up lists them.
    pub const ALL: [FieldShadingMode; 3] = [
        FieldShadingMode::Never,
        FieldShadingMode::Always,
        FieldShadingMode::WhenSelected,
    ];

    /// How the settings file keeps it.
    pub fn key(self) -> &'static str {
        match self {
            FieldShadingMode::Never => "never",
            FieldShadingMode::Always => "always",
            FieldShadingMode::WhenSelected => "selected",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.key() == key)
    }

    /// Its label's i18n key.
    pub fn label_key(self) -> &'static str {
        match self {
            FieldShadingMode::Never => "options.field_shading.never",
            FieldShadingMode::Always => "options.field_shading.always",
            FieldShadingMode::WhenSelected => "options.field_shading.selected",
        }
    }
}

/// What the renderer is to shade for `mode` in `document` as it is now:
/// with 選択時に表示する, the fields the selection (or the caret) is in.
pub fn shading_for<S: WordSession + ?Sized>(document: &S, mode: FieldShadingMode) -> FieldShading {
    match mode {
        FieldShadingMode::Never => FieldShading::Never,
        FieldShadingMode::Always => FieldShading::Always,
        FieldShadingMode::WhenSelected => {
            let runs = document
                .selection_range()
                .map(|range| {
                    jade_docx_editor::fields::field_shading_runs(document.word_document(), range)
                })
                .unwrap_or_default();
            if runs.is_empty() {
                FieldShading::Never
            } else {
                FieldShading::Runs(Rc::new(runs))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_keep_their_keys() {
        for mode in FieldShadingMode::ALL {
            assert_eq!(FieldShadingMode::from_key(mode.key()), Some(mode));
        }
        assert_eq!(FieldShadingMode::from_key("sometimes"), None);
        assert_eq!(FieldShadingMode::default(), FieldShadingMode::WhenSelected);
    }
}
