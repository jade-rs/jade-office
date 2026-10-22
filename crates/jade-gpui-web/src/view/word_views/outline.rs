//! アウトライン in the browser: the document as its outline — each
//! paragraph set in by its level, ⊕ before a heading with something under
//! it (⊖ with nothing), ○ before body text — and the アウトライン tab: the
//! level to give, レベル上げ and レベル下げ (and straight to 見出し 1 or to
//! 本文), 上へ移動 and 下へ移動, 展開 and 折りたたみ, and レベルの表示. The
//! levels and indents are `jade-office-model`'s (`word::outline`); the
//! engine lays the paragraphs out set in by them.

use gpui::{div, prelude::*, px, AnyElement, Context, MouseButton, SharedString, Window};
use jade_web::word::outline::{self as model, BODY_LEVEL};
use jade_web::word::references::navigation::WordNavigation;
use jade_web::word::view::ViewKind;
use jade_web::word::WordSession;

use super::ViewsAction;
use crate::i18n;
use crate::ribbon::{group, row, Action};
use crate::view::word_kit::chip;
use crate::view::DocView as DocumentView;

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
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

/// Keeps the engine's indents in step with the outline while アウトライン
/// shows, and takes them off after.
pub fn follow(view: &mut DocumentView) {
    if view.word_ref().is_none() {
        return;
    }
    let wanted =
        (view.views.kind == ViewKind::Outline).then(|| model::indents(view.doc().word_document()));
    let caret = view.doc().caret();
    match view.doc_mut().set_outline_indents(wanted) {
        Ok(true) => {
            if let Some((paragraph, offset)) = caret {
                view.doc_mut().place_caret(paragraph, offset);
            }
            view.refresh_repainted();
        }
        Ok(false) => {}
        Err(error) => view.set_status(error.into()),
    }
}

/// The body elements レベルの表示 hides, merged with the collapsed
/// headings' (`word_references`).
pub fn extra_hidden(view: &DocumentView) -> Vec<usize> {
    if view.views.kind != ViewKind::Outline {
        return Vec::new();
    }
    model::hidden_by_level(view.doc().word_document(), view.views.outline_level)
}

/// The paragraphs the commands act on: the selection's, or the caret's.
fn chosen(view: &DocumentView) -> (usize, usize) {
    match view.doc().selection() {
        Some((anchor, focus)) => (anchor.0.min(focus.0), anchor.0.max(focus.0)),
        None => {
            let caret = view.caret_at().0;
            (caret, caret)
        }
    }
}

