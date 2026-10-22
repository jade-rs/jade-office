//! アウトライン: the document as its outline — each paragraph set in by its
//! level, a ⊕ before a heading with something under it (⊖ with nothing),
//! a ○ before body text — and the アウトライン tab: the level to give, レベル
//! 上げ and レベル下げ (and straight to 見出し 1 or to 本文), 上へ移動 and
//! 下へ移動, 展開 and 折りたたみ, and レベルの表示.
//!
//! The engine lays the paragraphs out in a page box of the view's own,
//! each set in by its level (`set_outline_indents`); collapsed headings and
//! the levels not shown are hidden as the navigation pane's collapse hides
//! them.

use std::collections::HashMap;

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, SharedString};
use jade_docx_editor::canvas_renderer::ViewGeometry;
use jade_docx_editor::word_document::WordDocument;
use jade_office_model::word::references::navigation::WordNavigation;
use jade_office_model::word::selection::caret_rect;
use jade_office_model::word::view::{points_to_px, ViewKind};

use crate::ribbon::{group, row};
use crate::tooltip::Tooltip;
use crate::word_dialog::{chip, on};
use crate::{i18n, theme, DocumentView};

/// How far each outline level is set in, in CSS pixels.
pub const LEVEL_STEP: f64 = 24.0;
/// The room left of the text for the symbols.
pub const SYMBOL_ROOM: f64 = 40.0;
/// What 本文 counts as when levels are compared.
pub const BODY_LEVEL: u8 = 10;

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// A body paragraph's outline level: 1–9 for a heading, 10 for 本文.
fn level_of(document: &WordDocument, paragraph: usize) -> Option<u8> {
    if !document.is_body_paragraph_surface(paragraph) {
        return None;
    }
    let paragraph = document.paragraph(paragraph)?;
    Some(WordDocument::paragraph_heading_level(paragraph).unwrap_or(BODY_LEVEL))
}

/// Every body paragraph with its level, in order.
pub fn levels(document: &WordDocument) -> Vec<(usize, u8)> {
    (0..document.paragraph_count())
        .filter_map(|index| Some((index, level_of(document, index)?)))
        .collect()
}

/// How far each paragraph is set in: a heading by its level, body text
/// one step past the heading it is under.
pub fn indents(document: &WordDocument) -> HashMap<usize, f64> {
    let mut parent = 0u8;
    levels(document)
        .into_iter()
        .map(|(paragraph, level)| {
            let steps = if level == BODY_LEVEL {
                parent
            } else {
                parent = level;
                level - 1
            };
            (paragraph, SYMBOL_ROOM + f64::from(steps) * LEVEL_STEP)
        })
        .collect()
}

/// The page box: the first section's width, the text from just past the
/// symbols, in strips with nothing round them.
pub fn geometry(view: &DocumentView) -> Option<ViewGeometry> {
    let layout = view.document.main().word_document().section_layout(Some(0));
    if layout.width <= 0.0 {
        return None;
    }
    Some(ViewGeometry {
        page_width: f64::from(points_to_px(layout.width)),
        page_height: super::layout::WEB_STRIP_HEIGHT,
        margin_top: 0.0,
        margin_right: f64::from(points_to_px(layout.margin_right)),
        margin_bottom: 0.0,
        margin_left: 8.0,
        hide_page_chrome: true,
    })
}

/// Keeps the engine's indents in step with the outline (after an edit, a
/// level changed) while アウトライン shows, and takes them off after.
pub fn follow(view: &mut DocumentView) {
    let wanted = (view.views.kind == ViewKind::Outline)
        .then(|| indents(view.document.main().word_document()));
    match view.document.set_outline_indents(wanted) {
        Ok(true) => {
            view.caret = view
                .document
                .place_caret(view.insert_at.0, view.insert_at.1);
            view.rebuild_all_images();
        }
        Ok(false) => {}
        Err(error) => view.table_status = Some(error.into()),
    }
}

/// The body elements レベルの表示 hides: every paragraph deeper than the
/// level shown. Merged with the collapsed headings' (`word_references`).
pub fn extra_hidden(view: &DocumentView) -> Vec<usize> {
    let shown = view.views.outline_level;
    if view.views.kind != ViewKind::Outline || shown == 0 || shown >= BODY_LEVEL {
        return Vec::new();
    }
    let document = view.document.main().word_document();
    levels(document)
        .into_iter()
        .filter(|(_, level)| *level > shown)
        .filter_map(|(paragraph, _)| document.body_element_of(paragraph))
        .collect()
}

