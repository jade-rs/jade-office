//! An editable presentation, painted through a [`Backend`].
//!
//! Edits go through jade-pptx-writer's `EditSession`, the session the
//! browser editor uses: each change rewrites the slide part in the package
//! bytes and keeps undo history, and the deck is read back from those bytes
//! to lay out and paint. A shell hit-tests through [`Deck::shape_at`] and
//! edits through [`Deck::set_text`], [`Deck::set_shape_rect`] and the rest.
//! Text is typed into a [`TextTarget`]: a shape, or a cell of a table.

use std::collections::HashMap;
use std::rc::Rc;

use jade_painter::{ImageRect, ImageSource, Painter};
use jade_pptx_layout::{layout_deck_slide, PlacedOrigin, PlacedShape, SlideLayout};
pub use jade_pptx_layout::{TextFormatState, TextSelectionGeometry};
use jade_pptx_reader::{emu_to_px, Transform};
use jade_pptx_render::{render_slide_with, PainterMeasure, RenderOptions};
use jade_pptx_writer::EditSession;
pub use jade_pptx_writer::{ShapeTransformInfo, SlideLayoutInfo, TableChange, TextFormat};

mod text;
pub use text::{Motion, ShapeText};

use crate::Backend;

/// EMU per px at 96 dpi.
const EMU_PER_PX: f64 = 9525.0;

/// A shape's box in slide px.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ShapeRect {
    pub fn of(transform: &Transform) -> Self {
        Self {
            x: emu_to_px(transform.offset_emu.0),
            y: emu_to_px(transform.offset_emu.1),
            width: emu_to_px(transform.extent_emu.0),
            height: emu_to_px(transform.extent_emu.1),
        }
    }

    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    /// `transform` moved and sized to this box, its rotation and flips kept.
    pub fn apply_to(&self, transform: Transform) -> Transform {
        Transform {
            offset_emu: (
                (self.x * EMU_PER_PX).round() as i64,
                (self.y * EMU_PER_PX).round() as i64,
            ),
            extent_emu: (
                (self.width * EMU_PER_PX).round() as i64,
                (self.height * EMU_PER_PX).round() as i64,
            ),
            ..transform
        }
    }
}

/// The smallest a resize leaves a shape, in slide px.
pub const MIN_SHAPE_PX: f64 = 4.0;

/// What a drag on a slide does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragMode {
    Move,
    /// A corner handle: which edges it moves, as (left?, top?). The opposite
    /// corner stays put.
    Resize {
        left: bool,
        top: bool,
    },
}

/// A mouse drag on a shape, from press to release: the shells draw
/// [`ShapeDrag::rect`] while it lasts and write it with
/// [`Deck::drag_shape`] on release.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShapeDrag {
    pub id: u32,
    pub mode: DragMode,
    /// Where the drag started, in slide px.
    from: (f64, f64),
    /// How far it has gone, in slide px.
    delta: (f64, f64),
}

impl ShapeDrag {
    pub fn new(id: u32, mode: DragMode, from: (f64, f64)) -> Self {
        Self {
            id,
            mode,
            from,
            delta: (0.0, 0.0),
        }
    }

    /// The pointer is now at `(x, y)` slide px.
    pub fn drag_to(&mut self, x: f64, y: f64) {
        self.delta = (x - self.from.0, y - self.from.1);
    }

    /// Whether the pointer has gone far enough to count as a drag rather
    /// than a click.
    pub fn moved(&self) -> bool {
        self.delta.0.abs() >= 0.5 || self.delta.1.abs() >= 0.5
    }

    /// `rect`, the shape's box when the drag started, where the drag has
    /// taken it.
    pub fn rect(&self, rect: ShapeRect) -> ShapeRect {
        rect.dragged(self.mode, self.delta.0, self.delta.1)
    }
}

