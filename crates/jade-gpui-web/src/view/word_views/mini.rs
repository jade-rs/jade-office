//! ミニ ツールバー in the browser: it floats over a selection just made with
//! the mouse — the font, its size, 拡大/縮小, 書式のコピー, the styles, B I U,
//! the highlight and font colour and the lists — and goes as soon as a key
//! is pressed or the pages are clicked, as Word's does. Its lists preview
//! what the pointer rests on, on the selection (リアルタイムのプレビュー,
//! `word_preview`): their rows are the ribbon's.

use gpui::{
    anchored, deferred, div, point, prelude::*, px, svg, AnyElement, Context, MouseButton, Pixels,
    Point,
};
use jade_web::word::CharacterFormat;

use super::ViewsAction;
use crate::ribbon::{listener, menu_panel, menu_row, Action};
use crate::view::{DocView as DocumentView, Document};
use crate::{i18n, palette as theme};

/// The mini toolbar's own lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MiniPopup {
    Family,
    Size,
    Styles,
}

impl DocumentView {
    /// After the mouse comes up on the pages: the toolbar shows over a
    /// selection the mouse made.
    pub(crate) fn mini_after_mouse_up(&mut self) {
        let selected = self
            .word_ref()
            .and_then(|document| document.selection())
            .is_some_and(|(anchor, focus)| anchor != focus);
        let painting =
            matches!(&self.document, Document::Pages { ribbon, .. } if ribbon.painter.is_some());
        let shown = selected && !painting && !super::modes::read_only(self) && !self.views.focus;
        if shown != self.views.mini {
            self.views.mini = shown;
            self.views.mini_popup = None;
        }
    }

    /// Where the toolbar goes: above the caret at the selection's end, in
    /// window coordinates.
    fn mini_position(&self, zoom: f32) -> Option<Point<Pixels>> {
        let Document::Pages {
            document,
            page_bounds,
            ..
        } = &self.document
        else {
            return None;
        };
        // The selection's moving end, where Word puts the toolbar.
        let (_, focus) = document.selection()?;
        let (x, y, _) = jade_web::word::WordSession::position_metrics(&**document, focus)?;
        let (page, local_y) = document.page_and_offset(y);
        let origin = page_bounds.borrow().get(page).copied().flatten()?.origin;
        Some(point(
            origin.x + px((x as f32 * zoom - 120.0).max(0.0)),
            origin.y + px((local_y as f32 * zoom - 74.0).max(0.0)),
        ))
    }

    /// The toolbar, while it shows.
    pub(crate) fn mini_toolbar(&mut self, zoom: f32, cx: &mut Context<Self>) -> Option<AnyElement> {
        if !self.views.mini {
            return None;
        }
        let selected = self
            .word_ref()?
            .selection()
            .is_some_and(|(anchor, focus)| anchor != focus);
        if !selected {
            return None;
        }
        let at = self.mini_position(zoom)?;
        let state = self.word_format_state();
        let t = i18n::text;
        let family = state
            .font_family
            .clone()
            .unwrap_or_else(|| crate::ribbon::DEFAULT_FAMILY.to_string());
        let size = state.font_size.unwrap_or(crate::ribbon::DEFAULT_SIZE);
        let size_label = |points: f32| {
            if points.fract() == 0.0 {
                format!("{points:.0}")
            } else {
                format!("{points}")
            }
        };
        let styles = match &self.document {
            Document::Pages { ribbon, .. } => ribbon.styles(),
            _ => Vec::new(),
        };
        let list =
            self.views.mini_popup.map(|popup| match popup {
                MiniPopup::Family => crate::view::word_characters::font_family_menu(&family, cx),
                MiniPopup::Size => menu_panel("mini-size-menu")
                    .w(px(72.0))
                    .max_h(px(300.0))
                    .overflow_y_scroll()
                    .children(crate::ribbon::FONT_SIZES.iter().enumerate().map(
                        |(index, points)| {
                            menu_row(
                                ("mini-size", index),
                                size_label(*points),
                                (size - points).abs() < 0.01,
                                Action::FontSize(*points),
                                cx,
                            )
                        },
                    ))
                    .into_any_element(),
                MiniPopup::Styles => menu_panel("mini-styles-menu")
                    .w(px(200.0))
                    .max_h(px(300.0))
                    .overflow_y_scroll()
                    .children(styles.into_iter().enumerate().map(|(index, (id, label))| {
                        let checked = id.is_some() && id == state.style_id;
                        menu_row(("mini-style", index), label, checked, Action::Style(id), cx)
                    }))
                    .into_any_element(),
            });
        let open = self.views.mini_popup;
        let combo = |id: &'static str,
                     value: String,
                     width: f32,
                     popup: MiniPopup,
                     cx: &mut Context<Self>| {
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
                .on_click(listener(
                    Action::Views(ViewsAction::MiniPopup(
                        (open != Some(popup)).then_some(popup),
                    )),
                    cx,
                ))
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
        };
        let icon = |id: &'static str,
                    path: &'static str,
                    tooltip: &'static str,
                    active: bool,
                    action: Action,
                    cx: &mut Context<Self>| {
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
                .tooltip(crate::tooltip::Tooltip::text(tooltip))
                .on_click(listener(action, cx))
                .child(svg().path(path).size_4().text_color(if active {
                    theme::accent()
                } else {
                    theme::text()
                }))
                .into_any_element()
        };
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
                size_label(size),
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
            .child(icon(
                "mini-painter",
                "icons/paintbrush.svg",
                t("painter.title"),
                false,
                Action::FormatPainter(false),
                cx,
            ))
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
}
