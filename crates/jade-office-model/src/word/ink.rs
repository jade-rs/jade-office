//! 描画 → ペン over any [`WordSession`]: strokes drawn on a page go in as
//! Word ink — an InkML part and a drawing whose `w14:contentPart` (in place
//! of `a:graphic`, inside an `mc:AlternateContent` choice requiring `wpi`)
//! points at it, floating in front of the text where the strokes were
//! drawn.

use jade_docx_editor::commands::edit_drawing::replace_drawing_command;
use jade_docx_editor::commands::insert_shape::InsertShapeCommand;
use jade_docx_editor::commands::Command;
pub use jade_docx_editor::document::InkStroke;
use jade_docx_editor::document::TextBoxShape;
use jade_docx_editor::word_document::objects::parse_element;
use jade_docx_editor::word_document::ShapeInsertSpec;

use super::objects::WordObjects;
use super::shapes::{InsertThen, Inserted};
use super::WordSession;

/// The relationship Word relates an ink part with.
const INK_RELATIONSHIP: &str =
    "http://schemas.openxmlformats.org/officeDocument/2006/relationships/customXml";

/// The pens 描画 offers: colour, width in px, highlighter.
pub const PENS: [(&str, f64, bool, &str, &str); 4] = [
    (
        "#000000",
        1.9,
        false,
        "ペン: 黒、0.5 mm",
        "Pen: Black, 0.5 mm",
    ),
    (
        "#E71224",
        1.9,
        false,
        "ペン: 赤、0.5 mm",
        "Pen: Red, 0.5 mm",
    ),
    ("#0070C0", 3.8, false, "ペン: 青、1 mm", "Pen: Blue, 1 mm"),
    (
        "#FFFF00",
        22.7,
        true,
        "蛍光ペン: 黄、6 mm",
        "Highlighter: Yellow, 6 mm",
    ),
];

/// Ink, for every session.
pub trait WordInk: WordSession + WordObjects {
    /// `strokes` (page coordinates, widths in px) go in as one ink object
    /// anchored at the caret's paragraph.
    fn insert_ink(&mut self, strokes: Vec<InkStroke>) -> Result<(), String> {
        if strokes.iter().all(|stroke| stroke.points.is_empty()) {
            return Err("nothing was drawn".into());
        }
        let range = self.selection_range().ok_or("no caret")?;
        let (caret, _) = range.normalized();
        let (inkml, (x, y, width, height)) = jade_docx_editor::ink::write_inkml(&strokes);
        let (width, height) = (width.max(4.0), height.max(4.0));
        let emu = |px: f64| (px * 9525.0).round() as i64;
        self.execute_command(
            Box::new(InsertThen::new(
                Inserted::Shape(InsertShapeCommand::new(
                    caret.paragraph,
                    caret.offset,
                    ShapeInsertSpec {
                        shape: TextBoxShape::Rectangle,
                        width_px: width.clamp(12.0, 4096.0),
                        height_px: height.clamp(12.0, 4096.0),
                        fill_color: None,
                        border_color: None,
                        border_width_px: 0.0,
                        text: String::new(),
                        text_color: None,
                        text_font_size_pt: None,
                        text_bold: None,
                        text_italic: None,
                        text_alignment: None,
                    },
                )),
                move |target| {
                    let inkml = inkml.clone();
                    vec![Box::new(replace_drawing_command("insert_ink", target, move |document, drawing| {
                        let relationship = document.add_object_part(
                            "word/ink/ink",
                            "xml",
                            inkml.as_bytes(),
                            "application/inkml+xml",
                            INK_RELATIONSHIP,
                        )?;
                        let id = document.new_drawing_object_id();
                        let (cx, cy) = (emu(width), emu(height));
                        *drawing = parse_element(&format!(
                            r#"<w:drawing><wp:anchor distT="0" distB="0" distL="114300" distR="114300" simplePos="0" relativeHeight="251665408" behindDoc="0" locked="0" layoutInCell="1" allowOverlap="1"><wp:simplePos x="0" y="0"/><wp:positionH relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionH><wp:positionV relativeFrom="page"><wp:posOffset>{}</wp:posOffset></wp:positionV><wp:extent cx="{cx}" cy="{cy}"/><wp:effectExtent l="0" t="0" r="0" b="0"/><wp:wrapNone/><wp:docPr id="{id}" name="インク {id}"/><wp:cNvGraphicFramePr/><w14:contentPart bwMode="auto" r:id="{relationship}"><w14:nvContentPartPr><w14:cNvContentPartPr/></w14:nvContentPartPr><w14:xfrm><a:off x="0" y="0"/><a:ext cx="{cx}" cy="{cy}"/></w14:xfrm></w14:contentPart></wp:anchor></w:drawing>"#,
                            emu(x),
                            emu(y),
                        ))?;
                        Ok(())
                    })) as Box<dyn Command>]
                },
            )),
            false,
        )
    }
}

impl<T: WordSession + ?Sized> WordInk for T {}
