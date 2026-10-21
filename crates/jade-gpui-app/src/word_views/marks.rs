//! 常に画面に表示する編集記号 (Word のオプション → 表示): the formatting
//! marks shown by kind whatever the ¶ toggle says, kept between runs and
//! given to every document opened.

use jade_docx_editor::FormattingMarkKinds;

use super::settings;
use crate::DocumentView;

/// The kinds, by their saved key, with their labels' keys.
pub const KINDS: [(&str, &str); 6] = [
    ("tabs", "options.marks.tabs"),
    ("spaces", "options.marks.spaces"),
    ("paragraph_marks", "options.marks.paragraph_marks"),
    ("hidden_text", "options.marks.hidden_text"),
    ("optional_hyphens", "options.marks.optional_hyphens"),
    ("anchors", "options.marks.anchors"),
];

/// The kinds chosen, by key.
pub struct Shown(FormattingMarkKinds);

impl Shown {
    pub fn get(&self, key: &str) -> bool {
        let kinds = &self.0;
        match key {
            "tabs" => kinds.tabs,
            "spaces" => kinds.spaces,
            "paragraph_marks" => kinds.paragraph_marks,
            "hidden_text" => kinds.hidden_text,
            "optional_hyphens" => kinds.optional_hyphens,
            _ => kinds.anchors,
        }
    }
}

fn flag(key: &str) -> bool {
    settings::value("options.txt", &format!("marks.{key}")).as_deref() == Some("1")
}

/// The kinds the settings show.
pub fn kinds() -> Shown {
    Shown(current())
}

pub fn current() -> FormattingMarkKinds {
    FormattingMarkKinds {
        tabs: flag("tabs"),
        spaces: flag("spaces"),
        paragraph_marks: flag("paragraph_marks"),
        hidden_text: flag("hidden_text"),
        optional_hyphens: flag("optional_hyphens"),
        anchors: flag("anchors"),
    }
}

pub fn set(key: &str, on: bool) {
    settings::set_value(
        "options.txt",
        &format!("marks.{key}"),
        if on { "1" } else { "0" },
    );
}

/// Gives the document the kinds the settings show, repainting when they
/// changed.
pub fn apply(view: &mut DocumentView) {
    let kinds = current();
    if view.document.formatting_mark_kinds() == kinds {
        return;
    }
    match view.document.set_formatting_mark_kinds(kinds) {
        Ok(()) => {
            view.caret = view
                .document
                .place_caret(view.insert_at.0, view.insert_at.1);
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A kind switched on in the settings reaches the document.
    #[gpui::test]
    fn a_kind_set_reaches_the_document(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            set("spaces", true);
            apply(view);
            assert!(view.document.formatting_mark_kinds().spaces);
            assert!(kinds().get("spaces"));
            set("spaces", false);
            apply(view);
            assert!(!view.document.formatting_mark_kinds().spaces);
        });
    }
}
