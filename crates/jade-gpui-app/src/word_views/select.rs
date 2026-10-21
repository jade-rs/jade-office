//! Selecting and moving text with the mouse beyond a plain drag:
//!
//! - ⌘-drag adds a range to the ones already selected (複数範囲の選択);
//!   formatting then goes on every range, as one undo step;
//! - ⌥-drag selects a rectangle of text (矩形選択), a range per line;
//! - dragging a selection moves it, ⌥ held copies it, as one undo step;
//! - a double-click takes a word — a run of one script in Japanese — and a
//!   triple-click the paragraph (`DocumentView::select_word_at_caret`).

use gpui::{
    div, prelude::*, px, AnyElement, Context, MouseDownEvent, MouseMoveEvent, SharedString,
};
use jade_office_model::word::clipboard::WordClipboard;
use jade_office_model::word::selection::{block_ranges, caret_rect, range_rects, ranges_text};
use jade_office_model::word::Position;

use crate::ribbon::Action;
use crate::{theme, DocumentView};

/// A selection being dragged to somewhere else.
#[derive(Clone, Debug)]
pub struct TextDrag {
    /// What is being dragged.
    pub source: (Position, Position),
    /// Where the mouse went down, for a click that never became a drag.
    pub pressed: Position,
    /// Where it would land now.
    pub target: Option<Position>,
    /// ⌥: a copy rather than a move.
    pub copy: bool,
}

/// What the mouse is doing beyond a plain drag.
#[derive(Clone, Debug)]
pub enum MouseGesture {
    Drag(TextDrag),
    /// 矩形選択 from a point in the document's coordinates.
    Block((f64, f64)),
}

fn ordered(range: (Position, Position)) -> (Position, Position) {
    if range.0 <= range.1 {
        range
    } else {
        (range.1, range.0)
    }
}

/// The mouse went down on the pages at a point in the document's
/// coordinates. `true` when a gesture of this module took it.
pub fn mouse_down(
    view: &mut DocumentView,
    x: f64,
    document_y: f64,
    event: &MouseDownEvent,
    cx: &mut Context<DocumentView>,
) -> bool {
    let modifiers = event.modifiers;
    let selection = view
        .document
        .selection()
        .filter(|(anchor, focus)| anchor != focus);
    // ⌘: the selection so far is kept beside the one about to be made.
    if modifiers.platform {
        if let Some(range) = selection {
            view.views.extra_ranges.push(ordered(range));
        }
        return false;
    }
    if event.click_count == 1 && !modifiers.shift {
        let at = view.document.position_at(x, document_y);
        if let (Some(range), Some(at)) = (selection, at) {
            let (start, end) = ordered(range);
            if at >= start && at < end && view.views.extra_ranges.is_empty() {
                view.views.gesture = Some(MouseGesture::Drag(TextDrag {
                    source: (start, end),
                    pressed: at,
                    target: None,
                    copy: modifiers.alt,
                }));
                return true;
            }
        }
        view.views.extra_ranges.clear();
        if modifiers.alt {
            view.views.gesture = Some(MouseGesture::Block((x, document_y)));
            view.drag_anchor = None;
            if let Some(at) = at {
                view.insert_at = at;
                view.caret = view.document.place_caret(at.0, at.1);
            }
            view.rebuild_all_images();
            cx.notify();
            return true;
        }
    }
    view.views.extra_ranges.clear();
    false
}

/// The pointer moved with the button down; `true` when a gesture here
/// follows it.
pub fn drag(
    view: &mut DocumentView,
    page_index: usize,
    event: &MouseMoveEvent,
    cx: &mut Context<DocumentView>,
) -> bool {
    let Some(gesture) = view.views.gesture.clone() else {
        return false;
    };
    let Some(Some(bounds)) = view.page_bounds.borrow().get(page_index).copied() else {
        return true;
    };
    let zoom = f64::from(view.zoom);
    let x = f64::from(event.position.x - bounds.origin.x) / zoom;
    let y = view.document.document_y(
        page_index,
        f64::from(event.position.y - bounds.origin.y) / zoom,
    );
    match gesture {
        MouseGesture::Drag(mut drag) => {
            drag.target = view.document.position_at(x, y);
            drag.copy = drag.copy || event.modifiers.alt;
            view.views.gesture = Some(MouseGesture::Drag(drag));
        }
        MouseGesture::Block(from) => {
            let ranges = block_ranges(view.document.renderer().selection_manager(), from, (x, y));
            set_ranges(view, ranges);
        }
    }
    cx.notify();
    true
}

/// Puts `ranges` in as the selection: the first as the document's, the
/// rest beside it.
pub fn set_ranges(view: &mut DocumentView, ranges: Vec<(Position, Position)>) {
    let mut ranges = ranges.into_iter();
    let first = ranges.next();
    view.views.extra_ranges = ranges.collect();
    if let Some((start, end)) = first {
        let page = view.caret_page_1idx();
        if view.document.select(start, end, page).is_ok() {
            view.insert_at = end;
            view.caret = view.document.place_caret(end.0, end.1);
            let _ = view.document.select(start, end, page);
        }
        view.rebuild_all_images();
    }
}

