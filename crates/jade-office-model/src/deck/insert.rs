//! New objects on a slide, as PowerPoint's Insert tab makes them: a text
//! box or a shape drawn with a drag — or dropped at its usual size by a
//! click — and a picture placed in the middle of the slide at its own size,
//! shrunk to fit.

use jade_pptx_reader::Transform;
pub use jade_pptx_writer::ShapePreset;
use jade_pptx_writer::{ShapeInsert, TextFormat};

use super::{Deck, ShapeRect};
use crate::Backend;

/// What a drag on the slide draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insert {
    TextBox,
    Shape(ShapePreset),
}

/// The shapes the Shapes list offers, in its order.
pub const SHAPE_PRESETS: [ShapePreset; 6] = [
    ShapePreset::Rect,
    ShapePreset::RoundRect,
    ShapePreset::Ellipse,
    ShapePreset::Triangle,
    ShapePreset::Diamond,
    ShapePreset::Hexagon,
];

/// A drag shorter than this, in slide px, is a click.
const CLICK_PX: f64 = 4.0;

impl Insert {
    /// The box a press at `from` and a release at `to` (slide px) draws: the
    /// rectangle between them, or for a click the usual size from `from` —
    /// a line of text for a text box, an inch square for a shape.
    pub fn rect(self, from: (f64, f64), to: (f64, f64)) -> ShapeRect {
        let (width, height) = ((to.0 - from.0).abs(), (to.1 - from.1).abs());
        if width < CLICK_PX && height < CLICK_PX {
            let (width, height) = match self {
                Self::TextBox => (240.0, 40.0),
                Self::Shape(_) => (96.0, 96.0),
            };
            return ShapeRect {
                x: from.0,
                y: from.1,
                width,
                height,
            };
        }
        ShapeRect {
            x: from.0.min(to.0),
            y: from.1.min(to.1),
            width: width.max(CLICK_PX),
            height: height.max(CLICK_PX),
        }
    }
}

fn transform(rect: ShapeRect) -> Transform {
    rect.apply_to(Transform {
        offset_emu: (0, 0),
        extent_emu: (0, 0),
        rotation_60000ths: 0,
        flip_h: false,
        flip_v: false,
    })
}

impl<B: Backend> Deck<B> {
    fn inserted(&mut self, result: jade_pptx_writer::Result<u32>) -> Result<u32, String> {
        let id = result.map_err(|error| error.to_string())?;
        self.reload()?;
        Ok(id)
    }

    /// Inserts an empty text box or a shape over `rect`, in front of the
    /// slide's other objects, as one step of Undo. Its id comes back.
    pub fn insert(&mut self, slide: usize, insert: Insert, rect: ShapeRect) -> Result<u32, String> {
        let result = match insert {
            Insert::TextBox => {
                self.session
                    .insert_text_box(slide, transform(rect), "", &TextFormat::default())
            }
            Insert::Shape(preset) => self.session.insert_shape(
                slide,
                transform(rect),
                &ShapeInsert {
                    preset,
                    text: String::new(),
                    format: TextFormat::default(),
                    fill: None,
                    outline: None,
                    outline_width_pt: None,
                },
            ),
        };
        self.inserted(result)
    }

    /// Inserts a PNG or JPEG picture in the middle of the slide at its own
    /// size — 96 px to the inch — shrunk to fit inside the slide.
    pub fn insert_picture(&mut self, slide: usize, image: &[u8]) -> Result<u32, String> {
        let info = jade_pptx_writer::picture_info(image).map_err(|error| error.to_string())?;
        let (slide_width, slide_height) = self.slide_size();
        let (width, height) = (f64::from(info.width.max(1)), f64::from(info.height.max(1)));
        let fit = (slide_width * 0.9 / width)
            .min(slide_height * 0.9 / height)
            .min(1.0);
        let (width, height) = (width * fit, height * fit);
        let rect = ShapeRect {
            x: (slide_width - width) / 2.0,
            y: (slide_height - height) / 2.0,
            width,
            height,
        };
        let result = self
            .session
            .insert_picture(slide, transform(rect), image)
            .map(|inserted| inserted.id);
        self.inserted(result)
    }
}
