//! スペル チェックと文章校正 in the native app: macOS's spell checker
//! (`NSSpellChecker`, which knows the system's languages and the user's
//! dictionary) marks misspellings with a red wave and grammar with a blue
//! double line as the pages are drawn; a right-click on a marked word offers
//! its suggestions, 無視 and 辞書に追加; and the 文章校正 pane walks the
//! issues one at a time. What is checked and what an answer does is
//! `jade-office-model`'s `word::proofing`.

#[cfg(test)]
use gpui::point;
use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, ClickEvent, Context, MouseDownEvent,
    Pixels, Point,
};
use jade_office_model::word::proofing::{
    Issue, ProofingKind, ProofingPane, SpellChecker, WordProofing, WordProofingDisplay,
};

use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

/// How often the pages are checked for spans drawn since: after typing,
/// the new word is marked within this.
const PROOFING_TICK: std::time::Duration = std::time::Duration::from_millis(400);

/// The view's share of proofing.
pub struct Proofing {
    pub checker: Box<dyn SpellChecker>,
    /// The 文章校正 pane, while it is open.
    pub pane: Option<ProofingPane>,
    /// The menu a right-click on a marked word opened, where it opened.
    pub menu: Option<(Point<Pixels>, Issue)>,
    /// Whether the ticker that checks newly drawn text is running.
    ticking: bool,
    /// Whether the marks were put on when the document was first shown.
    started: bool,
}

impl Default for Proofing {
    fn default() -> Self {
        Self {
            checker: system_checker(),
            pane: None,
            menu: None,
            ticking: false,
            started: false,
        }
    }
}

/// The platform's checker: macOS's, or none elsewhere.
pub fn system_checker() -> Box<dyn SpellChecker> {
    #[cfg(target_os = "macos")]
    {
        Box::new(mac::MacSpellChecker::new())
    }
    #[cfg(not(target_os = "macos"))]
    {
        Box::new(jade_office_model::word::proofing::WordListChecker::default())
    }
}

#[cfg(target_os = "macos")]
mod mac {
    //! `NSSpellChecker`, which answers in UTF-16 ranges of an `NSString`.

    use jade_office_model::word::proofing::{ProofingIssue, ProofingKind, SpellChecker};
    use objc2::rc::Retained;
    use objc2_app_kit::NSSpellChecker;
    use objc2_foundation::{NSRange, NSString};

    /// `NSNotFound`.
    const NOT_FOUND: usize = isize::MAX as usize;

    pub struct MacSpellChecker {
        /// The document's tag: what すべて無視 ignores is kept under it.
        tag: isize,
    }

    impl MacSpellChecker {
        pub fn new() -> Self {
            Self {
                tag: NSSpellChecker::uniqueSpellDocumentTag(),
            }
        }
    }

    /// `w:lang` as the spell checker names languages: `en-US` → `en_US`.
    /// Japanese has no spelling to check; `None` lets the checker tell.
    fn language(lang: Option<&str>) -> Option<Retained<NSString>> {
        let lang = lang?;
        let primary = lang.split(['-', '_']).next()?.to_ascii_lowercase();
        if matches!(primary.as_str(), "ja" | "zh" | "ko") {
            return None;
        }
        Some(NSString::from_str(&lang.replace('-', "_")))
    }

    /// Byte offsets of `text` by UTF-16 offset, one past the end too.
    fn byte_offsets(text: &str) -> Vec<usize> {
        let mut offsets = Vec::with_capacity(text.len() + 1);
        for (at, ch) in text.char_indices() {
            for _ in 0..ch.len_utf16() {
                offsets.push(at);
            }
        }
        offsets.push(text.len());
        offsets
    }

    fn utf16_range(text: &str, start: usize, end: usize) -> NSRange {
        let from = text[..start].encode_utf16().count();
        let length = text[start..end].encode_utf16().count();
        NSRange::new(from, length)
    }