/// The paragraphs the commands act on: the selection's, or the caret's.
fn chosen(view: &DocumentView) -> (usize, usize) {
    match view.document.selection() {
        Some((anchor, focus)) => (anchor.0.min(focus.0), anchor.0.max(focus.0)),
        None => (view.insert_at.0, view.insert_at.0),
    }
}

/// What the アウトライン tab does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineCommand {
    /// レベル: 見出し 1–9, or 本文 (10).
    Level(u8),
    Promote,
    Demote,
    /// 見出し 1 にレベル上げ.
    ToTop,
    /// 本文にレベル下げ.
    ToBody,
    MoveUp,
    MoveDown,
    Expand,
    Collapse,
    /// レベルの表示: 1–9, or すべて (10).
    ShowLevel(u8),
    Close,
}

/// Runs an アウトライン command.
pub fn run(view: &mut DocumentView, command: OutlineCommand, cx: &mut Context<DocumentView>) {
    use crate::word_references::refresh_collapsed;
    let (first, last) = chosen(view);
    let level = level_of(view.document.main().word_document(), first).unwrap_or(BODY_LEVEL);
    let result: Result<Option<(usize, usize)>, String> = match command {
        OutlineCommand::Level(wanted) => set_levels(view, first, last, wanted).map(|_| None),
        OutlineCommand::Promote if level == BODY_LEVEL => {
            // 本文 becomes a heading at the level of the one it is under.
            let parent = levels(view.document.main().word_document())
                .into_iter()
                .take_while(|(paragraph, _)| *paragraph < first)
                .filter(|(_, level)| *level < BODY_LEVEL)
                .last()
                .map_or(1, |(_, level)| level);
            set_levels(view, first, last, parent).map(|_| None)
        }
        OutlineCommand::Promote => view.document.shift_heading_level(first, true).map(|_| None),
        OutlineCommand::Demote if level == BODY_LEVEL => Ok(None),
        OutlineCommand::Demote if level == 9 => {
            set_levels(view, first, last, BODY_LEVEL).map(|_| None)
        }
        OutlineCommand::Demote => view
            .document
            .shift_heading_level(first, false)
            .map(|_| None),
        OutlineCommand::ToTop => set_levels(view, first, last, 1).map(|_| None),
        OutlineCommand::ToBody => set_levels(view, first, last, BODY_LEVEL).map(|_| None),
        OutlineCommand::MoveUp | OutlineCommand::MoveDown => {
            let up = command == OutlineCommand::MoveUp;
            if level < BODY_LEVEL && view.references.collapsed.contains(&first) {
                // A collapsed heading moves with what it holds.
                view.document.move_heading_section(first, up).map(Some)
            } else {
                move_paragraphs(view, first, last, up).map(Some)
            }
        }
        OutlineCommand::Expand => {
            view.references.collapsed.remove(&first);
            refresh_collapsed(view);
            Ok(None)
        }
        OutlineCommand::Collapse => {
            if level < BODY_LEVEL {
                view.references.collapsed.insert(first);
            }
            refresh_collapsed(view);
            Ok(None)
        }
        OutlineCommand::ShowLevel(shown) => {
            view.views.outline_level = shown;
            refresh_collapsed(view);
            Ok(None)
        }
        OutlineCommand::Close => {
            return super::run(view, super::ViewAction::SetView(ViewKind::Print), cx)
        }
    };
    match result {
        Ok(moved) => {
            if let Some(caret) = moved {
                view.insert_at = caret;
                view.selection_anchor = None;
            }
            view.dirty = view.dirty
                || !matches!(
                    command,
                    OutlineCommand::Expand
                        | OutlineCommand::Collapse
                        | OutlineCommand::ShowLevel(_)
                );
            follow(view);
            view.caret = view
                .document
                .place_caret(view.insert_at.0, view.insert_at.1);
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

/// Gives paragraphs `first..=last` a level: 見出し n's style, or the
/// document's default paragraph style for 本文.
fn set_levels(view: &mut DocumentView, first: usize, last: usize, level: u8) -> Result<(), String> {
    use jade_office_model::word::styles::WordStyles;
    let page = view.caret_page_1idx();
    let end = view
        .document
        .paragraph_text(last)
        .map_or(0, |text| text.len());
    view.document.select((first, 0), (last, end), page)?;
    if level >= BODY_LEVEL {
        jade_office_model::word::WordFormatting::set_paragraph_style(&mut *view.document, None)
    } else {
        WordStyles::apply_style(&mut *view.document, &format!("Heading{level}"))
    }?;
    view.ribbon.read_styles(view.document.main());
    Ok(())
}

/// 上へ移動 / 下へ移動 for paragraphs: they change places with the paragraph
/// above or below them.
fn move_paragraphs(
    view: &mut DocumentView,
    first: usize,
    last: usize,
    up: bool,
) -> Result<(usize, usize), String> {
    let document = view.document.main().word_document();
    let start = document
        .body_element_of(first)
        .ok_or("本文の段落ではありません")?;
    let end = document
        .body_element_of(last)
        .ok_or("本文の段落ではありません")?;
    let before = if up {
        start.checked_sub(1).ok_or("これ以上移動できません")?
    } else {
        end + 2
    };
    view.document.move_section_to(start, end, before)
}

/// The symbols beside the paragraphs on page `index`: ⊕ before a heading
/// with something under it, ⊖ before one without, ○ before body text. A
/// click on a heading's collapses or expands it.
pub fn symbols(
    view: &DocumentView,
    index: usize,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    if view.views.kind != ViewKind::Outline {
        return None;
    }
    let document = view.document.main().word_document();
    let manager = view.document.renderer().selection_manager();
    let all = levels(document);
    let zoom = view.zoom;
    let mut marks = Vec::new();
    for (position, (paragraph, level)) in all.iter().enumerate() {
        let Some((x, y, _, height)) = caret_rect(manager, (*paragraph, 0)) else {
            continue;
        };
        let (page, local_y) = view.document.page_and_offset(y);
        if page != index {
            continue;
        }
        let has_children = all
            .get(position + 1)
            .is_some_and(|(_, next)| *next > *level);
        let symbol = if *level == BODY_LEVEL {
            "○"
        } else if has_children || view.references.collapsed.contains(paragraph) {
            "⊕"
        } else {
            "⊖"
        };
        marks.push((*paragraph, *level, symbol, x - 18.0, local_y, height));
    }
    if marks.is_empty() {
        return None;
    }
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(
                marks
                    .into_iter()
                    .map(|(paragraph, level, symbol, x, y, height)| {
                        div()
                            .id(("outline-symbol", paragraph))
                            .absolute()
                            .left(px(x as f32 * zoom))
                            .top(px(y as f32 * zoom))
                            .h(px(height as f32 * zoom))
                            .flex()
                            .items_center()
                            .text_size(px((11.0 * zoom).max(8.0)))
                            .text_color(gpui::rgb(0x6b6b70))
                            .when(level < BODY_LEVEL, |mark| {
                                mark.cursor_pointer()
                                    .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                        cx.stop_propagation()
                                    })
                                    .on_click(on(cx, move |view, cx| {
                                        view.insert_at = (paragraph, 0);
                                        view.document.place_caret(paragraph, 0);
                                        let command =
                                            if view.references.collapsed.contains(&paragraph) {
                                                OutlineCommand::Expand
                                            } else {
                                                OutlineCommand::Collapse
                                            };
                                        run(view, command, cx);
                                    }))
                            })
                            .child(symbol)
                    }),
            )
            .into_any_element(),
    )
}