/// Runs an アウトライン command.
pub fn run(
    view: &mut DocumentView,
    command: OutlineCommand,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) {
    use crate::view::word_references::refresh_collapsed;
    if view.word_ref().is_none() {
        return;
    }
    let (first, last) = chosen(view);
    let level = model::level_of(view.doc().word_document(), first).unwrap_or(BODY_LEVEL);
    let result: Result<Option<(usize, usize)>, String> = match command {
        OutlineCommand::Level(wanted) => set_levels(view, first, last, wanted).map(|_| None),
        OutlineCommand::Promote if level == BODY_LEVEL => {
            let parent =
                model::promoted_body_level(&model::levels(view.doc().word_document()), first);
            set_levels(view, first, last, parent).map(|_| None)
        }
        OutlineCommand::Promote => view
            .doc_mut()
            .shift_heading_level(first, true)
            .map(|_| None),
        OutlineCommand::Demote if level == BODY_LEVEL => Ok(None),
        OutlineCommand::Demote if level == 9 => {
            set_levels(view, first, last, BODY_LEVEL).map(|_| None)
        }
        OutlineCommand::Demote => view
            .doc_mut()
            .shift_heading_level(first, false)
            .map(|_| None),
        OutlineCommand::ToTop => set_levels(view, first, last, 1).map(|_| None),
        OutlineCommand::ToBody => set_levels(view, first, last, BODY_LEVEL).map(|_| None),
        OutlineCommand::MoveUp | OutlineCommand::MoveDown => {
            let up = command == OutlineCommand::MoveUp;
            if level < BODY_LEVEL && view.references.collapsed.contains(&first) {
                // A collapsed heading moves with what it holds.
                view.doc_mut().move_heading_section(first, up).map(Some)
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
            return view.views_action(ViewsAction::SetView(ViewKind::Print), window, cx)
        }
    };
    match result {
        Ok(moved) => {
            if let Some((paragraph, offset)) = moved {
                view.doc_mut().place_caret(paragraph, offset);
            }
            follow(view);
            view.refresh_repainted();
            view.word_result(Ok(()), cx);
        }
        Err(error) => view.word_result(Err(error), cx),
    }
}

/// Gives paragraphs `first..=last` a level: 見出し n's style, or the
/// document's default paragraph style for 本文.
fn set_levels(view: &mut DocumentView, first: usize, last: usize, level: u8) -> Result<(), String> {
    use jade_web::word::styles::WordStyles;
    let end = view.doc().paragraph_text(last).map_or(0, |text| text.len());
    view.doc_mut().select((first, 0), (last, end))?;
    if level >= BODY_LEVEL {
        jade_web::word::WordFormatting::set_paragraph_style(view.doc_mut(), None)
    } else {
        view.doc_mut().apply_style(&format!("Heading{level}"))
    }?;
    if let crate::view::Document::Pages {
        ribbon, document, ..
    } = &mut view.document
    {
        ribbon.read_styles(document);
    }
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
    let document = view.doc().word_document();
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
    view.doc_mut().move_section_to(start, end, before)
}

/// The symbols beside the paragraphs on page `index`; a click on a
/// heading's collapses or expands it.
pub fn symbols(
    view: &DocumentView,
    index: usize,
    zoom: f32,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    if view.views.kind != ViewKind::Outline {
        return None;
    }
    let document = view.doc();
    let all = model::levels(document.word_document());
    let mut marks = Vec::new();
    for (position, (paragraph, level)) in all.iter().enumerate() {
        let Some((x, y, height)) = document.position_metrics((*paragraph, 0)) else {
            continue;
        };
        let (page, local_y) = document.page_and_offset(y);
        if page != index {
            continue;
        }
        let next = all.get(position + 1).map(|(_, next)| *next);
        let symbol = model::symbol(*level, next, view.references.collapsed.contains(paragraph));
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
                                    .on_click(cx.listener(move |view, _, window, cx| {
                                        view.doc_mut().place_caret(paragraph, 0);
                                        let command =
                                            if view.references.collapsed.contains(&paragraph) {
                                                OutlineCommand::Expand
                                            } else {
                                                OutlineCommand::Collapse
                                            };
                                        run(view, command, window, cx);
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
    crate::word_page::text_button(
        id,
        label,
        tooltip,
        false,
        Action::Views(ViewsAction::Outline(command)),
        cx,
    )
}

/// The アウトライン tab's groups.
pub fn groups(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let current = model::level_of(view.doc().word_document(), chosen(view).0).unwrap_or(BODY_LEVEL);
    let shown = view.views.outline_level;
    let levels: Vec<AnyElement> = (1..=BODY_LEVEL)
        .map(|level| {
            // The level's number: レベル n is its tooltip's word.
            let label: SharedString = if level == BODY_LEVEL {
                t("outline.body").into()
            } else {
                level.to_string().into()
            };
            chip(("outline-level", level as usize), label, current == level)
                .on_click(crate::ribbon::listener(
                    Action::Views(ViewsAction::Outline(OutlineCommand::Level(level))),
                    cx,
                ))
                .into_any_element()
        })
        .collect();
    let shows: Vec<AnyElement> = (1..=BODY_LEVEL)
        .map(|level| {
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
            .on_click(crate::ribbon::listener(
                Action::Views(ViewsAction::Outline(OutlineCommand::ShowLevel(level))),
                cx,
            ))
            .into_any_element()
        })
        .collect();
    let tools = [
        ("outline-top", "⇐", "outline.to_top", OutlineCommand::ToTop),
        (
            "outline-promote",
            "←",
            "outline.promote",
            OutlineCommand::Promote,
        ),
        (
            "outline-demote",
            "→",
            "outline.demote",
            OutlineCommand::Demote,
        ),
        (
            "outline-body",
            "⇒",
            "outline.to_body",
            OutlineCommand::ToBody,
        ),
        ("outline-up", "▲", "outline.move_up", OutlineCommand::MoveUp),
        (
            "outline-down",
            "▼",
            "outline.move_down",
            OutlineCommand::MoveDown,
        ),
        (
            "outline-expand",
            "＋",
            "outline.expand",
            OutlineCommand::Expand,
        ),
        (
            "outline-collapse",
            "－",
            "outline.collapse",
            OutlineCommand::Collapse,
        ),
    ]
    .into_iter()
    .map(|(id, label, tooltip, command)| button(id, label, t(tooltip), command, cx))
    .collect();
    vec![
        group(t("outline.group.tools"), vec![row(tools)]),
        group(t("outline.group.level"), vec![row(levels)]),
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
