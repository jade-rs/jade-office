//! An editable presentation on the pure-Rust painter.
//!
//! Edits go through jade-pptx-writer's `EditSession`, as in the native app:
//! each change rewrites the slide part in the package bytes and keeps undo
//! history, and the deck is read back from those bytes to paint. Everything
//! here is UI-free; a shell hit-tests through [`Deck::shape_at`] and edits
//! through [`Deck::set_shape_text`], [`Deck::move_shape`] and friends.

use std::collections::HashMap;
use std::rc::Rc;

use jade_painter::{ImageRect, ImageSource, Painter};
use jade_pptx_layout::{layout_deck_slide, PlacedOrigin, PlacedShape, SlideLayout};
use jade_pptx_reader::{emu_to_px, Transform};
use jade_pptx_render::{render_slide_with, RenderOptions};
use jade_pptx_writer::{EditSession, ShapeTransformInfo};
use jade_tiny_skia_painter::{FontBook, TinySkiaPainter};

/// EMU per px at 96 dpi.
const EMU_PER_PX: f64 = 9525.0;

/// A shape's box in slide px: (x, y, width, height).
pub type ShapeRect = (f64, f64, f64, f64);

fn rect_of(transform: &Transform) -> ShapeRect {
    (
        emu_to_px(transform.offset_emu.0),
        emu_to_px(transform.offset_emu.1),
        emu_to_px(transform.extent_emu.0),
        emu_to_px(transform.extent_emu.1),
    )
}

pub struct Deck {
    session: EditSession,
    deck: jade_pptx_reader::Deck,
    parts: HashMap<String, Vec<u8>>,
    fonts: Rc<FontBook>,
    options: RenderOptions,
    layouts: HashMap<usize, Rc<SlideLayout>>,
    shapes: HashMap<usize, Vec<ShapeTransformInfo>>,
}