impl ShapeRect {
    /// This box after dragging by `(dx, dy)` slide px in `mode`: a move
    /// keeps the size, a corner keeps the opposite corner, and no side gets
    /// shorter than [`MIN_SHAPE_PX`].
    pub fn dragged(self, mode: DragMode, dx: f64, dy: f64) -> Self {
        match mode {
            DragMode::Move => Self {
                x: self.x + dx,
                y: self.y + dy,
                ..self
            },
            DragMode::Resize { left, top } => {
                let (x, width) = if left {
                    let width = (self.width - dx).max(MIN_SHAPE_PX);
                    (self.x + self.width - width, width)
                } else {
                    (self.x, (self.width + dx).max(MIN_SHAPE_PX))
                };
                let (y, height) = if top {
                    let height = (self.height - dy).max(MIN_SHAPE_PX);
                    (self.y + self.height - height, height)
                } else {
                    (self.y, (self.height + dy).max(MIN_SHAPE_PX))
                };
                Self {
                    x,
                    y,
                    width,
                    height,
                }
            }
        }
    }
}

/// What typed text goes into: a shape's text body, or one cell of a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextTarget {
    Shape(u32),
    Cell { id: u32, row: usize, column: usize },
}

impl TextTarget {
    /// The shape — for a cell, its table — on the slide.
    pub fn id(&self) -> u32 {
        match *self {
            Self::Shape(id) | Self::Cell { id, .. } => id,
        }
    }
}

pub struct Deck<B: Backend> {
    session: EditSession,
    deck: jade_pptx_reader::Deck,
    parts: HashMap<String, Vec<u8>>,
    backend: B,
    options: RenderOptions,
    layouts: HashMap<usize, Rc<SlideLayout>>,
    shapes: HashMap<usize, Vec<ShapeTransformInfo>>,
}

impl<B: Backend> Deck<B> {
    pub fn open(bytes: &[u8], backend: B) -> Result<Self, String> {
        let session = EditSession::open(bytes)
            .map_err(|error| format!("cannot read the presentation: {error}"))?;
        let (deck, parts) = read(bytes)?;
        if deck.slides.is_empty() {
            return Err("the presentation has no slides".to_string());
        }
        Ok(Self {
            session,
            deck,
            parts,
            backend,
            options: RenderOptions::default(),
            layouts: HashMap::new(),
            shapes: HashMap::new(),
        })
    }

    /// Reads the deck back from the session's bytes after a change.
    fn reload(&mut self) -> Result<(), String> {
        let (deck, parts) = read(self.session.bytes())?;
        self.deck = deck;
        self.parts = parts;
        self.layouts.clear();
        self.shapes.clear();
        Ok(())
    }

    pub fn slide_count(&self) -> usize {
        self.deck.slides.len()
    }

    pub fn slide_size(&self) -> (f64, f64) {
        (
            self.deck.slide_size.width_px(),
            self.deck.slide_size.height_px(),
        )
    }

    pub fn notes(&self, index: usize) -> Option<&str> {
        self.deck.slides.get(index)?.notes.as_deref()
    }

    /// The presentation's current bytes — what a slide show opens, or a save
    /// writes.
    pub fn bytes(&self) -> &[u8] {
        self.session.bytes()
    }

    /// What text is measured with: the backend's fonts, as the slides are
    /// painted.
    fn measure(&self) -> Option<PainterMeasure<B::Painter>> {
        let measuring = self.backend.surface(1, 1)?;
        Some(PainterMeasure::new(
            measuring,
            self.options.default_font.clone(),
        ))
    }

    /// The laid-out slide, measured once and cached until the next change.
    pub fn layout(&mut self, index: usize) -> Option<Rc<SlideLayout>> {
        if let Some(layout) = self.layouts.get(&index) {
            return Some(Rc::clone(layout));
        }
        let measure = self.measure()?;
        let layout = Rc::new(layout_deck_slide(&self.deck, index, &measure)?);
        self.layouts.insert(index, Rc::clone(&layout));
        Some(layout)
    }

