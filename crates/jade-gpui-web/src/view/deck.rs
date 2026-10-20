//! A deck in the browser, worked as PowerPoint is worked: the slide list,
//! a shape selected with a click or Tab, moved by dragging or with the
//! arrows, resized from its corners, typed into; tables typed into cell by
//! cell, their rows and columns added and removed, their cells merged and
//! split; a slide show.
//!
//! What each of those does to the deck is `jade-office-model`'s
//! (`deck::Deck`), the same code the native app runs; this is the gpui
//! around it.

use jade_web::deck::{
    FormatStrip, Insert, Motion, ParagraphAlignment, ShapePreset, ShapeText, TextCommand,
    FONT_SIZES, SHAPE_PRESETS,
};

use crate::tooltip::Tooltip;

use super::*;

/// The format strip under the toolbar: font and paragraph.
pub(super) const FORMAT_BAR_HEIGHT: f32 = 36.0;

/// A list open under a format strip button.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Popup {
    Font,
    Size,
    Color,
    Spacing,
    /// Insert's shapes.
    Shapes,
}

/// A thumbnail being dragged to another place in the list.
#[derive(Clone, Copy)]
pub(super) struct DraggedSlide(usize);

/// What follows the pointer while a thumbnail is dragged.
struct DraggedSlideView(usize);

impl Render for DraggedSlideView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded_md()
            .bg(palette::accent())
            .text_xs()
            .text_color(gpui::white())
            .font_family(UI_FONT)
            .child(i18n::format("deck.slide_n", &[("n", &(self.0 + 1))]))
    }
}

impl DocView {
    pub(super) fn select_shape(&mut self, id: Option<u32>) {
        if let Document::Deck {
            deck,
            current,
            selected,
            selected_name,
            slides_focused,
            ..
        } = &mut self.document
        {
            if id.is_some() {
                *slides_focused = false;
            }
            *selected = id;
            *selected_name = id
                .and_then(|id| deck.shape(*current, id))
                .map(|shape| SharedString::from(shape.name));
        }
    }

    pub(super) fn go_to_slide(&mut self, index: usize, cx: &mut Context<Self>) {
        self.commit_slide_text(cx);
        self.select_shape(None);
        if let Document::Deck {
            deck,
            current,
            thumbnail_scroll,
            drag,
            ..
        } = &mut self.document
        {
            *drag = None;
            *current = index.min(deck.slide_count().saturating_sub(1));
            thumbnail_scroll.scroll_to_item(*current);
        }
        cx.notify();
    }

    pub(super) fn step_slide(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Document::Deck { current, .. } = &self.document {
            let to = current.saturating_add_signed(delta);
            self.go_to_slide(to, cx);
        }
    }

    // ---- slides -----------------------------------------------------------