impl Deck {
    pub fn open(bytes: &[u8], fonts: Rc<FontBook>) -> Result<Self, String> {
        let session = EditSession::open(bytes)
            .map_err(|error| format!("cannot read the presentation: {error}"))?;
        let (deck, parts) = read(bytes)?;
        Ok(Self {
            session,
            deck,
            parts,
            fonts,
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

    fn layout(&mut self, index: usize) -> Option<Rc<SlideLayout>> {
        if let Some(layout) = self.layouts.get(&index) {
            return Some(Rc::clone(layout));
        }
        let measuring = TinySkiaPainter::new(1, 1, Rc::clone(&self.fonts))?;
        let measure =
            jade_pptx_render::PainterMeasure::new(measuring, self.options.default_font.clone());
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

    fn shape(&mut self, slide: usize, id: u32) -> Option<ShapeTransformInfo> {
        self.shapes(slide)
            .iter()
            .find(|shape| shape.id == id)
            .cloned()
    }

    /// The selectable shape at a slide point: the top-most of the slide's own
    /// shapes, or the group it belongs to — what PowerPoint selects first.
    pub fn shape_at(&mut self, slide: usize, x: f64, y: f64) -> Option<u32> {
        let layout = self.layout(slide)?;
        let selectable: Vec<u32> = self
            .shapes(slide)
            .iter()
            .filter(|shape| shape.can_select && shape.transform.is_some())
            .map(|shape| shape.id)
            .collect();
        for placed in layout.shapes.iter().rev() {
            if placed.origin != PlacedOrigin::Slide {
                continue;
            }
            let rect = placed.rect;
            if !(x >= rect.x
                && x <= rect.x + rect.width
                && y >= rect.y
                && y <= rect.y + rect.height)
            {
                continue;
            }
            if let Some(id) = placed
                .group_ids
                .iter()
                .copied()
                .chain(placed.source_id)
                .find(|id| selectable.contains(id))
            {
                return Some(id);
            }
        }
        None
    }

    /// The next selectable shape after `current` (or before, `backwards`),
    /// for Tab.
    pub fn next_shape(
        &mut self,
        slide: usize,
        current: Option<u32>,
        backwards: bool,
    ) -> Option<u32> {
        let selectable: Vec<u32> = self
            .shapes(slide)
            .iter()
            .filter(|shape| shape.can_select && shape.transform.is_some())
            .map(|shape| shape.id)
            .collect();
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
        self.shape(slide, id)?.transform.as_ref().map(rect_of)
    }

    pub fn shape_name(&mut self, slide: usize, id: u32) -> Option<String> {
        Some(self.shape(slide, id)?.name)
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

    /// Replaces a shape's text, as one undoable change.
    pub fn set_shape_text(&mut self, slide: usize, id: u32, text: &str) -> Result<bool, String> {
        let result = self.session.set_shape_text(slide, id, text, false);
        self.after(result)
    }

    /// Moves a shape by `(dx, dy)` slide px.
    pub fn move_shape(&mut self, slide: usize, id: u32, dx: f64, dy: f64) -> Result<bool, String> {
        let Some(shape) = self.shape(slide, id) else {
            return Ok(false);
        };
        let (Some(transform), true) = (shape.transform, shape.can_move) else {
            return Ok(false);
        };
        let next = Transform {
            offset_emu: (
                transform.offset_emu.0 + (dx * EMU_PER_PX).round() as i64,
                transform.offset_emu.1 + (dy * EMU_PER_PX).round() as i64,
            ),
            ..transform
        };
        let result = self.session.set_shape_transform(slide, id, next);
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

    /// Paints slide `index` `width` px wide.
    pub fn render_slide(&mut self, index: usize, width: u32) -> Option<TinySkiaPainter> {
        let layout = self.layout(index)?;
        let (slide_width, slide_height) = self.slide_size();
        let scale = f64::from(width.max(1)) / slide_width.max(1.0);
        let height = (slide_height * scale).round().max(1.0) as u32;
        let painter = TinySkiaPainter::new(width.max(1), height, Rc::clone(&self.fonts))?;
        painter.save();
        painter.scale(scale, scale).ok()?;
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
        render_slide_with(&painter, &layout, &self.options, &mut draw_image).ok()?;
        painter.restore();
        Some(painter)
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn open() -> Deck {
        let core =
            std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
        let bytes = std::fs::read(core.join("public/fixtures/sample-deck.pptx")).unwrap();
        Deck::open(&bytes, crate::tests::fonts()).unwrap()
    }

    #[test]
    fn a_click_selects_the_shape_under_it_and_its_text_can_be_rewritten() {
        let mut deck = open();
        let id = deck
            .shapes(0)
            .iter()
            .map(|shape| shape.id)
            .find(|id| deck_text_is_editable(&open(), *id))
            .expect("the title slide has a text shape");
        let (x, y, w, h) = deck.shape_rect(0, id).unwrap();
        assert_eq!(deck.shape_at(0, x + w / 2.0, y + h / 2.0), Some(id));
        assert!(deck.set_shape_text(0, id, "Jade on the web").unwrap());
        assert_eq!(deck.shape_text(0, id).unwrap(), "Jade on the web");
        assert!(deck.is_dirty());
        assert!(deck.undo().unwrap());
        assert_ne!(deck.shape_text(0, id).unwrap(), "Jade on the web");
    }

    fn deck_text_is_editable(deck: &Deck, id: u32) -> bool {
        deck.shape_text(0, id).is_some_and(|text| !text.is_empty())
    }

    #[test]
    fn moving_and_deleting_a_shape_are_undoable() {
        let mut deck = open();
        let id = deck.next_shape(1, None, false).unwrap();
        let before = deck.shape_rect(1, id).unwrap();
        assert!(deck.move_shape(1, id, 12.0, -6.0).unwrap());
        let after = deck.shape_rect(1, id).unwrap();
        assert!((after.0 - before.0 - 12.0).abs() < 0.01);
        assert!((after.1 - before.1 + 6.0).abs() < 0.01);
        assert!(deck.delete_shape(1, id).unwrap());
        assert!(deck.shape_rect(1, id).is_none());
        assert!(deck.undo().unwrap());
        assert!(deck.undo().unwrap());
        assert_eq!(deck.shape_rect(1, id), Some(before));
        assert!(!deck.is_dirty());
        assert!(!deck.save_bytes().is_empty());
    }
}