    /// The slide's shapes as the writer sees them: what can be selected,
    /// moved, resized, deleted.
    pub fn shapes(&mut self, slide: usize) -> &[ShapeTransformInfo] {
        let session = &self.session;
        self.shapes.entry(slide).or_insert_with(|| {
            jade_pptx_writer::editable_shapes(session.bytes(), slide).unwrap_or_default()
        })
    }

    pub fn shape(&mut self, slide: usize, id: u32) -> Option<ShapeTransformInfo> {
        self.shapes(slide)
            .iter()
            .find(|shape| shape.id == id)
            .cloned()
    }

    fn selectable(&mut self, slide: usize) -> Vec<u32> {
        self.shapes(slide)
            .iter()
            .filter(|shape| shape.can_select && shape.transform.is_some())
            .map(|shape| shape.id)
            .collect()
    }

    /// The selectable shape at a slide point: the top-most of the slide's own
    /// shapes there, or the group it belongs to — what PowerPoint selects on
    /// a first click.
    pub fn shape_at(&mut self, slide: usize, x: f64, y: f64) -> Option<u32> {
        let layout = self.layout(slide)?;
        let selectable = self.selectable(slide);
        layout
            .shapes
            .iter()
            .rev()
            .filter(|placed| placed.origin == PlacedOrigin::Slide)
            .filter(|placed| {
                let rect = placed.rect;
                x >= rect.x && x <= rect.x + rect.width && y >= rect.y && y <= rect.y + rect.height
            })
            .find_map(|placed| {
                placed
                    .group_ids
                    .iter()
                    .copied()
                    .chain(placed.source_id)
                    .find(|id| selectable.contains(id))
            })
    }

    /// The next selectable shape after `current` (or before, `backwards`):
    /// Tab's order.
    pub fn next_shape(
        &mut self,
        slide: usize,
        current: Option<u32>,
        backwards: bool,
    ) -> Option<u32> {
        let selectable = self.selectable(slide);
        if selectable.is_empty() {
            return None;
        }
        let at = current.and_then(|id| selectable.iter().position(|candidate| *candidate == id));
        let next = match (at, backwards) {
            (None, false) => 0,
            (None, true) => selectable.len() - 1,
            (Some(at), false) => (at + 1) % selectable.len(),
            (Some(at), true) => (at + selectable.len() - 1) % selectable.len(),
        };
        Some(selectable[next])
    }

    pub fn shape_rect(&mut self, slide: usize, id: u32) -> Option<ShapeRect> {
        self.shape(slide, id)?.transform.as_ref().map(ShapeRect::of)
    }

    /// The shape's text, when it has a text body that may be edited.
    pub fn shape_text(&self, slide: usize, id: u32) -> Option<String> {
        let snapshot = jade_pptx_writer::shape_text(self.session.bytes(), slide, id).ok()?;
        (snapshot.can_edit && !snapshot.locked).then_some(snapshot.text)
    }

    fn after(&mut self, result: jade_pptx_writer::Result<bool>) -> Result<bool, String> {
        let changed = result.map_err(|error| error.to_string())?;
        if changed {
            self.reload()?;
        }
        Ok(changed)
    }

    /// The character formatting of `range` (UTF-16 offsets) of shape
    /// `id`'s text, as inherited and drawn; a field is `None` where the
    /// range mixes values.
    pub fn text_format(
        &self,
        slide: usize,
        id: u32,
        range: std::ops::Range<usize>,
    ) -> Option<TextFormatState> {
        jade_pptx_layout::text_format_for_deck_shape(&self.deck, slide, id, range.start, range.end)
    }

    /// Ends a run of typing: what is typed next is a new step of Undo.
    pub fn end_typing(&mut self) {
        self.session.end_text_history();
    }

    /// Replaces a shape's text, as one undoable change.
    pub fn set_shape_text(&mut self, slide: usize, id: u32, text: &str) -> Result<bool, String> {
        let result = self.session.set_shape_text(slide, id, text, false);
        self.after(result)
    }

