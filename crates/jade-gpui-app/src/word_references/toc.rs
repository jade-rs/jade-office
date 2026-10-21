//! 目次: the gallery's 自動作成の目次 and 手動作成目次, the ユーザー設定の
//! 目次 dialog with its オプション, the 図表目次 dialog, and 目次の更新's
//! ページ番号だけを更新する / 目次をすべて更新する.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, SharedString};
use jade_office_model::word::references::captions::WordCaptions;
use jade_office_model::word::references::toc::{TabLeader, TocDialog, TocKind, WordToc};

use super::{
    action_button, check_box, choice, edit, heading, line, select, t, Dialog, RefList, References,
    TocPreset,
};
use crate::{theme, DocumentView};

/// The gallery's entries.
pub fn insert_preset(view: &mut DocumentView, preset: TocPreset, cx: &mut Context<DocumentView>) {
    let done = match preset {
        TocPreset::Manual => view.document.insert_manual_toc(),
        // Word's 自動作成の目次 1: levels 1–3, linked, dot leaders — the
        // engine's default table.
        TocPreset::Automatic1 => return view.insert_table_of_contents(cx),
        TocPreset::Automatic2 => {
            let mut dialog = view.document.toc_dialog(None);
            dialog.replacing = None;
            view.document.apply_toc_dialog(&dialog)
        }
    };
    view.after_reference_edit(done.map(|()| None), cx);
}

/// ユーザー設定の目次… (or 図表目次の挿入… with a label).
pub fn open(view: &mut DocumentView, figures: Option<String>, cx: &mut Context<DocumentView>) {
    let dialog = view.document.toc_dialog(figures.as_deref());
    view.references
        .open(Dialog::Toc(Box::new(dialog), false), Vec::new());
    cx.notify();
}

/// 目次の更新: Word asks what to update when there is a table.
pub fn open_update(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.document.toc_to_update().is_none() {
        view.table_status = Some(SharedString::from(t("toc.none")));
        return cx.notify();
    }
    view.references.open(Dialog::UpdateToc(true), Vec::new());
    cx.notify();
}

pub fn confirm_update(
    view: &mut DocumentView,
    page_numbers_only: bool,
    cx: &mut Context<DocumentView>,
) {
    view.references.close();
    if !page_numbers_only && view.document.toc_to_update() == Some(0) {
        // The first table, made again: the engine's own 目次の更新.
        return view.update_table_of_contents(cx);
    }
    let done = view.document.update_toc(page_numbers_only);
    view.after_reference_edit(done.map(|()| None), cx);
}

/// 図表目次の更新: every table of figures made again.
pub fn update_figures(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let contents = view.document.word_document().tables_of_contents();
    let figures: Vec<usize> = contents
        .iter()
        .enumerate()
        .filter(|(_, block)| {
            jade_office_model::word::references::toc::is_table_of_figures(&block.instruction)
        })
        .map(|(index, _)| index)
        .collect();
    let Some(&first) = figures.first() else {
        view.table_status = Some(SharedString::from(t("toc.figures.none")));
        return cx.notify();
    };
    let start = contents[first].start;
    view.document.place_caret(start, 0);
    let done = view.document.update_toc(false);
    view.after_reference_edit(done.map(|()| None), cx);
}

pub fn confirm(
    view: &mut DocumentView,
    dialog: TocDialog,
    options: bool,
    cx: &mut Context<DocumentView>,
) {
    if options {
        // オプション's OK: back to the dialog with the styles' levels.
        view.references
            .open(Dialog::Toc(Box::new(dialog), false), Vec::new());
        return cx.notify();
    }
    view.references.close();
    let done = view.document.apply_toc_dialog(&dialog);
    if let Err(problem) = &done {
        view.references
            .open(Dialog::Toc(Box::new(dialog), false), Vec::new());
        view.references.problem = Some(problem.clone());
        return cx.notify();
    }
    view.after_reference_edit(done.map(|()| None), cx);
}

fn toc_form(references: &mut References) -> Option<&mut TocDialog> {
    match references.dialog.as_mut() {
        Some(Dialog::Toc(dialog, _)) => Some(dialog),
        _ => None,
    }
}

/// 目次の更新's body.
pub fn update_body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::UpdateToc(page_only)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    let page_only = *page_only;
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(t("toc.update.prompt"))
        .child(
            choice("toc-update-pages", t("toc.update.pages"), page_only).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    edit(view, cx, |references| {
                        references.dialog = Some(Dialog::UpdateToc(true))
                    })
                },
            )),
        )
        .child(
            choice("toc-update-all", t("toc.update.all"), !page_only).on_click(cx.listener(
                |view, _: &ClickEvent, _, cx| {
                    edit(view, cx, |references| {
                        references.dialog = Some(Dialog::UpdateToc(false))
                    })
                },
            )),
        )
        .into_any_element()
}

