//! The views that lay the document out anew rather than show its pages:
//! Web レイアウト (the text wrapped to the window, in strips that close up
//! into one long sheet) and 閲覧モード (screen-sized columns turned side by
//! side, no ribbon, nothing editable). Both hand the engine a page box of
//! the view's own (`ViewGeometry`) and give the sections' pages back on the
//! way out.

use gpui::{div, prelude::*, px, AnyElement, Context};
use jade_docx_editor::canvas_renderer::ViewGeometry;
use jade_office_model::word::view::{PageMovement, ViewKind, ZoomFit, PAGE_PADDING};

use crate::word_dialog::on;
use crate::{i18n, theme, DocumentView};

/// Web レイアウト's strips: each this tall, closing up one under another.
pub const WEB_STRIP_HEIGHT: f64 = 1600.0;
/// Its text's distance from the window's sides.
pub const WEB_MARGIN: f64 = 24.0;
/// 閲覧モード's room round a column and between two.
pub const READ_MARGIN: f64 = 48.0;
pub const READ_GAP: f32 = 32.0;
/// The widest a column of 閲覧モード reads comfortably at.
pub const READ_MAX_COLUMN: f64 = 760.0;

/// The page box `kind` lays the document out in, for a window `area` wide
/// and high at `zoom`; `None` for the views that keep the sections' pages.
pub fn geometry(kind: ViewKind, area: (f32, f32), zoom: f32) -> Option<ViewGeometry> {
    let zoom = f64::from(zoom.max(0.1));
    let width = f64::from(area.0) / zoom;
    let height = f64::from(area.1) / zoom;
    match kind {
        ViewKind::Web => Some(ViewGeometry {
            page_width: (width - 2.0 * f64::from(PAGE_PADDING) / zoom)
                .max(240.0)
                .floor(),
            page_height: WEB_STRIP_HEIGHT,
            margin_top: 0.0,
            margin_right: WEB_MARGIN,
            margin_bottom: 0.0,
            margin_left: WEB_MARGIN,
            hide_page_chrome: true,
        }),
        ViewKind::Read => {
            let room = width - 2.0 * f64::from(PAGE_PADDING) / zoom;
            let two = room >= 2.0 * 520.0;
            let column = if two {
                ((room - f64::from(READ_GAP) / zoom) / 2.0).min(READ_MAX_COLUMN)
            } else {
                room.min(READ_MAX_COLUMN)
            };
            Some(ViewGeometry {
                page_width: column.max(240.0).floor(),
                page_height: (height - 2.0 * f64::from(PAGE_PADDING) / zoom)
                    .max(240.0)
                    .floor(),
                margin_top: READ_MARGIN,
                margin_right: READ_MARGIN,
                margin_bottom: READ_MARGIN,
                margin_left: READ_MARGIN,
                hide_page_chrome: true,
            })
        }
        _ => None,
    }
}

/// Whether two boxes are close enough not to lay the document out again
/// for: a window dragged a pixel wider is not worth a re-flow.
fn close(a: Option<ViewGeometry>, b: Option<ViewGeometry>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => {
            (a.page_width - b.page_width).abs() < 6.0
                && (a.page_height - b.page_height).abs() < 6.0
                && a.margin_left == b.margin_left
                && a.margin_top == b.margin_top
        }
        _ => false,
    }
}

/// Gives the document the page box its view wants, as the window is now.
pub fn follow(view: &mut DocumentView) {
    let kind = view.views.kind;
    let wanted = match kind {
        ViewKind::Web | ViewKind::Read => {
            let Some(area) = super::area(view) else {
                return;
            };
            geometry(kind, area, view.zoom)
        }
        ViewKind::Outline => super::outline::geometry(view),
        _ => None,
    };
    let current = view.document.main().view_geometry();
    if close(current, wanted) {
        return;
    }
    if view.editing_story() {
        // A header open over the pages goes first: the views have none.
        if let Err(error) = view.close_story_editing() {
            view.table_status = Some(error.into());
            return;
        }
    }
    match view.document.set_view_geometry(wanted) {
        Ok(_) => {
            view.caret = view
                .document
                .place_caret(view.insert_at.0, view.insert_at.1);
            view.rebuild_all_images();
        }
        Err(error) => view.table_status = Some(error.into()),
    }
}

/// Entering a view: how its pages move and how far they zoom.
pub fn enter(view: &mut DocumentView, kind: ViewKind) {
    match kind {
        ViewKind::Read => {
            view.views.movement = PageMovement::SideToSide;
            view.views.fit = ZoomFit::Custom;
            view.views.mini = false;
        }
        ViewKind::Web | ViewKind::Outline => {
            view.views.movement = PageMovement::Vertical;
            view.views.fit = ZoomFit::Custom;
        }
        _ => {
            if view.views.movement == PageMovement::SideToSide
                && view.document.main().view_geometry().is_some()
            {
                view.views.movement = PageMovement::Vertical;
            }
        }
    }
}

/// Whether the ribbon and the page list give way: フォーカス and 閲覧モード.
pub fn hides_chrome(view: &DocumentView) -> bool {
    view.views.focus || view.views.kind == ViewKind::Read
}