    /// Where text typed into shape `id` goes, from a click at `point` or,
    /// without one, from Enter or typing: in a table, the cell under the
    /// point — a merged cell where it starts — or the first cell, as in
    /// PowerPoint; anything else takes the text itself.
    pub fn text_target(&mut self, slide: usize, id: u32, point: Option<(f64, f64)>) -> TextTarget {
        if self
            .shape(slide, id)
            .is_none_or(|shape| shape.kind != "table")
        {
            return TextTarget::Shape(id);
        }
        let cells = self.placed_cells(slide, id);
        let hit = match point {
            Some((x, y)) => cells.iter().find(|(_, _, rect)| rect.contains(x, y)),
            None => cells.iter().min_by_key(|(row, column, _)| (*row, *column)),
        };
        match hit {
            Some(&(row, column, _)) => TextTarget::Cell { id, row, column },
            None => TextTarget::Shape(id),
        }
    }

    /// The table's cells as laid out: row, column and box in slide px. A
    /// merged cell is placed once, where it starts, spanning the cells it
    /// covers.
    fn placed_cells(&mut self, slide: usize, id: u32) -> Vec<(usize, usize, ShapeRect)> {
        let Some(layout) = self.layout(slide) else {
            return Vec::new();
        };
        layout
            .shapes
            .iter()
            .filter(|placed| placed.origin == PlacedOrigin::Slide && placed.source_id == Some(id))
            .filter_map(|placed| {
                // Cells are named `table!B3`; their borders `table!B3:left`.
                let (_, cell) = placed.name.rsplit_once('!')?;
                let (row, column) = cell_position(cell)?;
                let rect = placed.rect;
                Some((
                    row,
                    column,
                    ShapeRect {
                        x: rect.x,
                        y: rect.y,
                        width: rect.width,
                        height: rect.height,
                    },
                ))
            })
            .collect()
    }

    /// The box text typed into `target` is shown in, in slide px.
    pub fn text_rect(&mut self, slide: usize, target: TextTarget) -> Option<ShapeRect> {
        match target {
            TextTarget::Shape(id) => self.shape_rect(slide, id),
            TextTarget::Cell { id, row, column } => self
                .placed_cells(slide, id)
                .into_iter()
                .find(|cell| (cell.0, cell.1) == (row, column))
                .map(|cell| cell.2),
        }
    }

    /// The text of `target`, when it may be edited: a shape's unlocked text,
    /// or a table cell's — not one covered by a merge.
    pub fn text(&self, slide: usize, target: TextTarget) -> Option<String> {
        match target {
            TextTarget::Shape(id) => self.shape_text(slide, id),
            TextTarget::Cell { id, row, column } => {
                let cells = jade_pptx_writer::table_cells(self.session.bytes(), slide, id).ok()?;
                let cell = cells.get(row)?.get(column)?;
                (!cell.merged).then(|| cell.text.clone())
            }
        }
    }

    /// The cell Tab goes to from `from` (⇧Tab: `backwards`): along the row,
    /// then the next row's first, a merged cell being one stop. `None` past
    /// either end, and for a shape.
    pub fn next_cell(&self, slide: usize, from: TextTarget, backwards: bool) -> Option<TextTarget> {
        let TextTarget::Cell { id, row, column } = from else {
            return None;
        };
        let cells = jade_pptx_writer::table_cells(self.session.bytes(), slide, id).ok()?;
        let stops: Vec<(usize, usize)> = cells
            .iter()
            .enumerate()
            .flat_map(|(r, cells)| {
                cells
                    .iter()
                    .enumerate()
                    .filter(|(_, cell)| !cell.merged)
                    .map(move |(c, _)| (r, c))
            })
            .collect();
        let at = stops.iter().position(|stop| *stop == (row, column))?;
        let next = if backwards {
            at.checked_sub(1)?
        } else {
            at + 1
        };
        stops
            .get(next)
            .map(|&(row, column)| TextTarget::Cell { id, row, column })
    }

