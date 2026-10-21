//! Find and Replace over a Word document's pages (⌘F, ⌘⇧H): a query and a
//! replacement typed into the bar over the pages, Word's two options — 大文字
//! と小文字を区別する and 完全に一致する単語だけ — and the match the search is
//! on, selected in the text. Enter goes to the next match (⇧ the one
//! before), the bar's buttons replace it or every match at once.
//!
//! What a search finds and a replace does is `jade-office-model`'s
//! (`word::WordFind`), the code the native app runs; this is the bar.

use jade_web::editing::{FindOptions, Found, WordFind};

use super::sheet::caret_line;
use super::*;

/// The bar's state while it is open.
pub(super) struct FindBar {
    pub query: TextEdit,
    pub replacement: TextEdit,
    /// Typing goes into the replacement rather than the query.
    pub replacing: bool,
    pub options: FindOptions,
    /// The match the search is on, selected in the text.
    pub current: Option<Found>,
    /// What the last search or replace came to.
    pub status: Option<SharedString>,
    /// Whether typing goes into the bar: a click on a page gives it back to
    /// the document, a click on a field or ⌘F to the bar.
    pub active: bool,
}

impl FindBar {
    fn new(query: String) -> Self {
        Self {
            query: TextEdit::selected(query),
            replacement: TextEdit::new(String::new()),
            replacing: false,
            options: FindOptions::default(),
            current: None,
            status: None,
            active: true,
        }
    }

    /// The field typing goes into.
    pub fn field_mut(&mut self) -> &mut TextEdit {
        if self.replacing {
            &mut self.replacement
        } else {
            &mut self.query
        }
    }

    pub fn field(&self) -> &TextEdit {
        if self.replacing {
            &self.replacement
        } else {
            &self.query
        }
    }
}

/// What the bar's buttons do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FindCommand {
    Next,
    Previous,
    Replace,
    ReplaceAll,
    MatchCase,
    WholeWord,
    Close,
}

impl DocView {
    /// The find bar, if the Word document has one open.
    pub(super) fn find_bar(&self) -> Option<&FindBar> {
        match &self.document {
            Document::Pages { find, .. } => find.as_deref(),
            _ => None,
        }
    }

    pub(super) fn find_bar_mut(&mut self) -> Option<&mut FindBar> {
        match &mut self.document {
            Document::Pages { find, .. } => find.as_deref_mut(),
            _ => None,
        }
    }

    /// Whether typing goes into the find bar rather than the document.
    pub(super) fn finding(&self) -> bool {
        self.find_bar().is_some_and(|bar| bar.active)
    }

    /// ⌘F (or ⌘⇧H, on the replacement): opens the bar with the selection
    /// as the query, or focuses the field when it is open.
    pub(super) fn open_find(&mut self, replacing: bool, cx: &mut Context<Self>) {
        let Document::Pages {
            document,
            find,
            dialog,
            ..
        } = &mut self.document
        else {
            return;
        };
        *dialog = None;
        let selected = document
            .selected_text()
            .filter(|text| !text.trim().is_empty() && !text.contains('\n'));
        let bar = find.get_or_insert_with(|| Box::new(FindBar::new(String::new())));
        if let Some(selected) = selected {
            bar.query = TextEdit::selected(selected);
        }
        bar.replacing = replacing;
        bar.active = true;
        self.count_matches();
        cx.notify();
    }