fn button(
    id: &'static str,
    label: &'static str,
    tooltip: &'static str,
    command: OutlineCommand,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .h(px(26.0))
        .px_2()
        .rounded_md()
        .text_xs()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(on(cx, move |view, cx| run(view, command, cx)))
        .child(label)
        .into_any_element()
}

/// The アウトライン tab's groups.
pub fn groups(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let document = view.document.main().word_document();
    let current = level_of(document, chosen(view).0).unwrap_or(BODY_LEVEL);
    let level_chip = |level: u8, cx: &mut Context<DocumentView>| {
        let label: SharedString = if level == BODY_LEVEL {
            t("outline.body").into()
        } else {
            i18n::format("outline.level", &[("n", &level)]).into()
        };
        chip(("outline-level", level as usize), label, current == level)
            .on_click(on(cx, move |view, cx| {
                run(view, OutlineCommand::Level(level), cx)
            }))
            .into_any_element()
    };
    let shown = view.views.outline_level;
    let show_chip = |level: u8, cx: &mut Context<DocumentView>| {
        let label: SharedString = if level == BODY_LEVEL {
            t("outline.all").into()
        } else {
            level.to_string().into()
        };
        chip(
            ("outline-show", level as usize),
            label,
            shown == level || (shown == 0 && level == BODY_LEVEL),
        )
        .on_click(on(cx, move |view, cx| {
            run(view, OutlineCommand::ShowLevel(level), cx)
        }))
        .into_any_element()
    };
    let levels: Vec<AnyElement> = (1..=BODY_LEVEL)
        .map(|level| level_chip(level, cx))
        .collect();
    let shows: Vec<AnyElement> = (1..=BODY_LEVEL).map(|level| show_chip(level, cx)).collect();
    vec![
        group(
            t("outline.group.tools"),
            vec![
                row(vec![
                    button(
                        "outline-top",
                        "⇐",
                        t("outline.to_top"),
                        OutlineCommand::ToTop,
                        cx,
                    ),
                    button(
                        "outline-promote",
                        "←",
                        t("outline.promote"),
                        OutlineCommand::Promote,
                        cx,
                    ),
                    button(
                        "outline-demote",
                        "→",
                        t("outline.demote"),
                        OutlineCommand::Demote,
                        cx,
                    ),
                    button(
                        "outline-body",
                        "⇒",
                        t("outline.to_body"),
                        OutlineCommand::ToBody,
                        cx,
                    ),
                    button(
                        "outline-up",
                        "▲",
                        t("outline.move_up"),
                        OutlineCommand::MoveUp,
                        cx,
                    ),
                    button(
                        "outline-down",
                        "▼",
                        t("outline.move_down"),
                        OutlineCommand::MoveDown,
                        cx,
                    ),
                    button(
                        "outline-expand",
                        "＋",
                        t("outline.expand"),
                        OutlineCommand::Expand,
                        cx,
                    ),
                    button(
                        "outline-collapse",
                        "－",
                        t("outline.collapse"),
                        OutlineCommand::Collapse,
                        cx,
                    ),
                ]),
                row(levels),
            ],
        ),
        group(t("outline.show_level"), vec![row(shows)]),
        group(
            t("outline.group.close"),
            vec![row(vec![button(
                "outline-close",
                t("outline.close"),
                t("outline.close"),
                OutlineCommand::Close,
                cx,
            )])],
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_views::{run as run_view, ViewAction};

    /// アウトライン: headings set in by level, a paragraph made 見出し 2 and
    /// moved up, レベルの表示 hiding the body text, and the pages back after.
    #[gpui::test]
    fn the_outline_sets_levels_moves_and_shows(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            let page = view.document.layout_page_size();
            let paragraph = view.insert_at.0;
            run_view(view, ViewAction::SetView(ViewKind::Outline), cx);
            assert!(view.document.main().view_geometry().is_some());

            view.insert_at = (paragraph, 0);
            view.document.place_caret(paragraph, 0);
            run(view, OutlineCommand::Level(2), cx);
            let document = view.document.main().word_document();
            assert_eq!(level_of(document, paragraph), Some(2));
            let indent = indents(document)[&paragraph];
            assert_eq!(indent, SYMBOL_ROOM + LEVEL_STEP);

            let text = view.document.paragraph_text(paragraph).unwrap();
            view.insert_at = (paragraph, 0);
            view.document.place_caret(paragraph, 0);
            run(view, OutlineCommand::MoveUp, cx);
            assert_eq!(view.document.paragraph_text(paragraph - 1).unwrap(), text);

            run(view, OutlineCommand::ShowLevel(2), cx);
            assert!(
                !view.document.collapsed_blocks().is_empty(),
                "body text hidden"
            );
            run(view, OutlineCommand::ShowLevel(BODY_LEVEL), cx);
            assert!(view.document.collapsed_blocks().is_empty());

            run(view, OutlineCommand::Close, cx);
            assert_eq!(view.views.kind, ViewKind::Print);
            assert_eq!(view.document.layout_page_size(), page);
        });
    }
}