    /// Runs a change to the deck's slides; when it changes something, shows
    /// slide `then` with every thumbnail repainted, as slides have moved.
    pub(super) fn change_slides(
        &mut self,
        then: usize,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut jade_web::Deck) -> Result<bool, String>,
    ) {
        self.commit_slide_text(cx);
        let Document::Deck {
            deck, layouts_open, ..
        } = &mut self.document
        else {
            return;
        };
        *layouts_open = false;
        let result = change(deck);
        let changed = matches!(result, Ok(true));
        self.edited(result, cx);
        if changed {
            self.select_shape(None);
            if let Document::Deck { cells, drag, .. } = &mut self.document {
                *cells = None;
                *drag = None;
            }
            self.go_to_slide(then, cx);
        }
    }

    fn current_slide(&self) -> usize {
        match &self.document {
            Document::Deck { current, .. } => *current,
            _ => 0,
        }
    }

    /// New Slide: an empty one after the slide on show, on its layout.
    pub(super) fn new_slide(&mut self, cx: &mut Context<Self>) {
        let at = self.current_slide() + 1;
        self.change_slides(at, cx, |deck| deck.insert_slide(at));
    }

    pub(super) fn duplicate_slide(&mut self, cx: &mut Context<Self>) {
        let index = self.current_slide();
        self.change_slides(index + 1, cx, |deck| deck.duplicate_slide(index));
    }

    /// Deletes the slide on show, showing the one after it — or before, for
    /// the last.
    pub(super) fn delete_slide(&mut self, cx: &mut Context<Self>) {
        let index = self.current_slide();
        self.change_slides(index, cx, |deck| deck.delete_slide(index));
    }

    /// A thumbnail dropped on another: the slide moves to its place.
    pub(super) fn move_slide(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        self.change_slides(to, cx, |deck| deck.move_slide(from, to));
    }

    pub(super) fn toggle_slide_hidden(&mut self, cx: &mut Context<Self>) {
        let index = self.current_slide();
        self.change_slides(index, cx, |deck| {
            let hidden = !deck.is_hidden(index);
            deck.set_slide_hidden(index, hidden)
        });
    }

    pub(super) fn set_slide_layout(&mut self, part_name: &str, cx: &mut Context<Self>) {
        let index = self.current_slide();
        self.change_slides(index, cx, |deck| deck.set_slide_layout(index, part_name));
    }

    pub(super) fn toggle_layouts(&mut self, cx: &mut Context<Self>) {
        if let Document::Deck {
            layouts_open,
            format_popup,
            ..
        } = &mut self.document
        {
            *layouts_open = !*layouts_open;
            *format_popup = None;
        }
        cx.notify();
    }

    // ---- speaker notes --------------------------------------------------------

    /// A click in the notes pane: typing into the slide's notes, the caret
    /// at their end.
    fn begin_notes(&mut self, cx: &mut Context<Self>) {
        if let Document::Deck {
            notes_edit: Some(_),
            ..
        } = &self.document
        {
            return;
        }
        self.commit_slide_text(cx);
        self.select_shape(None);
        let Document::Deck {
            deck,
            current,
            notes_edit,
            ..
        } = &mut self.document
        else {
            return;
        };
        match deck.notes_text(*current) {
            Some(text) => *notes_edit = Some(TextEdit::new(text)),
            None => self.status = i18n::text("deck.text_locked").into(),
        }
        cx.notify();
    }

    /// Writes the notes being typed, as one step of Undo.
    fn commit_notes(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            current,
            notes_edit,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(edit) = notes_edit.take() else {
            return;
        };
        let result = deck.set_notes(*current, &edit.text);
        self.edited(result, cx);
    }

    /// The keys of the notes being typed: Escape leaves them, Enter starts
    /// a paragraph; the caret keys and deletion. `None` when no notes are
    /// typed into.
    fn notes_key(&mut self, key: &str, cx: &mut Context<Self>) -> Option<bool> {
        let Document::Deck {
            notes_edit: Some(edit),
            ..
        } = &mut self.document
        else {
            return None;
        };
        if edit.marked.is_some() {
            return Some(false);
        }
        match key {
            "escape" => {
                self.commit_notes(cx);
                return Some(true);
            }
            "enter" => edit.insert("\n"),
            "backspace" => edit.backspace(),
            "delete" => edit.delete(),
            "left" => edit.step(false),
            "right" => edit.step(true),
            "home" | "up" => edit.to_end(false),
            "end" | "down" => edit.to_end(true),
            _ => return Some(false),
        }
        cx.notify();
        Some(true)
    }

    // ---- inserting ----------------------------------------------------------

    /// Text Box, or a shape from Shapes: the next drag on the slide draws it,
    /// as in PowerPoint; a click drops it at its usual size.
    fn start_insert(&mut self, insert: Insert, cx: &mut Context<Self>) {
        self.commit_slide_text(cx);
        self.close_popups();
        self.select_shape(None);
        if let Document::Deck { inserting, .. } = &mut self.document {
            *inserting = Some(insert);
        }
        cx.notify();
    }

    /// Picture: the page's file picker, whose PNG or JPEG comes back through
    /// the workspace to [`DocView::insert_picture`].
    fn pick_picture(&mut self, cx: &mut Context<Self>) {
        self.commit_slide_text(cx);
        self.close_popups();
        self.host.pick();
        cx.notify();
    }

    /// A PNG or JPEG placed in the middle of the slide on show, selected.
    pub(crate) fn insert_picture(&mut self, bytes: &[u8], cx: &mut Context<Self>) {
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let result = deck.insert_picture(*current, bytes);
        if let Err(error) = &result {
            self.status = i18n::format("deck.picture_failed", &[("error", error)]).into();
            cx.notify();
            return;
        }
        self.inserted(result, cx);
    }

    /// After an insert: the slide repainted and the new object selected.
    fn inserted(&mut self, result: Result<u32, String>, cx: &mut Context<Self>) -> Option<u32> {
        let id = match result {
            Ok(id) => id,
            Err(error) => {
                self.edited(Err(error), cx);
                return None;
            }
        };
        self.typed(Ok(true), cx);
        self.select_shape(Some(id));
        Some(id)
    }

    // ---- formatting ---------------------------------------------------------

    fn toggle_popup(&mut self, popup: Popup, cx: &mut Context<Self>) {
        if let Document::Deck {
            layouts_open,
            format_popup,
            ..
        } = &mut self.document
        {
            *format_popup = (*format_popup != Some(popup)).then_some(popup);
            *layouts_open = false;
        }
        cx.notify();
    }

    fn close_popups(&mut self) {
        if let Document::Deck {
            layouts_open,
            format_popup,
            ..
        } = &mut self.document
        {
            *format_popup = None;
            *layouts_open = false;
        }
    }

    /// What the format strip shows: the text typed into, or all of the
    /// selected shape's — `None` when neither has text to format.
    fn format_strip(&mut self) -> Option<FormatStrip> {
        let Document::Deck {
            deck,
            current,
            selected,
            typing,
            edit,
            cells,
            ..
        } = &mut self.document
        else {
            return None;
        };
        if let Some(typing) = typing {
            return Some(typing.format_strip(deck));
        }
        let id = selected.filter(|_| edit.is_none() && cells.is_none())?;
        deck.shape_format(*current, id)
    }

    /// A format strip command — or ⌘B and the rest — on the text typed
    /// into, or on all of the selected shape's.
    fn format_slide_text(&mut self, command: TextCommand, cx: &mut Context<Self>) {
        self.close_popups();
        if self.typing_state().is_some() {
            self.edit_typing(cx, |typing, deck| typing.format(deck, &command));
            return;
        }
        let Document::Deck {
            deck,
            current,
            selected: Some(id),
            edit: None,
            cells: None,
            ..
        } = &mut self.document
        else {
            cx.notify();
            return;
        };
        let result = deck.format_shape(*current, *id, &command);
        self.typed(result, cx);
    }

    /// PowerPoint's formatting shortcuts: ⌘B, ⌘I, ⌘U, ⌘L, ⌘E, ⌘R, ⌘J, and
    /// ⌘⇧> and ⌘⇧< for the font size.
    fn format_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let shift = event.keystroke.modifiers.shift;
        let command = match (event.keystroke.key.as_str(), shift) {
            ("b", false) => TextCommand::Bold,
            ("i", false) => TextCommand::Italic,
            ("u", false) => TextCommand::Underline,
            ("l", false) => TextCommand::Align(ParagraphAlignment::Left),
            ("e", false) => TextCommand::Align(ParagraphAlignment::Center),
            ("r", false) => TextCommand::Align(ParagraphAlignment::Right),
            ("j", false) => TextCommand::Align(ParagraphAlignment::Justify),
            (".", true) | (">", _) => TextCommand::Grow,
            (",", true) | ("<", _) => TextCommand::Shrink,
            _ => return false,
        };
        if self.format_strip().is_none() {
            return false;
        }
        self.format_slide_text(command, cx);
        true
    }

    /// ⌘ shortcuts for slides: New Slide (⌘M, as PowerPoint for Windows
    /// has it, or ⌘⇧N, as for Mac — browsers keep ⌘N) and Duplicate (⌘D)
    /// when no shape is selected.
    pub(super) fn deck_shortcut(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if let Document::Deck {
            notes_edit: Some(_),
            ..
        } = &self.document
        {
            return false;
        }
        if self.format_shortcut(event, cx) {
            return true;
        }
        if let Some(composing) = self.typing_state() {
            let modifiers = event.keystroke.modifiers;
            return !composing && self.typing_key(&event.keystroke.key, modifiers, cx);
        }
        let Document::Deck { edit, selected, .. } = &self.document else {
            return false;
        };
        if edit.is_some() {
            return false;
        }
        let no_shape = selected.is_none();
        match (
            event.keystroke.key.as_str(),
            event.keystroke.modifiers.shift,
        ) {
            ("m", false) | ("n", true) => self.new_slide(cx),
            ("d", false) if no_shape => self.duplicate_slide(cx),
            _ => return false,
        }
        true
    }

    /// The slide show's next slide, or the one before: hidden slides are
    /// skipped, and past the last one the show is over, as in PowerPoint.
    pub(super) fn step_show(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Document::Deck { deck, current, .. } = &self.document else {
            return;
        };
        match deck.next_shown(*current, forward) {
            Some(next) => self.go_to_slide(next, cx),
            None if forward => {
                self.slideshow = false;
                cx.notify();
            }
            None => {}
        }
    }

    /// A show from the beginning starts on the first slide not hidden.
    pub(super) fn start_slideshow(&mut self, from: Option<usize>, cx: &mut Context<Self>) {
        if let Some(from) = from {
            self.go_to_slide(from, cx);
        } else {
            self.commit_slide_text(cx);
            self.select_shape(None);
        }
        self.slideshow = true;
        cx.notify();
    }

    /// Writes the text being typed into a cell, if any. Text typed in
    /// place is in the deck already; it just stops being typed into, an
    /// IME's composition kept.
    pub(super) fn commit_slide_text(&mut self, cx: &mut Context<Self>) {
        self.commit_notes(cx);
        let Document::Deck {
            deck,
            edit,
            typing,
            selecting_text,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        *selecting_text = false;
        if let Some(mut typing) = typing.take() {
            let result = typing.unmark(deck);
            deck.end_typing();
            self.typed(result, cx);
            return;
        }
        let Some((target, text)) = edit.take() else {
            return;
        };
        let result = deck.set_text(*current, target, &text.text);
        self.edited(result, cx);
    }

    /// Starts typing into the selected shape — in a table, into the cell at
    /// `point`, or the first cell without one.
    pub(super) fn begin_slide_text(&mut self, point: Option<(f64, f64)>, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            edit,
            typing,
            drag,
            current,
            selected,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(id) = *selected else {
            return;
        };
        let target = deck.text_target(*current, id, point);
        let begun = match target {
            TextTarget::Shape(id) => {
                *typing = ShapeText::begin(deck, *current, id, point);
                *drag = None;
                typing.is_some()
            }
            _ => match deck.text(*current, target) {
                Some(text) => {
                    *edit = Some((target, TextEdit::new(text)));
                    true
                }
                None => false,
            },
        };
        if !begun {
            self.status = i18n::text("deck.text_locked").into();
        }
        cx.notify();
    }

    /// After an edit to text typed in place: the slide and its thumbnail
    /// repainted, no other.
    fn typed(&mut self, result: Result<bool, String>, cx: &mut Context<Self>) {
        if !matches!(result, Ok(true)) {
            return self.edited(result, cx);
        }
        self.generation += 1;
        self.status = SharedString::default();
        if let Document::Deck {
            current,
            thumbnails,
            ..
        } = &mut self.document
        {
            if let Some(old) = thumbnails.get_mut(*current).and_then(Option::take) {
                self.retired.push(old);
            }
        }
        cx.notify();
    }

    /// Runs `edit` on the text being typed in place.
    pub(super) fn edit_typing(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut ShapeText, &mut jade_web::Deck) -> Result<bool, String>,
    ) {
        let Document::Deck {
            deck,
            typing: Some(typing),
            ..
        } = &mut self.document
        else {
            return;
        };
        let result = edit(typing, deck);
        self.typed(result, cx);
    }

    /// What an IME sees of the text typed in place: the text, the
    /// selection and the composition, in UTF-16 offsets.
    pub(super) fn slide_input(&self) -> Option<(String, Range<usize>, Option<Range<usize>>)> {
        match &self.document {
            Document::Deck {
                deck,
                typing: Some(typing),
                ..
            } => Some(typing.input_text(deck)),
            _ => None,
        }
    }

    pub(super) fn slide_input_reversed(&self) -> bool {
        matches!(&self.document, Document::Deck { typing: Some(typing), .. }
            if typing.focus < typing.anchor)
    }

    /// Text from the input handler into a shape — typed, or an IME's
    /// composition when `mark` — starting to type into the selected shape
    /// if need be. `false` when it is not for a shape.
    pub(super) fn type_into_slide(
        &mut self,
        range: Option<Range<usize>>,
        text: &str,
        mark: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.slideshow {
            return false;
        }
        match &self.document {
            // The notes take it, through the cells' route.
            Document::Deck {
                notes_edit: Some(_),
                ..
            } => return false,
            Document::Deck {
                edit: None,
                typing: None,
                selected: Some(_),
                ..
            } => self.begin_slide_text(None, cx),
            _ => {}
        }
        let Document::Deck {
            typing: Some(typing),
            ..
        } = &mut self.document
        else {
            return false;
        };
        if mark {
            typing.compose(range, text);
            cx.notify();
        } else {
            let text = text.to_string();
            self.edit_typing(cx, move |typing, deck| typing.commit(deck, range, &text));
        }
        true
    }

    /// Where the IME's candidate window goes: at the caret in the text typed
    /// in place.
    pub(super) fn slide_caret_bounds(&mut self) -> Option<Bounds<Pixels>> {
        let slide = self.surface.get()?;
        let Document::Deck {
            deck,
            typing: Some(typing),
            scale,
            ..
        } = &mut self.document
        else {
            return None;
        };
        let [a, b] = typing.geometry(deck)?.caret;
        Some(Bounds {
            origin: point(
                slide.origin.x + px(a.x.min(b.x) as f32 * *scale),
                slide.origin.y + px(a.y.min(b.y) as f32 * *scale),
            ),
            size: size(px(1.0), px((a.y - b.y).abs() as f32 * *scale)),
        })
    }

    /// The keys of text typed in place: the caret keys — ⇧ selecting, ⌥ by
    /// words, ⌘ to the ends of the line and the text — ⌘A, Backspace,
    /// Delete, Enter for a paragraph and ⇧Enter for a line break, Tab.
    fn typing_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let shift = modifiers.shift;
        let command = modifiers.platform || modifiers.control;
        let motion = match key {
            "left" if command => Some(Motion::LineStart),
            "right" if command => Some(Motion::LineEnd),
            "up" if command => Some(Motion::Start),
            "down" if command => Some(Motion::End),
            "left" if modifiers.alt => Some(Motion::WordLeft),
            "right" if modifiers.alt => Some(Motion::WordRight),
            "left" => Some(Motion::Left),
            "right" => Some(Motion::Right),
            "up" => Some(Motion::Up),
            "down" => Some(Motion::Down),
            "home" => Some(Motion::LineStart),
            "end" => Some(Motion::LineEnd),
            _ => None,
        };
        if let Some(motion) = motion {
            self.edit_typing(cx, |typing, deck| {
                typing.move_caret(deck, motion, shift);
                Ok(false)
            });
            return true;
        }
        if command {
            if key != "a" {
                return false;
            }
            self.edit_typing(cx, |typing, deck| {
                typing.select_all(deck);
                Ok(false)
            });
            return true;
        }
        match key {
            // As in PowerPoint, Escape leaves the text, the shape still
            // selected.
            "escape" => self.commit_slide_text(cx),
            "enter" if shift => self.edit_typing(cx, |typing, deck| typing.line_break(deck)),
            "enter" => self.edit_typing(cx, |typing, deck| typing.type_text(deck, "\n")),
            "tab" => self.edit_typing(cx, |typing, deck| typing.type_text(deck, "\t")),
            "backspace" => self.edit_typing(cx, |typing, deck| typing.backspace(deck)),
            "delete" => self.edit_typing(cx, |typing, deck| typing.delete(deck)),
            _ => return false,
        }
        true
    }

    /// Whether text is being typed in place, and an IME is composing in it.
    fn typing_state(&self) -> Option<bool> {
        match &self.document {
            Document::Deck {
                typing: Some(typing),
                ..
            } => Some(typing.composition.is_some()),
            _ => None,
        }
    }

    /// Starts typing into `target` from the text it has — all of it
    /// selected, when `whole`, so typing replaces it.
    pub(super) fn edit_slide_text(
        &mut self,
        target: TextTarget,
        whole: bool,
        cx: &mut Context<Self>,
    ) {
        let Document::Deck {
            deck,
            edit,
            current,
            ..
        } = &mut self.document
        else {
            return;
        };
        if let Some(text) = deck.text(*current, target) {
            let text = if whole {
                TextEdit::selected(text)
            } else {
                TextEdit::new(text)
            };
            *edit = Some((target, text));
        }
        cx.notify();
    }

    /// The cell being typed into, if a table's.
    pub(super) fn edited_cell(&self) -> Option<TextTarget> {
        match &self.document {
            Document::Deck {
                edit: Some((target @ TextTarget::Cell { .. }, _)),
                ..
            } => Some(*target),
            _ => None,
        }
    }

    /// Tab in a table: the text written, then the next cell (⇧Tab: the one
    /// before). Tab in the last cell adds a row and goes to its first, as
    /// in PowerPoint.
    pub(super) fn tab_in_table(&mut self, backwards: bool, cx: &mut Context<Self>) {
        let Some(from @ TextTarget::Cell { id, row, .. }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let slide = *current;
        match deck.next_cell(slide, from, backwards) {
            // The next cell's text selected, as PowerPoint's Tab does.
            Some(next) => self.edit_slide_text(next, true, cx),
            None if !backwards => {
                let result = deck.change_table(slide, id, TableChange::InsertRow(row + 1));
                let added = matches!(result, Ok(true));
                self.edited(result, cx);
                if !added {
                    return;
                }
                self.edit_slide_text(
                    TextTarget::Cell {
                        id,
                        row: row + 1,
                        column: 0,
                    },
                    false,
                    cx,
                );
            }
            None => self.edit_slide_text(from, true, cx),
        }
    }

    /// Extends a cell selection — from the cell being typed into, or the
    /// selection's first — to the cell at (`x`, `y`) of the same table.
    pub(super) fn select_slide_cells_to(&mut self, x: f64, y: f64, cx: &mut Context<Self>) -> bool {
        let Document::Deck {
            deck,
            current,
            edit,
            cells,
            ..
        } = &mut self.document
        else {
            return false;
        };
        let (id, from) = match (*cells, edit.as_ref().map(|(target, _)| *target)) {
            (Some(cells), _) => (cells.id, cells.from),
            (None, Some(TextTarget::Cell { id, row, column })) => (id, (row, column)),
            _ => return false,
        };
        let TextTarget::Cell { row, column, .. } = deck.text_target(*current, id, Some((x, y)))
        else {
            return false;
        };
        let Some(rect) = deck.cell_range_rect(*current, id, from, (row, column)) else {
            return false;
        };
        self.commit_slide_text(cx);
        if let Document::Deck { cells, .. } = &mut self.document {
            *cells = Some(CellSelection {
                id,
                from,
                to: (row, column),
                rect,
            });
        }
        cx.notify();
        true
    }

    /// Merge Cells: the selected cells become one, typed into next.
    pub(super) fn merge_slide_cells(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            current,
            cells,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(selection) = cells.take() else {
            return;
        };
        let change = jade_web::deck::merge_cells(selection.from, selection.to);
        let result = deck.change_table(*current, selection.id, change);
        let merged = matches!(result, Ok(true));
        self.edited(result, cx);
        if let (true, TableChange::MergeCells { row, column, .. }) = (merged, change) {
            let target = TextTarget::Cell {
                id: selection.id,
                row,
                column,
            };
            self.edit_slide_text(target, false, cx);
        }
        cx.notify();
    }

    /// Split Cells: the merged cell being typed into becomes its cells.
    pub(super) fn split_slide_cell(&mut self, cx: &mut Context<Self>) {
        let Some(target @ TextTarget::Cell { id, row, column }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let result = deck.change_table(*current, id, TableChange::SplitCell { row, column });
        let split = matches!(result, Ok(true));
        self.edited(result, cx);
        if split {
            self.edit_slide_text(target, false, cx);
        }
    }

    /// A row or column added or removed beside the cell being typed into,
    /// its text written first.
    pub(super) fn change_slide_table(
        &mut self,
        change: impl FnOnce(usize, usize) -> TableChange,
        cx: &mut Context<Self>,
    ) {
        let Some(TextTarget::Cell { id, row, column }) = self.edited_cell() else {
            return;
        };
        self.commit_slide_text(cx);
        let Document::Deck { deck, current, .. } = &mut self.document else {
            return;
        };
        let result = deck.change_table(*current, id, change(row, column));
        self.edited(result, cx);
        cx.notify();
    }

    pub(super) fn deck_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let modifiers = event.keystroke.modifiers;
        let key = event.keystroke.key.as_str();
        if self.slideshow {
            match key {
                "right" | "down" | "space" | "enter" | "pagedown" => self.step_show(true, cx),
                "left" | "up" | "backspace" | "pageup" => self.step_show(false, cx),
                "escape" => {
                    self.slideshow = false;
                    cx.notify();
                }
                _ => return false,
            }
            return true;
        }
        if let Some(handled) = self.notes_key(key, cx) {
            return handled;
        }
        if let Some(composing) = self.typing_state() {
            // An IME composing has the keys to itself.
            return !composing && self.typing_key(key, modifiers, cx);
        }
        let (slides_focused, layouts_open, format_open, inserting) = match &self.document {
            Document::Deck {
                slides_focused,
                layouts_open,
                format_popup,
                inserting,
                ..
            } => (
                *slides_focused,
                *layouts_open,
                format_popup.is_some(),
                inserting.is_some(),
            ),
            _ => (false, false, false, false),
        };
        if let Document::Deck {
            cells: cells @ Some(_),
            ..
        } = &mut self.document
        {
            if key == "escape" {
                *cells = None;
                cx.notify();
                return true;
            }
        }
        let Document::Deck {
            deck,
            current,
            selected,
            edit,
            ..
        } = &mut self.document
        else {
            return false;
        };
        if let Some((target, text)) = edit.as_mut() {
            if text.marked.is_some() {
                return false;
            }
            let in_table = matches!(target, TextTarget::Cell { .. });
            match key {
                // As in PowerPoint, Escape leaves the text, keeping it.
                "escape" => self.commit_slide_text(cx),
                "tab" if in_table => self.tab_in_table(modifiers.shift, cx),
                "enter" => text.insert("\n"),
                "backspace" => text.backspace(),
                "delete" => text.delete(),
                "left" => text.step(false),
                "right" => text.step(true),
                "home" => text.to_end(false),
                "end" => text.to_end(true),
                _ => return false,
            }
            cx.notify();
            return true;
        }
        let slide = *current;
        if let Some(id) = *selected {
            let step = if modifiers.alt { 1.0 } else { NUDGE_PX };
            let result = match key {
                "left" => deck.move_shape(slide, id, -step, 0.0),
                "right" => deck.move_shape(slide, id, step, 0.0),
                "up" => deck.move_shape(slide, id, 0.0, -step),
                "down" => deck.move_shape(slide, id, 0.0, step),
                "delete" | "backspace" => {
                    let result = deck.delete_shape(slide, id);
                    self.select_shape(None);
                    result
                }
                "enter" | "f2" => {
                    self.begin_slide_text(None, cx);
                    return true;
                }
                "escape" => {
                    self.select_shape(None);
                    cx.notify();
                    return true;
                }
                "tab" => {
                    let next = deck.next_shape(slide, Some(id), modifiers.shift);
                    self.select_shape(next);
                    cx.notify();
                    return true;
                }
                _ => return false,
            };
            self.edited(result, cx);
            return true;
        }
        match key {
            // In the slide list, Delete deletes the slide.
            "delete" | "backspace" if slides_focused => self.delete_slide(cx),
            "escape" if inserting => {
                if let Document::Deck {
                    inserting, drawing, ..
                } = &mut self.document
                {
                    *inserting = None;
                    *drawing = None;
                }
                cx.notify();
            }
            "escape" if layouts_open => self.toggle_layouts(cx),
            "escape" if format_open => {
                self.close_popups();
                cx.notify();
            }
            "down" | "right" | "pagedown" => self.step_slide(1, cx),
            "up" | "left" | "pageup" => self.step_slide(-1, cx),
            "tab" => {
                let next = deck.next_shape(slide, None, modifiers.shift);
                self.select_shape(next);
                cx.notify();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn slide_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Document::Deck {
            inserting: Some(_),
            drawing,
            ..
        } = &mut self.document
        {
            *drawing = Some(((x, y), (x, y)));
            cx.notify();
            return;
        }
        // Shift+click in a table whose cell is being typed into, or whose
        // cells are selected, selects the cells between, as in PowerPoint.
        if event.modifiers.shift && self.select_slide_cells_to(x, y, cx) {
            return;
        }
        let Document::Deck {
            deck,
            current,
            edit,
            typing,
            selecting_text,
            cells,
            slides_focused,
            layouts_open,
            format_popup,
            ..
        } = &mut self.document
        else {
            return;
        };
        *cells = None;
        *slides_focused = false;
        *layouts_open = false;
        *format_popup = None;
        // In the text being typed into, a click puts the caret, ⇧ or a drag
        // selects, and a double-click selects a word.
        let typed = typing.as_ref().map(|typing| typing.id);
        if let Some(id) = typed {
            if deck
                .shape_rect(*current, id)
                .is_some_and(|rect| rect.contains(x, y))
            {
                let shift = event.modifiers.shift;
                let double = event.click_count >= 2;
                *selecting_text = !double;
                self.edit_typing(cx, |typing, deck| {
                    if double {
                        typing.select_word(deck, x, y);
                    } else {
                        typing.click(deck, x, y, shift);
                    }
                    Ok(false)
                });
                return;
            }
        }
        // A click inside the shape or cell being typed into stays in the
        // text.
        if let Some((target, _)) = edit {
            let inside = deck
                .text_rect(*current, *target)
                .is_some_and(|rect| rect.contains(x, y));
            if inside {
                return;
            }
        }
        let hit = deck.shape_at(*current, x, y);
        let movable = hit
            .and_then(|id| deck.shape(*current, id))
            .is_some_and(|shape| shape.can_move);
        self.commit_slide_text(cx);
        self.select_shape(hit);
        match hit {
            // In a table, the cell under the click.
            Some(_) if event.click_count >= 2 => self.begin_slide_text(Some((x, y)), cx),
            // Dragging the shape moves it.
            Some(id) if movable => {
                if let Document::Deck { drag, .. } = &mut self.document {
                    *drag = Some(ShapeDrag::new(id, DragMode::Move, (x, y)));
                }
            }
            _ => {}
        }
        cx.notify();
    }

    /// A press on a corner handle of the selected shape: dragging it resizes
    /// the shape from the opposite corner.
    pub(super) fn start_slide_resize(
        &mut self,
        left: bool,
        top: bool,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus, cx);
        cx.stop_propagation();
        let Some(from) = self.slide_point(event.position) else {
            return;
        };
        let Document::Deck {
            deck,
            current,
            selected,
            ..
        } = &mut self.document
        else {
            return;
        };
        let Some(id) = *selected else {
            return;
        };
        if !deck
            .shape(*current, id)
            .is_some_and(|shape| shape.can_resize)
        {
            return;
        }
        self.commit_slide_text(cx);
        if let Document::Deck { drag, .. } = &mut self.document {
            *drag = Some(ShapeDrag::new(id, DragMode::Resize { left, top }, from));
        }
        cx.notify();
    }

    /// The pointer moving with the button down drags the shape along.
    pub(super) fn slide_mouse_move(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        if !event.dragging() {
            return;
        }
        let Some((x, y)) = self.slide_point(event.position) else {
            return;
        };
        if let Document::Deck {
            drawing: Some((_, to)),
            ..
        } = &mut self.document
        {
            *to = (x, y);
            cx.notify();
            return;
        }
        if let Document::Deck {
            selecting_text: true,
            ..
        } = &self.document
        {
            self.edit_typing(cx, |typing, deck| {
                typing.click(deck, x, y, true);
                Ok(false)
            });
            return;
        }
        if let Document::Deck {
            drag: Some(drag), ..
        } = &mut self.document
        {
            drag.drag_to(x, y);
            cx.notify();
        }
    }

    /// Letting go writes the drag, as one undoable change.
    pub(super) fn slide_mouse_up(&mut self, cx: &mut Context<Self>) {
        let Document::Deck {
            deck,
            current,
            drag,
            selecting_text,
            inserting,
            drawing,
            ..
        } = &mut self.document
        else {
            return;
        };
        *selecting_text = false;
        if let (Some((from, to)), Some(insert)) = (drawing.take(), *inserting) {
            *inserting = None;
            let result = deck.insert(*current, insert, insert.rect(from, to));
            // A text box takes the caret.
            if let (Some(_), Insert::TextBox) = (self.inserted(result, cx), insert) {
                self.begin_slide_text(None, cx);
            }
            return;
        }
        let Some(drag) = drag.take() else {
            return;
        };
        let result = deck.drag_shape(*current, &drag);
        self.edited(result, cx);
    }

    /// A window point as a point on the slide on show, in slide px.
    pub(super) fn slide_point(&self, position: gpui::Point<Pixels>) -> Option<(f64, f64)> {
        let bounds = self.surface.get()?;
        let Document::Deck { scale, .. } = &self.document else {
            return None;
        };
        let to_slide = |value: Pixels| f64::from(value) / f64::from(*scale);
        Some((
            to_slide(position.x - bounds.origin.x),
            to_slide(position.y - bounds.origin.y),
        ))
    }

    /// Paints the current slide at `width` window px, unless it already is.
    pub(super) fn paint_slide(&mut self, width: f32, scale: f32) {
        let generation = self.generation;
        let Document::Deck {
            deck,
            current,
            slide,
            ..
        } = &mut self.document
        else {
            return;
        };
        let device_width = (width * scale).round() as u32;
        let fresh = matches!(slide, Some((index, w, g, _))
            if *index == *current && *w == device_width && *g == generation);
        if fresh {
            return;
        }
        match deck.render_slide(*current, device_width).and_then(image) {
            Some(painted) => {
                if let Some((_, _, _, old)) =
                    slide.replace((*current, device_width, generation, painted))
                {
                    self.retired.push(old);
                }
            }
            None => {
                self.status = i18n::format("deck.paint_failed", &[("n", &(*current + 1))]).into()
            }
        }
    }

    /// The slide alone on black, filling the window, as a slideshow.
    pub(super) fn slideshow_body(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let (area_width, area_height) = editor_area(window, cx);
        let Document::Deck { deck, .. } = &self.document else {
            unreachable!()
        };
        let (slide_width, slide_height) = deck.slide_size();
        let fit = (area_width / slide_width as f32).min(area_height / slide_height as f32);
        let (width, height) = (slide_width as f32 * fit, slide_height as f32 * fit);
        self.paint_slide(width, window.scale_factor());
        let Document::Deck { slide, .. } = &self.document else {
            unreachable!()
        };
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::black())
            .cursor_pointer()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.step_show(true, cx)),
            )
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(|view, _event, _window, cx| view.step_show(false, cx)),
            )
            .children(
                slide
                    .as_ref()
                    .map(|(_, _, _, image)| img(image.clone()).w(px(width)).h(px(height))),
            )
    }

    pub(super) fn deck_body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui::Div {
        let scale = window.scale_factor();
        let strip = self.format_strip();
        let format_popup = match &self.document {
            Document::Deck { format_popup, .. } => *format_popup,
            _ => None,
        };
        let format_bar = self
            .format_bar(&strip.clone().unwrap_or_default(), format_popup, cx)
            .when(strip.is_none(), |bar| bar.opacity(0.5));
        let editing_cell = self.edited_cell().is_some();
        let editing_merged = match (&self.document, self.edited_cell()) {
            (Document::Deck { deck, current, .. }, Some(target)) => deck
                .cell_span(*current, target)
                .is_some_and(|span| span != (1, 1)),
            _ => false,
        };
        let selected_cells = match &self.document {
            Document::Deck { cells, .. } => cells.map(|cells| cells.rect),
            _ => None,
        };
        let (area_width, area_height) = editor_area(window, cx);
        let notes_height = if self.show_notes { NOTES_HEIGHT } else { 0.0 };
        let Document::Deck {
            deck, thumbnails, ..
        } = &mut self.document
        else {
            unreachable!()
        };
        for (index, thumbnail) in thumbnails.iter_mut().enumerate() {
            if thumbnail.is_none() {
                *thumbnail = deck
                    .render_slide(index, (SLIDE_THUMBNAIL_WIDTH * scale) as u32)
                    .and_then(image);
            }
        }
        let (slide_width, slide_height) = deck.slide_size();
        let zoom = self.zoom.unwrap_or_else(|| {
            deck_fit(deck.slide_size(), (area_width, area_height - notes_height))
        });
        self.shown_zoom = zoom;
        let (width, height) = (slide_width as f32 * zoom, slide_height as f32 * zoom);
        self.paint_slide(width, scale);

        let Document::Deck {
            deck,
            current,
            thumbnails,
            thumbnail_scroll,
            slide,
            selected,
            edit,
            typing,
            notes_edit,
            drag,
            scale: shown_scale,
            layouts_open,
            inserting,
            drawing,
            ..
        } = &mut self.document
        else {
            unreachable!()
        };
        *shown_scale = zoom;
        let current_index = *current;
        let count = deck.slide_count();
        let thumbnail_height = SLIDE_THUMBNAIL_WIDTH * (slide_height / slide_width) as f32;
        // One slide in the list: its number — struck through with an eye
        // for a hidden slide, which is also dimmed, as PowerPoint marks it —
        // and its picture. A click shows it; a drag onto another moves it.
        let rows: Vec<_> = thumbnails
            .iter()
            .enumerate()
            .map(|(index, thumbnail)| {
                let selected = index == current_index;
                let hidden = deck.is_hidden(index);
                let number_color = if selected {
                    palette::accent()
                } else {
                    palette::faint()
                };
                div()
                    .id(("slide-thumbnail", index))
                    .flex()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .cursor_pointer()
                    .on_click(cx.listener(move |view, _event, window, cx| {
                        window.focus(&view.focus, cx);
                        view.go_to_slide(index, cx);
                        if let Document::Deck { slides_focused, .. } = &mut view.document {
                            *slides_focused = true;
                        }
                    }))
                    .on_drag(DraggedSlide(index), |dragged, _offset, _window, cx| {
                        let index = dragged.0;
                        cx.new(|_| DraggedSlideView(index))
                    })
                    .drag_over::<DraggedSlide>(|style, _, _, _| style.bg(palette::hovered()))
                    .on_drop(
                        cx.listener(move |view, dragged: &DraggedSlide, _window, cx| {
                            view.move_slide(dragged.0, index, cx)
                        }),
                    )
                    .child(
                        div()
                            .w(px(16.0))
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(number_color)
                            .child(
                                div()
                                    .when(hidden, |number| number.line_through())
                                    .child(format!("{}", index + 1)),
                            )
                            .when(hidden, |column| {
                                column.child(
                                    svg()
                                        .path("icons/eye-off.svg")
                                        .size_3()
                                        .text_color(number_color),
                                )
                            }),
                    )
                    .child(
                        div()
                            .w(px(SLIDE_THUMBNAIL_WIDTH))
                            .h(px(thumbnail_height))
                            .flex_none()
                            .bg(gpui::white())
                            .border_2()
                            .rounded_sm()
                            .overflow_hidden()
                            .when(hidden, |thumbnail| thumbnail.opacity(0.45))
                            .border_color(if selected {
                                palette::accent()
                            } else {
                                palette::border()
                            })
                            .hover(|thumbnail| thumbnail.border_color(palette::faint()))
                            .children(thumbnail.clone().map(|image| img(image).size_full())),
                    )
            })
            .collect();
        let current_hidden = deck.is_hidden(current_index);
        let layouts = layouts_open.then(|| deck.slide_layouts(current_index));
        let layouts_button = layouts_button(layouts, cx);
        let slide_image = slide.as_ref().map(|(_, _, _, image)| image.clone());
        let notes = deck
            .notes(current_index)
            .filter(|notes| !notes.is_empty())
            .map(|notes| SharedString::from(notes.to_string()));
        let notes_lines = notes_edit.as_ref().map(notes_lines);
        let show_notes = self.show_notes;

        // Text typed in place: the shape's outline, the selection shaded over
        // the text, and the caret — or an IME's composition there.
        let typed = typing.as_ref().and_then(|typing| {
            let rect = deck.shape_rect(current_index, typing.id)?;
            let geometry = typing.geometry(deck);
            let mut out = vec![div()
                .absolute()
                .left(px(rect.x as f32 * zoom))
                .top(px(rect.y as f32 * zoom))
                .w(px(rect.width as f32 * zoom))
                .h(px(rect.height as f32 * zoom))
                .border_1()
                .border_color(palette::accent())];
            let geometry = geometry?;
            for quad in &geometry.highlights {
                let (x0, y0, x1, y1) = quad.iter().fold(
                    (f64::MAX, f64::MAX, f64::MIN, f64::MIN),
                    |(x0, y0, x1, y1), p| (x0.min(p.x), y0.min(p.y), x1.max(p.x), y1.max(p.y)),
                );
                out.push(
                    div()
                        .absolute()
                        .left(px(x0 as f32 * zoom))
                        .top(px(y0 as f32 * zoom))
                        .w(px((x1 - x0) as f32 * zoom))
                        .h(px((y1 - y0) as f32 * zoom))
                        .bg(gpui::rgba(0x3b82f655)),
                );
            }
            let [a, b] = geometry.caret;
            let (x, y, line) = (a.x.min(b.x), a.y.min(b.y), (a.y - b.y).abs());
            let caret = div()
                .absolute()
                .left(px(x as f32 * zoom))
                .top(px(y as f32 * zoom))
                .h(px(line as f32 * zoom));
            out.push(match &typing.composition {
                Some(composition) => caret
                    .flex()
                    .items_center()
                    .px_0p5()
                    .bg(gpui::white())
                    .text_color(gpui::black())
                    .text_size(px((line as f32 * zoom * 0.7).max(10.0)))
                    .child(div().underline().child(composition.clone())),
                None => caret.w(px(1.5)).bg(gpui::black()),
            });
            Some(out)
        });
        let typed_id = typing.as_ref().map(|typing| typing.id);

        // The selection outline and its corner handles, or the text editor,
        // over the slide; a shape being dragged where the drag has taken it.
        let resizable = selected
            .and_then(|id| deck.shape(current_index, id))
            .is_some_and(|shape| shape.can_resize);
        // A text box or shape being drawn.
        let drawn = match (*drawing, *inserting) {
            (Some((from, to)), Some(insert)) => {
                let rect = insert.rect(from, to);
                Some(
                    div()
                        .absolute()
                        .left(px(rect.x as f32 * zoom))
                        .top(px(rect.y as f32 * zoom))
                        .w(px(rect.width as f32 * zoom))
                        .h(px(rect.height as f32 * zoom))
                        .border_1()
                        .border_color(palette::accent()),
                )
            }
            _ => None,
        };
        let inserting = *inserting;
        let shapes_button = shapes_button(inserting, format_popup == Some(Popup::Shapes), cx);
        let overlay = selected.filter(|id| Some(*id) != typed_id).and_then(|id| {
            let editing = edit.as_ref().filter(|(target, _)| target.id() == id);
            let rect = match editing {
                Some((target, _)) => deck.text_rect(current_index, *target)?,
                None => {
                    let rect = deck.shape_rect(current_index, id)?;
                    match drag.as_ref().filter(|drag| drag.id == id) {
                        Some(drag) => drag.rect(rect),
                        None => rect,
                    }
                }
            };
            let (x, y, w, h) = (
                rect.x as f32 * zoom,
                rect.y as f32 * zoom,
                rect.width as f32 * zoom,
                rect.height as f32 * zoom,
            );
            Some(match editing {
                Some((_, text)) => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .min_w(px(w))
                    .min_h(px(h))
                    .p(px(6.0))
                    .bg(gpui::rgba(0xffffffee))
                    .border_1()
                    .border_color(palette::accent())
                    .text_color(gpui::black())
                    .text_size(px((18.0 * zoom).max(11.0)))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .items_end()
                            .child(if text.whole {
                                div()
                                    .bg(palette::selected())
                                    .child(text.split().0.to_string())
                            } else {
                                div().child(text.split().0.to_string())
                            })
                            .child(
                                div()
                                    .w(px(1.5))
                                    .h(px((20.0 * zoom).max(12.0)))
                                    .bg(gpui::black()),
                            )
                            .child(text.split().1.to_string()),
                    ),
                None => div()
                    .absolute()
                    .left(px(x))
                    .top(px(y))
                    .w(px(w))
                    .h(px(h))
                    .border_1()
                    .border_color(palette::accent())
                    .when(resizable, |outline| {
                        outline.children(
                            [(true, true), (false, true), (true, false), (false, false)]
                                .into_iter()
                                .enumerate()
                                .map(|(index, (is_left, is_top))| {
                                    let at = |near: bool, length: f32| {
                                        px(if near { 0.0 } else { length } - HANDLE_SIZE / 2.0)
                                    };
                                    div()
                                        .id(("resize-handle", index))
                                        .absolute()
                                        .left(at(is_left, w))
                                        .top(at(is_top, h))
                                        .size(px(HANDLE_SIZE))
                                        .rounded_full()
                                        .bg(gpui::white())
                                        .border_1()
                                        .border_color(palette::accent())
                                        .cursor(if is_left == is_top {
                                            gpui::CursorStyle::ResizeUpLeftDownRight
                                        } else {
                                            gpui::CursorStyle::ResizeUpRightDownLeft
                                        })
                                        .on_mouse_down(
                                            MouseButton::Left,
                                            cx.listener(move |view, event, window, cx| {
                                                view.start_slide_resize(
                                                    is_left, is_top, event, window, cx,
                                                )
                                            }),
                                        )
                                }),
                        )
                    }),
            })
        });

        let store = Rc::clone(&self.surface);
        let entity = cx.entity();
        div()
            .size_full()
            .flex()
            .bg(palette::window())
            // A drag follows the pointer anywhere over the view, and ends
            // wherever the button comes up.
            .on_mouse_move(cx.listener(|view, event, _window, cx| view.slide_mouse_move(event, cx)))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.slide_mouse_up(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|view, _event, _window, cx| view.slide_mouse_up(cx)),
            )
            .child(
                div()
                    .w(px(DECK_SIDEBAR_WIDTH))
                    .h_full()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .bg(palette::sidebar())
                    .border_r_1()
                    .border_color(palette::border())
                    .child(
                        div()
                            .px_3()
                            .pt_3()
                            .pb_2()
                            .text_xs()
                            .text_color(palette::faint())
                            .child(i18n::format("deck.slides", &[("n", &count)])),
                    )
                    .child(
                        div()
                            .id("slide-thumbnails")
                            .flex_1()
                            .overflow_y_scroll()
                            .track_scroll(thumbnail_scroll)
                            .flex()
                            .flex_col()
                            .gap_1()
                            .pb_3()
                            .children(rows),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .flex()
                    .flex_col()
                    .child(
                        toolbar(i18n::format(
                            "deck.position",
                            &[("current", &(current_index + 1)), ("count", &count)],
                        ))
                        .child(
                            toolbar_button(
                                "new-slide",
                                "icons/file-plus.svg",
                                i18n::text("deck.new_slide"),
                            )
                            .on_click(cx.listener(|view, _event, _window, cx| view.new_slide(cx))),
                        )
                        .child(layouts_button)
                        .child(
                            toolbar_button(
                                "duplicate-slide",
                                "icons/files.svg",
                                i18n::text("deck.duplicate_slide"),
                            )
                            .on_click(
                                cx.listener(|view, _event, _window, cx| view.duplicate_slide(cx)),
                            ),
                        )
                        .child(
                            toolbar_button(
                                "delete-slide",
                                "icons/trash-2.svg",
                                i18n::text("deck.delete_slide"),
                            )
                            .on_click(
                                cx.listener(|view, _event, _window, cx| view.delete_slide(cx)),
                            ),
                        )
                        .child(
                            toolbar_button(
                                "hide-slide",
                                "icons/eye-off.svg",
                                i18n::text(if current_hidden {
                                    "deck.show_slide"
                                } else {
                                    "deck.hide_slide"
                                }),
                            )
                            .when(current_hidden, |button| button.bg(palette::selected()))
                            .on_click(
                                cx.listener(|view, _event, _window, cx| {
                                    view.toggle_slide_hidden(cx)
                                }),
                            ),
                        )
                        .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(palette::border()))
                        .child(
                            toolbar_button(
                                "insert-text-box",
                                "icons/type.svg",
                                i18n::text("deck.text_box"),
                            )
                            .when(inserting == Some(Insert::TextBox), |button| {
                                button.bg(palette::selected())
                            })
                            .on_click(cx.listener(
                                |view, _event, _window, cx| view.start_insert(Insert::TextBox, cx),
                            )),
                        )
                        .child(shapes_button)
                        .child(
                            toolbar_button(
                                "insert-picture",
                                "icons/image.svg",
                                i18n::text("deck.picture"),
                            )
                            .on_click(
                                cx.listener(|view, _event, _window, cx| view.pick_picture(cx)),
                            ),
                        )
                        .child(div().w(px(1.0)).h(px(18.0)).mx_1().bg(palette::border()))
                        // A table's layout, while one of its cells is typed
                        // into: PowerPoint's Layout tab.
                        .when(editing_cell, |bar| {
                            bar.child(
                                toolbar_button(
                                    "row-below",
                                    "icons/arrow-down-to-line.svg",
                                    i18n::text("deck.row_below"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |row, _| TableChange::InsertRow(row + 1),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "column-right",
                                    "icons/arrow-right-to-line.svg",
                                    i18n::text("deck.column_right"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |_, column| TableChange::InsertColumn(column + 1),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "delete-row",
                                    "icons/trash-2.svg",
                                    i18n::text("deck.delete_row"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |row, _| TableChange::DeleteRow(row),
                                            cx,
                                        )
                                    },
                                )),
                            )
                            .child(
                                toolbar_button(
                                    "delete-column",
                                    "icons/trash-2.svg",
                                    i18n::text("deck.delete_column"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| {
                                        view.change_slide_table(
                                            |_, column| TableChange::DeleteColumn(column),
                                            cx,
                                        )
                                    },
                                )),
                            )
                        })
                        .when(selected_cells.is_some(), |bar| {
                            bar.child(
                                toolbar_button(
                                    "merge-cells",
                                    "icons/merge.svg",
                                    i18n::text("deck.merge_cells"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| view.merge_slide_cells(cx),
                                )),
                            )
                        })
                        .when(editing_merged, |bar| {
                            bar.child(
                                toolbar_button(
                                    "split-cell",
                                    "icons/split.svg",
                                    i18n::text("deck.split_cell"),
                                )
                                .on_click(cx.listener(
                                    |view, _event, _window, cx| view.split_slide_cell(cx),
                                )),
                            )
                        })
                        .child(
                            toolbar_button(
                                "notes",
                                "icons/file-text.svg",
                                i18n::text("deck.notes"),
                            )
                            .when(show_notes, |button| button.bg(palette::selected()))
                            .on_click(cx.listener(
                                |view, _event, _window, cx| {
                                    view.show_notes = !view.show_notes;
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            toolbar_button(
                                "from-current",
                                "icons/play.svg",
                                i18n::text("deck.from_current"),
                            )
                            .on_click(cx.listener(
                                |view, _event, _window, cx| view.start_slideshow(None, cx),
                            )),
                        )
                        .child(
                            div()
                                .id("slideshow")
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .ml_1()
                                .px_3()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .text_color(gpui::white())
                                .bg(palette::accent())
                                .cursor_pointer()
                                .hover(|button| button.opacity(0.9))
                                .child(
                                    svg()
                                        .path("icons/play.svg")
                                        .size_3()
                                        .text_color(gpui::white()),
                                )
                                .child(i18n::text("deck.slideshow"))
                                .on_click(cx.listener(|view, _event, _window, cx| {
                                    let first = match &view.document {
                                        Document::Deck { deck, .. } => deck.first_shown(),
                                        _ => 0,
                                    };
                                    view.start_slideshow(Some(first), cx)
                                })),
                        ),
                    )
                    .child(format_bar)
                    .child(
                        // Scrolls once a chosen zoom makes the slide bigger
                        // than the space. A click off the slide clears the
                        // selection.
                        div()
                            .id("slide-canvas")
                            .flex_1()
                            .min_h_0()
                            .overflow_scroll()
                            .bg(palette::canvas())
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(|view, _event, window, cx| {
                                    window.focus(&view.focus, cx);
                                    view.commit_slide_text(cx);
                                    view.select_shape(None);
                                    if let Document::Deck {
                                        slides_focused,
                                        layouts_open,
                                        ..
                                    } = &mut view.document
                                    {
                                        *slides_focused = false;
                                        *layouts_open = false;
                                    }
                                    view.close_popups();
                                    cx.notify();
                                }),
                            )
                            .child(
                                div()
                                    .min_w_full()
                                    .min_h_full()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .p(px(SLIDE_MARGIN))
                                    .child(
                                        div()
                                            .relative()
                                            .flex_none()
                                            .w(px(width))
                                            .h(px(height))
                                            .when(inserting.is_some(), |slide| {
                                                slide.cursor(gpui::CursorStyle::Crosshair)
                                            })
                                            .bg(gpui::white())
                                            .shadow_lg()
                                            .on_mouse_down(
                                                MouseButton::Left,
                                                cx.listener(Self::slide_mouse_down),
                                            )
                                            .children(
                                                slide_image.map(|image| {
                                                    img(image).w(px(width)).h(px(height))
                                                }),
                                            )
                                            .children(overlay)
                                            .children(drawn)
                                            .children(typed.into_iter().flatten())
                                            // The selected cells, shaded as
                                            // PowerPoint shades them.
                                            .children(selected_cells.map(|rect| {
                                                div()
                                                    .absolute()
                                                    .left(px(rect.x as f32 * zoom))
                                                    .top(px(rect.y as f32 * zoom))
                                                    .w(px(rect.width as f32 * zoom))
                                                    .h(px(rect.height as f32 * zoom))
                                                    .bg(gpui::rgba(0x3b82f633))
                                                    .border_1()
                                                    .border_color(palette::accent())
                                            }))
                                            .child(
                                                canvas(
                                                    move |bounds, _window, _cx| {
                                                        store.set(Some(bounds));
                                                        bounds
                                                    },
                                                    move |_bounds, slide, window, cx| {
                                                        let focus = entity.read(cx).focus.clone();
                                                        window.handle_input(
                                                            &focus,
                                                            ElementInputHandler::new(
                                                                slide,
                                                                entity.clone(),
                                                            ),
                                                            cx,
                                                        );
                                                    },
                                                )
                                                .absolute()
                                                .top_0()
                                                .left_0()
                                                .size_full(),
                                            ),
                                    ),
                            ),
                    )
                    .when(show_notes, |column| {
                        column.child(
                            div()
                                .id("slide-notes")
                                .h(px(NOTES_HEIGHT))
                                .flex_none()
                                .overflow_y_scroll()
                                .border_t_1()
                                .border_color(palette::border())
                                .bg(palette::panel())
                                .px_4()
                                .py_2()
                                .text_sm()
                                .cursor(gpui::CursorStyle::IBeam)
                                .when(notes_lines.is_some(), |pane| {
                                    pane.border_t_2().border_color(palette::accent())
                                })
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|view, _event, window, cx| {
                                        window.focus(&view.focus, cx);
                                        view.begin_notes(cx);
                                    }),
                                )
                                .child(match (notes_lines, notes) {
                                    (Some(lines), _) => div().children(lines),
                                    (None, Some(notes)) => div().child(notes),
                                    (None, None) => div()
                                        .text_color(palette::faint())
                                        .child(i18n::text("deck.add_notes")),
                                }),
                        )
                    }),
            )
    }
}

