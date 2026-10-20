//! Text typed into a shape where it lies, as PowerPoint types it.
//!
//! Each key goes straight into the slide through the writer's
//! `apply_text_input`, so the runs round what is typed keep their font,
//! size and colour, and the slide repaints with the text as it will be
//! saved. Typing runs together into one step of Undo, as PowerPoint's does.
//!
//! The caret and the selection are UTF-16 offsets into the shape's text as
//! the writer counts it — paragraphs joined by `\n` — and are drawn where
//! the layout puts the text: [`ShapeText::geometry`]. Moving by a
//! character steps over what the layout draws as one (a grapheme), and Up
//! and Down keep to the column they started from.

use std::ops::Range;

use jade_pptx_layout::{PlacedOrigin, PlacedShape, TextSelectionGeometry};
use jade_pptx_writer::{TextFormat, TextInputEdit, TypingKind};

use super::Deck;
use crate::Backend;

/// Where a caret key takes the caret.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Left,
    Right,
    /// ⌥← and ⌥→: to the start of this word or the one before, and to the
    /// end of this word or the one after.
    WordLeft,
    WordRight,
    Up,
    Down,
    /// Home and End, ⌘← and ⌘→: the ends of the line the caret is on.
    LineStart,
    LineEnd,
    /// ⌘↑ and ⌘↓: the ends of the text.
    Start,
    End,
}

/// A caret and selection in one shape's text, on one slide.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeText {
    pub slide: usize,
    pub id: u32,
    /// Where the selection started; it runs from here to `focus`.
    pub anchor: usize,
    /// Where the caret is.
    pub focus: usize,
    /// At a wrapped line's end, whether the caret is drawn there rather
    /// than at the start of the next line.
    pub upstream: bool,
    /// Where Up and Down aim, kept across a run of them.
    preferred_x: Option<f64>,
    /// Formatting picked with nothing selected: what is typed next takes
    /// it, as in PowerPoint, until the caret moves.
    pub pending: TextFormat,
    /// An IME's composition: shown at the caret over the selection, and
    /// typed in when it is committed.
    pub composition: Option<String>,
}

impl ShapeText {
    /// Starts typing into shape `id` — at the point clicked, or at the end
    /// of its text. `None` when its text may not be edited.
    pub fn begin<B: Backend>(
        deck: &mut Deck<B>,
        slide: usize,
        id: u32,
        point: Option<(f64, f64)>,
    ) -> Option<Self> {
        let text = deck.shape_text(slide, id)?;
        let end = text.encode_utf16().count();
        let mut edit = Self {
            slide,
            id,
            anchor: end,
            focus: end,
            upstream: false,
            preferred_x: None,
            pending: TextFormat::default(),
            composition: None,
        };
        if let Some((x, y)) = point {
            edit.click(deck, x, y, false);
        }
        Some(edit)
    }

    /// The shape's text as it stands.
    pub fn text<B: Backend>(&self, deck: &Deck<B>) -> String {
        deck.shape_text(self.slide, self.id).unwrap_or_default()
    }