/// The 目次 / 図表目次 dialog's body, or its オプション.
pub fn body(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let Some(Dialog::Toc(dialog, options)) = view.references.dialog.as_ref() else {
        return div().into_any_element();
    };
    if *options {
        return options_body(dialog, cx);
    }
    let figures = matches!(dialog.kind, TocKind::Figures { .. });
    let preview = dialog
        .preview()
        .into_iter()
        .map(|(level, text, page)| {
            let indent = f32::from(level.saturating_sub(1)) * 12.0;
            let leader = if dialog.right_align {
                match dialog.leader {
                    TabLeader::None => " ",
                    TabLeader::Dots => ".",
                    TabLeader::Dashes => "-",
                    TabLeader::Underline => "_",
                }
            } else {
                ""
            };
            div()
                .flex()
                .pl(px(indent))
                .child(div().flex_none().child(text))
                .when(dialog.show_page_numbers && dialog.right_align, |row| {
                    row.child(
                        div()
                            .flex_1()
                            .overflow_hidden()
                            .whitespace_nowrap()
                            .text_color(theme::text_dim())
                            .child(leader.repeat(60)),
                    )
                    .child(div().flex_none().child(page))
                })
                .when(dialog.show_page_numbers && !dialog.right_align, |row| {
                    row.child(div().pl_1().child(page))
                })
                .into_any_element()
        })
        .collect::<Vec<_>>();
    let leaders: Vec<(SharedString, bool)> = TabLeader::ALL
        .iter()
        .map(|leader| (leader.sample().into(), *leader == dialog.leader))
        .collect();
    let mut body = div()
        .w(px(440.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(heading("toc.preview"))
        .child(
            div()
                .p_2()
                .flex()
                .flex_col()
                .gap_1()
                .rounded_sm()
                .border_1()
                .border_color(theme::border())
                .bg(theme::canvas())
                .children(preview),
        )
        .child(check_box(
            "toc-page-numbers",
            "toc.page_numbers",
            dialog.show_page_numbers,
            cx,
            |references| {
                if let Some(dialog) = toc_form(references) {
                    dialog.show_page_numbers = !dialog.show_page_numbers;
                }
            },
        ))
        .child(check_box(
            "toc-right-align",
            "toc.right_align",
            dialog.right_align,
            cx,
            |references| {
                if let Some(dialog) = toc_form(references) {
                    dialog.right_align = !dialog.right_align;
                }
            },
        ))
        .child(line("toc.leader").child(select(
            view,
            RefList::Leader,
            dialog.leader.sample(),
            120.0,
            leaders,
            cx,
            |references, index| {
                if let Some(dialog) = toc_form(references) {
                    dialog.leader = TabLeader::ALL[index];
                }
            },
        )))
        .child(check_box(
            "toc-hyperlinks",
            "toc.hyperlinks",
            dialog.hyperlinks,
            cx,
            |references| {
                if let Some(dialog) = toc_form(references) {
                    dialog.hyperlinks = !dialog.hyperlinks;
                }
            },
        ));
    if let TocKind::Figures {
        label,
        include_label,
    } = &dialog.kind
    {
        let labels = view
            .document
            .caption_dialog(&view.references.custom_labels)
            .labels;
        let entries: Vec<(SharedString, bool)> = labels
            .iter()
            .map(|entry| (entry.clone().into(), entry == label))
            .collect();
        body = body
            .child(line("toc.figures.label").child(select(
                view,
                RefList::CaptionLabel,
                label.clone(),
                120.0,
                entries,
                cx,
                move |references, index| {
                    if let Some(TocDialog {
                        kind: TocKind::Figures { label, .. },
                        ..
                    }) = toc_form(references)
                    {
                        *label = labels[index].clone();
                    }
                },
            )))
            .child(check_box(
                "toc-include-label",
                "toc.figures.include_label",
                *include_label,
                cx,
                |references| {
                    if let Some(TocDialog {
                        kind: TocKind::Figures { include_label, .. },
                        ..
                    }) = toc_form(references)
                    {
                        *include_label = !*include_label;
                    }
                },
            ));
    }
    if !figures {
        let levels: Vec<(SharedString, bool)> = (1..=9u8)
            .map(|level| (level.to_string().into(), level == dialog.levels))
            .collect();
        body = body
            .child(line("toc.levels").child(select(
                view,
                RefList::Levels,
                dialog.levels.to_string(),
                60.0,
                levels,
                cx,
                |references, index| {
                    if let Some(dialog) = toc_form(references) {
                        dialog.levels = index as u8 + 1;
                    }
                },
            )))
            .child(action_button(
                "toc-options",
                "toc.options",
                cx,
                |view, cx| {
                    if let Some(Dialog::Toc(dialog, _)) = view.references.dialog.take() {
                        view.references.open(Dialog::Toc(dialog, true), Vec::new());
                    }
                    cx.notify();
                },
            ));
    }
    body.into_any_element()
}

/// オプション: アウトライン レベル and each style's 目次レベル.
fn options_body(dialog: &TocDialog, cx: &mut Context<DocumentView>) -> AnyElement {
    let rows = dialog
        .styles
        .iter()
        .enumerate()
        .map(|(index, (_, name, level))| {
            let level = *level;
            div()
                .flex()
                .items_center()
                .gap_2()
                .h(px(22.0))
                .child(
                    div()
                        .w(px(220.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .child(name.clone()),
                )
                .child(div().flex().gap_0p5().children((0..=9u8).map(|value| {
                    let label = if value == 0 {
                        "–".to_string()
                    } else {
                        value.to_string()
                    };
                    let on = level.unwrap_or(0) == value;
                    div()
                        .id(("toc-style-level", index * 10 + value as usize))
                        .w(px(18.0))
                        .flex()
                        .justify_center()
                        .rounded_sm()
                        .border_1()
                        .border_color(if on { theme::accent() } else { theme::border() })
                        .when(on, |cell| cell.bg(theme::selected()))
                        .cursor_pointer()
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            edit(view, cx, |references| {
                                if let Some(dialog) = toc_form(references) {
                                    dialog.styles[index].2 = (value > 0).then_some(value);
                                }
                            })
                        }))
                        .child(label)
                })))
                .into_any_element()
        })
        .collect::<Vec<_>>();
    div()
        .w(px(460.0))
        .flex()
        .flex_col()
        .gap_2()
        .child(check_box(
            "toc-outline-levels",
            "toc.outline_levels",
            dialog.outline_levels,
            cx,
            |references| {
                if let Some(dialog) = toc_form(references) {
                    dialog.outline_levels = !dialog.outline_levels;
                }
            },
        ))
        .child(heading("toc.styles"))
        .child(
            div()
                .id("toc-styles")
                .h(px(260.0))
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .children(rows),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::super::tests::{fixture, open_path};
    use super::super::{Dialog, RefAction, TocPreset};
    use jade_office_model::word::references::toc::{TabLeader, WordToc};

    fn contents(view: &crate::DocumentView) -> String {
        (0..view.document.paragraph_count())
            .filter_map(|index| view.document.paragraph_text(index))
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// ユーザー設定の目次: two levels, dashes, no right alignment — and 目次
    /// の更新's ページ番号だけ keeps an edited entry while すべて remakes it.
    #[gpui::test]
    fn the_toc_dialog_builds_and_updates_a_table(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("toc.docx"));
        view.update(cx, |view, cx| {
            let before = view.document.word_document().tables_of_contents().len();
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            super::super::run(view, RefAction::OpenToc, cx);
            let Some(Dialog::Toc(dialog, false)) = view.references.dialog.as_mut() else {
                panic!("the toc dialog");
            };
            assert_eq!(dialog.replacing, None);
            dialog.levels = 2;
            dialog.leader = TabLeader::Dashes;
            super::super::confirm(view, cx);
            assert!(
                view.references.dialog.is_none(),
                "{:?}",
                view.references.problem
            );
            let tables = view.document.word_document().tables_of_contents();
            assert_eq!(tables.len(), before + 1);
            assert!(
                tables[0].instruction.contains("\\o \"1-2\""),
                "{}",
                tables[0].instruction
            );
            assert_eq!(
                view.document.word_document().toc_leader(&tables[0]),
                "hyphen"
            );

            // The caret in the table opens the dialog on it.
            view.insert_at = (tables[0].start, 0);
            view.caret = view.document.place_caret(tables[0].start, 0);
            super::super::run(view, RefAction::OpenToc, cx);
            let Some(Dialog::Toc(dialog, false)) = view.references.dialog.as_ref() else {
                panic!("the toc dialog");
            };
            assert_eq!(dialog.replacing, Some(0));
            assert_eq!(dialog.levels, 2);
            assert_eq!(dialog.leader, TabLeader::Dashes);
            super::super::cancel(view);

            // Update: page numbers only, then all.
            super::super::run(view, RefAction::OpenUpdateToc, cx);
            assert!(matches!(
                view.references.dialog,
                Some(Dialog::UpdateToc(true))
            ));
            super::super::confirm(view, cx);
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
            super::super::run(view, RefAction::OpenUpdateToc, cx);
            view.references.dialog = Some(Dialog::UpdateToc(false));
            super::super::confirm(view, cx);
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
            assert_eq!(
                view.document.word_document().tables_of_contents().len(),
                before + 1
            );
        });
    }

    /// 手動作成目次 is text to type over, not a field.
    #[gpui::test]
    fn a_manual_table_of_contents_goes_in(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_path(cx, &fixture("hello.docx"));
        view.update(cx, |view, cx| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
            super::super::run(view, RefAction::Toc(TocPreset::Manual), cx);
            assert!(view.table_status.is_none(), "{:?}", view.table_status);
            let text = contents(view);
            assert!(text.contains("目次"));
            assert!(text.contains("(レベル 2)"));
            assert_eq!(view.document.toc_to_update(), None);
        });
    }
}
