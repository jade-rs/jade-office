//! Character and paragraph formatting of a shape's text, as PowerPoint's
//! Home tab applies it: to the selection while text is typed into, to all
//! of a shape's text while the shape itself is selected, and — with nothing
//! selected in the text — to what is typed next.
//!
//! Both shells' format strips send a [`TextCommand`] and read back a
//! [`FormatStrip`] to show, so they behave alike.

use std::ops::Range;

use jade_pptx_layout::{ParagraphFormatState, TextFormatState};
use jade_pptx_writer::{
    ParagraphAlignment, ParagraphBullet, ParagraphFormat, ParagraphFormatEdit, ParagraphSpacing,
    TextFormat, TextFormatEdit,
};

use super::{Deck, ShapeText};
use crate::Backend;

/// PowerPoint's font size list, which ⌘] and ⌘[ step through.
pub const FONT_SIZES: [f32; 21] = [
    8.0, 9.0, 10.0, 10.5, 11.0, 12.0, 14.0, 16.0, 18.0, 20.0, 24.0, 28.0, 32.0, 36.0, 40.0, 44.0,
    48.0, 54.0, 60.0, 66.0, 72.0,
];

/// One command of a format strip.
#[derive(Debug, Clone, PartialEq)]
pub enum TextCommand {
    Bold,
    Italic,
    Underline,
    Font(String),
    Size(f32),
    /// Increase and Decrease Font Size: the next size in [`FONT_SIZES`].
    Grow,
    Shrink,
    /// `#RRGGBB`.
    Color(String),
    Align(ParagraphAlignment),
    /// Bullets on, or off where they are on.
    Bullets,
    /// Numbering (1. 2. 3.) on, or off where it is on.
    Numbering,
    /// Line spacing as a multiple of single: 1.0, 1.5, 2.0…
    LineSpacing(f64),
    /// Space after each paragraph, in points.
    SpaceAfter(f64),
}

/// What a format strip shows for the text it would change: `None` where
/// the text mixes values.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FormatStrip {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub font: Option<String>,
    pub size: Option<f32>,
    pub color: Option<String>,
    pub alignment: Option<ParagraphAlignment>,
    pub bullets: bool,
    pub numbering: bool,
}

impl FormatStrip {
    fn of(text: Option<TextFormatState>, paragraph: Option<ParagraphFormatState>) -> Self {
        let text = text.unwrap_or(TextFormatState {
            bold: None,
            italic: None,
            underline: None,
            size_pt: None,
            font: None,
            east_asian_font: None,
            color: None,
        });
        let paragraph = paragraph.as_ref();
        Self {
            bold: text.bold == Some(true),
            italic: text.italic == Some(true),
            underline: text.underline == Some(true),
            font: text.east_asian_font.or(text.font),
            size: text.size_pt,
            color: text.color,
            alignment: paragraph
                .and_then(|p| p.alignment.as_deref())
                .and_then(alignment),
            bullets: paragraph.and_then(|p| p.bullet.as_deref()) == Some("character"),
            numbering: paragraph.and_then(|p| p.bullet.as_deref()) == Some("numbered"),
        }
    }
}

fn alignment(name: &str) -> Option<ParagraphAlignment> {
    Some(match name {
        "l" | "left" => ParagraphAlignment::Left,
        "ctr" | "center" => ParagraphAlignment::Center,
        "r" | "right" => ParagraphAlignment::Right,
        "just" | "justify" => ParagraphAlignment::Justify,
        "dist" | "distributed" => ParagraphAlignment::Distributed,
        _ => return None,
    })
}

/// The next size up from `size` in [`FONT_SIZES`] — or down — past the
/// list's end by tens, as PowerPoint goes.
pub fn step_size(size: f32, up: bool) -> f32 {
    if up {
        FONT_SIZES
            .iter()
            .copied()
            .find(|&s| s > size + 0.01)
            .unwrap_or((size / 10.0).floor() * 10.0 + 10.0)
    } else {
        FONT_SIZES
            .iter()
            .rev()
            .copied()
            .find(|&s| s < size - 0.01)
            .unwrap_or(FONT_SIZES[0].min(size))
    }
}

impl<B: Backend> Deck<B> {
    /// Formats characters `range` (UTF-16) of shape `id`'s text, as one
    /// step of Undo.
    pub fn format_text(
        &mut self,
        slide: usize,
        id: u32,
        range: Range<usize>,
        format: &TextFormat,
    ) -> Result<bool, String> {
        let edit = TextFormatEdit {
            shape_id: id,
            start: range.start,
            end: range.end,
            format: format.clone(),
        };
        let result = self.session.format_text_range(slide, &edit);
        self.after(result)
    }

    /// Formats the paragraphs `range` touches, as one step of Undo.
    pub fn format_paragraphs(
        &mut self,
        slide: usize,
        id: u32,
        range: Range<usize>,
        format: &ParagraphFormat,
    ) -> Result<bool, String> {
        let edit = ParagraphFormatEdit {
            shape_id: id,
            start: range.start,
            end: range.end,
            format: format.clone(),
        };
        let result = self.session.format_paragraphs(slide, &edit);
        self.after(result)
    }

    /// The paragraph formatting of the paragraphs `range` touches.
    pub fn paragraph_format(
        &self,
        slide: usize,
        id: u32,
        range: Range<usize>,
    ) -> Option<ParagraphFormatState> {
        jade_pptx_layout::paragraph_format_for_deck_shape(
            &self.deck,
            slide,
            id,
            range.start,
            range.end,
        )
    }

