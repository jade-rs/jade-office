//! Right-click menus over a Word page in the browser: the entries
//! `jade_web::word::context_menu::menu` lists — the one list both shells
//! draw — for text, a link, a field, a table, a picture, a shape or the
//! header or footer, run through the ribbon's actions and the dialogs the
//! native app opens from them. The paste items read the clipboard on the
//! click (a user gesture, which the browser's clipboard asks for).

use gpui::{
    anchored, deferred, div, prelude::*, px, AnyElement, Context, MouseButton, MouseDownEvent,
    Pixels, Point, Window,
};
use jade_web::word::clipboard::PasteMode;
use jade_web::word::context_menu::{menu, ContextCommand, ContextEntry, ContextTarget};
use jade_web::word::page::{Story, WordPage};
use jade_web::word::picture::Wrap;
use jade_web::word::table_ui::TableAction;
use jade_web::word::{TableCommand, WordFormatting, WordLinks};

use super::{DocView, Document, ObjectGrab};
use crate::ribbon::Action;
use crate::{i18n, palette};

/// The open menu: what was clicked and where, in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WordMenu {
    pub target: ContextTarget,
    pub at: Point<Pixels>,
    /// The point clicked, in the document's coordinates.
    pub point: (f64, f64),
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
        self.views.mini = false;
        let target = self.menu_target_at(index, x, y);
        self.word_menu = Some(WordMenu {
            target,
            at: event.position,
            point: (x, y),
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
        // In the header or footer open in place: its own menu, and a way
        // out.
        if document.story().is_some() {
            if !document
                .selection()
                .is_some_and(|(anchor, focus)| anchor != focus)
            {
                let _ = document.place_caret_at(x, y);
            }
            return ContextTarget {
                header_footer: true,
                selection: document
                    .selection()
                    .is_some_and(|(anchor, focus)| anchor != focus),
                ..ContextTarget::default()
            };
        }
        let page_y = y - document.document_y(index, 0.0);
        let setup = document.page_setup();
        let to_px = |points: f32| f64::from(points) * 4.0 / 3.0;
        let (_, page_height) = document.page_size();
        let print_layout = self.views.kind == jade_web::word::view::ViewKind::Print;
        if print_layout
            && (page_y < to_px(setup.margins.top)
                || page_y > page_height - to_px(setup.margins.bottom))
        {
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
            link: document.hyperlink_at_point(x, y).is_some(),
            field: {
                use jade_web::word::fields::WordFields;
                !document.selected_fields().is_empty()
            },
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
        let Some(opened) = self.word_menu.take() else {
            return;
        };
        let table = |command: TableCommand| Action::Table(command);
        let tables = |action: TableAction| Action::Tables(action);
        let action = match command {
            C::Cut | C::Copy => {
                self.word_copy(command == C::Cut, cx);
                return cx.notify();
            }
            // The clipboard, read on this click.
            C::Paste | C::PasteKeepSource => {
                return super::word_characters::paste_with(self, PasteMode::KeepSource, cx)
            }
            C::PasteMerge => {
                return super::word_characters::paste_with(self, PasteMode::MergeFormatting, cx)
            }
            C::PasteTextOnly => {
                return super::word_characters::paste_with(self, PasteMode::TextOnly, cx)
            }
            C::PasteSpecial => {
                super::word_characters::open_paste_special(self, cx);
                return cx.notify();
            }
            // スマート検索: the find bar, on the word or the selection.
            C::SmartLookup => return self.open_find(false, cx),
            C::EditLink => Action::InsertLink,
            C::OpenLink => {
                let (x, y) = opened.point;
                self.follow_link_at(x, y, cx);
                return cx.notify();
            }
            C::RemoveLink => {
                return self.word_edit(cx, |document| {
                    let draft = document.link_draft();
                    let display = draft.display.clone();
                    document.apply_link(&draft, &display, "").map(|_| ())
                });
            }
            C::UpdateField => return self.update_fields(cx),
            C::EditField => return self.open_edit_field_dialog(cx),
            C::ToggleFieldCodes => return self.toggle_selected_field_codes(cx),
            C::CloseHeaderFooter => Action::Story(super::word_story::StoryAction::Close),
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
        };
        self.ribbon_action(action, window, cx);
    }

    /// The open menu, drawn over everything at the pointer.
    pub(super) fn word_menu_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let open = self.word_menu?;
        let entries = menu(open.target);
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