    /// The box, in slide px, of the cells from `from` to `to` (row, column)
    /// of table `id`, either way round: what a selection across them covers.
    pub fn cell_range_rect(
        &mut self,
        slide: usize,
        id: u32,
        from: (usize, usize),
        to: (usize, usize),
    ) -> Option<ShapeRect> {
        let (rows, columns) = (
            from.0.min(to.0)..=from.0.max(to.0),
            from.1.min(to.1)..=from.1.max(to.1),
        );
        self.placed_cells(slide, id)
            .into_iter()
            .filter(|(row, column, _)| rows.contains(row) && columns.contains(column))
            .map(|(_, _, rect)| rect)
            .reduce(|a, b| {
                let (x, y) = (a.x.min(b.x), a.y.min(b.y));
                ShapeRect {
                    x,
                    y,
                    width: (a.x + a.width).max(b.x + b.width) - x,
                    height: (a.y + a.height).max(b.y + b.height) - y,
                }
            })
    }

    /// How many rows and columns a table cell spans; `None` for a shape, or
    /// a cell a merge covers.
    pub fn cell_span(&self, slide: usize, target: TextTarget) -> Option<(usize, usize)> {
        let TextTarget::Cell { id, row, column } = target else {
            return None;
        };
        let cells = jade_pptx_writer::table_cells(self.session.bytes(), slide, id).ok()?;
        let cell = cells.get(row)?.get(column)?;
        (!cell.merged).then_some((cell.row_span, cell.column_span))
    }

    /// Changes table `id`'s layout — a row or column added or removed, cells
    /// merged or split — as one undoable change.
    pub fn change_table(
        &mut self,
        slide: usize,
        id: u32,
        change: TableChange,
    ) -> Result<bool, String> {
        let result = self.session.change_table(slide, id, change);
        self.after(result)
    }

    /// Replaces the text of `target`, as one undoable change.
    pub fn set_text(
        &mut self,
        slide: usize,
        target: TextTarget,
        text: &str,
    ) -> Result<bool, String> {
        match target {
            TextTarget::Shape(id) => self.set_shape_text(slide, id, text),
            TextTarget::Cell { id, row, column } => {
                let result = self
                    .session
                    .set_table_cell_text(slide, id, row, column, text);
                self.after(result)
            }
        }
    }

    /// Moves and sizes a shape to `rect`, its rotation and flips kept.
    pub fn set_shape_rect(
        &mut self,
        slide: usize,
        id: u32,
        rect: ShapeRect,
    ) -> Result<bool, String> {
        let Some(transform) = self.shape(slide, id).and_then(|shape| shape.transform) else {
            return Ok(false);
        };
        let next = rect.apply_to(transform);
        if next == transform {
            return Ok(false);
        }
        let result = self.session.set_shape_transform(slide, id, next);
        self.after(result)
    }

    /// Moves a shape by `(dx, dy)` slide px, if it may move.
    pub fn move_shape(&mut self, slide: usize, id: u32, dx: f64, dy: f64) -> Result<bool, String> {
        let Some(shape) = self.shape(slide, id) else {
            return Ok(false);
        };
        let (Some(transform), true) = (shape.transform, shape.can_move) else {
            return Ok(false);
        };
        let rect = ShapeRect::of(&transform);
        self.set_shape_rect(
            slide,
            id,
            ShapeRect {
                x: rect.x + dx,
                y: rect.y + dy,
                ..rect
            },
        )
    }

    /// Writes a finished mouse drag, as one undoable change: a move if the
    /// shape may move, a corner if it may resize — nothing for a press that
    /// never went anywhere.
    pub fn drag_shape(&mut self, slide: usize, drag: &ShapeDrag) -> Result<bool, String> {
        let Some(shape) = self.shape(slide, drag.id) else {
            return Ok(false);
        };
        let allowed = match drag.mode {
            DragMode::Move => shape.can_move,
            DragMode::Resize { .. } => shape.can_resize,
        };
        let (Some(transform), true, true) = (shape.transform, allowed, drag.moved()) else {
            return Ok(false);
        };
        self.set_shape_rect(slide, drag.id, drag.rect(ShapeRect::of(&transform)))
    }