    impl SpellChecker for MacSpellChecker {
        fn check(
            &mut self,
            text: &str,
            start: usize,
            end: usize,
            lang: Option<&str>,
        ) -> Vec<ProofingIssue> {
            let span = &text[start..end];
            if span.trim().is_empty() {
                return Vec::new();
            }
            let checker = NSSpellChecker::sharedSpellChecker();
            let string = NSString::from_str(span);
            let language = language(lang);
            let offsets = byte_offsets(span);
            let units = offsets.len() - 1;
            let mut issues = Vec::new();
            let mut push = |range: NSRange, kind| {
                let (from, to) = (range.location, range.location + range.length);
                if range.length > 0 && to <= units {
                    issues.push(ProofingIssue {
                        start: start + offsets[from],
                        end: start + offsets[to],
                        kind,
                    });
                }
            };
            let mut at = 0usize;
            while at < units {
                // SAFETY: a null word count is allowed.
                let range = unsafe {
                    checker.checkSpellingOfString_startingAt_language_wrap_inSpellDocumentWithTag_wordCount(
                        &string,
                        at as isize,
                        language.as_deref(),
                        false,
                        self.tag,
                        std::ptr::null_mut(),
                    )
                };
                if range.location == NOT_FOUND || range.length == 0 || range.location < at {
                    break;
                }
                push(range, ProofingKind::Spelling);
                at = range.location + range.length;
            }
            let mut at = 0usize;
            while at < units {
                // SAFETY: no details are asked for.
                let range = unsafe {
                    checker.checkGrammarOfString_startingAt_language_wrap_inSpellDocumentWithTag_details(
                        &string,
                        at as isize,
                        language.as_deref(),
                        false,
                        self.tag,
                        None,
                    )
                };
                if range.location == NOT_FOUND || range.length == 0 || range.location < at {
                    break;
                }
                push(range, ProofingKind::Grammar);
                at = range.location + range.length;
            }
            issues.sort_by_key(|issue| (issue.start, issue.end));
            issues
        }

