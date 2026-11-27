//! 挿入 → 画像 on the browser build's Word document, as the native app does
//! it too: a picture in line at the caret at its own size, moved, given a
//! wrap, each one undo step, and saved into the package.

mod common;
use common::{contract, typed};
use jade_docx_editor::word_document::{FloatingObjectKind, FloatingObjectTarget};
use jade_web::editing::WordSession;
use jade_web::word::objects::WordObjects;
use jade_web::word::picture::{PlacedObject, WordPicture, Wrap};

/// A 2×1 PNG.
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
        .unwrap()
        .write_to(
            &mut std::io::Cursor::new(&mut bytes),
            image::ImageFormat::Png,
        )
        .unwrap();
    bytes
}

fn image_runs(document: &jade_web::editing::EditableDocument, paragraph: usize) -> Vec<usize> {
    document
        .word_document()
        .paragraph(paragraph)
        .map(|paragraph| {
            paragraph
                .runs
                .iter()
                .enumerate()
                .filter(|(_, run)| run.image.is_some())
                .map(|(index, _)| index)
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_picture_goes_in_at_the_caret_and_wraps() {
    let mut document = contract();
    let paragraph = typed(&mut document, "here ");
    document.place_caret(paragraph, "here".len());
    document.insert_picture(png()).unwrap();
    let runs = image_runs(&document, paragraph);
    assert_eq!(runs.len(), 1);
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    assert!(package
        .part_names()
        .iter()
        .any(|name| name.starts_with("word/media/")));

    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: runs[0],
        kind: FloatingObjectKind::Image,
    };
    let object = document
        .object_region(target)
        .expect("the picture is on a page");
    assert!(object.is_picture());
    assert_eq!((object.width.round(), object.height.round()), (2.0, 1.0));
    document.set_picture_wrap(object, Wrap::Square).unwrap();
    let image = document.word_document().paragraph(paragraph).unwrap().runs[runs[0]]
        .image
        .clone()
        .unwrap();
    assert_eq!(
        image.layout.wrap,
        jade_docx_editor::document::WrapMode::Square
    );

    let moved: PlacedObject = document.object_region(target).unwrap();
    document
        .place_object(moved, (moved.x + 40.0, moved.y + 20.0, 20.0, 10.0))
        .unwrap();
    let placed = document.object_region(target).unwrap();
    assert_eq!((placed.width.round(), placed.height.round()), (20.0, 10.0));

    document.undo().unwrap();
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(image_runs(&document, paragraph).is_empty());
}

/// A picture placed from the margin and moving with its line, dragged, is
/// still placed from the margin and still moves with the text, as Word
/// keeps it: its offsets are taken again from those frames.
#[test]
fn a_dragged_picture_keeps_its_frames() {
    let bytes = std::fs::read(common::core().join("public/fixtures/image-floating.docx")).unwrap();
    let mut document = common::open_bytes(&bytes);
    let (paragraph, run) = (0..document.paragraph_count())
        .find_map(|paragraph| {
            image_runs(&document, paragraph)
                .first()
                .map(|&run| (paragraph, run))
        })
        .unwrap();
    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: run,
        kind: FloatingObjectKind::Image,
    };
    let frames = |document: &jade_web::editing::EditableDocument| {
        let layout = document.word_document().paragraph(paragraph).unwrap().runs[run]
            .image
            .clone()
            .unwrap()
            .layout;
        (layout.position_h.unwrap(), layout.position_v.unwrap())
    };
    let (h, v) = frames(&document);
    assert_eq!(h.relative_from.as_deref(), Some("margin"));
    assert_eq!(v.relative_from.as_deref(), Some("line"));

    let object = document.object_region(target).unwrap();
    document
        .place_object(
            object,
            (
                object.x + 30.0,
                object.y + 50.0,
                object.width,
                object.height,
            ),
        )
        .unwrap();
    let placed = document.object_region(target).unwrap();
    assert!((placed.x - (object.x + 30.0)).abs() < 1.0, "{placed:?}");
    assert!((placed.y - (object.y + 50.0)).abs() < 1.0, "{placed:?}");
    let (h, v) = frames(&document);
    assert_eq!(h.relative_from.as_deref(), Some("margin"));
    assert_eq!(v.relative_from.as_deref(), Some("paragraph"));
    // Moved 30 px right of where the margin put it before.
    let margin_left = object.x - 1517660.0 / 9525.0;
    assert!((h.offset_px.unwrap() - (placed.x - margin_left)).abs() < 1.0);
}

/// The text of `paragraph` from `from` on, an inline picture as U+FFFC.
fn text_from(
    document: &jade_web::editing::EditableDocument,
    paragraph: usize,
    from: usize,
) -> String {
    document.caret_text(paragraph).unwrap()[from..].to_string()
}

/// An inline picture is one character of its line, as in Word: the caret
/// goes after it when it goes in, typing there follows it, the arrows step
/// over it in one, Shift+← selects it, and Backspace after it or Delete
/// before it removes it — each one undo step.
#[test]
fn an_inline_picture_is_one_character_for_the_caret() {
    use jade_web::editing::Motion;
    const PICTURE: &str = "\u{fffc}";
    let mut document = contract();
    let paragraph = typed(&mut document, "ab ");
    document.place_caret(paragraph, "ab".len());
    document.insert_picture(png()).unwrap();
    let after = "ab".len() + PICTURE.len();
    assert_eq!(document.caret(), Some((paragraph, after)));
    assert!(text_from(&document, paragraph, 0).starts_with("ab\u{fffc} "));

    // Typing after it goes behind it.
    document.insert_text("X").unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("ab\u{fffc}X "));
    assert_eq!(image_runs(&document, paragraph).len(), 1);
    document.undo().unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("ab\u{fffc} "));

    // The arrows step over it in one.
    document.place_caret(paragraph, "ab".len());
    document.move_caret(Motion::Right, false).unwrap();
    assert_eq!(document.caret(), Some((paragraph, after)));
    document.move_caret(Motion::Left, false).unwrap();
    assert_eq!(document.caret(), Some((paragraph, "ab".len())));

    // Backspace after it removes it, and nothing else.
    document.place_caret(paragraph, after);
    document.delete(true).unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("ab "));
    assert!(image_runs(&document, paragraph).is_empty());
    assert_eq!(document.caret(), Some((paragraph, "ab".len())));
    document.undo().unwrap();
    assert_eq!(image_runs(&document, paragraph).len(), 1);

    // Delete in front of it removes it.
    document.place_caret(paragraph, "ab".len());
    document.delete(false).unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("ab "));
    assert!(image_runs(&document, paragraph).is_empty());
    document.undo().unwrap();
    assert_eq!(image_runs(&document, paragraph).len(), 1);

    // Shift+← selects it; deleting the selection takes it with the text.
    document.place_caret(paragraph, after);
    document.move_caret(Motion::Left, true).unwrap();
    document.move_caret(Motion::Left, true).unwrap();
    document.delete(true).unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("a "));
    assert!(image_runs(&document, paragraph).is_empty());
    document.undo().unwrap();
    assert!(text_from(&document, paragraph, 0).starts_with("ab\u{fffc} "));
    assert_eq!(image_runs(&document, paragraph).len(), 1);

    // Deleted as a selected object, it leaves the caret where it was.
    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: image_runs(&document, paragraph)[0],
        kind: FloatingObjectKind::Image,
    };
    document.delete_object(target).unwrap();
    assert!(image_runs(&document, paragraph).is_empty());
    assert_eq!(document.caret(), Some((paragraph, "ab".len())));
    let saved = document.export_docx().unwrap();
    assert!(!saved.is_empty());
}

