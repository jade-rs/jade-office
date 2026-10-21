//! Right-click menus over a Word page (`jade_office_model::word::context_menu`
//! lists the items): over text, a table, a picture, a shape, or the header
//! or footer band.

use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent,
    Pixels, Point, Window,
};
use jade_office_model::word::context_menu::{menu_rows, MenuCommand, MenuTarget};
use jade_office_model::word::page::Story;
use jade_office_model::word::TableCommand;

use crate::ribbon::Action;
use crate::{i18n, theme, DocumentView};

/// The open menu: what was clicked and where, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WordMenu {
    pub target: MenuTarget,
    pub at: Point<Pixels>,
    /// The page clicked, for opening its header or footer.
    pub page: usize,
}

impl DocumentView {
    /// A right-click on page `page_index`: picks what is under the pointer
    /// the way a left click would (the picture, the cell, the caret
    /// position), then opens that thing's menu.
    pub(crate) fn right_click_page(
        &mut self,
        page_index: usize,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(Some(bounds)) = self.page_bounds.borrow().get(page_index).copied() else {
            return;
        };
        let zoom = f64::from(self.zoom);
        let local_x = f64::from(event.position.x - bounds.origin.x) / zoom;
        let local_y = f64::from(event.position.y - bounds.origin.y) / zoom;
        let target = self.menu_target_at(page_index, local_x, local_y);
        self.word_menu = Some(WordMenu {
            target,
            at: event.position,
            page: page_index,
        });
        cx.notify();
    }

    /// What a right-click at a point on page `page` is over, moving the
    /// selection there unless the point is already selected.
    fn menu_target_at(&mut self, page: usize, x: f64, y: f64) -> MenuTarget {
        if self.editing_story() && !self.document.in_story(page, y) || !self.editing_story() {
            if let Some(role) = self.margin_story(page, y) {
                let open = self.editing_story();
                return match role {
                    Story::Header => MenuTarget::Header(open),
                    Story::Footer => MenuTarget::Footer(open),
                };
            }
        }
        let document_y = self.document.document_y(page, y);
        if let Some(object) = self.document.object_at(x, document_y) {
            self.selected_object = Some(object);
            return if crate::placed_object(&object).is_picture() {
                MenuTarget::Picture
            } else {
                MenuTarget::Shape
            };
        }
        self.selected_object = None;
        if let Some(position) = self.document.position_at(x, document_y) {
            let inside_selection = self.selection_anchor.is_some_and(|anchor| {
                let (start, end) = if anchor <= self.insert_at {
                    (anchor, self.insert_at)
                } else {
                    (self.insert_at, anchor)
                };
                start != end && start <= position && position <= end
            });
            if !inside_selection {
                self.insert_at = position;
                self.selection_anchor = None;
                self.drag_anchor = None;
                self.caret = self.place_caret_near(position, document_y);
                self.refresh_caret_page_image();
            }
        }
        if self.editing_story() {
            return MenuTarget::Text;
        }
        if self.format_state().table.is_some() {
            MenuTarget::Table
        } else {
            MenuTarget::Text
        }
    }

    /// Runs a menu command and closes the menu.
    pub(crate) fn run_menu_command(
        &mut self,
        command: MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let menu = self.word_menu.take();
        match command {
            MenuCommand::Cut => window.dispatch_action(Box::new(crate::Cut), cx),
            MenuCommand::Copy => window.dispatch_action(Box::new(crate::Copy), cx),
            MenuCommand::Paste => window.dispatch_action(Box::new(crate::Paste), cx),
            MenuCommand::PasteSpecial => {
                crate::word_characters::open_paste_special(self, cx);
            }
            MenuCommand::Font => crate::word_characters::open_font_dialog(self, cx),
            MenuCommand::Paragraph => self.open_paragraph_dialog(cx),
            MenuCommand::Bullets => self.run_ribbon(Action::List(true), cx),
            MenuCommand::Numbering => self.run_ribbon(Action::List(false), cx),
            MenuCommand::Hyperlink => self.open_link_dialog(cx),
            MenuCommand::NewComment => self.add_comment(cx),
            MenuCommand::InsertRowAbove => self.table_command(TableCommand::InsertRowAbove, cx),
            MenuCommand::InsertRowBelow => self.table_command(TableCommand::InsertRowBelow, cx),
            MenuCommand::InsertColumnLeft => self.table_command(TableCommand::InsertColumnLeft, cx),
            MenuCommand::InsertColumnRight => {
                self.table_command(TableCommand::InsertColumnRight, cx)
            }
            MenuCommand::DeleteRow => self.table_command(TableCommand::DeleteRow, cx),
            MenuCommand::DeleteColumn => self.table_command(TableCommand::DeleteColumn, cx),
            MenuCommand::DeleteTable => self.table_command(TableCommand::DeleteTable, cx),
            MenuCommand::MergeCells => self.table_command(TableCommand::MergeCells, cx),
            MenuCommand::SplitCells => self.run_ribbon(
                Action::Tables(jade_office_model::word::table_ui::TableAction::SplitCells),
                cx,
            ),
            MenuCommand::TableProperties => self.run_ribbon(
                Action::Tables(jade_office_model::word::table_ui::TableAction::Properties(
                    jade_office_model::word::table::PropertiesTab::Table,
                )),
                cx,
            ),
            MenuCommand::Wrap(wrap) => self.wrap_picture(wrap, cx),
            MenuCommand::EditHeader | MenuCommand::EditFooter => {
                let role = if command == MenuCommand::EditHeader {
                    Story::Header
                } else {
                    Story::Footer
                };
                let page = menu.map_or(0, |menu| menu.page);
                self.open_story(page, role, cx);
            }
            MenuCommand::CloseHeaderFooter => {
                if let Err(error) = self.close_story_editing() {
                    self.table_status = Some(error.into());
                }
            }
        }
        cx.notify();
    }

