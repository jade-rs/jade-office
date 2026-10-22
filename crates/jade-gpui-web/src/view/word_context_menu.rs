//! Right-click menus over a Word page in the browser: the items
//! `jade_web::word::context_menu` lists for text, a table, a picture, a
//! shape or the header or footer band, run through the ribbon's actions.
//! Paste is left out: the browser brings the clipboard only with ⌘V.

use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent,
    Pixels, Point, Window,
};
use jade_web::word::context_menu::{menu_rows, MenuCommand, MenuTarget};
use jade_web::word::page::{Story, WordPage};
use jade_web::word::{TableCommand, WordFormatting};

use super::{DocView, Document, ObjectGrab};
use crate::ribbon::Action;
use crate::{i18n, palette};

/// The open menu: what was clicked and where, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WordMenu {
    pub target: MenuTarget,
    pub at: Point<Pixels>,
}

/// Whether the browser's menu can run `command`.
fn offered(command: MenuCommand) -> bool {
    !matches!(
        command,
        MenuCommand::Paste | MenuCommand::PasteSpecial | MenuCommand::CloseHeaderFooter
    )
}

impl DocView {
    /// A right-click on page `index`: takes hold of the picture or puts the
    /// caret where a left click would, then opens that thing's menu.
    pub(super) fn page_right_click(
        &mut self,
        index: usize,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        let Some((x, y)) = self.page_point(index, event.position) else {
            return;
        };
        let target = self.menu_target_at(index, x, y);
        self.word_menu = Some(WordMenu {
            target,
            at: event.position,
        });
        cx.notify();
    }

    /// What a right-click at document point (`x`, `y`) on page `index` is
    /// over, the selection moved there unless the point is selected.
    pub(super) fn menu_target_at(&mut self, index: usize, x: f64, y: f64) -> MenuTarget {
        let Document::Pages {
            document, object, ..
        } = &mut self.document
        else {
            return MenuTarget::Text;
        };
        let page_y = y - document.document_y(index, 0.0);
        let setup = document.page_setup();
        let to_px = |points: f32| f64::from(points) * 4.0 / 3.0;
        let (_, page_height) = document.page_size();
        if page_y < to_px(setup.margins.top) {
            return MenuTarget::Header(false);
        }
        if page_y > page_height - to_px(setup.margins.bottom) {
            return MenuTarget::Footer(false);
        }
        if let Some(found) = document.object_at(x, y).filter(|found| found.page == index) {
            *object = Some(ObjectGrab {
                object: found,
                from: (x, page_y),
                rect: (found.x, found.y, found.width, found.height),
                resizing: false,
                dragging: false,
            });
            return if found.is_picture() {
                MenuTarget::Picture
            } else {
                MenuTarget::Shape
            };
        }
        *object = None;
        let selected = document
            .selection()
            .is_some_and(|(anchor, focus)| anchor != focus);
        if !selected {
            let _ = document.place_caret_at(x, y);
        }
        if document.format_state().table.is_some() {
            MenuTarget::Table
        } else {
            MenuTarget::Text
        }
    }

    /// Runs a menu command and closes the menu.
    pub(super) fn run_menu_command(
        &mut self,
        command: MenuCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.word_menu = None;
        let action = match command {
            MenuCommand::Cut | MenuCommand::Copy => {
                self.word_copy(command == MenuCommand::Cut, cx);
                return cx.notify();
            }
            MenuCommand::Paste | MenuCommand::PasteSpecial | MenuCommand::CloseHeaderFooter => {
                return cx.notify();
            }
            MenuCommand::Font => {
                super::word_characters::open_font_dialog(self, cx);
                return cx.notify();
            }
            MenuCommand::Paragraph => Action::ParagraphDialog,
            MenuCommand::Bullets => Action::List(true),
            MenuCommand::Numbering => Action::List(false),
            MenuCommand::Hyperlink => Action::InsertLink,
            MenuCommand::NewComment => Action::Review(crate::word_review::ReviewAction::NewComment),
            MenuCommand::InsertRowAbove => Action::Table(TableCommand::InsertRowAbove),
            MenuCommand::InsertRowBelow => Action::Table(TableCommand::InsertRowBelow),
            MenuCommand::InsertColumnLeft => Action::Table(TableCommand::InsertColumnLeft),
            MenuCommand::InsertColumnRight => Action::Table(TableCommand::InsertColumnRight),
            MenuCommand::DeleteRow => Action::Table(TableCommand::DeleteRow),
            MenuCommand::DeleteColumn => Action::Table(TableCommand::DeleteColumn),
            MenuCommand::DeleteTable => Action::Table(TableCommand::DeleteTable),
            MenuCommand::MergeCells => Action::Table(TableCommand::MergeCells),
            MenuCommand::SplitCells => Action::Tables(crate::word_table::TableAction::SplitCells),
            MenuCommand::TableProperties => {
                Action::Tables(crate::word_table::TableAction::Properties(
                    jade_web::word::table::PropertiesTab::Table,
                ))
            }
            MenuCommand::Wrap(wrap) => Action::Wrap(wrap),
            MenuCommand::EditHeader => Action::EditStory(Story::Header),
            MenuCommand::EditFooter => Action::EditStory(Story::Footer),
        };
        self.ribbon_action(action, window, cx);
    }

    /// The open menu, drawn over everything at the pointer.
    pub(super) fn word_menu_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.word_menu?;
        let rows: Vec<_> = menu_rows(menu.target)
            .into_iter()
            .filter(|row| row.is_none_or(|(command, _)| offered(command)))
            .collect();
        let items = rows.into_iter().enumerate().map(|(index, row)| match row {
            None => div()
                .my_1()
                .h(px(1.0))
                .bg(palette::border())
                .into_any_element(),
            Some((command, label)) => div()
                .id(("word-menu-item", index))
                .px_3()
                .py_1()
                .cursor_pointer()
                .hover(|item| item.bg(palette::hovered()))
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
                            .border_color(palette::border())
                            .bg(palette::panel())
                            .shadow_lg()
                            .text_xs()
                            .text_color(palette::text())
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