/// The button came up: a drag drops its text, a click that never moved
/// puts the caret where it went down.
pub fn mouse_up(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(gesture) = view.views.gesture.take() else {
        return;
    };
    let MouseGesture::Drag(drag) = gesture else {
        return;
    };
    match drag.target {
        Some(target) if target < drag.source.0 || target > drag.source.1 => {
            drop_text(view, &drag, target, cx)
        }
        // Back where it started, or never moved: a click.
        _ => {
            view.document.place_caret(drag.pressed.0, drag.pressed.1);
            view.insert_at = drag.pressed;
            view.caret = view.document.place_caret(drag.pressed.0, drag.pressed.1);
            view.selection_anchor = None;
            view.rebuild_all_images();
            cx.notify();
        }
    }
}

/// Moves (or copies) the dragged text to `target`, as one undo step.
pub fn drop_text(
    view: &mut DocumentView,
    drag: &TextDrag,
    target: Position,
    cx: &mut Context<DocumentView>,
) {
    let page = view.caret_page_1idx();
    if view
        .document
        .select(drag.source.0, drag.source.1, page)
        .is_err()
    {
        return;
    }
    let Some(clip) = WordClipboard::copy_rich(&view.document) else {
        return;
    };
    let moved_from = (!drag.copy).then_some(drag.source);
    match WordClipboard::drop_rich(&mut view.document, &clip, moved_from, target) {
        Ok(end) => {
            // The dropped text stays selected, as Word leaves it.
            let start = if drag.copy {
                target
            } else {
                jade_office_model::word::clipboard::position_after_removal(
                    target,
                    drag.source.0,
                    drag.source.1,
                )
            };
            view.rebuild_after_edit();
            let page = view.caret_page_1idx();
            let _ = view.document.select(start, end, page);
            view.insert_at = end;
            view.caret = view.document.place_caret(end.0, end.1);
            let _ = view.document.select(start, end, page);
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(SharedString::from(error)),
    }
    cx.notify();
}

/// Whether `action` formats what is selected, and so goes on every range.
fn formats(action: &Action) -> bool {
    use crate::word_characters::CharAction;
    matches!(
        action,
        Action::Toggle(_)
            | Action::ClearFormatting
            | Action::FontSize(_)
            | Action::StepFontSize(_)
            | Action::FontColor(_)
            | Action::Highlight(_)
            | Action::List(_)
            | Action::Indent(_)
            | Action::Align(_)
            | Action::LineSpacing(_)
            | Action::Shading(_)
            | Action::ParagraphBorders(_)
            | Action::Style(_)
            | Action::FontFamilyName(_)
            | Action::Characters(
                CharAction::Underline(_)
                    | CharAction::UnderlineColor(_)
                    | CharAction::ThemeColor(..)
                    | CharAction::Shading
                    | CharAction::Border
                    | CharAction::ChangeCase(_)
                    | CharAction::Scale(_)
            )
    )
}

/// Runs a formatting command on every selected range, as one undo step.
/// `true` when it did.
pub fn run_on_ranges(
    view: &mut DocumentView,
    action: &Action,
    cx: &mut Context<DocumentView>,
) -> bool {
    if view.views.extra_ranges.is_empty() || view.views.on_ranges || !formats(action) {
        return false;
    }
    let primary = view.document.selection().map(ordered);
    let mut ranges = view.views.extra_ranges.clone();
    ranges.extend(primary);
    let mark = view.document.history_len();
    view.views.on_ranges = true;
    for (start, end) in &ranges {
        let page = view.caret_page_1idx();
        if view.document.select(*start, *end, page).is_ok() {
            view.run_ribbon(action.clone(), cx);
        }
    }
    view.views.on_ranges = false;
    view.document.merge_history_since(mark);
    if let Some((start, end)) = primary {
        let page = view.caret_page_1idx();
        let _ = view.document.select(start, end, page);
    }
    view.rebuild_all_images();
    cx.notify();
    true
}

/// ⌘C with several ranges: their text, a line each. `true` when it did.
pub fn copy(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> bool {
    if view.views.extra_ranges.is_empty() {
        return false;
    }
    let mut ranges = view.views.extra_ranges.clone();
    ranges.extend(view.document.selection().map(ordered));
    ranges.sort();
    let text = ranges_text(|index| view.document.paragraph_text(index), &ranges);
    cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
    true
}

/// The extra ranges and the drop mark, over page `index`.
pub fn overlay(view: &DocumentView, index: usize) -> Option<AnyElement> {
    let drop_target = match &view.views.gesture {
        Some(MouseGesture::Drag(drag)) => drag.target,
        _ => None,
    };
    if view.views.extra_ranges.is_empty() && drop_target.is_none() {
        return None;
    }
    let manager = view.document.renderer().selection_manager();
    let zoom = view.zoom;
    let mut rects = Vec::new();
    for &(start, end) in &view.views.extra_ranges {
        for rect in range_rects(manager, start, end) {
            rects.push((rect, false));
        }
    }
    if let Some(rect) = drop_target.and_then(|target| caret_rect(manager, target)) {
        rects.push((rect, true));
    }
    let on_page: Vec<_> = rects
        .into_iter()
        .filter_map(|((x, y, width, height), caret)| {
            let (page, local_y) = view.document.page_and_offset(y);
            (page == index).then_some((x, local_y, width, height, caret))
        })
        .collect();
    if on_page.is_empty() {
        return None;
    }
    Some(
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .children(on_page.into_iter().map(|(x, y, width, height, caret)| {
                div()
                    .absolute()
                    .left(px(x as f32 * zoom))
                    .top(px(y as f32 * zoom))
                    .w(px((width as f32 * zoom).max(1.0)))
                    .h(px(height as f32 * zoom))
                    .bg(if caret {
                        theme::caret()
                    } else {
                        gpui::rgba(0x3f7fdf44)
                    })
            }))
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(view: &DocumentView) -> (usize, String) {
        let paragraph = view.insert_at.0;
        (paragraph, view.document.paragraph_text(paragraph).unwrap())
    }

    /// Two ranges, the second added with ⌘, both go bold in one step.
    #[gpui::test]
    fn formatting_goes_on_every_range(cx: &mut gpui::TestAppContext) {
        use jade_office_model::word::CharacterFormat;
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            let (paragraph, text) = words(view);
            let third = text.char_indices().nth(3).unwrap().0;
            let sixth = text.char_indices().nth(6).unwrap().0;
            let ninth = text.char_indices().nth(9).unwrap().0;
            let bold_at = |view: &DocumentView, offset: usize| {
                view.document
                    .run_style_at(paragraph, offset)
                    .and_then(|style| style.bold)
                    .unwrap_or(false)
            };
            let before = (bold_at(view, 0), bold_at(view, sixth));
            set_ranges(view, vec![((paragraph, 0), (paragraph, third))]);
            view.views
                .extra_ranges
                .push(((paragraph, sixth), (paragraph, ninth)));
            let mark = view.document.history_len();
            view.run_ribbon(Action::Toggle(CharacterFormat::Bold), cx);
            assert_eq!(view.document.history_len(), mark + 1, "one step");
            assert_ne!(bold_at(view, 0), before.0);
            assert_ne!(bold_at(view, sixth), before.1);
            assert!(overlay(view, view.current_page).is_some() || view.current_page > 0);
            view.step_history(true, cx);
            assert_eq!((bold_at(view, 0), bold_at(view, sixth)), before);
        });
    }

    /// Dragging selected text moves it; ⌥ copies it; each is one undo.
    #[gpui::test]
    fn dragged_text_moves_or_copies(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        cx.simulate_input("abc def ");
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            let original = view.document.paragraph_text(paragraph).unwrap();
            let drag = TextDrag {
                source: ((paragraph, 0), (paragraph, 4)),
                pressed: (paragraph, 1),
                target: Some((paragraph, 8)),
                copy: false,
            };
            drop_text(view, &drag, (paragraph, 8), cx);
            let moved = view.document.paragraph_text(paragraph).unwrap();
            assert!(moved.starts_with("def abc "), "{moved:?}");
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 4), (paragraph, 8))),
                "the dropped text is selected"
            );
            view.step_history(true, cx);
            assert_eq!(view.document.paragraph_text(paragraph).unwrap(), original);

            let copy = TextDrag { copy: true, ..drag };
            drop_text(view, &copy, (paragraph, 8), cx);
            let copied = view.document.paragraph_text(paragraph).unwrap();
            assert!(copied.starts_with("abc def abc "), "{copied:?}");
        });
    }

    /// ⌥-drag over two lines selects the same columns of each.
    #[gpui::test]
    fn an_option_drag_selects_a_block(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            let caret = view.caret.expect("a caret");
            let manager = view.document.renderer().selection_manager();
            let ranges = block_ranges(
                manager,
                (caret.x + 10.0, caret.document_y),
                (caret.x + 80.0, caret.document_y + caret.height * 8.0),
            );
            assert!(ranges.len() >= 2, "{ranges:?}");
            for (start, end) in &ranges {
                assert!(start < end);
            }
            let text = ranges_text(|index| view.document.paragraph_text(index), &ranges);
            assert_eq!(text.lines().count(), ranges.len());
        });
    }

    /// A double-click in Japanese takes the run of one script; a triple
    /// click the paragraph.
    #[gpui::test]
    fn clicks_take_a_word_then_the_paragraph(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 0);
            view.caret = view.document.place_caret(paragraph, 0);
        });
        cx.simulate_input("東京へ行く");
        view.update(cx, |view, cx| {
            let paragraph = view.insert_at.0;
            view.insert_at = (paragraph, 3);
            view.select_word_at_caret(cx);
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 0), (paragraph, 6)))
            );
            view.select_paragraph(cx);
            let length = view.document.paragraph_text(paragraph).unwrap().len();
            assert_eq!(
                view.document.selection(),
                Some(((paragraph, 0), (paragraph, length)))
            );
        });
    }
}