    fn table_command(&mut self, command: TableCommand, cx: &mut Context<Self>) {
        self.run_ribbon(Action::Table(command), cx);
    }

    /// The open menu, drawn over everything at the pointer.
    pub(crate) fn word_menu_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.word_menu?;
        let rows = menu_rows(menu.target);
        let items = rows.into_iter().enumerate().map(|(index, row)| match row {
            None => div()
                .my_1()
                .h(px(1.0))
                .bg(theme::border())
                .into_any_element(),
            Some((command, label)) => div()
                .id(("word-menu-item", index))
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(theme::hovered()))
                .child(i18n::text(label))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |view, _event, window, cx| {
                    view.run_menu_command(command, window, cx)
                }))
                .into_any_element(),
        });
        Some(
            deferred(
                anchored()
                    .position(menu.at)
                    .snap_to_window_with_margin(px(8.0))
                    .child(
                        div()
                            .id("word-menu")
                            .min_w(px(200.0))
                            .py_1()
                            .rounded_md()
                            .border_1()
                            .border_color(theme::border())
                            .bg(theme::panel())
                            .shadow_lg()
                            .text_xs()
                            .text_color(theme::text())
                            .occlude()
                            .on_mouse_down_out(cx.listener(|view, _, _, cx| {
                                view.word_menu = None;
                                cx.notify();
                            }))
                            .children(items),
                    ),
            )
            .with_priority(2)
            .into_any_element(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ribbon::Action;
    use gpui::Focusable;
    use jade_office_model::word::WordSession;

    /// A Japanese blank in a window with a paragraph of text and a 2 × 2
    /// table under it.
    fn with_text_and_table(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &mut gpui::VisualTestContext) {
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::Japanese,
        );
        let (view, cx) = cx.add_window_view(|_window, cx| {
            crate::untitled_view(&bytes, "メニュー.docx", cx).unwrap()
        });
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        cx.simulate_input("本文");
        view.update(cx, |view, cx| {
            view.run_ribbon(Action::InsertTable(2, 2), cx)
        });
        cx.run_until_parked();
        (view, cx)
    }

    #[gpui::test]
    fn a_right_click_opens_the_menu_for_what_is_under_it(cx: &mut gpui::TestAppContext) {
        let (view, cx) = with_text_and_table(cx);
        view.update(cx, |view, cx| {
            // The top margin is the header's band, the bottom the footer's.
            assert_eq!(
                view.menu_target_at(0, 300.0, 10.0),
                MenuTarget::Header(false)
            );
            let (_, height) = view.document.layout_page_size();
            assert_eq!(
                view.menu_target_at(0, 300.0, height - 10.0),
                MenuTarget::Footer(false)
            );
            // The caret's paragraph line, then a cell of the table.
            let caret = view.caret.expect("a caret");
            let table_target = view.menu_target_at(0, caret.x + 2.0, caret.document_y + 2.0);
            assert_eq!(table_target, MenuTarget::Table);
            let outside = (0..view.document.paragraph_count())
                .find(|paragraph| view.document.cell_id_for_paragraph(*paragraph).is_none())
                .expect("a paragraph outside the table");
            let first = view
                .document
                .place_caret(outside, 0)
                .expect("caret outside the table");
            assert_eq!(
                view.menu_target_at(0, first.x + 2.0, first.document_y + 2.0),
                MenuTarget::Text
            );
            cx.notify();
        });
        cx.run_until_parked();
        // The header item opens the header for editing.
        cx.update(|window, cx| {
            view.update(cx, |view, cx| {
                view.word_menu = Some(WordMenu {
                    target: MenuTarget::Header(false),
                    at: gpui::point(px(40.0), px(40.0)),
                    page: 0,
                });
                view.run_menu_command(MenuCommand::EditHeader, window, cx);
                assert!(view.editing_story());
                assert!(view.word_menu.is_none());
            })
        });
    }
}