    /// The selection, start first.
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.focus)..self.anchor.max(self.focus)
    }

    pub fn is_collapsed(&self) -> bool {
        self.anchor == self.focus
    }

    fn collapse_to(&mut self, offset: usize) {
        self.anchor = offset;
        self.focus = offset;
        self.preferred_x = None;
        self.pending = TextFormat::default();
    }

    /// Keeps the caret inside the text, which Undo may have shortened.
    pub fn clamp<B: Backend>(&mut self, deck: &Deck<B>) {
        let length = self.text(deck).encode_utf16().count();
        self.anchor = self.anchor.min(length);
        self.focus = self.focus.min(length);
    }

    /// Types `text` over the selection, or at the caret, in the formatting
    /// round it — or the one picked for it. A new line starts a paragraph.
    pub fn type_text<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        text: &str,
    ) -> Result<bool, String> {
        self.replace(deck, text, false)
    }

    /// ⇧Enter: a line break inside the paragraph.
    pub fn line_break<B: Backend>(&mut self, deck: &mut Deck<B>) -> Result<bool, String> {
        self.replace(deck, "\n", true)
    }

    fn replace<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        text: &str,
        soft_break: bool,
    ) -> Result<bool, String> {
        let Range { start, end } = self.selection();
        let history =
            (!soft_break && !text.contains('\n') && !text.is_empty()).then_some(TypingKind::Insert);
        let pending = std::mem::take(&mut self.pending);
        let changed = self.input(deck, start..end, text, soft_break, history, pending.clone())?;
        if changed {
            self.collapse_to(start + text.encode_utf16().count());
            // Typing on keeps what was picked, until the caret moves.
            self.pending = pending;
        }
        Ok(changed)
    }

    // ---- input methods ------------------------------------------------------

    /// What an input method sees: the text with the composition in place of
    /// the selection, the selection — after the composition, while there is
    /// one — and the composition's range, all in UTF-16 offsets.
    pub fn input_text<B: Backend>(
        &self,
        deck: &Deck<B>,
    ) -> (String, Range<usize>, Option<Range<usize>>) {
        let text = self.text(deck);
        let selection = self.selection();
        let Some(composition) = &self.composition else {
            return (text, selection, None);
        };
        let units: Vec<u16> = text.encode_utf16().collect();
        let start = selection.start.min(units.len());
        let end = selection.end.min(units.len());
        let mut spliced = String::from_utf16_lossy(&units[..start]);
        spliced.push_str(composition);
        spliced.push_str(&String::from_utf16_lossy(&units[end..]));
        let marked = start..start + composition.encode_utf16().count();
        (spliced, marked.end..marked.end, Some(marked))
    }

    /// The input method's composition, `text`, over `range` of what it sees
    /// — or over the selection.
    pub fn compose(&mut self, range: Option<Range<usize>>, text: &str) {
        if let (Some(range), None) = (range, &self.composition) {
            // Reconversion: the composition replaces this range.
            self.anchor = range.start;
            self.focus = range.end;
        }
        self.composition = (!text.is_empty()).then(|| text.to_string());
    }

    /// Text the input method commits: the composition's, or typing — over
    /// `range` of what it sees, or the selection.
    pub fn commit<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        range: Option<Range<usize>>,
        text: &str,
    ) -> Result<bool, String> {
        if self.composition.take().is_none() {
            if let Some(range) = range {
                self.anchor = range.start;
                self.focus = range.end;
            }
        }
        self.type_text(deck, text)
    }

    /// The composition kept as it reads.
    pub fn unmark<B: Backend>(&mut self, deck: &mut Deck<B>) -> Result<bool, String> {
        match self.composition.take() {
            Some(text) => self.type_text(deck, &text),
            None => Ok(false),
        }
    }

    /// Backspace: the selection, or what is drawn as one character before
    /// the caret.
    pub fn backspace<B: Backend>(&mut self, deck: &mut Deck<B>) -> Result<bool, String> {
        let range = if self.is_collapsed() {
            let Some(before) = self.step(deck, self.focus, false) else {
                return Ok(false);
            };
            before..self.focus
        } else {
            self.selection()
        };
        self.remove(deck, range, TypingKind::Backspace)
    }

    /// Forward Delete: the selection, or the character after the caret.
    pub fn delete<B: Backend>(&mut self, deck: &mut Deck<B>) -> Result<bool, String> {
        let range = if self.is_collapsed() {
            let Some(after) = self.step(deck, self.focus, true) else {
                return Ok(false);
            };
            self.focus..after
        } else {
            self.selection()
        };
        self.remove(deck, range, TypingKind::Delete)
    }

    fn remove<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        range: Range<usize>,
        kind: TypingKind,
    ) -> Result<bool, String> {
        if range.is_empty() {
            return Ok(false);
        }
        let start = range.start;
        let changed = self.input(deck, range, "", false, Some(kind), TextFormat::default())?;
        if changed {
            self.collapse_to(start);
        }
        Ok(changed)
    }

    fn input<B: Backend>(
        &self,
        deck: &mut Deck<B>,
        range: Range<usize>,
        text: &str,
        soft_break: bool,
        history: Option<TypingKind>,
        format: TextFormat,
    ) -> Result<bool, String> {
        let edit = TextInputEdit {
            shape_id: self.id,
            start: range.start,
            end: range.end,
            text: text.to_string(),
            expected_text: self.text(deck),
            soft_break,
            history,
            format,
        };
        deck.apply_text_input(self.slide, &edit)
    }

    /// Moves the caret; with `extend`, the selection's moving end.
    pub fn move_caret<B: Backend>(&mut self, deck: &mut Deck<B>, motion: Motion, extend: bool) {
        // Left and Right on a selection go to its ends, as everywhere.
        if !extend && !self.is_collapsed() && matches!(motion, Motion::Left | Motion::Right) {
            let range = self.selection();
            let to = if motion == Motion::Left {
                range.start
            } else {
                range.end
            };
            self.collapse_to(to);
            return;
        }
        let text = self.text(deck);
        let length = text.encode_utf16().count();
        let mut upstream = false;
        let mut preferred_x = None;
        let to = match motion {
            Motion::Left => self.step(deck, self.focus, false).unwrap_or(0),
            Motion::Right => self.step(deck, self.focus, true).unwrap_or(length),
            Motion::WordLeft => word_left(&text, self.focus),
            Motion::WordRight => word_right(&text, self.focus),
            Motion::Start => 0,
            Motion::End => length,
            Motion::Up | Motion::Down | Motion::LineStart | Motion::LineEnd => {
                let direction = match motion {
                    Motion::Up => "up",
                    Motion::Down => "down",
                    Motion::LineStart => "line_start",
                    _ => "line_end",
                };
                let moved = deck.with_placed_text(self.slide, self.id, |placed, measure| {
                    placed.move_text_caret_with_x(
                        self.focus,
                        direction,
                        self.upstream,
                        self.preferred_x,
                        measure,
                    )
                });
                match moved.flatten() {
                    Some(moved) => {
                        upstream = moved.hit.upstream;
                        preferred_x = moved.preferred_x;
                        moved.hit.offset
                    }
                    None => match motion {
                        Motion::Up | Motion::LineStart => 0,
                        _ => length,
                    },
                }
            }
        };
        self.focus = to.min(length);
        if !extend {
            self.anchor = self.focus;
        }
        self.upstream = upstream;
        self.preferred_x = preferred_x;
        self.pending = TextFormat::default();
    }

    /// A click at slide point (`x`, `y`) puts the caret there; with
    /// `extend` (⇧-click, or a drag) the selection runs to it.
    pub fn click<B: Backend>(&mut self, deck: &mut Deck<B>, x: f64, y: f64, extend: bool) {
        let hit = deck
            .with_placed_text(self.slide, self.id, |placed, measure| {
                placed.hit_text(x, y, measure)
            })
            .flatten();
        let Some(hit) = hit else {
            return;
        };
        self.focus = hit.offset;
        if !extend {
            self.anchor = hit.offset;
        }
        self.upstream = hit.upstream;
        self.preferred_x = None;
        self.pending = TextFormat::default();
    }

    /// A double-click: the word at (`x`, `y`).
    pub fn select_word<B: Backend>(&mut self, deck: &mut Deck<B>, x: f64, y: f64) {
        self.click(deck, x, y, false);
        let text = self.text(deck);
        let units: Vec<u16> = text.encode_utf16().collect();
        let at = self.focus.min(units.len());
        let word = |index: usize| {
            char::decode_utf16([units[index]])
                .next()
                .and_then(Result::ok)
                .is_some_and(is_word)
        };
        let mut start = at;
        while start > 0 && word(start - 1) {
            start -= 1;
        }
        let mut end = at;
        while end < units.len() && word(end) {
            end += 1;
        }
        self.anchor = start;
        self.focus = end;
    }

    /// ⌘A: all the shape's text.
    pub fn select_all<B: Backend>(&mut self, deck: &Deck<B>) {
        self.anchor = 0;
        self.focus = self.text(deck).encode_utf16().count();
        self.preferred_x = None;
    }

    /// Where to draw the caret and the selection, in slide px.
    pub fn geometry<B: Backend>(&self, deck: &mut Deck<B>) -> Option<TextSelectionGeometry> {
        let Range { start, end } = self.selection();
        deck.with_placed_text(self.slide, self.id, |placed, measure| {
            placed.selection_geometry(start, end, self.focus, self.upstream, measure)
        })
        .flatten()
    }

    /// The offset one drawn character on from `from` — or back — or `None`
    /// at that end of the text.
    fn step<B: Backend>(&self, deck: &mut Deck<B>, from: usize, forward: bool) -> Option<usize> {
        let mut stops = deck
            .with_placed_text(self.slide, self.id, |placed, _| boundaries(placed))
            .unwrap_or_default();
        let text = self.text(deck);
        if stops.is_empty() {
            // Nothing laid out to ask: one character at a time.
            let mut offset = 0;
            stops.push(0);
            for ch in text.chars() {
                offset += ch.len_utf16();
                stops.push(offset);
            }
        }
        stops.push(0);
        stops.push(text.encode_utf16().count());
        stops.sort_unstable();
        stops.dedup();
        if forward {
            stops.into_iter().find(|&stop| stop > from)
        } else {
            stops.into_iter().rev().find(|&stop| stop < from)
        }
    }
}