        fn suggestions(
            &mut self,
            text: &str,
            start: usize,
            end: usize,
            kind: ProofingKind,
            lang: Option<&str>,
        ) -> Vec<String> {
            if kind == ProofingKind::Grammar {
                // The grammar checker's corrections come in its details.
                let checker = NSSpellChecker::sharedSpellChecker();
                let string = NSString::from_str(&text[start..end]);
                let mut details = None;
                unsafe {
                    checker.checkGrammarOfString_startingAt_language_wrap_inSpellDocumentWithTag_details(
                        &string,
                        0,
                        language(lang).as_deref(),
                        false,
                        self.tag,
                        Some(&mut details),
                    );
                }
                let mut corrections = Vec::new();
                if let Some(details) = details {
                    for detail in details.iter() {
                        let key = NSString::from_str("NSGrammarCorrections");
                        if let Some(list) = detail.objectForKey(&key) {
                            if let Ok(list) = list.downcast::<objc2_foundation::NSArray>() {
                                for item in list.iter() {
                                    if let Ok(item) = item.downcast::<NSString>() {
                                        corrections.push(item.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
                return corrections;
            }
            let checker = NSSpellChecker::sharedSpellChecker();
            let string = NSString::from_str(text);
            checker
                .guessesForWordRange_inString_language_inSpellDocumentWithTag(
                    utf16_range(text, start, end),
                    &string,
                    language(lang).as_deref(),
                    self.tag,
                )
                .map(|guesses| guesses.iter().map(|guess| guess.to_string()).collect())
                .unwrap_or_default()
        }

        fn description(&mut self, text: &str, start: usize, end: usize) -> Option<String> {
            let checker = NSSpellChecker::sharedSpellChecker();
            let string = NSString::from_str(&text[start..end]);
            let mut details = None;
            unsafe {
                checker
                    .checkGrammarOfString_startingAt_language_wrap_inSpellDocumentWithTag_details(
                        &string,
                        0,
                        None,
                        false,
                        self.tag,
                        Some(&mut details),
                    );
            }
            let key = NSString::from_str("NSGrammarUserDescription");
            details?
                .iter()
                .find_map(|detail| detail.objectForKey(&key))
                .and_then(|value| value.downcast::<NSString>().ok())
                .map(|value| value.to_string())
        }

        fn ignore(&mut self, word: &str) {
            NSSpellChecker::sharedSpellChecker()
                .ignoreWord_inSpellDocumentWithTag(&NSString::from_str(word), self.tag);
        }

        fn learn(&mut self, word: &str) {
            NSSpellChecker::sharedSpellChecker().learnWord(&NSString::from_str(word));
        }
    }

    impl Drop for MacSpellChecker {
        fn drop(&mut self) {
            NSSpellChecker::sharedSpellChecker().closeSpellDocumentWithTag(self.tag);
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        /// macOS's checker finds the misspelt word and suggests the right
        /// one, in byte offsets past text before it in Japanese.
        #[test]
        fn the_system_checker_finds_and_corrects_a_misspelling() {
            let mut checker = MacSpellChecker::new();
            let text = "日本 The recieve here.";
            let start = "日本 ".len();
            let issues = checker.check(text, start, text.len(), Some("en-US"));
            let found = issues
                .iter()
                .find(|issue| issue.kind == ProofingKind::Spelling)
                .expect("recieve is misspelt");
            assert_eq!(&text[found.start..found.end], "recieve");
            let guesses =
                checker.suggestions(text, found.start, found.end, ProofingKind::Spelling, None);
            assert!(
                guesses.iter().any(|guess| guess == "receive"),
                "{guesses:?}"
            );
        }
    }
}

/// The marks on or off (校閲 › スペル チェック), and the ticker that checks
/// what is drawn while they are on.
pub fn set_marks(view: &mut DocumentView, on: bool, cx: &mut Context<DocumentView>) {
    if let Err(error) = view.document.set_proofing(on) {
        view.table_status = Some(error.into());
    }
    if on {
        proof_now(view);
        start_ticker(view, cx);
    }
    view.rebuild_all_images();
    cx.notify();
}

/// Checks what was drawn since last time; redraws the pages it marked.
pub fn proof_now(view: &mut DocumentView) {
    if !view.document.proofing() {
        return;
    }
    let checker = view.review.proofing.checker.as_mut();
    match view.document.proof_with(checker) {
        Ok(true) => view.rebuild_all_images(),
        Ok(false) => {}
        Err(error) => view.table_status = Some(error.into()),
    }
}

fn start_ticker(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.review.proofing.ticking {
        return;
    }
    view.review.proofing.ticking = true;
    let executor = cx.background_executor().clone();
    cx.spawn(async move |view, cx| loop {
        executor.timer(PROOFING_TICK).await;
        let going = view
            .update(cx, |view, cx| {
                if !view.document.proofing() {
                    view.review.proofing.ticking = false;
                    return false;
                }
                let before = view.edits;
                proof_now(view);
                if view.edits != before {
                    cx.notify();
                }
                true
            })
            .unwrap_or(false);
        if !going {
            break;
        }
    })
    .detach();
}

/// スペル チェックと文章校正 (F7): the marks on, and the pane on the first
/// issue after the caret.
pub fn open_pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if !view.document.proofing() {
        set_marks(view, true, cx);
    }
    let mut pane = ProofingPane::default();
    let from = view.insert_at;
    pane.advance(&view.document, from, view.review.proofing.checker.as_mut());
    view.review.proofing.pane = Some(pane);
    show_current(view, cx);
}

/// Selects the pane's issue in the document.
fn show_current(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let current = view
        .review
        .proofing
        .pane
        .as_ref()
        .and_then(|pane| pane.current.clone());
    if let Some(issue) = current {
        view.select_range(issue.start(), issue.end(), cx);
    }
    cx.notify();
}

/// What the pane's buttons and the right-click menu do.
#[derive(Clone, Debug, PartialEq)]
pub enum ProofAction {
    /// 変更 / すべて変更 with the chosen suggestion.
    Change {
        every: bool,
    },
    /// A suggestion picked from the list.
    Choose(usize),
    /// 無視 / すべて無視.
    Ignore {
        every: bool,
    },
    /// 辞書に追加.
    Learn,
    Close,
}

/// Runs a pane action.
pub fn pane_action(view: &mut DocumentView, action: ProofAction, cx: &mut Context<DocumentView>) {
    let Some(mut pane) = view.review.proofing.pane.take() else {
        return;
    };
    let Some(issue) = pane.current.clone() else {
        if action != ProofAction::Close {
            view.review.proofing.pane = Some(pane);
        }
        return cx.notify();
    };
    let checker = view.review.proofing.checker.as_mut();
    match action {
        ProofAction::Close => return cx.notify(),
        ProofAction::Choose(index) => pane.chosen = index,
        ProofAction::Change { every } => {
            let Some(replacement) = pane.replacement().map(str::to_string) else {
                view.review.proofing.pane = Some(pane);
                return cx.notify();
            };
            let result = if every {
                view.document
                    .change_every(&issue.text, &replacement)
                    .map(|_| (issue.paragraph, issue.start + replacement.len()))
            } else {
                view.document.change_issue(&issue, &replacement)
            };
            match result {
                Ok(next) => {
                    view.dirty = true;
                    pane.advance(&view.document, next, checker);
                }
                Err(error) => view.table_status = Some(error.into()),
            }
        }
        ProofAction::Ignore { every: false } => pane.ignore_once(&view.document, checker),
        ProofAction::Ignore { every: true } | ProofAction::Learn => {
            if action == ProofAction::Learn {
                checker.learn(&issue.text);
            } else {
                checker.ignore(&issue.text);
            }
            pane.advance(&view.document, issue.end(), checker);
            restart_marks(view);
        }
    }
    view.review.proofing.pane = Some(pane);
    view.rebuild_all_images();
    show_current(view, cx);
}

/// After a word is ignored or learnt: every span asked about again.
fn restart_marks(view: &mut DocumentView) {
    if view.document.proofing() {
        if let Err(error) = view.document.set_proofing(true) {
            view.table_status = Some(error.into());
        }
        proof_now(view);
    }
}

/// A right-click on the pages: on a marked word, its menu.
pub fn right_click(
    view: &mut DocumentView,
    page: usize,
    event: &MouseDownEvent,
    cx: &mut Context<DocumentView>,
) {
    view.review.proofing.menu = None;
    if !view.document.proofing() {
        return;
    }
    let Some(at) = view.position_at_window_point(page, event.position) else {
        return;
    };
    let checker = view.review.proofing.checker.as_mut();
    if let Some(issue) = view.document.issue_at(at, checker) {
        view.review.proofing.menu = Some((event.position, issue));
        cx.notify();
    }
}

/// What the right-click menu does.
fn menu_action(view: &mut DocumentView, action: ProofAction, cx: &mut Context<DocumentView>) {
    let Some((_, issue)) = view.review.proofing.menu.take() else {
        return;
    };
    let checker = view.review.proofing.checker.as_mut();
    match action {
        ProofAction::Choose(index) => {
            if let Some(replacement) = issue.suggestions.get(index).cloned() {
                match view.document.change_issue(&issue, &replacement) {
                    Ok(end) => {
                        view.dirty = true;
                        view.insert_at = end;
                        view.caret = view.document.place_caret(end.0, end.1);
                    }
                    Err(error) => view.table_status = Some(error.into()),
                }
            }
        }
        ProofAction::Ignore { .. } => {
            checker.ignore(&issue.text);
            restart_marks(view);
        }
        ProofAction::Learn => {
            checker.learn(&issue.text);
            restart_marks(view);
        }
        ProofAction::Change { .. } | ProofAction::Close => {}
    }
    view.rebuild_all_images();
    cx.notify();
}

fn menu_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<gpui::SharedString>,
    bold: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .rounded_md()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(theme::hovered()))
        .when(bold, |row| row.font_weight(gpui::FontWeight::SEMIBOLD))
        .child(label.into())
}

/// The right-click menu, at the point it opened.
pub fn menu(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let (position, issue) = view.review.proofing.menu.as_ref()?;
    let mut rows: Vec<AnyElement> = issue
        .suggestions
        .iter()
        .take(6)
        .enumerate()
        .map(|(index, suggestion)| {
            menu_row(("proof-menu-suggestion", index), suggestion.clone(), true)
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    menu_action(view, ProofAction::Choose(index), cx)
                }))
                .into_any_element()
        })
        .collect();
    if rows.is_empty() {
        rows.push(
            div()
                .px_2()
                .text_color(theme::text_faint())
                .child(i18n::text("proof.no_suggestions"))
                .into_any_element(),
        );
    }
    rows.push(
        div()
            .h(px(1.0))
            .my_1()
            .bg(theme::border())
            .into_any_element(),
    );
    rows.push(
        menu_row("proof-menu-ignore", i18n::text("proof.ignore_all"), false)
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                menu_action(view, ProofAction::Ignore { every: true }, cx)
            }))
            .into_any_element(),
    );
    if issue.kind == ProofingKind::Spelling {
        rows.push(
            menu_row("proof-menu-learn", i18n::text("proof.learn"), false)
                .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                    menu_action(view, ProofAction::Learn, cx)
                }))
                .into_any_element(),
        );
    }
    rows.push(
        menu_row("proof-menu-pane", i18n::text("proof.open_pane"), false)
            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                view.review.proofing.menu = None;
                open_pane(view, cx)
            }))
            .into_any_element(),
    );
    Some(
        deferred(
            anchored().position(*position).child(
                div()
                    .id("proof-menu")
                    .occlude()
                    .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                        view.review.proofing.menu = None;
                        cx.notify();
                    }))
                    .p_1()
                    .min_w(px(180.0))
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .bg(theme::panel())
                    .border_1()
                    .border_color(theme::border())
                    .shadow_lg()
                    .font_family(crate::UI_FONT_FAMILY)
                    .text_xs()
                    .text_color(theme::text())
                    .children(rows),
            ),
        )
        .with_priority(4)
        .into_any_element(),
    )
}

