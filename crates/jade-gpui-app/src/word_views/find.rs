//! 高度な検索と置換: Word's Find and Replace dialog with its オプション —
//! wildcards, special characters, formatting and styles, direction, scope
//! and the Japanese matching options — over the shared search
//! (`jade_office_model::word::search`).

use gpui::{div, prelude::*, px, AnyElement, Context, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::search::{
    Direction, FormatFilter, Scope, SearchHit, SearchOptions, WordSearch,
};

use super::{dialog_frame_with, ViewDialog};
use crate::word_dialog::{button, check, chip, on};
use crate::{i18n, theme, DocumentView};

/// The dialog being used.
pub struct FindDialog {
    /// 置換 rather than 検索.
    pub replace: bool,
    /// 検索する文字列 and 置換後の文字列.
    pub fields: [TextEdit; 2],
    pub focus: usize,
    pub options: SearchOptions,
    /// オプション ▼ open.
    pub more: bool,
    /// The 特殊文字 or 書式 list open.
    pub list: Option<FindList>,
    /// The match shown, which 置換 replaces.
    pub current: Option<SearchHit>,
    pub status: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FindList {
    Special,
    Format,
}

/// The `^` codes the 特殊文字 list puts in, with their names' keys.
const SPECIALS: [(&str, &str); 16] = [
    ("^p", "find.special.paragraph"),
    ("^t", "find.special.tab"),
    ("^?", "find.special.any_char"),
    ("^#", "find.special.any_digit"),
    ("^$", "find.special.any_letter"),
    ("^^", "find.special.caret"),
    ("^n", "find.special.column_break"),
    ("^+", "find.special.em_dash"),
    ("^=", "find.special.en_dash"),
    ("^l", "find.special.line_break"),
    ("^m", "find.special.page_break"),
    ("^~", "find.special.nonbreaking_hyphen"),
    ("^s", "find.special.nonbreaking_space"),
    ("^-", "find.special.optional_hyphen"),
    ("^w", "find.special.white_space"),
    ("^&", "find.special.found_text"),
];

impl FindDialog {
    pub fn open(query: String, replace: bool) -> Self {
        let mut fields = [TextEdit::new(query), TextEdit::new(String::new())];
        fields[0].whole = !fields[0].text.is_empty();
        let focus = if replace && !fields_empty(&fields) {
            1
        } else {
            0
        };
        FindDialog {
            replace,
            fields,
            focus,
            options: SearchOptions::default(),
            more: false,
            list: None,
            current: None,
            status: None,
        }
    }

    pub fn query(&self) -> &str {
        &self.fields[0].text
    }

    /// The formatting the 書式 buttons change: the replacement's while its
    /// field has the keyboard, as in Word.
    fn format_mut(&mut self) -> &mut FormatFilter {
        if self.replace && self.focus == 1 {
            &mut self.options.replace_format
        } else {
            &mut self.options.format
        }
    }

    fn format(&self) -> &FormatFilter {
        if self.replace && self.focus == 1 {
            &self.options.replace_format
        } else {
            &self.options.format
        }
    }
}

fn fields_empty(fields: &[TextEdit; 2]) -> bool {
    fields[0].text.is_empty()
}

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

fn edit(view: &mut DocumentView, cx: &mut Context<DocumentView>, f: impl FnOnce(&mut FindDialog)) {
    if let Some(ViewDialog::Find(dialog)) = view.views.dialog.as_mut() {
        f(dialog);
        cx.notify();
    }
}

fn dialog(view: &DocumentView) -> Option<&FindDialog> {
    match view.views.dialog.as_ref()? {
        ViewDialog::Find(dialog) => Some(dialog),
        _ => None,
    }
}

/// Opens the dialog, seeded with the selection or the find bar's query.
pub fn open(view: &mut DocumentView, replace: bool) {
    let query = view
        .document
        .selected_text()
        .filter(|text| !text.trim().is_empty() && !text.contains('\n'))
        .or_else(|| view.find_query.clone())
        .unwrap_or_default();
    view.find_bar = None;
    view.views.dialog = Some(ViewDialog::Find(Box::new(FindDialog::open(query, replace))));
}

/// 次を検索.
pub fn find_next(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = dialog(view) else {
        return;
    };
    let (query, options, current) = (
        dialog.query().to_string(),
        dialog.options.clone(),
        dialog.current.clone(),
    );
    let from = match options.direction {
        Direction::Up => view
            .document
            .selection()
            .map(|(anchor, focus)| anchor.min(focus))
            .unwrap_or(view.insert_at),
        _ => view.insert_at,
    };
    let found = view
        .document
        .search_next(&query, &options, from, current.as_ref());
    match found {
        Ok(Some(hit)) => {
            view.find_query = (!query.is_empty()).then(|| query.clone());
            view.reveal(hit.start, hit.end, cx);
            edit(view, cx, |dialog| {
                dialog.current = Some(hit);
                dialog.status = None;
            });
        }
        Ok(None) => edit(view, cx, |dialog| {
            dialog.current = None;
            dialog.status = Some(
                match options.direction {
                    Direction::Down => t("find.end_reached"),
                    Direction::Up => t("find.start_reached"),
                    Direction::All => t("find.not_found"),
                }
                .to_string(),
            );
        }),
        Err(error) => edit(view, cx, |dialog| {
            dialog.status = Some(i18n::format("find.bad_pattern", &[("why", &error.0)]))
        }),
    }
}

/// 置換: the match shown replaced, then the next found.
pub fn replace_one(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = dialog(view) else {
        return;
    };
    let Some(hit) = dialog.current.clone() else {
        return find_next(view, cx);
    };
    let (template, options) = (dialog.fields[1].text.clone(), dialog.options.clone());
    match view.document.replace_hit(&hit, &template, &options) {
        Ok(end) => {
            view.insert_at = end;
            view.caret = view.document.place_caret(end.0, end.1);
            view.rebuild_after_edit();
            edit(view, cx, |dialog| dialog.current = None);
            find_next(view, cx);
        }
        Err(error) => edit(view, cx, |dialog| dialog.status = Some(error)),
    }
}

/// すべて置換, as one undo step.
pub fn replace_all(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(dialog) = dialog(view) else {
        return;
    };
    let (query, template, options) = (
        dialog.query().to_string(),
        dialog.fields[1].text.clone(),
        dialog.options.clone(),
    );
    match view.document.replace_every(&query, &template, &options) {
        Ok(count) => {
            if count > 0 {
                view.caret = view
                    .document
                    .place_caret(view.insert_at.0, view.insert_at.1);
                view.rebuild_after_edit();
            }
            edit(view, cx, |dialog| {
                dialog.current = None;
                dialog.status = Some(i18n::format("find.replaced_count", &[("n", &count)]));
            });
        }
        Err(error) => edit(view, cx, |dialog| dialog.status = Some(error)),
    }
}

/// A key while the dialog is open; `true` when taken.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    let Some(ViewDialog::Find(dialog)) = view.views.dialog.as_mut() else {
        return false;
    };
    match key {
        "escape" => {
            if dialog.list.take().is_none() {
                view.views.dialog = None;
            }
        }
        "enter" | "return" => {
            find_next(view, cx);
            return true;
        }
        "tab" if dialog.replace => dialog.focus = 1 - dialog.focus.min(1),
        "tab" => {}
        key => {
            let focus = dialog.focus.min(1);
            if !dialog.fields[focus].edit_key(key) {
                return false;
            }
            dialog.current = None;
        }
    }
    cx.notify();
    true
}