    /// Keys while the bar is open. Returns whether it took the keystroke;
    /// what it leaves — ⌘Z, ⌘S — is the document's.
    pub(super) fn find_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        let command = modifiers.platform || modifiers.control;
        let Some(bar) = self.find_bar_mut() else {
            return false;
        };
        if bar.field().marked.is_some() {
            return false;
        }
        let run = match key {
            "escape" => Some(FindCommand::Close),
            "enter" if command => Some(FindCommand::ReplaceAll),
            "enter" if modifiers.alt => Some(FindCommand::Replace),
            "enter" if modifiers.shift => Some(FindCommand::Previous),
            "enter" => Some(FindCommand::Next),
            "g" if command && modifiers.shift => Some(FindCommand::Previous),
            "g" if command => Some(FindCommand::Next),
            "c" if command && modifiers.alt => Some(FindCommand::MatchCase),
            "w" if command && modifiers.alt => Some(FindCommand::WholeWord),
            _ => None,
        };
        if let Some(run) = run {
            self.find_command(run, cx);
            return true;
        }
        let field = bar.field_mut();
        match key {
            "tab" => {
                bar.replacing = !bar.replacing;
                bar.field_mut().whole = true;
            }
            "backspace" => field.backspace(),
            "delete" => field.delete(),
            "left" => field.step(false),
            "right" => field.step(true),
            "home" => field.to_end(false),
            "end" => field.to_end(true),
            "a" if command => field.whole = true,
            // A one-line field: nowhere to go.
            "up" | "down" => {}
            // ⌘F and ⌘⇧H again, and the rest of the shortcuts, go on.
            _ => return false,
        }
        self.typed_into_find(cx);
        true
    }

    /// What typing into the bar changes: the count, and no match is current
    /// until Enter finds one.
    pub(super) fn typed_into_find(&mut self, cx: &mut Context<Self>) {
        if let Some(bar) = self.find_bar_mut() {
            if !bar.replacing {
                bar.current = None;
            }
        }
        self.count_matches();
        cx.notify();
    }

    /// The bar's status: how many matches the query has.
    fn count_matches(&mut self) {
        let Document::Pages { document, find, .. } = &mut self.document else {
            return;
        };
        let Some(bar) = find.as_deref_mut() else {
            return;
        };
        let query = bar.query.text.clone();
        bar.status = (!query.is_empty()).then(|| {
            let count = document.find_all(&query, bar.options).len();
            if count == 0 {
                i18n::format("doc.find.none", &[("query", &query)])
            } else {
                i18n::format("doc.find.count", &[("query", &query), ("n", &count)])
            }
            .into()
        });
    }

    fn find_command(&mut self, run: FindCommand, cx: &mut Context<Self>) {
        let Document::Pages { document, find, .. } = &mut self.document else {
            return;
        };
        let Some(bar) = find.as_deref_mut() else {
            return;
        };
        match run {
            FindCommand::Close => {
                *find = None;
                return cx.notify();
            }
            FindCommand::MatchCase => {
                bar.options.match_case = !bar.options.match_case;
                bar.current = None;
                self.count_matches();
                return cx.notify();
            }
            FindCommand::WholeWord => {
                bar.options.whole_word = !bar.options.whole_word;
                bar.current = None;
                self.count_matches();
                return cx.notify();
            }
            _ => {}
        }
        let query = bar.query.text.clone();
        let replacement = bar.replacement.text.clone();
        let options = bar.options;
        if query.is_empty() {
            return;
        }
        let caret = document.caret().unwrap_or((0, 0));
        let edited = match run {
            FindCommand::Replace => match bar.current {
                // Replaced, then on to the next match after it.
                Some(found) => document.replace_found(found, &replacement).map(Some),
                None => Ok(None),
            },
            FindCommand::ReplaceAll => {
                let replaced = document.replace_all(&query, &replacement, options);
                if let Ok(replaced) = replaced {
                    bar.current = None;
                    bar.status = Some(
                        i18n::format(
                            "doc.find.replaced",
                            &[
                                ("query", &query),
                                ("replacement", &replacement),
                                ("n", &replaced),
                            ],
                        )
                        .into(),
                    );
                }
                let result = replaced.map(|_| ());
                self.refresh_repainted();
                return self.word_result(result, cx);
            }
            _ => Ok(None),
        };
        let from = match (&edited, run, bar.current) {
            (Ok(Some(after)), ..) => *after,
            (_, FindCommand::Next, Some(current)) => current.end(),
            (_, _, Some(current)) => current.start(),
            _ => caret,
        };
        let found = if run == FindCommand::Previous {
            document.find_previous(&query, options, from)
        } else {
            document.find_next(&query, options, from)
        };
        bar.current = found;
        let all = document.find_all(&query, options);
        bar.status = Some(
            match found {
                Some(found) => i18n::format(
                    "doc.find.position",
                    &[
                        ("query", &query),
                        (
                            "n",
                            &(all.iter().position(|hit| *hit == found).unwrap_or(0) + 1),
                        ),
                        ("count", &all.len()),
                    ],
                ),
                None => i18n::format("doc.find.none", &[("query", &query)]),
            }
            .into(),
        );
        let selected = match found {
            Some(found) => document.select(found.start(), found.end()),
            None => Ok(()),
        };
        self.refresh_repainted();
        self.reveal_caret();
        self.word_result(edited.map(|_| ()).and(selected), cx);
    }

    /// The bar, drawn over the pages while it is open.
    pub(super) fn find_bar_element(&self, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let bar = self.find_bar()?;
        let status = bar.status.clone().unwrap_or_default();
        Some(
            div()
                .flex()
                .flex_none()
                .flex_wrap()
                .items_center()
                .gap_2()
                .px_4()
                .py_1p5()
                .bg(palette::panel())
                .border_b_1()
                .border_color(palette::border())
                .text_xs()
                .text_color(palette::text())
                .child(find_field(
                    "find-query",
                    "doc.find.query",
                    &bar.query,
                    !bar.replacing,
                    cx,
                ))
                .child(find_toggle(
                    "find-case",
                    "Aa",
                    "doc.find.match_case",
                    bar.options.match_case,
                    FindCommand::MatchCase,
                    cx,
                ))
                .child(find_toggle(
                    "find-word",
                    "ab|",
                    "doc.find.whole_word",
                    bar.options.whole_word,
                    FindCommand::WholeWord,
                    cx,
                ))
                .child(find_button(
                    "find-previous",
                    "doc.find.previous",
                    FindCommand::Previous,
                    cx,
                ))
                .child(find_button(
                    "find-next",
                    "doc.find.next",
                    FindCommand::Next,
                    cx,
                ))
                .child(find_field(
                    "find-replacement",
                    "doc.find.replacement",
                    &bar.replacement,
                    bar.replacing,
                    cx,
                ))
                .child(find_button(
                    "find-replace",
                    "doc.find.replace_one",
                    FindCommand::Replace,
                    cx,
                ))
                .child(find_button(
                    "find-replace-all",
                    "doc.find.replace_all",
                    FindCommand::ReplaceAll,
                    cx,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(80.0))
                        .text_color(palette::dim())
                        .child(status),
                )
                .child(find_toggle(
                    "find-close",
                    "×",
                    "doc.find.close",
                    false,
                    FindCommand::Close,
                    cx,
                ))
                .into_any_element(),
        )
    }
}

