//! The right-click (⌃-click) menu over the pages, with Word's items for
//! what is under the pointer (`jade_office_model::word::context_menu`).

use gpui::{
    anchored, deferred, div, point, prelude::*, px, AnyElement, Context, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString,
};
use jade_office_model::word::clipboard::PasteMode;
use jade_office_model::word::context_menu::{menu, ContextCommand, ContextEntry, ContextTarget};
use jade_office_model::word::page::Story;
use jade_office_model::word::picture::Wrap;
use jade_office_model::word::table_ui::TableAction;
use jade_office_model::word::{TableCommand, WordLinks};

use crate::ribbon::Action;
use crate::word_dialog::on;
use crate::{i18n, theme, DocumentView};

/// The menu while it is open: where, for what, and the point clicked in
/// the document's coordinates.
#[derive(Clone, Debug)]
pub struct OpenMenu {
    pub at: Point<Pixels>,
    pub target: ContextTarget,
    pub point: (f64, f64),
}

/// A right-click on page `page_index`: the caret goes there (unless it is
/// in the selection) and the menu opens for what is under it.
pub fn open(
    view: &mut DocumentView,
    page_index: usize,
    event: &MouseDownEvent,
    cx: &mut Context<DocumentView>,
) {
    // On a misspelling, its suggestions (`word_proofing`), as Word puts them
    // first.
    crate::word_proofing::right_click(view, page_index, event, cx);
    if view.review.proofing.menu.is_some() {
        return;
    }
    let Some(Some(bounds)) = view.page_bounds.borrow().get(page_index).copied() else {
        return;
    };
    view.views.mini = false;
    let zoom = f64::from(view.zoom);
    let x = f64::from(event.position.x - bounds.origin.x) / zoom;
    let local_y = f64::from(event.position.y - bounds.origin.y) / zoom;
    let y = view.document.document_y(page_index, local_y);
    let mut target = ContextTarget {
        header_footer: view.editing_story(),
        ..ContextTarget::default()
    };
    if let Some(object) = view.document.object_at(x, y) {
        view.selected_object = Some(object);
        let placed = crate::placed_object(&object);
        target.picture = placed.is_picture();
        target.shape = !target.picture;
    } else if let Some(at) = view.document.position_at(x, y) {
        let inside = view.document.selection().is_some_and(|(anchor, focus)| {
            let (start, end) = if anchor <= focus {
                (anchor, focus)
            } else {
                (focus, anchor)
            };
            at >= start && at <= end
        });
        if !inside {
            view.selection_anchor = None;
            view.insert_at = at;
            view.caret = view.document.place_caret(at.0, at.1);
            view.rebuild_all_images();
        }
        target.selection = view.document.selection().is_some();
        target.table = view.document.caret_table_cell().is_some();
        target.link = view.document.hyperlink_at_point(x, y).is_some();
        target.field = {
            use jade_office_model::word::fields::WordFields;
            !view.document.selected_fields().is_empty()
        };
    }
    view.views.context = Some(OpenMenu {
        at: event.position,
        target,
        point: (x, y),
    });
    cx.notify();
}

/// Runs a menu item.
pub fn run(view: &mut DocumentView, command: ContextCommand, cx: &mut Context<DocumentView>) {
    let opened = view.views.context.take();
    use ContextCommand as C;
    let table = |command: TableCommand| Action::Table(command);
    let action = match command {
        C::Cut => return view.cut(cx),
        C::Copy => return view.copy(cx),
        C::Paste => return view.paste(cx),
        C::PasteKeepSource => {
            return crate::word_characters::paste_with(view, PasteMode::KeepSource, cx)
        }
        C::PasteMerge => {
            return crate::word_characters::paste_with(view, PasteMode::MergeFormatting, cx)
        }
        C::PasteTextOnly => {
            return crate::word_characters::paste_with(view, PasteMode::TextOnly, cx)
        }
        C::Font => return crate::word_characters::open_font_dialog(view, cx),
        C::Paragraph => return view.open_paragraph_dialog(cx),
        C::Bullets => Action::List(true),
        C::Numbering => Action::List(false),
        C::SmartLookup => return view.open_find_bar(cx),
        C::NewComment => return view.add_comment(cx),
        C::Link | C::EditLink => return view.open_link_dialog(cx),
        C::OpenLink => {
            if let Some(opened) = opened {
                view.follow_link_at(opened.point.0, opened.point.1, cx);
            }
            return;
        }
        C::RemoveLink => {
            let draft = view.document.link_draft();
            let display = draft.display.clone();
            let done = view.document.apply_link(&draft, &display, "").map(Some);
            return view.after_reference_edit(done, cx);
        }
        C::UpdateField => return view.update_fields(cx),
        C::EditField => return view.open_edit_field_dialog(cx),
        C::ToggleFieldCodes => return view.toggle_selected_field_codes(cx),
        C::InsertRowAbove => table(TableCommand::InsertRowAbove),
        C::InsertRowBelow => table(TableCommand::InsertRowBelow),
        C::InsertColumnLeft => table(TableCommand::InsertColumnLeft),
        C::InsertColumnRight => table(TableCommand::InsertColumnRight),
        C::DeleteRow => table(TableCommand::DeleteRow),
        C::DeleteColumn => table(TableCommand::DeleteColumn),
        C::DeleteTable => table(TableCommand::DeleteTable),
        C::MergeCells => table(TableCommand::MergeCells),
        C::DistributeRows => table(TableCommand::DistributeRows),
        C::DistributeColumns => table(TableCommand::DistributeColumns),
        C::InsertCells => Action::Tables(TableAction::InsertCells),
        C::DeleteCells => Action::Tables(TableAction::DeleteCells),
        C::SplitCells => Action::Tables(TableAction::SplitCells),
        C::TableBorders => Action::Tables(TableAction::Borders),
        C::TextDirection => Action::Tables(TableAction::TextDirection(None)),
        C::AutoFitContents => Action::Tables(TableAction::AutoFit(
            jade_office_model::word::table::AutoFit::Contents,
        )),
        C::AutoFitWindow => Action::Tables(TableAction::AutoFit(
            jade_office_model::word::table::AutoFit::Window,
        )),
        C::TableProperties => Action::Tables(TableAction::Properties(
            jade_office_model::word::table::PropertiesTab::Table,
        )),
        C::WrapInline => {
            Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(Wrap::Inline))
        }
        C::WrapSquare => {
            Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(Wrap::Square))
        }
        C::WrapTight => Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(Wrap::Tight)),
        C::WrapThrough => Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(
            Wrap::Through,
        )),
        C::WrapTopBottom => Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(
            Wrap::TopAndBottom,
        )),
        C::WrapBehind => Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(
            Wrap::BehindText,
        )),
        C::WrapInFront => Action::Object(crate::word_objects_ribbon::ObjectAction::Wrap(
            Wrap::InFrontOfText,
        )),
        C::EditHeader => Action::EditStory(Story::Header),
        C::EditFooter => Action::EditStory(Story::Footer),
        C::CloseHeaderFooter => Action::Story(crate::word_story::StoryAction::Close),
    };
    view.run_ribbon(action, cx);
}