fn field(dialog: &FindDialog, index: usize, cx: &Context<DocumentView>) -> AnyElement {
    let text = &dialog.fields[index];
    let focused = dialog.focus == index;
    let (before, after) = text.split();
    let format = if index == 0 {
        &dialog.options.format
    } else {
        &dialog.options.replace_format
    };
    div()
        .flex()
        .flex_col()
        .gap_0p5()
        .child(
            div()
                .id(("find-field", index))
                .flex()
                .items_center()
                .w(px(360.0))
                .h(px(24.0))
                .px_2()
                .rounded_sm()
                .border_1()
                .border_color(if focused {
                    theme::accent()
                } else {
                    theme::border()
                })
                .bg(theme::canvas())
                .overflow_hidden()
                .whitespace_nowrap()
                .on_click(on(cx, move |view, cx| {
                    edit(view, cx, |dialog| dialog.focus = index)
                }))
                .child(
                    div()
                        .when(text.whole && focused, |text| text.bg(theme::selected()))
                        .child(before.to_string()),
                )
                .when(focused && !text.whole, |field| {
                    field.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
                })
                .child(after.to_string()),
        )
        .children((!format.is_empty()).then(|| {
            div().text_color(theme::text_faint()).child(format!(
                "{}: {}",
                t("find.format"),
                describe(format)
            ))
        }))
        .into_any_element()
}