/// 閲覧モード's bar over its columns: where the reader is, and the way back
/// to editing.
pub fn read_bar(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    if view.views.kind != ViewKind::Read {
        return None;
    }
    let pages = view.images.len();
    Some(
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap_3()
            .px_4()
            .py_2()
            .bg(theme::panel())
            .border_b_1()
            .border_color(theme::border())
            .text_sm()
            .child(
                div()
                    .id("read-back")
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .border_1()
                    .border_color(theme::border())
                    .cursor_pointer()
                    .hover(|button| button.bg(theme::hovered()))
                    .on_click(on(cx, |view, cx| {
                        super::run(view, super::ViewAction::SetView(ViewKind::Print), cx)
                    }))
                    .child(i18n::text("views.read.edit")),
            )
            .child(
                div()
                    .id("read-previous")
                    .px_2()
                    .cursor_pointer()
                    .on_click(on(cx, |view, cx| view.go_to_page(-1, cx)))
                    .child("◀"),
            )
            .child(div().text_color(theme::text_dim()).child(i18n::format(
                "views.read.position",
                &[("current", &(view.current_page + 1)), ("count", &pages)],
            )))
            .child(
                div()
                    .id("read-next")
                    .px_2()
                    .cursor_pointer()
                    .on_click(on(cx, |view, cx| view.go_to_page(1, cx)))
                    .child("▶"),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme::text_faint())
                    .child(i18n::text("views.read.keys")),
            )
            .into_any_element(),
    )
}

/// 閲覧モード's keys: ← → and the space bar turn the columns, Escape goes
/// back to editing. `true` when taken.
pub fn read_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    if view.views.kind != ViewKind::Read {
        return false;
    }
    match key {
        "right" | "down" | "space" | "pagedown" => view.go_to_page(1, cx),
        "left" | "up" | "pageup" => view.go_to_page(-1, cx),
        "escape" => super::run(view, super::ViewAction::SetView(ViewKind::Print), cx),
        _ => return false,
    }
    true
}

/// Whether `kind` sets its pages out itself.
pub fn arranges(kind: ViewKind) -> bool {
    matches!(kind, ViewKind::Web | ViewKind::Read | ViewKind::Outline)
}

/// How the pages are set out in the views of this module.
pub fn arrange(pages: gpui::Stateful<gpui::Div>, kind: ViewKind) -> gpui::Stateful<gpui::Div> {
    match kind {
        // One long sheet: the strips touch, white to the window's sides.
        ViewKind::Web => pages
            .flex_col()
            .items_center()
            .gap_0()
            .p_0()
            .bg(theme::paper()),
        // Columns side by side on a quiet ground.
        ViewKind::Read => pages
            .flex_row()
            .items_center()
            .gap(px(READ_GAP))
            .p_8()
            .bg(theme::canvas()),
        ViewKind::Outline => pages
            .flex_col()
            .items_start()
            .gap_0()
            .p_0()
            .bg(theme::paper()),
        _ => pages,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_views::{run, ViewAction};

    #[test]
    fn the_boxes_follow_the_window() {
        let web = geometry(ViewKind::Web, (1200.0, 800.0), 1.0).unwrap();
        assert_eq!(web.page_width, 1200.0 - 2.0 * f64::from(PAGE_PADDING));
        let zoomed = geometry(ViewKind::Web, (1200.0, 800.0), 2.0).unwrap();
        assert!(zoomed.page_width < web.page_width / 1.9);
        let read = geometry(ViewKind::Read, (1600.0, 900.0), 1.0).unwrap();
        assert!(read.page_width <= READ_MAX_COLUMN);
        assert!(read.page_height < 900.0);
        assert!(geometry(ViewKind::Print, (1200.0, 800.0), 1.0).is_none());
    }

    /// Web レイアウト wraps the text to the window; 印刷レイアウト gives the
    /// pages back.
    #[gpui::test]
    fn web_layout_wraps_to_the_window(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            super::super::AREA.with(|area| area.set(Some((1000.0, 800.0))));
            let page = view.document.layout_page_size();
            run(view, ViewAction::SetView(ViewKind::Web), cx);
            let (width, height) = view.document.layout_page_size();
            assert_eq!(height, WEB_STRIP_HEIGHT);
            assert!(width < page.0 + 400.0 && width > 300.0, "{width}");
            // The window grows: the text follows.
            super::super::AREA.with(|area| area.set(Some((1400.0, 800.0))));
            follow(view);
            assert!(view.document.layout_page_size().0 > width);
            run(view, ViewAction::SetView(ViewKind::Print), cx);
            assert_eq!(view.document.layout_page_size(), page);
        });
    }

    /// 閲覧モード: columns side by side, the ribbon gone, nothing editable,
    /// → turns a column and Escape goes back.
    #[gpui::test]
    fn read_mode_turns_columns(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            super::super::AREA.with(|area| area.set(Some((1400.0, 900.0))));
            run(view, ViewAction::SetView(ViewKind::Read), cx);
            assert!(hides_chrome(view));
            assert!(super::super::modes::read_only(view));
            assert_eq!(view.views.movement, PageMovement::SideToSide);
            assert!(view.images.len() > 1);
        });
        cx.run_until_parked();
        let before = view.read_with(cx, |view, _| view.document.paragraph_text(view.insert_at.0));
        cx.simulate_input("x");
        view.read_with(cx, |view, _| {
            assert_eq!(view.document.paragraph_text(view.insert_at.0), before)
        });
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| {
            assert_eq!(view.views.kind, ViewKind::Print);
            assert!(view.document.main().view_geometry().is_none());
        });
    }
}
