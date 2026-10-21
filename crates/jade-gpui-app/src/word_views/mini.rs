//! ミニ ツールバー and リアルタイムのプレビュー.
//!
//! The mini toolbar floats over a selection just made with the mouse: the
//! font, its size, 拡大/縮小, B I U, the highlight and font colour, the
//! lists, the styles and 書式のコピー. It goes as soon as a key is pressed
//! or the pages are clicked, as Word's does.
//!
//! Live preview: while the pointer rests on a font, a size, a colour, a
//! highlight or a style in a gallery, the selection shows it; moving off
//! takes it back, and a click keeps it. The preview is the command itself,
//! applied and undone, so what shows is exactly what a click gives.

use gpui::{
    anchored, deferred, div, point, prelude::*, px, svg, AnyElement, Context, MouseButton, Pixels,
    Point, SharedString,
};
use jade_office_model::word::SelectionRangeOf as SelectionRange;
use jade_office_model::word::{CharacterFormat, WordSession};

use crate::ribbon::{listener, menu_panel, menu_row, Action};
use crate::tooltip::Tooltip;
use crate::word_characters::CharAction;
use crate::{i18n, theme, DocumentView};

/// The mini toolbar's own lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniPopup {
    Family,
    Size,
    Styles,
}

/// A preview on the document: the gallery entry under the pointer, and
/// what to put back when the pointer leaves it.
pub struct Preview {
    pub action: Action,
    selection: Option<SelectionRange>,
    dirty: bool,
}

/// Whether hovering `action` in a gallery previews it.
pub fn previewable(action: &Action) -> bool {
    matches!(
        action,
        Action::FontFamilyName(_)
            | Action::FontSize(_)
            | Action::FontColor(_)
            | Action::Highlight(_)
            | Action::Style(_)
            | Action::Shading(_)
            | Action::Characters(CharAction::ThemeColor(..))
            | Action::Characters(CharAction::Underline(_))
    )
}

/// The pointer entering (`true`) or leaving a gallery entry.
pub fn hover(
    action: Action,
    cx: &mut Context<DocumentView>,
) -> impl Fn(&bool, &mut gpui::Window, &mut gpui::App) + 'static {
    let view = cx.weak_entity();
    move |entered, _window, cx| {
        let action = action.clone();
        let entered = *entered;
        view.update(cx, move |view, cx| {
            if entered {
                start_preview(view, action, cx);
            } else if view
                .views
                .preview
                .as_ref()
                .is_some_and(|preview| preview.action == action)
            {
                end_preview(view, cx);
            }
        })
        .ok();
    }
}

/// Shows `action` on the selection, in place of any preview before it.
pub fn start_preview(view: &mut DocumentView, action: Action, cx: &mut Context<DocumentView>) {
    if view
        .views
        .preview
        .as_ref()
        .is_some_and(|preview| preview.action == action)
    {
        return;
    }
    end_preview(view, cx);
    // Word previews on a selection; a bare caret has nothing to show it on.
    let Some((anchor, focus)) = view.document.selection() else {
        return;
    };
    if anchor == focus {
        return;
    }
    let selection = view.document.selection_range();
    let dirty = view.dirty;
    let undo_before = view.document.can_undo();
    let popup = view.ribbon.popup;
    let mini_popup = view.views.mini_popup;
    view.views.previewing = true;
    view.run_ribbon(action.clone(), cx);
    view.views.previewing = false;
    // The list stays open under the pointer.
    view.ribbon.popup = popup;
    view.views.mini_popup = mini_popup;
    if view.document.can_undo() && (view.dirty || !undo_before) {
        view.views.preview = Some(Preview {
            action,
            selection,
            dirty,
        });
    }
    cx.notify();
}

/// Takes the preview back off, if one is showing.
pub fn end_preview(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    let Some(preview) = view.views.preview.take() else {
        return;
    };
    match view.document.undo() {
        Ok(_) => {
            view.document.set_selection_range(preview.selection);
            if let Some(range) = preview.selection {
                view.insert_at = (range.focus.paragraph, range.focus.offset);
            }
            view.dirty = preview.dirty;
            view.ribbon.read_styles(view.document.main());
            view.rebuild_all_images();
        }
        Err(error) => eprintln!("preview undo failed: {error}"),
    }
    cx.notify();
}

/// Before a ribbon command runs: a click on the entry being previewed
/// keeps it (and the command need not run again); anything else takes the
/// preview off first. `true` when the command is already done.
pub fn settle(view: &mut DocumentView, action: &Action, cx: &mut Context<DocumentView>) -> bool {
    if view.views.previewing {
        return false;
    }
    if !action.is_chrome() {
        view.views.mini_popup = None;
    }
    let kept = view
        .views
        .preview
        .as_ref()
        .is_some_and(|preview| &preview.action == action);
    if kept {
        view.views.preview = None;
        view.ribbon.popup = None;
        view.dirty = true;
        cx.notify();
        return true;
    }
    end_preview(view, cx);
    false
}