/// Layout ▾, and while open the layouts the slide can take, its own checked.
fn layouts_button(
    layouts: Option<Vec<jade_web::deck::SlideLayoutInfo>>,
    cx: &mut Context<DocView>,
) -> gpui::AnyElement {
    let open = layouts.is_some();
    let menu = layouts.map(|layouts| {
        let rows: Vec<_> = layouts
            .into_iter()
            .enumerate()
            .map(|(index, layout)| {
                let part = layout.part_name.clone();
                div()
                    .id(("slide-layout", index))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(26.0))
                    .pl_1()
                    .pr_3()
                    .rounded_md()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|row| row.bg(palette::hovered()))
                    .on_click(cx.listener(move |view, _event, _window, cx| {
                        view.set_slide_layout(&part, cx)
                    }))
                    .child(div().size_4().flex_none().when(layout.current, |cell| {
                        cell.child(
                            svg()
                                .path("icons/check.svg")
                                .size_4()
                                .text_color(palette::accent()),
                        )
                    }))
                    .child(if layout.name.is_empty() {
                        layout.kind
                    } else {
                        layout.name
                    })
            })
            .collect();
        div()
            .id("slide-layouts")
            .occlude()
            .mt_1()
            .p_1()
            .flex()
            .flex_col()
            .max_h(px(360.0))
            .overflow_y_scroll()
            .rounded_md()
            .bg(palette::panel())
            .border_1()
            .border_color(palette::border())
            .shadow_lg()
            .text_xs()
            .text_color(palette::text())
            .children(rows)
    });
    div()
        .relative()
        .flex_none()
        .child(
            toolbar_button(
                "slide-layout",
                "icons/panels-top-left.svg",
                i18n::text("deck.layout"),
            )
            .when(open, |button| button.bg(palette::hovered()))
            .on_click(cx.listener(|view, _event, _window, cx| view.toggle_layouts(cx))),
        )
        .children(menu.map(|menu| {
            div().absolute().top_full().left_0().child(
                gpui::deferred(
                    gpui::anchored()
                        .snap_to_window_with_margin(px(8.0))
                        .child(menu),
                )
                .with_priority(2),
            )
        }))
        .into_any_element()
}