    // ---- slides --------------------------------------------------------------

    /// Whether slide `index` is hidden: kept in the deck, skipped in a show.
    pub fn is_hidden(&self, index: usize) -> bool {
        self.deck
            .slides
            .get(index)
            .is_some_and(|slide| slide.content.hidden)
    }

    /// The next slide after `from` (before, unless `forward`) that a slide
    /// show plays: hidden ones are skipped. `None` past either end.
    pub fn next_shown(&self, from: usize, forward: bool) -> Option<usize> {
        let mut index = from;
        loop {
            index = if forward {
                index + 1
            } else {
                index.checked_sub(1)?
            };
            if index >= self.slide_count() {
                return None;
            }
            if !self.is_hidden(index) {
                return Some(index);
            }
        }
    }

    /// Where a show from the beginning starts: the first slide not hidden,
    /// or the first slide when every one is.
    pub fn first_shown(&self) -> usize {
        (0..self.slide_count())
            .find(|&index| !self.is_hidden(index))
            .unwrap_or(0)
    }

    /// New Slide: an empty slide at `index`, on the layout of the slide it
    /// follows, its placeholders empty.
    pub fn insert_slide(&mut self, index: usize) -> Result<bool, String> {
        let index = index.min(self.slide_count());
        let result = self.session.insert_slide(index);
        self.after(result)
    }

    /// Duplicate Slide: a copy straight after slide `index`.
    pub fn duplicate_slide(&mut self, index: usize) -> Result<bool, String> {
        let result = self.session.duplicate_slide(index);
        self.after(result)
    }

    /// Deletes slide `index` — not the last one left, which the views need
    /// to show something.
    pub fn delete_slide(&mut self, index: usize) -> Result<bool, String> {
        if self.slide_count() <= 1 || index >= self.slide_count() {
            return Ok(false);
        }
        let result = self.session.delete_slide(index);
        self.after(result)
    }

    /// Moves slide `from` so it ends up at `to`.
    pub fn move_slide(&mut self, from: usize, to: usize) -> Result<bool, String> {
        let to = to.min(self.slide_count().saturating_sub(1));
        if from == to {
            return Ok(false);
        }
        let result = self.session.move_slide(from, to);
        self.after(result)
    }

    /// Hide Slide, or show it again.
    pub fn set_slide_hidden(&mut self, index: usize, hidden: bool) -> Result<bool, String> {
        let result = self.session.set_slide_hidden(index, hidden);
        self.after(result)
    }

    /// The layouts slide `index` can take, the one it has marked `current`.
    pub fn slide_layouts(&self, index: usize) -> Vec<SlideLayoutInfo> {
        jade_pptx_writer::slide_layouts(self.session.bytes(), index).unwrap_or_default()
    }

    /// Puts slide `index` on the layout `part_name` names, its placeholders
    /// moved to the layout's.
    pub fn set_slide_layout(&mut self, index: usize, part_name: &str) -> Result<bool, String> {
        let result = self.session.set_slide_layout(index, part_name);
        self.after(result)
    }

    pub fn delete_shape(&mut self, slide: usize, id: u32) -> Result<bool, String> {
        if !self.shape(slide, id).is_some_and(|shape| shape.can_delete) {
            return Ok(false);
        }
        let result = self.session.delete_shape(slide, id);
        self.after(result)
    }

    pub fn undo(&mut self) -> Result<bool, String> {
        let undone = self.session.undo();
        if undone {
            self.reload()?;
        }
        Ok(undone)
    }

    pub fn redo(&mut self) -> Result<bool, String> {
        let redone = self.session.redo();
        if redone {
            self.reload()?;
        }
        Ok(redone)
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty()
    }

    /// The presentation as `.pptx` bytes, marking it saved.
    pub fn save_bytes(&mut self) -> Vec<u8> {
        self.session.mark_saved();
        self.session.bytes().to_vec()
    }