/// In a header or footer, too, an inline picture is one character: the
/// caret goes after it, typing follows it, the arrows step over it and
/// Backspace after it removes it.
#[test]
fn a_header_or_footer_picture_is_one_character_for_the_caret() {
    use jade_web::editing::{Motion, WordDoc};
    use jade_web::word::page::Story;
    const PICTURE: &str = "\u{fffc}";
    for role in [Story::Header, Story::Footer] {
        let mut document = WordDoc::new(contract());
        document.open_story(0, role).unwrap();
        document.insert_text("ab").unwrap();
        let paragraph = document.caret().unwrap().0;
        document.place_caret(paragraph, 1);
        document.insert_picture(png()).unwrap();
        let after = "a".len() + PICTURE.len();
        assert_eq!(document.caret(), Some((paragraph, after)), "{role:?}");
        document.insert_text("X").unwrap();
        assert!(
            text_from(&document, paragraph, 0).starts_with("a\u{fffc}Xb"),
            "{role:?}: {:?}",
            text_from(&document, paragraph, 0)
        );
        document.place_caret(paragraph, 1);
        document.move_caret(Motion::Right, false).unwrap();
        assert_eq!(document.caret(), Some((paragraph, after)), "{role:?}");
        document.delete(true).unwrap();
        assert!(image_runs(&document, paragraph).is_empty(), "{role:?}");
        assert!(
            text_from(&document, paragraph, 0).starts_with("aXb"),
            "{role:?}: {:?}",
            text_from(&document, paragraph, 0)
        );
        document.undo().unwrap();
        assert_eq!(image_runs(&document, paragraph).len(), 1, "{role:?}");
        // Closed, the picture stays in the story it went into.
        document.close_story().unwrap();
        let package = jade_opc::OpcPackage::open(&document.word_docx().unwrap()).unwrap();
        let prefix = match role {
            Story::Header => "word/header",
            _ => "word/footer",
        };
        assert!(
            package.part_names().iter().any(|name| {
                name.trim_start_matches('/').starts_with(prefix)
                    && package
                        .part_str(name)
                        .ok()
                        .flatten()
                        .is_some_and(|xml| xml.contains("<w:drawing"))
            }),
            "{role:?}: the picture is saved in the story"
        );
    }
}