/// A click listener, as the format strip hands them round.
type ClickHandler = Box<dyn Fn(&gpui::ClickEvent, &mut Window, &mut App)>;

/// The font list the format strip offers, before the text's own.
const FONT_FAMILIES: [&str; 9] = [
    "游ゴシック",
    "メイリオ",
    "Hiragino Sans",
    "Noto Sans JP",
    "游明朝",
    "Calibri",
    "Arial",
    "Georgia",
    "Times New Roman",
];
/// PowerPoint's standard colours, under its font colour button.
const FONT_COLORS: [u32; 12] = [
    0x000000, 0x7f7f7f, 0xc00000, 0xff0000, 0xffc000, 0xffff00, 0x92d050, 0x00b050, 0x00b0f0,
    0x0070c0, 0x002060, 0x7030a0,
];
/// Line spacing (multiples of single) and space after (points), PowerPoint's
/// Line Spacing list.
const LINE_SPACINGS: [f64; 4] = [1.0, 1.15, 1.5, 2.0];
const SPACES_AFTER: [f64; 3] = [0.0, 6.0, 12.0];

/// A format strip button: an icon, shaded while what it sets is on.
fn format_button(
    id: &'static str,
    icon: &'static str,
    tip: &'static str,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .size(px(26.0))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .rounded_md()
        .cursor_pointer()
        .when(on, |button| button.bg(palette::selected()))
        .hover(|button| button.bg(palette::hovered()))
        .tooltip(Tooltip::text(i18n::text(tip)))
        .child(svg().path(icon).size_4().text_color(palette::dim()))
}

