//! Right-click menus over a Word page in the browser: the entries
//! `jade_web::word::context_menu::menu` lists — the one list both shells
//! draw — for text, a table, a picture, a shape or the header or footer
//! band, run through the ribbon's actions. What the browser cannot do from
//! a menu is left out: pasting (the clipboard comes only with ⌘V), and the
//! field and link items the native shell runs itself.

use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent,
    Pixels, Point, Window,
};
use jade_web::word::context_menu::{menu, ContextCommand, ContextEntry, ContextTarget};
use jade_web::word::page::{Story, WordPage};
use jade_web::word::picture::Wrap;
use jade_web::word::table_ui::TableAction;
use jade_web::word::{TableCommand, WordFormatting};

use super::{DocView, Document, ObjectGrab};
use crate::ribbon::Action;
use crate::{i18n, palette};

/// The open menu: what was clicked and where, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WordMenu {
    pub target: ContextTarget,
    pub at: Point<Pixels>,
}

/// Whether the browser's menu can run `command`.
fn offered(command: ContextCommand) -> bool {
    use ContextCommand as C;
    !matches!(
        command,
        C::Paste
            | C::PasteKeepSource
            | C::PasteMerge
            | C::PasteTextOnly
            | C::PasteSpecial
            | C::SmartLookup
            | C::EditLink
            | C::OpenLink
            | C::RemoveLink
            | C::UpdateField
            | C::EditField
            | C::ToggleFieldCodes
            | C::CloseHeaderFooter
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
    pub(super) fn menu_target_at(&mut self, index: usize, x: f64, y: f64) -> ContextTarget {
        let Document::Pages {
            document, object, ..
        } = &mut self.document
        else {
            return ContextTarget::default();
        };
        let band = ContextTarget {
            header_footer: true,
            ..ContextTarget::default()
        };
        let page_y = y - document.document_y(index, 0.0);
        let setup = document.page_setup();
        let to_px = |points: f32| f64::from(points) * 4.0 / 3.0;
        let (_, page_height) = document.page_size();
        if page_y < to_px(setup.margins.top) || page_y > page_height - to_px(setup.margins.bottom) {
            return band;
        }
        if let Some(found) = document.object_at(x, y).filter(|found| found.page == index) {
            *object = Some(ObjectGrab {
                object: found,
                from: (x, page_y),
                rect: (found.x, found.y, found.width, found.height),
                resizing: false,
                dragging: false,
            });
            return ContextTarget {
                picture: found.is_picture(),
                shape: !found.is_picture(),
                ..ContextTarget::default()
            };
        }
        *object = None;
        let selected = document
            .selection()
            .is_some_and(|(anchor, focus)| anchor != focus);
        if !selected {
            let _ = document.place_caret_at(x, y);
        }
        ContextTarget {
            selection: selected,
            table: document.format_state().table.is_some(),
            ..ContextTarget::default()
        }
    }

    /// Runs a menu command and closes the menu.
    pub(super) fn run_menu_command(
        &mut self,
        command: ContextCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use ContextCommand as C;
        self.word_menu = None;
        let table = |command: TableCommand| Action::Table(command);
        let tables = |action: TableAction| Action::Tables(action);
        let action = match command {
            C::Cut | C::Copy => {
                self.word_copy(command == C::Cut, cx);
                return cx.notify();
            }
            C::Font => {
                super::word_characters::open_font_dialog(self, cx);
                return cx.notify();
            }
            C::Paragraph => Action::ParagraphDialog,
            C::Bullets => Action::List(true),
            C::Numbering => Action::List(false),
            C::Link => Action::InsertLink,
            C::NewComment => Action::Review(crate::word_review::ReviewAction::NewComment),
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
            C::InsertCells => tables(TableAction::InsertCells),
            C::DeleteCells => tables(TableAction::DeleteCells),
            C::SplitCells => tables(TableAction::SplitCells),
            C::TableBorders => tables(TableAction::Borders),
            C::TextDirection => tables(TableAction::TextDirection(None)),
            C::AutoFitContents => tables(TableAction::AutoFit(
                jade_web::word::table::AutoFit::Contents,
            )),
            C::AutoFitWindow => {
                tables(TableAction::AutoFit(jade_web::word::table::AutoFit::Window))
            }
            C::TableProperties => tables(TableAction::Properties(
                jade_web::word::table::PropertiesTab::Table,
            )),
            C::WrapInline => Action::Wrap(Wrap::Inline),
            C::WrapSquare => Action::Wrap(Wrap::Square),
            C::WrapTight => Action::Wrap(Wrap::Tight),
            C::WrapThrough => Action::Wrap(Wrap::Through),
            C::WrapTopBottom => Action::Wrap(Wrap::TopAndBottom),
            C::WrapBehind => Action::Wrap(Wrap::BehindText),
            C::WrapInFront => Action::Wrap(Wrap::InFrontOfText),
            C::EditHeader => Action::EditStory(Story::Header),
            C::EditFooter => Action::EditStory(Story::Footer),
            // Not offered in the browser (`offered`).
            _ => return cx.notify(),
        };
        self.ribbon_action(action, window, cx);
    }

    /// The open menu, drawn over everything at the pointer.
    pub(super) fn word_menu_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.word_menu?;
        // Captions and separators stay only where an offered item follows.
        let mut entries: Vec<ContextEntry> = Vec::new();
        for entry in menu(open.target) {
            match entry {
                ContextEntry::Item(_, command) if !offered(command) => {}
                ContextEntry::Separator
                    if entries.is_empty()
                        || matches!(
                            entries.last(),
                            Some(ContextEntry::Separator | ContextEntry::Caption(_))
                        ) => {}
                entry => entries.push(entry),
            }
        }
        while matches!(
            entries.last(),
            Some(ContextEntry::Separator | ContextEntry::Caption(_))
        ) {
            entries.pop();
        }
        let items = entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                ContextEntry::Separator => div()
                    .my_1()
                    .h(px(1.0))
                    .bg(palette::border())
                    .into_any_element(),
                ContextEntry::Caption(label) => div()
                    .px_3()
                    .pt_1()
                    .text_color(palette::dim())
                    .child(i18n::text(label))
                    .into_any_element(),
                ContextEntry::Item(label, command) => div()
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
                    .position(open.at)
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