    /// Paints slide `index` `width` device px wide.
    pub fn render_slide(&mut self, index: usize, width: u32) -> Option<B::Painter> {
        let (slide_width, slide_height) = self.slide_size();
        let scale = f64::from(width.max(1)) / slide_width.max(1.0);
        let height = (slide_height * scale).round().max(1.0) as u32;
        let painter = self.backend.surface(width.max(1), height)?;
        self.paint(&painter, index, scale)?;
        Some(painter)
    }

    /// Paints slide `index` onto `painter` at `scale` device px per slide px.
    pub fn paint(&mut self, painter: &B::Painter, index: usize, scale: f64) -> Option<()> {
        let layout = self.layout(index)?;
        painter.save();
        let scaled = painter.scale(scale, scale).is_ok();
        let parts = &self.parts;
        let mut draw_image = |shape: &PlacedShape| -> bool {
            let Some((part, bytes)) = shape.image_part.as_deref().and_then(|part| {
                parts
                    .get(part.trim_start_matches('/'))
                    .or_else(|| parts.get(part))
                    .map(|bytes| (part, bytes))
            }) else {
                return false;
            };
            // The crop is relative to the bitmap's own size; against a unit
            // square it is the normalised source rectangle `draw_image` takes.
            let Some((source, destination)) = shape.image_rectangles(1.0, 1.0) else {
                return false;
            };
            painter
                .draw_image(
                    ImageSource { key: part, bytes },
                    Some([source.x, source.y, source.width, source.height]),
                    ImageRect {
                        x: destination.x,
                        y: destination.y,
                        width: destination.width,
                        height: destination.height,
                    },
                )
                .unwrap_or(false)
        };
        let painted =
            scaled && render_slide_with(painter, &layout, &self.options, &mut draw_image).is_ok();
        painter.restore();
        painted.then_some(())
    }
}

#[cfg(feature = "skia")]
impl Deck<crate::skia::SkiaBackend> {
    /// Every slide as one vector PDF page, at the slide's own size.
    pub fn pdf(&mut self) -> Result<Vec<u8>, String> {
        use jade_skia_painter::SkiaPainter;
        let (width, height) = self.slide_size();
        let mut pages = Vec::with_capacity(self.slide_count());
        for index in 0..self.slide_count() {
            let page = SkiaPainter::new_recording(width, height)
                .ok_or_else(|| "cannot create a PDF page".to_string())?;
            self.backend.register_on(&page);
            self.paint(&page, index, 1.0)
                .ok_or_else(|| format!("cannot paint slide {}", index + 1))?;
            pages.push(page);
        }
        SkiaPainter::encode_pdf(&pages)
    }
}

/// The merge of the cells from `from` to `to` (row, column), either way
/// round: what Merge Cells does to a selection across them.
pub fn merge_cells(from: (usize, usize), to: (usize, usize)) -> TableChange {
    TableChange::MergeCells {
        row: from.0.min(to.0),
        column: from.1.min(to.1),
        rows: from.0.abs_diff(to.0) + 1,
        columns: from.1.abs_diff(to.1) + 1,
    }
}

/// `B3` as (row 2, column 1).
fn cell_position(name: &str) -> Option<(usize, usize)> {
    let split = name.find(|c: char| !c.is_ascii_uppercase())?;
    let (letters, digits) = name.split_at(split);
    if letters.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let column = letters
        .bytes()
        .fold(0usize, |n, letter| n * 26 + usize::from(letter - b'A' + 1));
    let row: usize = digits.parse().ok()?;
    Some((row.checked_sub(1)?, column - 1))
}

/// The parsed deck and its media parts, keyed by name.
fn read(bytes: &[u8]) -> Result<(jade_pptx_reader::Deck, HashMap<String, Vec<u8>>), String> {
    let deck = jade_pptx_reader::read_deck(bytes)
        .map_err(|error| format!("cannot read the presentation: {error}"))?;
    let package = jade_opc::OpcPackage::open(bytes)
        .map_err(|error| format!("cannot open the package: {error}"))?;
    let parts = package
        .part_names()
        .iter()
        .filter(|name| name.contains("/media/"))
        .filter_map(|name| Some((name.clone(), package.part(name)?.to_vec())))
        .collect();
    Ok((deck, parts))
}