/// Every offset the layout draws a caret stop at: each grapheme's, and each
/// line's ends.
fn boundaries(placed: &PlacedShape) -> Vec<usize> {
    let mut stops = Vec::new();
    for line in &placed.lines {
        if let Some((start, end)) = line.source_range {
            stops.push(start);
            stops.push(end);
        }
        for run in &line.runs {
            stops.extend_from_slice(&run.source_offsets);
        }
    }
    stops
}

fn is_word(ch: char) -> bool {
    ch.is_alphanumeric() || ch == '_'
}

/// The UTF-16 units of `text`, as characters where they decode.
fn units(text: &str) -> Vec<Option<char>> {
    text.encode_utf16()
        .map(|unit| char::decode_utf16([unit]).next().and_then(Result::ok))
        .collect()
}

fn word_left(text: &str, from: usize) -> usize {
    let units = units(text);
    let mut at = from.min(units.len());
    while at > 0 && !units[at - 1].is_some_and(is_word) {
        at -= 1;
    }
    while at > 0 && units[at - 1].is_some_and(is_word) {
        at -= 1;
    }
    at
}

fn word_right(text: &str, from: usize) -> usize {
    let units = units(text);
    let mut at = from.min(units.len());
    while at < units.len() && !units[at].is_some_and(is_word) {
        at += 1;
    }
    while at < units.len() && units[at].is_some_and(is_word) {
        at += 1;
    }
    at
}

impl<B: Backend> Deck<B> {
    /// Runs `f` on the laid-out shape that carries `id`'s text, with a
    /// measure that measures as the slide was laid out.
    fn with_placed_text<T>(
        &mut self,
        slide: usize,
        id: u32,
        f: impl FnOnce(&PlacedShape, &dyn jade_pptx_layout::TextMeasure) -> T,
    ) -> Option<T> {
        let layout = self.layout(slide)?;
        let placed = layout
            .shapes
            .iter()
            .filter(|placed| placed.origin == PlacedOrigin::Slide && placed.source_id == Some(id))
            .find(|placed| !placed.lines.is_empty())?;
        let measure = self.measure()?;
        Some(f(placed, &measure))
    }

    /// Types into a shape's text, as `apply_text_input` does: one step of
    /// Undo for a run of typing.
    pub fn apply_text_input(&mut self, slide: usize, edit: &TextInputEdit) -> Result<bool, String> {
        let result = self
            .session
            .apply_text_input(slide, edit)
            .map(|result| result.changed);
        self.after(result)
    }
}