/// After the mouse comes up on the pages: the toolbar shows over a
/// selection the mouse made.
pub fn after_mouse_up(view: &mut DocumentView) {
    view.views.mini = view
        .document
        .selection()
        .is_some_and(|(anchor, focus)| anchor != focus)
        && view.painter.is_none();
    view.views.mini_popup = None;
}

/// Where the toolbar goes: above the caret at the selection's end, in
/// window coordinates.
fn position(view: &DocumentView) -> Option<Point<Pixels>> {
    let caret = view.caret?;
    let (page, local_y) = view.document.page_and_offset(caret.document_y);
    let origin = view
        .page_bounds
        .borrow()
        .get(page)
        .copied()
        .flatten()?
        .origin;
    let zoom = view.zoom;
    Some(point(
        origin.x + px((caret.x as f32 * zoom - 120.0).max(0.0)),
        origin.y + px((local_y as f32 * zoom - 74.0).max(0.0)),
    ))
}

fn icon(
    id: &'static str,
    path: &'static str,
    tooltip: &'static str,
    active: bool,
    action: Action,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(24.0))
        .rounded_md()
        .cursor_pointer()
        .when(active, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(tooltip))
        .on_click(listener(action, cx))
        .child(svg().path(path).size_4().text_color(if active {
            theme::accent()
        } else {
            theme::text()
        }))
        .into_any_element()
}

fn combo(
    id: &'static str,
    value: String,
    width: f32,
    popup: MiniPopup,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(22.0))
        .pl_1p5()
        .pr_0p5()
        .rounded_sm()
        .border_1()
        .border_color(theme::border())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .on_click(cx.listener(move |view, _event, _window, cx| {
            view.views.mini_popup = (view.views.mini_popup != Some(popup)).then_some(popup);
            cx.notify();
        }))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_xs()
                .child(value),
        )
        .child(
            svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element()
}

const SIZES: [f32; 17] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 22.0, 24.0, 26.0, 28.0, 36.0, 48.0,
    72.0,
];