/// An inline equation in a footnote is one character of its line, as in
/// the body: the caret reaches the text after it, typing after it follows
/// it, and Backspace after it removes it.
#[test]
fn a_footnote_equation_is_one_character_for_the_caret() {
    use jade_web::editing::WordSession;
    use jade_web::word::headless::{HeadlessCommand, HeadlessEquationPlacement};
    const EQUATION: &str = "\u{fffc}";
    let mut document = contract();
    document
        .apply_commands(
            vec![HeadlessCommand::InsertFootnote {
                paragraph: 3,
                offset: 0,
                text: "abcdef".into(),
            }],
            false,
        )
        .unwrap();
    let note = (0..document.word_document().paragraph_count())
        .find(|&index| {
            document
                .word_document()
                .rendered_paragraph_text(index)
                .contains("abcdef")
        })
        .unwrap();
    let text = document.caret_text(note).unwrap();
    let at = text.find("def").unwrap();
    document
        .apply_commands(
            vec![HeadlessCommand::InsertEquation {
                paragraph: note,
                offset: at,
                omml: "<m:oMath><m:r><m:t>xyz</m:t></m:r></m:oMath>".into(),
                placement: HeadlessEquationPlacement::Inline,
            }],
            false,
        )
        .unwrap();
    assert!(text_from(&document, note, 0).ends_with("abc\u{fffc}def"));
    // The caret reaches the end of the note, past "def".
    let end = document.word_document().rendered_paragraph_len(note);
    let x = |offset| document.position_metrics((note, offset)).unwrap().0;
    assert!(x(end) > x(end - 1));

    // Typing after the equation follows it.
    let after = at + EQUATION.len();
    document.place_caret(note, after);
    document.insert_text("X").unwrap();
    assert!(text_from(&document, note, 0).ends_with("abc\u{fffc}Xdef"));
    document.undo().unwrap();

    // Backspace after it removes it, and nothing else.
    document.place_caret(note, after);
    document.delete(true).unwrap();
    assert!(text_from(&document, note, 0).ends_with("abcdef"));
    assert_eq!(document.caret(), Some((note, at)));
    document.undo().unwrap();
    assert!(text_from(&document, note, 0).ends_with("abc\u{fffc}def"));
}