    /// A format strip's command on all of shape `id`'s text, as when the
    /// shape is selected and not typed into.
    pub fn format_shape(
        &mut self,
        slide: usize,
        id: u32,
        command: &TextCommand,
    ) -> Result<bool, String> {
        let Some(mut text) = ShapeText::begin(self, slide, id, None) else {
            return Ok(false);
        };
        text.select_all(self);
        text.format(self, command)
    }

    /// What a format strip shows for all of shape `id`'s text.
    pub fn shape_format(&mut self, slide: usize, id: u32) -> Option<FormatStrip> {
        let mut text = ShapeText::begin(self, slide, id, None)?;
        text.select_all(self);
        Some(text.format_strip(self))
    }
}

impl ShapeText {
    /// Applies a format strip's command: to the selection, or — character
    /// formatting with nothing selected — to what is typed next. Paragraph
    /// formatting goes to the paragraphs the selection or caret is in.
    pub fn format<B: Backend>(
        &mut self,
        deck: &mut Deck<B>,
        command: &TextCommand,
    ) -> Result<bool, String> {
        let strip = self.format_strip(deck);
        let paragraph = match command {
            TextCommand::Align(alignment) => Some(ParagraphFormat {
                alignment: Some(*alignment),
                ..Default::default()
            }),
            TextCommand::Bullets => Some(ParagraphFormat {
                bullet: Some(if strip.bullets {
                    ParagraphBullet::None
                } else {
                    ParagraphBullet::Character {
                        character: "•".to_string(),
                        font: None,
                    }
                }),
                ..Default::default()
            }),
            TextCommand::Numbering => Some(ParagraphFormat {
                bullet: Some(if strip.numbering {
                    ParagraphBullet::None
                } else {
                    ParagraphBullet::Numbered {
                        scheme: "arabicPeriod".to_string(),
                        start_at: None,
                    }
                }),
                ..Default::default()
            }),
            TextCommand::LineSpacing(lines) => Some(ParagraphFormat {
                line_spacing: Some(ParagraphSpacing::Percent(lines * 100.0)),
                ..Default::default()
            }),
            TextCommand::SpaceAfter(points) => Some(ParagraphFormat {
                space_after: Some(ParagraphSpacing::Points(*points)),
                ..Default::default()
            }),
            _ => None,
        };
        if let Some(paragraph) = paragraph {
            return deck.format_paragraphs(self.slide, self.id, self.selection(), &paragraph);
        }
        let size = strip.size.unwrap_or(18.0);
        let format = match command {
            TextCommand::Bold => TextFormat {
                bold: Some(!strip.bold),
                ..Default::default()
            },
            TextCommand::Italic => TextFormat {
                italic: Some(!strip.italic),
                ..Default::default()
            },
            TextCommand::Underline => TextFormat {
                underline: Some(!strip.underline),
                ..Default::default()
            },
            TextCommand::Font(font) => TextFormat {
                font: Some(font.clone()),
                east_asian_font: Some(font.clone()),
                ..Default::default()
            },
            TextCommand::Size(size) => TextFormat {
                size_pt: Some(f64::from(*size)),
                ..Default::default()
            },
            TextCommand::Grow | TextCommand::Shrink => TextFormat {
                size_pt: Some(f64::from(step_size(size, *command == TextCommand::Grow))),
                ..Default::default()
            },
            TextCommand::Color(color) => TextFormat {
                color: Some(color.clone()),
                ..Default::default()
            },
            _ => return Ok(false),
        };
        if self.is_collapsed() {
            merge(&mut self.pending, format);
            return Ok(false);
        }
        let changed = deck.format_text(self.slide, self.id, self.selection(), &format)?;
        // Typing over the selection keeps what was picked.
        merge(&mut self.pending, format);
        Ok(changed)
    }

    /// What a format strip shows: the selection's formatting — at a caret,
    /// the character before it — with what was picked for typing over it.
    pub fn format_strip<B: Backend>(&self, deck: &Deck<B>) -> FormatStrip {
        let range = self.selection();
        let mut strip = FormatStrip::of(
            deck.text_format(self.slide, self.id, range.clone()),
            deck.paragraph_format(self.slide, self.id, range),
        );
        let pending = &self.pending;
        strip.bold = pending.bold.unwrap_or(strip.bold);
        strip.italic = pending.italic.unwrap_or(strip.italic);
        strip.underline = pending.underline.unwrap_or(strip.underline);
        if let Some(size) = pending.size_pt {
            strip.size = Some(size as f32);
        }
        if let Some(font) = &pending.font {
            strip.font = Some(font.clone());
        }
        if let Some(color) = &pending.color {
            strip.color = Some(color.clone());
        }
        strip
    }
}

/// `format`'s fields over `pending`'s.
fn merge(pending: &mut TextFormat, format: TextFormat) {
    pending.bold = format.bold.or(pending.bold);
    pending.italic = format.italic.or(pending.italic);
    pending.underline = format.underline.or(pending.underline);
    pending.size_pt = format.size_pt.or(pending.size_pt);
    pending.font = format.font.or(pending.font.take());
    pending.east_asian_font = format.east_asian_font.or(pending.east_asian_font.take());
    pending.color = format.color.or(pending.color.take());
}