/// The toolbar, while it shows.
pub fn toolbar(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    if !view.views.mini || view.document.selection().is_none() {
        return None;
    }
    let at = position(view)?;
    let state = view.format_state();
    let family = state
        .font_family
        .clone()
        .unwrap_or_else(|| "游明朝".to_string());
    let size = state.font_size.unwrap_or(10.5);
    let t = i18n::text;
    let list = view.views.mini_popup.map(|popup| match popup {
        MiniPopup::Family => crate::word_characters::font_family_menu(&family, cx),
        MiniPopup::Size => menu_panel("mini-size-menu")
            .w(px(72.0))
            .max_h(px(300.0))
            .overflow_y_scroll()
            .children(SIZES.iter().enumerate().map(|(index, points)| {
                menu_row(
                    ("mini-size", index),
                    if points.fract() == 0.0 {
                        format!("{points:.0}")
                    } else {
                        format!("{points}")
                    },
                    (size - points).abs() < 0.01,
                    Action::FontSize(*points),
                    cx,
                )
            }))
            .into_any_element(),
        MiniPopup::Styles => {
            let names = jade_office_model::word::styles::quick_style_gallery(
                &jade_office_model::word::styles::WordStyles::style_catalog(view.document.main()),
            );
            menu_panel("mini-styles-menu")
                .w(px(200.0))
                .max_h(px(300.0))
                .overflow_y_scroll()
                .children(names.iter().enumerate().map(|(index, entry)| {
                    let label: SharedString = match entry.preset {
                        Some(0) => t("doc.style.normal").into(),
                        Some(1) => t("doc.style.heading1").into(),
                        Some(2) => t("doc.style.heading2").into(),
                        Some(_) => t("doc.style.heading3").into(),
                        None => entry.name.clone().unwrap_or_default().into(),
                    };
                    menu_row(
                        ("mini-style", index),
                        label,
                        state.style_id.is_some() && entry.id == state.style_id,
                        Action::Style(entry.id.clone()),
                        cx,
                    )
                }))
                .into_any_element()
        }
    });
    let first = div()
        .flex()
        .items_center()
        .gap_0p5()
        .child(combo(
            "mini-family",
            family.clone(),
            110.0,
            MiniPopup::Family,
            cx,
        ))
        .child(combo(
            "mini-size",
            if size.fract() == 0.0 {
                format!("{size:.0}")
            } else {
                format!("{size}")
            },
            44.0,
            MiniPopup::Size,
            cx,
        ))
        .child(icon(
            "mini-grow",
            "icons/a-arrow-up.svg",
            t("doc.grow_title"),
            false,
            Action::StepFontSize(true),
            cx,
        ))
        .child(icon(
            "mini-shrink",
            "icons/a-arrow-down.svg",
            t("doc.shrink_title"),
            false,
            Action::StepFontSize(false),
            cx,
        ))
        .child(
            div()
                .id("mini-painter")
                .flex()
                .items_center()
                .justify_center()
                .size(px(24.0))
                .rounded_md()
                .cursor_pointer()
                .hover(|button| button.bg(theme::hovered()))
                .tooltip(Tooltip::text(t("painter.title")))
                .on_click(cx.listener(|view, _event, _window, cx| view.pick_up_format(false, cx)))
                .child(
                    svg()
                        .path("icons/paintbrush.svg")
                        .size_4()
                        .text_color(theme::text()),
                ),
        )
        .child(combo(
            "mini-styles",
            t("doc.group.styles").to_string(),
            72.0,
            MiniPopup::Styles,
            cx,
        ));
    let second = div()
        .flex()
        .items_center()
        .gap_0p5()
        .child(icon(
            "mini-bold",
            "icons/bold.svg",
            t("doc.bold_title"),
            state.bold,
            Action::Toggle(CharacterFormat::Bold),
            cx,
        ))
        .child(icon(
            "mini-italic",
            "icons/italic.svg",
            t("doc.italic_title"),
            state.italic,
            Action::Toggle(CharacterFormat::Italic),
            cx,
        ))
        .child(icon(
            "mini-underline",
            "icons/underline.svg",
            t("doc.underline_title"),
            state.underline,
            Action::Toggle(CharacterFormat::Underline),
            cx,
        ))
        .child(icon(
            "mini-highlight",
            "icons/highlighter.svg",
            t("doc.highlight_title"),
            false,
            Action::Highlight("yellow"),
            cx,
        ))
        .child(icon(
            "mini-color",
            "icons/baseline.svg",
            t("doc.font_color_title"),
            false,
            Action::FontColor("FF0000".to_string()),
            cx,
        ))
        .child(icon(
            "mini-bullets",
            "icons/list.svg",
            t("doc.bullets"),
            false,
            Action::List(true),
            cx,
        ))
        .child(icon(
            "mini-numbering",
            "icons/list-ordered.svg",
            t("doc.numbering"),
            false,
            Action::List(false),
            cx,
        ));
    let bar = div()
        .id("mini-toolbar")
        .occlude()
        .flex()
        .flex_col()
        .gap_0p5()
        .p_1()
        .rounded_md()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .shadow_lg()
        .text_color(theme::text())
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(first)
        .child(second)
        .children(list.map(|list| div().absolute().top_full().left_0().child(list)));
    Some(
        deferred(
            anchored()
                .position(at)
                .snap_to_window_with_margin(px(8.0))
                .child(bar),
        )
        .with_priority(2)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn select_start(view: &mut DocumentView) {
        let paragraph = view.insert_at.0;
        let text = view.current_paragraph_text();
        let end = text.char_indices().nth(3).map(|(index, _)| index).unwrap();
        view.document
            .select((paragraph, 0), (paragraph, end), 1)
            .unwrap();
        view.insert_at = (paragraph, end);
    }

    /// Resting on a size shows it on the selection; leaving takes it back
    /// and leaves the document as it was; a click on the entry keeps it as
    /// one undo step.
    #[gpui::test]
    fn a_previewed_size_shows_and_goes(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            select_start(view);
            let before = view.format_state().font_size;
            start_preview(view, Action::FontSize(36.0), cx);
            assert_eq!(view.format_state().font_size, Some(36.0));
            assert!(view.views.preview.is_some());
            // Another entry replaces it rather than stacking on it.
            start_preview(view, Action::FontSize(28.0), cx);
            assert_eq!(view.format_state().font_size, Some(28.0));
            end_preview(view, cx);
            assert_eq!(view.format_state().font_size, before);
            assert!(!view.dirty, "a preview taken back leaves nothing to save");
            assert!(view.document.selection().is_some(), "the selection stays");

            start_preview(view, Action::FontSize(36.0), cx);
            view.run_ribbon(Action::FontSize(36.0), cx);
            assert!(view.views.preview.is_none());
            assert_eq!(view.format_state().font_size, Some(36.0));
            assert!(view.dirty);
            view.step_history(true, cx);
            assert_eq!(
                view.format_state().font_size,
                before,
                "one undo takes it off"
            );
        });
    }

    /// A selection made with the mouse brings the toolbar; a key sends it
    /// away. Its buttons are the ribbon's commands.
    #[gpui::test]
    fn the_mini_toolbar_comes_with_a_mouse_selection(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            select_start(view);
            after_mouse_up(view);
            assert!(view.views.mini);
            assert!(toolbar(view, cx).is_some());
            let bold = view.format_state().bold;
            view.run_ribbon(Action::Toggle(CharacterFormat::Bold), cx);
            assert_eq!(view.format_state().bold, !bold);
        });
        cx.simulate_keystrokes("right");
        view.read_with(cx, |view, _| assert!(!view.views.mini));
    }
}