/// An inline equation in a text box is one character of its line too.
#[test]
fn a_text_box_equation_is_one_character_for_the_caret() {
    use jade_docx_editor::document::TextBoxShape;
    use jade_docx_editor::word_document::ParagraphStory;
    use jade_web::editing::WordSession;
    use jade_web::word::headless::{HeadlessCommand, HeadlessEquationPlacement};
    use jade_web::word::shapes::WordShapes;
    let mut document = contract();
    let host = typed(&mut document, "Body");
    document.place_caret(host, 0);
    document
        .insert_shape(TextBoxShape::Rectangle, None, true)
        .unwrap();
    let text_box = (0..document.paragraph_count())
        .find(|&surface| {
            matches!(
                document.word_document().paragraph_story(surface),
                Some(ParagraphStory::TextBox { .. })
            )
        })
        .unwrap();
    document.place_caret(text_box, 0);
    document.insert_text("abcdef").unwrap();
    document
        .apply_commands(
            vec![HeadlessCommand::InsertEquation {
                paragraph: text_box,
                offset: 3,
                omml: "<m:oMath><m:r><m:t>xyz</m:t></m:r></m:oMath>".into(),
                placement: HeadlessEquationPlacement::Inline,
            }],
            false,
        )
        .unwrap();
    assert_eq!(document.caret_text(text_box).unwrap(), "abc\u{fffc}def");
    let end = document.word_document().rendered_paragraph_len(text_box);
    assert_eq!(end, "abc\u{fffc}def".len());
    let x = |offset| document.position_metrics((text_box, offset)).unwrap().0;
    assert!(x(end) > x(end - 1), "{} {}", x(end), x(end - 1));
    document.place_caret(text_box, "abc\u{fffc}".len());
    document.delete(true).unwrap();
    assert_eq!(document.caret_text(text_box).unwrap(), "abcdef");
}

#[test]
fn a_copied_picture_pastes_as_a_picture_of_its_own() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = contract();
    let paragraph = typed(&mut document, "ab ");
    document.place_caret(paragraph, 1);
    document.insert_picture(png()).unwrap();
    // "a￼b": the picture is one character of the line.
    document.select((paragraph, 0), (paragraph, 5)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.plain_text(), "ab");
    assert_eq!(clip.pictures.len(), 1);
    assert_eq!(clip.pictures[0].offset, 1);

    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    let after = document.paste_rich(&clip).unwrap();
    let runs = image_runs(&document, paragraph);
    assert_eq!(runs.len(), 2);
    let shown = document.word_document().rendered_paragraph_text(paragraph);
    assert!(shown.ends_with("a\u{fffc}b"), "{shown}");
    assert_eq!(after, (paragraph, shown.len()));
    let saved = document.export_docx().unwrap();
    let xml = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert_eq!(xml.matches("<w:drawing").count(), 2, "{xml}");

    // One undo takes the text and the picture back.
    document.undo().unwrap();
    assert_eq!(image_runs(&document, paragraph).len(), 1);
    assert_eq!(
        document.word_document().rendered_paragraph_len(paragraph),
        end
    );
}

#[test]
fn a_picture_at_the_end_of_a_copy_leaves_the_caret_after_it() {
    use jade_web::word::clipboard::WordClipboard;
    let mut document = contract();
    let paragraph = typed(&mut document, "ab ");
    document.place_caret(paragraph, 1);
    document.insert_picture(png()).unwrap();
    document.select((paragraph, 1), (paragraph, 4)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!((clip.plain_text().as_str(), clip.pictures.len()), ("", 1));
    document.place_caret(paragraph, 0);
    let after = document.paste_rich(&clip).unwrap();
    assert_eq!(after, (paragraph, '\u{fffc}'.len_utf8()));
    assert_eq!(image_runs(&document, paragraph).len(), 2);
}

/// A picture floating from the copied text comes along floating the same
/// way, as Word pastes it, in one undo step with the text.
#[test]
fn a_copied_floating_picture_floats_the_same_way() {
    use jade_docx_editor::document::{RunImageKind, WrapMode};
    use jade_web::word::clipboard::WordClipboard;
    let mut document = contract();
    let paragraph = typed(&mut document, "ab ");
    document.place_caret(paragraph, 1);
    document.insert_picture(png()).unwrap();
    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: image_runs(&document, paragraph)[0],
        kind: FloatingObjectKind::Image,
    };
    let object = document.object_region(target).unwrap();
    document.set_picture_wrap(object, Wrap::Square).unwrap();
    let floating = |document: &jade_web::editing::EditableDocument| {
        let model = document.word_document().paragraph(paragraph).unwrap();
        image_runs(document, paragraph)
            .into_iter()
            .map(|run| model.runs[run].image.as_ref().unwrap().layout.clone())
            .collect::<Vec<_>>()
    };
    let before = floating(&document);
    assert_eq!(before[0].kind, RunImageKind::Floating);

    // "ab": the floating picture draws no character of the line.
    document.select((paragraph, 0), (paragraph, 2)).unwrap();
    let clip = document.copy_rich().unwrap();
    assert_eq!(clip.pictures.len(), 1);
    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    let caret = document.paste_rich(&clip).unwrap();
    let after = floating(&document);
    assert_eq!(after.len(), 2);
    let placed = |layout: &jade_docx_editor::document::RunImageLayout| {
        (
            layout.kind,
            layout.position_h.clone(),
            layout.position_v.clone(),
            layout.wrap,
            layout.wrap_text,
            layout.behind_doc,
        )
    };
    assert_eq!(placed(&after[1]), placed(&before[0]));
    assert_eq!(after[1].wrap, WrapMode::Square);
    assert_eq!(
        caret,
        (
            paragraph,
            document.word_document().rendered_paragraph_len(paragraph)
        )
    );
    let saved = document.export_docx().unwrap();
    let xml = jade_opc::OpcPackage::open(&saved)
        .unwrap()
        .part_str("word/document.xml")
        .unwrap()
        .unwrap();
    assert_eq!(xml.matches("<wp:anchor").count(), 2, "{xml}");
    document.undo().unwrap();
    assert_eq!(floating(&document).len(), 1);
}