/// A list trigger: its value and a chevron.
fn format_list(
    id: &'static str,
    width: f32,
    value: String,
    open: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .w(px(width))
        .h(px(24.0))
        .pl_2()
        .pr_1()
        .rounded_md()
        .border_1()
        .border_color(if open {
            palette::accent()
        } else {
            palette::border()
        })
        .cursor_pointer()
        .hover(|button| button.bg(palette::hovered()))
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
                .text_color(palette::dim()),
        )
}

/// A trigger with its list under it while open.
fn with_menu(trigger: gpui::AnyElement, menu: Option<gpui::AnyElement>) -> gpui::AnyElement {
    div()
        .relative()
        .flex_none()
        .child(trigger)
        .children(menu.map(|menu| {
            div().absolute().top_full().left_0().child(
                gpui::deferred(
                    gpui::anchored()
                        .snap_to_window_with_margin(px(8.0))
                        .child(menu),
                )
                .with_priority(2),
            )
        }))
        .into_any_element()
}

/// The panel a list is drawn on.
fn menu_panel(id: &'static str) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .occlude()
        .mt_1()
        .p_1()
        .flex()
        .flex_col()
        .max_h(px(360.0))
        .overflow_y_scroll()
        .rounded_md()
        .bg(palette::panel())
        .border_1()
        .border_color(palette::border())
        .shadow_lg()
        .text_xs()
        .text_color(palette::text())
}

