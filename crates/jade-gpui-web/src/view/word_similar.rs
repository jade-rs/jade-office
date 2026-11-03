//! The browser's 類似した書式の文字列を選択, as the native app's: every run
//! of the body formatted as the selection is, selected together. The first
//! range is the document's selection; the rest are kept here, drawn beside
//! it, and ホーム's font commands go on all of them as one undo step
//! (`jade_office_model`'s `similar::format`).

use gpui::Context;
use jade_web::editing::{HighlightRect, WordDoc};
use jade_web::word::similar::{still_selected, Range, WordSimilar};

use super::{DocView, Document};

impl DocView {
    /// ホーム › 編集 › 類似した書式の文字列を選択.
    pub(super) fn select_similar(&mut self, cx: &mut Context<Self>) {
        let Document::Pages { document, .. } = &mut self.document else {
            return;
        };
        self.similar = document.select_similar_formatting();
        self.word_result(Ok(()), cx);
    }
}

/// The highlight of the ranges beside the selection, while they are still
/// selected with it.
pub(super) fn extra_rects(document: &WordDoc, similar: &[Range]) -> Vec<HighlightRect> {
    if !still_selected(document, similar) {
        return Vec::new();
    }
    similar[1..]
        .iter()
        .flat_map(|&(start, end)| document.range_rects(start, end))
        .collect()
}