#[test]
fn a_copied_picture_keeps_its_rotation_and_crop() {
    use jade_docx_editor::document::ImageCrop;
    use jade_web::word::clipboard::WordClipboard;
    use jade_web::word::objects::Rotate;
    use jade_web::word::picture_format::WordPictureFormat;
    let mut document = contract();
    let paragraph = typed(&mut document, "ab ");
    document.place_caret(paragraph, 1);
    document.insert_picture(png()).unwrap();
    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: image_runs(&document, paragraph)[0],
        kind: FloatingObjectKind::Image,
    };
    document.rotate_object(target, Rotate::Right90).unwrap();
    document
        .crop_picture(
            target,
            Some(ImageCrop {
                left: 0.1,
                top: 0.0,
                right: 0.2,
                bottom: 0.0,
            }),
        )
        .unwrap();
    document.select((paragraph, 0), (paragraph, 5)).unwrap();
    let clip = document.copy_rich().unwrap();
    let saved = |document: &mut jade_web::editing::EditableDocument| {
        jade_opc::OpcPackage::open(&document.export_docx().unwrap())
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap()
    };

    // Pasted in the same document and in another, the copy is turned and
    // cropped as the picture was, and brings its image.
    let end = document.word_document().rendered_paragraph_len(paragraph);
    document.place_caret(paragraph, end);
    document.paste_rich(&clip).unwrap();
    assert_eq!(image_runs(&document, paragraph).len(), 2);
    let xml = saved(&mut document);
    assert_eq!(xml.matches("rot=\"5400000\"").count(), 2, "{xml}");
    assert_eq!(xml.matches("<a:srcRect").count(), 2, "{xml}");

    let mut other = contract();
    let into = typed(&mut other, "x");
    other.place_caret(into, 1);
    other.paste_rich(&clip).unwrap();
    assert_eq!(image_runs(&other, into).len(), 1);
    let xml = saved(&mut other);
    assert!(
        xml.contains("rot=\"5400000\"") && xml.contains("<a:srcRect"),
        "{xml}"
    );
    let reopened = common::open_bytes(&other.export_docx().unwrap());
    let image = reopened.word_document().paragraph(into).unwrap().runs
        [image_runs(&reopened, into)[0]]
        .image
        .clone()
        .unwrap();
    assert!(image.data_uri.is_some(), "the image came with it");

    // One undo takes the pasted one back.
    document.undo().unwrap();
    assert_eq!(image_runs(&document, paragraph).len(), 1);
}

#[test]
fn copying_after_a_picture_across_paragraphs_uses_rendered_offsets() {
    use jade_web::word::headless::HeadlessCommand;
    let mut document = common::open_bytes(&jade_web::word::new::blank_document(
        jade_web::word::new::Blank::Japanese,
    ));
    document.place_caret(0, 0);
    document.insert_text("ABCDE").unwrap();
    document.place_caret(0, 1);
    document.insert_picture(png()).unwrap();
    document
        .apply_command(
            HeadlessCommand::InsertParagraphBreak {
                paragraph: 0,
                offset: "A\u{fffc}BCDE".len(),
                count: 1,
            },
            false,
        )
        .unwrap();
    document.place_caret(1, 0);
    document.insert_text("Z").unwrap();
    document.select((0, "A\u{fffc}".len()), (1, 1)).unwrap();
    assert_eq!(document.selected_text().as_deref(), Some("BCDE\nZ"));
}