/// One row of a list, checked when it is the current value.
fn menu_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    checked: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .gap_2()
        .h(px(26.0))
        .pl_1()
        .pr_3()
        .rounded_md()
        .whitespace_nowrap()
        .cursor_pointer()
        .hover(|row| row.bg(palette::hovered()))
        .child(div().size_4().flex_none().when(checked, |cell| {
            cell.child(
                svg()
                    .path("icons/check.svg")
                    .size_4()
                    .text_color(palette::accent()),
            )
        }))
        .child(label.into())
}

fn size_label(size: f32) -> String {
    if size.fract() == 0.0 {
        format!("{size:.0}")
    } else {
        format!("{size}")
    }
}

impl DocView {
    fn on_format(command: TextCommand, cx: &mut Context<Self>) -> ClickHandler {
        Box::new(
            cx.listener(move |view, _event: &gpui::ClickEvent, _window, cx| {
                view.format_slide_text(command.clone(), cx)
            }),
        )
    }

    fn on_popup(popup: Popup, cx: &mut Context<Self>) -> ClickHandler {
        Box::new(
            cx.listener(move |view, _event: &gpui::ClickEvent, _window, cx| {
                view.toggle_popup(popup, cx)
            }),
        )
    }

    /// The format strip, PowerPoint's Font and Paragraph groups, over the
    /// text typed into or all of the selected shape's.
    fn format_bar(
        &self,
        strip: &FormatStrip,
        open: Option<Popup>,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let font = strip.font.clone().unwrap_or_default();
        let font_menu = (open == Some(Popup::Font)).then(|| {
            let mut fonts: Vec<String> = FONT_FAMILIES.iter().map(|f| f.to_string()).collect();
            if !font.is_empty() && !fonts.contains(&font) {
                fonts.insert(0, font.clone());
            }
            menu_panel("deck-fonts")
                .children(fonts.into_iter().enumerate().map(|(index, name)| {
                    menu_row(("deck-font", index), name.clone(), name == font)
                        .font_family(SharedString::from(name.clone()))
                        .on_click(Self::on_format(TextCommand::Font(name), cx))
                }))
                .into_any_element()
        });
        let size_menu = (open == Some(Popup::Size)).then(|| {
            menu_panel("deck-sizes")
                .children(FONT_SIZES.iter().enumerate().map(|(index, &size)| {
                    menu_row(
                        ("deck-size", index),
                        size_label(size),
                        strip.size == Some(size),
                    )
                    .on_click(Self::on_format(TextCommand::Size(size), cx))
                }))
                .into_any_element()
        });
        let color_menu = (open == Some(Popup::Color)).then(|| {
            menu_panel("deck-colors")
                .child(div().w(px(6.0 * 24.0)).flex().flex_wrap().gap_1().children(
                    FONT_COLORS.iter().enumerate().map(|(index, &color)| {
                        div()
                            .id(("deck-color", index))
                            .size(px(20.0))
                            .rounded_sm()
                            .bg(rgb(color))
                            .border_1()
                            .border_color(palette::border())
                            .cursor_pointer()
                            .hover(|square| square.border_color(palette::accent()))
                            .on_click(Self::on_format(
                                TextCommand::Color(format!("#{color:06X}")),
                                cx,
                            ))
                    }),
                ))
                .into_any_element()
        });
        let spacing_menu = (open == Some(Popup::Spacing)).then(|| {
            menu_panel("deck-spacing")
                .children(LINE_SPACINGS.iter().enumerate().map(|(index, &lines)| {
                    menu_row(
                        ("deck-line-spacing", index),
                        format!("{lines:.2}")
                            .trim_end_matches('0')
                            .trim_end_matches('.')
                            .to_string(),
                        false,
                    )
                    .on_click(Self::on_format(TextCommand::LineSpacing(lines), cx))
                }))
                .children(SPACES_AFTER.iter().enumerate().map(|(index, &points)| {
                    menu_row(
                        ("deck-space-after", index),
                        i18n::format("deck.space_after", &[("n", &points)]),
                        false,
                    )
                    .on_click(Self::on_format(TextCommand::SpaceAfter(points), cx))
                }))
                .into_any_element()
        });
        let separator = || div().w(px(1.0)).h(px(18.0)).mx_1().bg(palette::border());
        let alignment = strip.alignment;
        div()
            .h(px(FORMAT_BAR_HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .gap_0p5()
            .px_3()
            .bg(palette::panel())
            .border_b_1()
            .border_color(palette::border())
            .text_color(palette::text())
            .child(with_menu(
                format_list("deck-font", 150.0, font, open == Some(Popup::Font))
                    .tooltip(Tooltip::text(i18n::text("deck.font")))
                    .on_click(Self::on_popup(Popup::Font, cx))
                    .into_any_element(),
                font_menu,
            ))
            .child(with_menu(
                format_list(
                    "deck-size",
                    56.0,
                    strip.size.map(size_label).unwrap_or_default(),
                    open == Some(Popup::Size),
                )
                .tooltip(Tooltip::text(i18n::text("deck.font_size")))
                .on_click(Self::on_popup(Popup::Size, cx))
                .into_any_element(),
                size_menu,
            ))
            .child(
                format_button("deck-grow", "icons/a-arrow-up.svg", "deck.grow_font", false)
                    .on_click(Self::on_format(TextCommand::Grow, cx)),
            )
            .child(
                format_button(
                    "deck-shrink",
                    "icons/a-arrow-down.svg",
                    "deck.shrink_font",
                    false,
                )
                .on_click(Self::on_format(TextCommand::Shrink, cx)),
            )
            .child(separator())
            .child(
                format_button("deck-bold", "icons/bold.svg", "deck.bold", strip.bold)
                    .on_click(Self::on_format(TextCommand::Bold, cx)),
            )
            .child(
                format_button(
                    "deck-italic",
                    "icons/italic.svg",
                    "deck.italic",
                    strip.italic,
                )
                .on_click(Self::on_format(TextCommand::Italic, cx)),
            )
            .child(
                format_button(
                    "deck-underline",
                    "icons/underline.svg",
                    "deck.underline",
                    strip.underline,
                )
                .on_click(Self::on_format(TextCommand::Underline, cx)),
            )
            .child(with_menu(
                format_button(
                    "deck-color",
                    "icons/baseline.svg",
                    "deck.font_color",
                    open == Some(Popup::Color),
                )
                .on_click(Self::on_popup(Popup::Color, cx))
                .into_any_element(),
                color_menu,
            ))
            .child(separator())
            .children(
                [
                    (
                        ParagraphAlignment::Left,
                        "deck-align-left",
                        "icons/align-left.svg",
                        "deck.align_left",
                    ),
                    (
                        ParagraphAlignment::Center,
                        "deck-align-center",
                        "icons/align-center.svg",
                        "deck.align_center",
                    ),
                    (
                        ParagraphAlignment::Right,
                        "deck-align-right",
                        "icons/align-right.svg",
                        "deck.align_right",
                    ),
                    (
                        ParagraphAlignment::Justify,
                        "deck-justify",
                        "icons/align-justify.svg",
                        "deck.justify",
                    ),
                ]
                .into_iter()
                .map(|(value, id, icon, tip)| {
                    format_button(id, icon, tip, alignment == Some(value))
                        .on_click(Self::on_format(TextCommand::Align(value), cx))
                }),
            )
            .child(separator())
            .child(
                format_button(
                    "deck-bullets",
                    "icons/list.svg",
                    "deck.bullets",
                    strip.bullets,
                )
                .on_click(Self::on_format(TextCommand::Bullets, cx)),
            )
            .child(
                format_button(
                    "deck-numbering",
                    "icons/list-ordered.svg",
                    "deck.numbering",
                    strip.numbering,
                )
                .on_click(Self::on_format(TextCommand::Numbering, cx)),
            )
            .child(with_menu(
                format_button(
                    "deck-line-spacing",
                    "icons/chevrons-up-down.svg",
                    "deck.line_spacing",
                    open == Some(Popup::Spacing),
                )
                .on_click(Self::on_popup(Popup::Spacing, cx))
                .into_any_element(),
                spacing_menu,
            ))
    }
}

/// Shapes ▾: the shapes Insert offers; one picked is drawn next.
fn shapes_button(
    inserting: Option<Insert>,
    open: bool,
    cx: &mut Context<DocView>,
) -> gpui::AnyElement {
    let menu = open.then(|| {
        menu_panel("deck-shapes")
            .children(SHAPE_PRESETS.iter().enumerate().map(|(index, &preset)| {
                menu_row(
                    ("deck-shape", index),
                    i18n::text(shape_name(preset)),
                    inserting == Some(Insert::Shape(preset)),
                )
                .on_click(cx.listener(move |view, _event, _window, cx| {
                    view.start_insert(Insert::Shape(preset), cx)
                }))
            }))
            .into_any_element()
    });
    let drawing_shape = matches!(inserting, Some(Insert::Shape(_)));
    with_menu(
        toolbar_button(
            "insert-shape",
            "icons/shapes.svg",
            i18n::text("deck.shapes"),
        )
        .when(open || drawing_shape, |button| {
            button.bg(palette::selected())
        })
        .on_click(cx.listener(|view, _event, _window, cx| view.toggle_popup(Popup::Shapes, cx)))
        .into_any_element(),
        menu,
    )
}

/// The words for a shape in Insert's list, as PowerPoint names them.
fn shape_name(preset: ShapePreset) -> &'static str {
    match preset {
        ShapePreset::Rect => "deck.shape.rect",
        ShapePreset::RoundRect => "deck.shape.round_rect",
        ShapePreset::Ellipse => "deck.shape.ellipse",
        ShapePreset::Triangle => "deck.shape.triangle",
        ShapePreset::Diamond => "deck.shape.diamond",
        ShapePreset::Hexagon => "deck.shape.hexagon",
    }
}

/// The notes being typed, a row a line, the caret drawn in its line.
fn notes_lines(edit: &TextEdit) -> Vec<gpui::AnyElement> {
    let caret = edit.caret();
    let mut start = 0;
    let mut rows = Vec::new();
    for line in edit.text.split('\n') {
        let end = start + line.len();
        let row = div().flex().min_h(px(18.0));
        rows.push(if (start..=end).contains(&caret) {
            let at = caret - start;
            row.child(line[..at].to_string())
                .child(div().w(px(1.5)).h(px(16.0)).bg(palette::text()))
                .child(line[at..].to_string())
                .into_any_element()
        } else {
            row.child(line.to_string()).into_any_element()
        });
        start = end + 1;
    }
    rows
}