/// The 書式 under a field, as Word writes it there.
fn describe(format: &FormatFilter) -> String {
    let mut parts = Vec::new();
    let flag = |value: Option<bool>, on: &'static str, off: &'static str| {
        value.map(|value| t(if value { on } else { off }).to_string())
    };
    parts.extend(flag(
        format.bold,
        "find.format.bold",
        "find.format.not_bold",
    ));
    parts.extend(flag(
        format.italic,
        "find.format.italic",
        "find.format.not_italic",
    ));
    parts.extend(flag(
        format.underline,
        "find.format.underline",
        "find.format.no_underline",
    ));
    if let Some(highlight) = &format.highlight {
        parts.push(if highlight == "none" {
            t("find.format.no_highlight").to_string()
        } else {
            t("find.format.highlight").to_string()
        });
    }
    if let Some(style) = &format.style {
        parts.push(format!("{} {style}", t("find.format.style")));
    }
    parts.join(", ")
}

/// Cycles a condition: unset → on → off → unset.
fn cycle(value: Option<bool>) -> Option<bool> {
    match value {
        None => Some(true),
        Some(true) => Some(false),
        Some(false) => None,
    }
}

fn format_list(view: &DocumentView, dialog: &FindDialog, cx: &Context<DocumentView>) -> AnyElement {
    let format = dialog.format().clone();
    let tri = |id: &'static str, label: &'static str, value: Option<bool>| {
        chip(
            id,
            match value {
                None => label.to_string(),
                Some(true) => format!("✓ {label}"),
                Some(false) => format!("✗ {label}"),
            },
            value.is_some(),
        )
    };
    let styles = jade_office_model::word::styles::quick_style_gallery(
        &jade_office_model::word::styles::WordStyles::style_catalog(view.document.main()),
    );
    div()
        .flex()
        .flex_col()
        .gap_1()
        .p_2()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    tri("find-bold", t("find.format.bold"), format.bold).on_click(on(
                        cx,
                        |view, cx| {
                            edit(view, cx, |dialog| {
                                let format = dialog.format_mut();
                                format.bold = cycle(format.bold);
                            })
                        },
                    )),
                )
                .child(
                    tri("find-italic", t("find.format.italic"), format.italic).on_click(on(
                        cx,
                        |view, cx| {
                            edit(view, cx, |dialog| {
                                let format = dialog.format_mut();
                                format.italic = cycle(format.italic);
                            })
                        },
                    )),
                )
                .child(
                    tri(
                        "find-underline",
                        t("find.format.underline"),
                        format.underline,
                    )
                    .on_click(on(cx, |view, cx| {
                        edit(view, cx, |dialog| {
                            let format = dialog.format_mut();
                            format.underline = cycle(format.underline);
                        })
                    })),
                )
                .child(
                    chip(
                        "find-highlight",
                        t("find.format.highlight"),
                        format.highlight.is_some(),
                    )
                    .on_click(on(cx, |view, cx| {
                        edit(view, cx, |dialog| {
                            let replacing = dialog.replace && dialog.focus == 1;
                            let format = dialog.format_mut();
                            format.highlight = match format.highlight.as_deref() {
                                None if replacing => Some("yellow".to_string()),
                                None => Some("any".to_string()),
                                Some("any") | Some("yellow") => Some("none".to_string()),
                                _ => None,
                            };
                        })
                    })),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(t("find.format.style")),
                )
                .children(styles.into_iter().enumerate().filter_map(|(index, entry)| {
                    let id = entry.id.clone()?;
                    let label: SharedString =
                        entry.name.clone().unwrap_or_else(|| id.clone()).into();
                    let chosen = format.style.as_deref() == Some(id.as_str());
                    Some(
                        chip(("find-style", index), label, chosen)
                            .on_click(on(cx, move |view, cx| {
                                let id = id.clone();
                                edit(view, cx, move |dialog| {
                                    let format = dialog.format_mut();
                                    format.style = (format.style.as_deref() != Some(id.as_str()))
                                        .then_some(id);
                                })
                            }))
                            .into_any_element(),
                    )
                })),
        )
        .into_any_element()
}