/// The menu, while it is open, with a backdrop that closes it.
pub fn overlay(view: &DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let opened = view.views.context.as_ref()?;
    let entries = menu(opened.target);
    let rows = entries
        .into_iter()
        .enumerate()
        .map(|(index, entry)| match entry {
            ContextEntry::Separator => div()
                .my_0p5()
                .h(px(1.0))
                .bg(theme::border())
                .into_any_element(),
            ContextEntry::Caption(key) => div()
                .px_2()
                .pt_1()
                .text_color(theme::text_faint())
                .child(i18n::text(key))
                .into_any_element(),
            ContextEntry::Item(key, command) => div()
                .id(("context-item", index))
                .h(px(22.0))
                .px_3()
                .flex()
                .items_center()
                .rounded_sm()
                .whitespace_nowrap()
                .cursor_pointer()
                .hover(|row| row.bg(theme::hovered()))
                .on_click(on(cx, move |view, cx| run(view, command, cx)))
                .child(SharedString::from(i18n::text(key)))
                .into_any_element(),
        });
    let menu = div()
        .id("context-menu")
        .occlude()
        .min_w(px(200.0))
        .max_h(px(560.0))
        .overflow_y_scroll()
        .p_1()
        .flex()
        .flex_col()
        .rounded_md()
        .bg(theme::panel())
        .border_1()
        .border_color(theme::border())
        .shadow_lg()
        .text_xs()
        .text_color(theme::text())
        .children(rows);
    fn close(
        cx: &Context<DocumentView>,
    ) -> impl Fn(&MouseDownEvent, &mut gpui::Window, &mut gpui::App) + 'static {
        on(cx, |view: &mut DocumentView, cx| {
            view.views.context = None;
            cx.notify();
        })
    }
    Some(
        div()
            .child(
                deferred(
                    anchored().position(point(px(0.0), px(0.0))).child(
                        div()
                            .id("context-backdrop")
                            .size(px(10000.0))
                            .occlude()
                            .on_mouse_down(MouseButton::Left, close(cx))
                            .on_mouse_down(MouseButton::Right, close(cx)),
                    ),
                )
                .with_priority(3),
            )
            .child(
                deferred(
                    anchored()
                        .position(opened.at)
                        .snap_to_window_with_margin(px(8.0))
                        .child(menu),
                )
                .with_priority(4),
            )
            .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A right-click in a table opens a menu with the table's items; one of
    /// them inserts a row.
    #[gpui::test]
    fn the_table_menu_inserts_a_row(cx: &mut gpui::TestAppContext) {
        let (view, cx) = crate::word_views::tests::open_contract(cx);
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::InsertTable(2, 2), cx);
            let caret = view.caret.expect("caret");
            let (page, local_y) = view.document.page_and_offset(caret.document_y);
            let bounds = view.page_bounds.borrow()[page].expect("laid out");
            let zoom = view.zoom;
            let event = MouseDownEvent {
                button: MouseButton::Right,
                position: point(
                    bounds.origin.x + px((caret.x as f32 + 2.0) * zoom),
                    bounds.origin.y + px((local_y + caret.height / 2.0) as f32 * zoom),
                ),
                modifiers: gpui::Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            };
            open(view, page, &event, cx);
            let opened = view.views.context.clone().expect("the menu is open");
            assert!(opened.target.table, "{:?}", opened.target);
            assert!(overlay(view, cx).is_some());
            run(view, ContextCommand::InsertRowBelow, cx);
            assert!(view.views.context.is_none());
            assert_eq!(view.format_state().table.unwrap().rows, 3);
        });
    }
}