#[cfg(all(test, feature = "tiny-skia"))]
mod tests {
    use super::*;
    use crate::test_support::{faces, fixture};
    use crate::tiny::TinySkiaBackend;

    fn open() -> Deck<TinySkiaBackend> {
        Deck::open(
            &fixture("sample-deck.pptx"),
            TinySkiaBackend::new(&faces()).unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn a_click_selects_the_shape_under_it_and_its_text_can_be_rewritten() {
        let mut deck = open();
        let ids: Vec<u32> = deck.shapes(0).iter().map(|shape| shape.id).collect();
        let id = ids
            .into_iter()
            .find(|id| deck.shape_text(0, *id).is_some_and(|text| !text.is_empty()))
            .expect("the title slide has a text shape");
        let rect = deck.shape_rect(0, id).unwrap();
        assert_eq!(
            deck.shape_at(0, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
            Some(id)
        );
        assert!(deck.set_shape_text(0, id, "Jade on the web").unwrap());
        assert_eq!(deck.shape_text(0, id).unwrap(), "Jade on the web");
        assert!(deck.is_dirty());
        assert!(deck.undo().unwrap());
        assert_ne!(deck.shape_text(0, id).unwrap(), "Jade on the web");
    }

    #[test]
    fn moves_resizes_and_deletes_are_undoable() {
        let mut deck = open();
        let id = deck.next_shape(1, None, false).unwrap();
        let before = deck.shape_rect(1, id).unwrap();
        assert!(deck.move_shape(1, id, 12.0, -6.0).unwrap());
        let moved = deck.shape_rect(1, id).unwrap();
        assert!((moved.x - before.x - 12.0).abs() < 0.01);
        assert!((moved.y - before.y + 6.0).abs() < 0.01);
        let grown = ShapeRect {
            width: moved.width + 40.0,
            ..moved
        };
        assert!(deck.set_shape_rect(1, id, grown).unwrap());
        assert!((deck.shape_rect(1, id).unwrap().width - grown.width).abs() < 0.01);
        assert!(deck.delete_shape(1, id).unwrap());
        assert!(deck.shape_rect(1, id).is_none());
        for _ in 0..3 {
            assert!(deck.undo().unwrap());
        }
        assert_eq!(deck.shape_rect(1, id), Some(before));
        assert!(!deck.is_dirty());
    }

    #[test]
    fn every_fixture_slide_paints() {
        for name in [
            "sample-deck.pptx",
            "grouped-deck.pptx",
            "inherited-deck.pptx",
            "themed-deck.pptx",
        ] {
            let backend = TinySkiaBackend::new(&faces()).unwrap();
            let mut deck = Deck::open(&fixture(name), backend).unwrap();
            for index in 0..deck.slide_count() {
                let painter = deck.render_slide(index, 480).unwrap();
                let (pixels, width, _) = TinySkiaBackend::bgra(&painter).unwrap();
                assert_eq!(width, 480);
                let ink = pixels
                    .chunks_exact(4)
                    .filter(|pixel| pixel[0] < 200 || pixel[1] < 200 || pixel[2] < 200)
                    .count();
                assert!(ink > 100, "{name} slide {} is blank", index + 1);
            }
        }
    }
}

#[cfg(all(test, feature = "skia"))]
mod skia_tests {
    use super::*;
    use crate::skia::SkiaBackend;
    use crate::test_support::{faces, fixture};

    #[test]
    fn the_skia_backend_paints_and_exports_a_pdf() {
        let backend = SkiaBackend::new(&faces()).unwrap();
        let mut deck = Deck::open(&fixture("sample-deck.pptx"), backend).unwrap();
        let painter = deck.render_slide(1, 320).unwrap();
        assert!(SkiaBackend::png(&painter).is_some());
        assert!(deck.pdf().unwrap().starts_with(b"%PDF"));
    }
}