/// The 文章校正 pane beside the pages, while it is open.
pub fn pane(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let pane = view.review.proofing.pane.as_ref()?;
    let action = |id: &'static str, key: &'static str, primary: bool, what: ProofAction| {
        button(id, i18n::text(key), primary).on_click(
            cx.listener(move |view, _: &ClickEvent, _, cx| pane_action(view, what.clone(), cx)),
        )
    };
    let body: Vec<AnyElement> = match &pane.current {
        None => vec![div()
            .text_color(theme::text_dim())
            .child(i18n::text("proof.finished"))
            .into_any_element()],
        Some(issue) => {
            let heading = i18n::text(match issue.kind {
                ProofingKind::Spelling => "proof.spelling",
                ProofingKind::Grammar => "proof.grammar",
            });
            let mut body = vec![
                div()
                    .text_color(theme::text_dim())
                    .child(heading)
                    .into_any_element(),
                div()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(theme::canvas())
                    .border_1()
                    .border_color(theme::border())
                    .text_sm()
                    .text_color(theme::text())
                    .child(issue.text.clone())
                    .into_any_element(),
            ];
            if let Some(description) = &issue.description {
                body.push(
                    div()
                        .text_color(theme::text_dim())
                        .child(description.clone())
                        .into_any_element(),
                );
            }
            body.push(
                div()
                    .text_color(theme::text_dim())
                    .child(i18n::text("proof.suggestions"))
                    .into_any_element(),
            );
            body.push(
                div()
                    .flex()
                    .flex_col()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
                    .bg(theme::canvas())
                    .min_h(px(60.0))
                    .children(issue.suggestions.iter().enumerate().map(|(index, word)| {
                        div()
                            .id(("proof-suggestion", index))
                            .px_2()
                            .h(px(22.0))
                            .flex()
                            .items_center()
                            .cursor_pointer()
                            .when(index == pane.chosen, |row| row.bg(theme::selected()))
                            .hover(|row| row.bg(theme::hovered()))
                            .child(word.clone())
                            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                                pane_action(view, ProofAction::Choose(index), cx)
                            }))
                    }))
                    .into_any_element(),
            );
            body.push(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(action(
                        "proof-change",
                        "proof.change",
                        true,
                        ProofAction::Change { every: false },
                    ))
                    .child(action(
                        "proof-change-all",
                        "proof.change_all",
                        false,
                        ProofAction::Change { every: true },
                    ))
                    .child(action(
                        "proof-ignore",
                        "proof.ignore",
                        false,
                        ProofAction::Ignore { every: false },
                    ))
                    .child(action(
                        "proof-ignore-all",
                        "proof.ignore_all",
                        false,
                        ProofAction::Ignore { every: true },
                    ))
                    .when(issue.kind == ProofingKind::Spelling, |row| {
                        row.child(action(
                            "proof-learn",
                            "proof.learn",
                            false,
                            ProofAction::Learn,
                        ))
                    })
                    .into_any_element(),
            );
            body
        }
    };
    Some(
        div()
            .id("proofing-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .h_full()
            .gap_2()
            .p_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(i18n::text("proof.title")))
                    .child(
                        div()
                            .id("proof-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                                pane_action(view, ProofAction::Close, cx)
                            })),
                    ),
            )
            .children(body)
            .into_any_element(),
    )
}

