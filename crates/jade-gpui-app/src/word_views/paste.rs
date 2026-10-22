//! 貼り付けの既定: what ⌘V keeps, as Word のオプション → 編集 sets it for a
//! paste within the same document, between documents, and from other
//! programs.

use std::cell::RefCell;

use gpui::Context;
use jade_office_model::word::clipboard::PasteMode;

use super::settings;
use crate::DocumentView;

/// Where what is pasted came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    SameDocument,
    OtherDocument,
    OtherProgram,
}

impl Source {
    pub const ALL: [Source; 3] = [
        Source::SameDocument,
        Source::OtherDocument,
        Source::OtherProgram,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Source::SameDocument => "paste_same",
            Source::OtherDocument => "paste_between",
            Source::OtherProgram => "paste_other",
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Source::SameDocument => "options.paste_same",
            Source::OtherDocument => "options.paste_between",
            Source::OtherProgram => "options.paste_other",
        }
    }
}

pub fn mode_id(mode: PasteMode) -> &'static str {
    match mode {
        PasteMode::KeepSource => "keep",
        PasteMode::MergeFormatting => "merge",
        PasteMode::TextOnly => "text",
    }
}

/// The mode set for pastes from `source`; 元の書式を保持 unless changed.
pub fn mode(source: Source) -> PasteMode {
    match settings::value("options.txt", source.key()).as_deref() {
        Some("merge") => PasteMode::MergeFormatting,
        Some("text") => PasteMode::TextOnly,
        _ => PasteMode::KeepSource,
    }
}

pub fn set_mode(source: Source, mode: PasteMode) {
    settings::set_value("options.txt", source.key(), mode_id(mode));
}

thread_local! {
    /// The document the last copy came from.
    static COPIED_FROM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// A copy (or cut) in `view`: remembered, so a paste knows whether it is
/// within the same document.
pub fn note_copy(view: &DocumentView) {
    COPIED_FROM.with(|from| *from.borrow_mut() = Some(view.path.to_string()));
}

/// ⌘V, when the setting for where the clipboard came from asks for
/// something other than what a plain paste does. `rich` says whether the
/// clipboard holds this app's own copy. `true` when it pasted.
pub fn default_paste(view: &mut DocumentView, rich: bool, cx: &mut Context<DocumentView>) -> bool {
    let source = if !rich {
        Source::OtherProgram
    } else if COPIED_FROM.with(|from| from.borrow().as_deref() == Some(view.path.as_ref())) {
        Source::SameDocument
    } else {
        Source::OtherDocument
    };
    let mode = mode(source);
    // Text from elsewhere has no formatting of its own to keep: as it is,
    // it already takes the formatting where it goes.
    if mode == PasteMode::KeepSource || !rich {
        return false;
    }
    crate::word_characters::paste_with(view, mode, cx);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Set to keep the text only within the document, a copy pasted back
    /// loses its bold.
    #[gpui::test]
    fn the_default_decides_what_a_paste_keeps(cx: &mut gpui::TestAppContext) {
        use jade_office_model::word::CharacterFormat;
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        let paragraph = view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let text = view.current_paragraph_text();
            let end = text.char_indices().nth(3).map(|(at, _)| at).unwrap();
            view.document
                .select((paragraph, 0), (paragraph, end), 1)
                .unwrap();
            view.insert_at = (paragraph, end);
            view.run_ribbon(crate::ribbon::Action::Toggle(CharacterFormat::Bold), cx);
            view.copy(cx);
            set_mode(Source::SameDocument, PasteMode::TextOnly);
            assert_eq!(mode(Source::SameDocument), PasteMode::TextOnly);
            // Pasted at the paragraph's end.
            let length = view.current_paragraph_text().len();
            view.insert_at = (paragraph, length);
            view.caret = view.document.place_caret(paragraph, length);
            view.paste(cx);
            paragraph
        });
        view.read_with(cx, |view, _| {
            let length = view.document.paragraph_text(paragraph).unwrap().len();
            let bold = view
                .document
                .run_style_at(paragraph, length - 1)
                .and_then(|style| style.bold)
                .unwrap_or(false);
            assert!(!bold, "the text came without its bold");
        });
    }
}