/// One of the bar's two fields; a click puts typing into it.
fn find_field(
    id: &'static str,
    label: &'static str,
    text: &TextEdit,
    focused: bool,
    cx: &mut Context<DocView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .w(px(200.0))
        .h(px(24.0))
        .px_2()
        .rounded_md()
        .border_1()
        .border_color(if focused {
            palette::accent()
        } else {
            palette::border()
        })
        .bg(palette::canvas())
        .cursor_text()
        .child(
            div()
                .flex_none()
                .text_color(palette::faint())
                .child(i18n::text(label)),
        )
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .child(if focused {
                    caret_line(text, palette::accent().into(), false)
                } else {
                    div().whitespace_nowrap().child(text.text.clone())
                }),
        )
        .on_click(cx.listener(move |view, _event, _window, cx| {
            if let Some(bar) = view.find_bar_mut() {
                bar.replacing = id == "find-replacement";
                bar.active = true;
            }
            cx.notify();
        }))
}

/// An option the bar turns on and off, or its close button.
fn find_toggle(
    id: &'static str,
    glyph: &'static str,
    tip: &'static str,
    on: bool,
    run: FindCommand,
    cx: &mut Context<DocView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .h(px(24.0))
        .min_w(px(28.0))
        .px_1()
        .rounded_md()
        .cursor_pointer()
        .when(on, |button| {
            button.bg(palette::selected()).text_color(palette::accent())
        })
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(crate::tooltip::Tooltip::text(i18n::text(tip)))
        .child(glyph)
        .on_click(cx.listener(move |view, _event, _window, cx| view.find_command(run, cx)))
}

fn find_button(
    id: &'static str,
    label: &'static str,
    run: FindCommand,
    cx: &mut Context<DocView>,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .h(px(24.0))
        .px_2()
        .flex()
        .items_center()
        .rounded_md()
        .border_1()
        .border_color(palette::border())
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()))
        .child(i18n::text(label))
        .on_click(cx.listener(move |view, _event, _window, cx| view.find_command(run, cx)))
}