fn special_list(dialog: &FindDialog, cx: &Context<DocumentView>) -> AnyElement {
    let replacing = dialog.replace && dialog.focus == 1;
    div()
        .flex()
        .flex_wrap()
        .gap_1()
        .p_2()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .children(
            SPECIALS
                .iter()
                .enumerate()
                // ^& is the replacement's; ^? ^# ^$ ^w only find.
                .filter(|(_, (code, _))| {
                    replacing != matches!(*code, "^?" | "^#" | "^$" | "^w")
                        && (replacing || *code != "^&")
                })
                .map(|(index, (code, key))| {
                    let code: &'static str = code;
                    chip(("find-special", index), format!("{} {code}", t(key)), false)
                        .on_click(on(cx, move |view, cx| {
                            edit(view, cx, |dialog| {
                                let focus = dialog.focus.min(1);
                                dialog.fields[focus].insert(code);
                                dialog.list = None;
                                dialog.current = None;
                            })
                        }))
                        .into_any_element()
                }),
        )
        .into_any_element()
}

/// The dialog's body.
pub fn overlay(
    view: &DocumentView,
    dialog: &FindDialog,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let cx: &Context<DocumentView> = cx;
    let tabs = div()
        .flex()
        .gap_1()
        .child(
            chip("find-tab-find", t("find.tab.find"), !dialog.replace).on_click(on(
                cx,
                |view, cx| {
                    edit(view, cx, |dialog| {
                        dialog.replace = false;
                        dialog.focus = 0;
                    })
                },
            )),
        )
        .child(
            chip("find-tab-replace", t("find.tab.replace"), dialog.replace)
                .on_click(on(cx, |view, cx| {
                    edit(view, cx, |dialog| dialog.replace = true)
                })),
        )
        .child(
            chip("find-tab-goto", t("goto.title"), false).on_click(on(cx, |view, cx| {
                view.views.dialog = None;
                crate::word_references::run(view, crate::word_references::RefAction::OpenGoTo, cx);
            })),
        );
    let options = &dialog.options;
    let option =
        |id: &'static str, key: &'static str, on_now: bool, flip: fn(&mut SearchOptions)| {
            check(id, t(key), on_now).on_click(on(cx, move |view, cx| {
                edit(view, cx, |dialog| {
                    flip(&mut dialog.options);
                    dialog.current = None;
                })
            }))
        };
    let mut body = div()
        .flex()
        .flex_col()
        .gap_2()
        .child(tabs)
        .child(div().text_color(theme::text_dim()).child(t("find.query")))
        .child(field(dialog, 0, cx));
    if dialog.replace {
        body = body
            .child(
                div()
                    .text_color(theme::text_dim())
                    .child(t("find.replacement")),
            )
            .child(field(dialog, 1, cx));
    }
    let buttons = div()
        .flex()
        .flex_wrap()
        .gap_1()
        .child(
            button(
                "find-more",
                if dialog.more {
                    t("find.less")
                } else {
                    t("find.more")
                },
                false,
            )
            .on_click(on(cx, |view, cx| {
                edit(view, cx, |dialog| dialog.more = !dialog.more)
            })),
        )
        .child(button("find-next", t("find.next"), true).on_click(on(cx, find_next)))
        .when(dialog.replace, |row| {
            row.child(
                button("find-replace", t("find.replace"), false).on_click(on(cx, replace_one)),
            )
            .child(
                button("find-replace-all", t("find.replace_all"), false)
                    .on_click(on(cx, replace_all)),
            )
        });
    body = body.child(buttons);
    if dialog.more {
        let direction = |id: &'static str, key: &'static str, value: Direction| {
            chip(id, t(key), options.direction == value).on_click(on(cx, move |view, cx| {
                edit(view, cx, |dialog| dialog.options.direction = value)
            }))
        };
        let scope = |id: &'static str, key: &'static str, value: Scope| {
            chip(id, t(key), options.scope == value).on_click(on(cx, move |view, cx| {
                edit(view, cx, |dialog| {
                    dialog.options.scope = value;
                    dialog.current = None;
                })
            }))
        };
        body = body
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .text_color(theme::text_dim())
                            .child(t("find.direction")),
                    )
                    .child(direction("find-down", "find.down", Direction::Down))
                    .child(direction("find-up", "find.up", Direction::Up))
                    .child(direction("find-all", "find.all", Direction::All)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(div().text_color(theme::text_dim()).child(t("find.scope")))
                    .child(scope("find-main", "find.scope.main", Scope::Main))
                    .child(scope("find-notes", "find.scope.notes", Scope::Notes))
                    .child(scope(
                        "find-selection",
                        "find.scope.selection",
                        Scope::Selection,
                    ))
                    .child(
                        chip(
                            "find-headers",
                            t("find.scope.headers"),
                            view.editing_story(),
                        )
                        .on_click(on(cx, |view, cx| {
                            // A header or footer is searched open, as its
                            // own text.
                            if !view.editing_story() {
                                view.run_ribbon(
                                    crate::ribbon::Action::EditStory(
                                        jade_office_model::word::page::Story::Header,
                                    ),
                                    cx,
                                );
                            }
                            edit(view, cx, |dialog| dialog.current = None);
                        })),
                    ),
            )
            .child(
                div()
                    .grid()
                    .grid_cols(2)
                    .gap_1()
                    .child(option(
                        "find-case",
                        "doc.find.match_case",
                        options.match_case,
                        |o| o.match_case = !o.match_case,
                    ))
                    .child(option(
                        "find-prefix",
                        "find.match_prefix",
                        options.match_prefix,
                        |o| o.match_prefix = !o.match_prefix,
                    ))
                    .child(option(
                        "find-whole",
                        "doc.find.whole_word",
                        options.whole_word,
                        |o| o.whole_word = !o.whole_word,
                    ))
                    .child(option(
                        "find-suffix",
                        "find.match_suffix",
                        options.match_suffix,
                        |o| o.match_suffix = !o.match_suffix,
                    ))
                    .child(option(
                        "find-wild",
                        "find.wildcards",
                        options.wildcards,
                        |o| o.wildcards = !o.wildcards,
                    ))
                    .child(option(
                        "find-width",
                        "find.match_width",
                        options.match_width,
                        |o| o.match_width = !o.match_width,
                    ))
                    .child(option(
                        "find-kana",
                        "find.match_kana",
                        options.match_kana,
                        |o| o.match_kana = !o.match_kana,
                    ))
                    .child(option(
                        "find-punctuation",
                        "find.ignore_punctuation",
                        options.ignore_punctuation,
                        |o| o.ignore_punctuation = !o.ignore_punctuation,
                    ))
                    .child(option(
                        "find-space",
                        "find.ignore_whitespace",
                        options.ignore_whitespace,
                        |o| o.ignore_whitespace = !o.ignore_whitespace,
                    )),
            )
            .child(
                div()
                    .flex()
                    .gap_1()
                    .child(
                        button(
                            "find-format",
                            t("find.format"),
                            dialog.list == Some(FindList::Format),
                        )
                        .on_click(on(cx, |view, cx| {
                            edit(view, cx, |dialog| {
                                dialog.list = (dialog.list != Some(FindList::Format))
                                    .then_some(FindList::Format)
                            })
                        })),
                    )
                    .child(
                        button(
                            "find-special",
                            t("find.special"),
                            dialog.list == Some(FindList::Special),
                        )
                        .on_click(on(cx, |view, cx| {
                            edit(view, cx, |dialog| {
                                dialog.list = (dialog.list != Some(FindList::Special))
                                    .then_some(FindList::Special)
                            })
                        })),
                    )
                    .child(
                        button("find-no-format", t("find.no_format"), false).on_click(on(
                            cx,
                            |view, cx| {
                                edit(view, cx, |dialog| {
                                    *dialog.format_mut() = FormatFilter::default()
                                })
                            },
                        )),
                    ),
            );
        body = match dialog.list {
            Some(FindList::Format) => body.child(format_list(view, dialog, cx)),
            Some(FindList::Special) => body.child(special_list(dialog, cx)),
            None => body,
        };
    }
    if let Some(status) = &dialog.status {
        body = body.child(div().text_color(theme::text_faint()).child(status.clone()));
    }
    dialog_frame_with(
        "find-dialog",
        if dialog.replace {
            t("find.title_replace")
        } else {
            t("find.title")
        },
        480.0,
        body.into_any_element(),
        cx,
        |view, cx| {
            view.views.dialog = None;
            cx.notify();
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wildcards with groups through the dialog: typed, all replaced in one
    /// step, the count reported.
    #[gpui::test]
    fn the_dialog_replaces_with_wildcards(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
            let _ = cx;
        });
        cx.simulate_input("Smith, John; ");
        view.update(cx, |view, cx| {
            open(view, true);
            if let Some(ViewDialog::Find(dialog)) = view.views.dialog.as_mut() {
                dialog.fields[0] = TextEdit::new("(Smith), (John)".to_string());
                dialog.fields[1] = TextEdit::new("\\2 \\1".to_string());
                dialog.options.wildcards = true;
            }
            replace_all(view, cx);
            let paragraph = view.insert_at.0;
            assert!(
                view.document
                    .paragraph_text(paragraph)
                    .unwrap()
                    .starts_with("John Smith; "),
                "{:?}",
                view.document.paragraph_text(paragraph)
            );
            let status = dialog(view)
                .and_then(|dialog| dialog.status.clone())
                .unwrap();
            assert!(status.contains('1'), "{status}");
        });
    }

    /// 次を検索 goes down, then up when told, selecting each match.
    #[gpui::test]
    fn find_next_follows_the_direction(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        cx.simulate_input("zq1 zq2 zq3 ");
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            open(view, false);
            if let Some(ViewDialog::Find(dialog)) = view.views.dialog.as_mut() {
                dialog.fields[0] = TextEdit::new("zq^#".to_string());
            }
            find_next(view, cx);
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 0), (paragraph, 3)))
            );
            find_next(view, cx);
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 4), (paragraph, 7)))
            );
            edit(view, cx, |dialog| dialog.options.direction = Direction::Up);
            find_next(view, cx);
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 0), (paragraph, 3)))
            );
        });
    }
}