/// The right-click handler for a page, in the shape gpui wants.
pub fn on_right_click(
    page: usize,
    cx: &mut Context<DocumentView>,
) -> impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |event, _window, cx| {
        view.update(cx, |view, cx| right_click(view, page, event, cx))
            .ok();
    }
}

/// Where a right-click's menu opens, for tests.
#[cfg(test)]
pub fn origin() -> Point<Pixels> {
    point(px(0.0), px(0.0))
}

/// Word checks as you type unless told not to: the marks go on when a
/// document is first shown. Not in tests, whose pixels and timings are
/// their own.
pub fn start_once(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.review.proofing.started || cfg!(test) {
        return;
    }
    view.review.proofing.started = true;
    set_marks(view, true, cx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use jade_office_model::word::proofing::WordListChecker;

    /// With a word list for the checker: the marks go on, the pane walks
    /// the issues and 変更 puts the suggestion in; a right-click's menu
    /// offers it too.
    #[gpui::test]
    fn the_pane_and_the_menu_correct_misspellings(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "校正.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            view.review.proofing.checker =
                Box::new(WordListChecker::new(["the", "cat", "sat", "on", "mat"]));
        });
        cx.simulate_input("Teh cat sat on teh mat");
        view.update(cx, |view, cx| {
            open_pane(view, cx);
            assert!(view.document.proofing());
            let pane = view.review.proofing.pane.clone().unwrap();
            let issue = pane.current.clone().expect("Teh");
            assert_eq!(issue.text, "Teh");
            assert_eq!(
                view.document.selection(),
                Some((issue.start(), issue.end()))
            );
            pane_action(view, ProofAction::Change { every: false }, cx);
            assert!(view
                .document
                .paragraph_text(0)
                .unwrap()
                .starts_with("The cat"));
            let pane = view.review.proofing.pane.clone().unwrap();
            assert_eq!(
                pane.current.as_ref().map(|issue| issue.text.as_str()),
                Some("teh")
            );
            // The right-click menu on the second, choosing its suggestion.
            let issue = pane.current.clone().unwrap();
            view.review.proofing.menu = Some((origin(), issue));
            menu_action(view, ProofAction::Choose(0), cx);
            assert_eq!(
                view.document.paragraph_text(0).unwrap(),
                "The cat sat on the mat"
            );
            pane_action(view, ProofAction::Close, cx);
            assert!(view.review.proofing.pane.is_none());
            set_marks(view, false, cx);
            assert!(!view.document.proofing());
        });
    }
}
