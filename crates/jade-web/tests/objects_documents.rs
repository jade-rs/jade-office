//! 図の形式 and 図形の書式 on the browser build's Word document, as the
//! native app does them too: each tool edits the object's drawing as Word
//! stores it, the object is drawn from what was written, the save carries
//! it, and undo takes it back.

mod common;
use common::{contract, typed};
use jade_docx_editor::document::{BlipEffect, ImageCrop, TextBoxShape};
use jade_docx_editor::word_document::{FloatingObjectKind, FloatingObjectTarget};
use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::objects::{
    Align, AlignTo, Arrange, ObjectOptions, Placement, Position, RelativeTo, Rotate, WordObjects,
};
use jade_web::word::picture::{WordPicture, Wrap};
use jade_web::word::picture_format::{
    Effect, OutlineEdit, Recolor, ReflectionPreset, ShadowPreset, WordPictureFormat,
};

/// A `width`×`height` PNG: white with a red square in the middle.
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    image::RgbaImage::from_fn(width, height, |x, y| {
        let inside = x >= width / 4 && x < width * 3 / 4 && y >= height / 4 && y < height * 3 / 4;
        if inside {
            image::Rgba([200, 0, 0, 255])
        } else {
            image::Rgba([255, 255, 255, 255])
        }
    })
    .write_to(
        &mut std::io::Cursor::new(&mut bytes),
        image::ImageFormat::Png,
    )
    .unwrap();
    bytes
}

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="20" viewBox="0 0 40 20"><rect width="40" height="20" fill="#2f5597"/></svg>"##;

fn document_xml(document: &mut EditableDocument) -> String {
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    package.part_str("word/document.xml").unwrap().unwrap()
}

/// A picture put in at a typed paragraph's start, and where it is.
fn with_picture(bytes: Vec<u8>) -> (EditableDocument, FloatingObjectTarget) {
    let mut document = contract();
    let paragraph = typed(&mut document, "pic ");
    document.place_caret(paragraph, 0);
    document.insert_picture(bytes).unwrap();
    let run = document.word_document().paragraph(paragraph).unwrap().runs
        .iter()
        .position(|run| run.image.is_some())
        .unwrap();
    (
        document,
        FloatingObjectTarget {
            paragraph_index: paragraph,
            run_index: run,
            kind: FloatingObjectKind::Image,
        },
    )
}

fn image(document: &EditableDocument, target: FloatingObjectTarget) -> jade_docx_editor::document::RunImage {
    document.word_document().paragraph(target.paragraph_index).unwrap().runs[target.run_index]
        .image
        .clone()
        .unwrap()
}

#[test]
fn an_svg_goes_in_with_its_png_fallback() {
    let (mut document, target) = with_picture(SVG.as_bytes().to_vec());
    let picture = image(&document, target);
    assert_eq!(
        (picture.width_px.map(f64::round), picture.height_px.map(f64::round)),
        (Some(40.0), Some(20.0))
    );
    assert!(picture.format().svg_relationship_id.is_some());
    assert!(
        picture.data_uri.as_deref().unwrap().starts_with("data:image/svg+xml"),
        "the SVG is what is drawn"
    );
    let xml = document_xml(&mut document);
    assert!(xml.contains("asvg:svgBlip"), "{xml}");
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let names = package.part_names();
    assert!(names.iter().any(|name| name.ends_with(".svg")), "{names:?}");
    assert!(names.iter().any(|name| name.starts_with("word/media/") && name.ends_with(".png")));
    // One step back takes the whole picture.
    document.undo().unwrap();
    assert!(document
        .word_document()
        .paragraph(target.paragraph_index)
        .unwrap()
        .runs
        .iter()
        .all(|run| run.image.is_none()));
}

#[test]
fn replacing_a_picture_keeps_its_size_place_and_effects() {
    let (mut document, target) = with_picture(png(40, 20));
    document.set_object_wrap(target, Wrap::Square).unwrap();
    document.resize_object(target, 120.0, 60.0).unwrap();
    document
        .set_object_effect(target, Effect::Shadow(Some(ShadowPreset::OUTER[0])))
        .unwrap();
    let before = image(&document, target);
    document.replace_picture(target, png(10, 30)).unwrap();
    let after = image(&document, target);
    assert_ne!(before.relationship_id, after.relationship_id);
    assert_eq!(after.width_px, before.width_px);
    assert_eq!(after.height_px, before.height_px);
    assert_eq!(after.layout.position_h, before.layout.position_h);
    assert_eq!(after.layout.wrap, before.layout.wrap);
    assert!(after.format().shadow.is_some(), "the shadow stays");
    document.undo().unwrap();
    assert_eq!(image(&document, target).relationship_id, before.relationship_id);
}

#[test]
fn outline_effects_and_colour_are_written_and_drawn() {
    let (mut document, target) = with_picture(png(40, 20));
    document
        .set_picture_outline(target, OutlineEdit::Color("FF0000".into()))
        .unwrap();
    document.set_picture_outline(target, OutlineEdit::Width(3.0)).unwrap();
    document
        .set_object_effect(target, Effect::Reflection(Some(ReflectionPreset::Half)))
        .unwrap();
    document
        .set_object_effect(target, Effect::Glow(Some((5.0, "4472C4".into()))))
        .unwrap();
    document.set_object_effect(target, Effect::SoftEdge(Some(2.5))).unwrap();
    document
        .set_object_effect(target, Effect::Bevel(Some("circle".into())))
        .unwrap();
    document.set_picture_corrections(target, 0.2, -0.2).unwrap();
    document.recolor_picture(target, Recolor::Grayscale).unwrap();
    document.set_picture_transparency(target, 0.5).unwrap();

    let picture = image(&document, target);
    let format = picture.format();
    let outline = format.outline.as_ref().unwrap();
    assert_eq!(outline.color.as_deref(), Some("#FF0000"));
    assert_eq!(outline.width_px.map(|px| (px * 0.75).round()), Some(3.0));
    assert!(format.reflection.is_some());
    assert_eq!(format.glow.as_ref().unwrap().color.as_deref(), Some("#4472C4"));
    assert!(format.soft_edge_radius_px.is_some());
    assert_eq!(format.bevel.as_ref().unwrap().preset, "circle");
    assert_eq!(
        format.blip_effects,
        vec![
            BlipEffect::Luminance {
                brightness: 0.2,
                contrast: -0.2
            },
            BlipEffect::Grayscale,
            BlipEffect::AlphaModFix { amount: 0.5 },
        ]
    );
    // The picture is drawn from a copy with its adjustments applied.
    let drawn = jade_docx_editor::picture_effects::effective_data_uri(&picture).unwrap();
    assert_ne!(drawn.as_ref(), picture.data_uri.as_deref().unwrap());

    let xml = document_xml(&mut document);
    for expected in [
        r#"<a:lum bright="20000" contrast="-20000"/>"#,
        "<a:grayscl/>",
        r#"<a:alphaModFix amt="50000"/>"#,
        "<a:reflection ",
        "<a:glow ",
        "<a:softEdge ",
        "<a:bevelT ",
        r#"<a:srgbClr val="FF0000"/>"#,
    ] {
        assert!(xml.contains(expected), "missing {expected}: {xml}");
    }
    // Effects in their schema order.
    let glow = xml.find("<a:glow ").unwrap();
    let reflection = xml.find("<a:reflection ").unwrap();
    let soft = xml.find("<a:softEdge ").unwrap();
    assert!(glow < reflection && reflection < soft);

    document.reset_picture(target, false).unwrap();
    let picture = image(&document, target);
    assert!(picture.format().is_empty() || picture.format().blip_effects.is_empty());
    assert!(picture.format().outline.is_none());
    assert!(picture.format().reflection.is_none());
}

#[test]
fn saturation_artistic_effects_and_background_removal_are_baked_with_the_original_kept() {
    let (mut document, target) = with_picture(png(16, 16));
    let original = image(&document, target).relationship_id;
    document.set_picture_saturation(target, 0.0).unwrap();
    document.remove_picture_background(target, true).unwrap();
    let picture = image(&document, target);
    assert_ne!(picture.relationship_id, original, "the blip is the baked copy");
    let corrections = picture.format().corrections.clone().unwrap();
    assert_eq!(corrections.original_relationship_id, original);
    assert_eq!(corrections.saturation, Some(0.0));
    assert!(corrections.background_removal.is_some());
    // The corner went transparent, the middle stayed (grey now).
    let bytes = jade_docx_editor::picture_effects::data_uri_bytes(picture.data_uri.as_deref().unwrap()).unwrap();
    let pixels = jade_docx_editor::picture_effects::decode(&bytes).unwrap();
    assert_eq!(pixels.get_pixel(0, 0).0[3], 0);
    let middle = pixels.get_pixel(8, 8).0;
    assert_eq!(middle[3], 255);
    assert!(middle[0] == middle[1] && middle[1] == middle[2]);
    let xml = document_xml(&mut document);
    assert!(xml.contains("a14:backgroundRemoval"), "{xml}");
    assert!(xml.contains("a14:imgLayer"), "{xml}");

    document.reset_picture(target, true).unwrap();
    let picture = image(&document, target);
    assert_eq!(picture.relationship_id, original);
    assert!(picture.format().corrections.is_none());
    assert_eq!(picture.width_px.map(f64::round), Some(16.0));
}

#[test]
fn cropping_to_a_shape_and_an_aspect_ratio() {
    let (mut document, target) = with_picture(png(40, 20));
    document.crop_picture_to_shape(target, TextBoxShape::Ellipse).unwrap();
    assert_eq!(image(&document, target).format().geometry, Some(TextBoxShape::Ellipse));
    document.crop_picture_to_aspect(target, (1, 1)).unwrap();
    let picture = image(&document, target);
    let crop = picture.crop.unwrap();
    assert!((crop.left - 0.25).abs() < 1e-3 && (crop.right - 0.25).abs() < 1e-3);
    assert_eq!(crop.top, 0.0);
    assert_eq!(picture.width_px.map(f64::round), picture.height_px.map(f64::round));
    document
        .crop_picture(
            target,
            Some(ImageCrop {
                left: 0.1,
                top: 0.0,
                right: 0.0,
                bottom: 0.2,
            }),
        )
        .unwrap();
    assert_eq!(image(&document, target).crop.unwrap().bottom, 0.2);
    let xml = document_xml(&mut document);
    assert!(xml.contains(r#"<a:prstGeom prst="ellipse">"#), "{xml}");
    assert!(xml.contains(r#"<a:srcRect l="10000" t="0" r="0" b="20000"/>"#), "{xml}");
}

#[test]
fn rotating_and_flipping() {
    let (mut document, target) = with_picture(png(40, 20));
    document.rotate_object(target, Rotate::Right90).unwrap();
    document.rotate_object(target, Rotate::Right90).unwrap();
    document.rotate_object(target, Rotate::Left90).unwrap();
    document.rotate_object(target, Rotate::FlipHorizontal).unwrap();
    let format = document.object_format(target).unwrap();
    assert_eq!(format.rotation.round(), 90.0);
    assert!(format.flip_horizontal && !format.flip_vertical);
    document.set_object_rotation(target, -30.0).unwrap();
    assert_eq!(document.object_format(target).unwrap().rotation.round(), 330.0);
    // A drag keeps the rotation.
    let placed = document.object_region(target).unwrap();
    document
        .move_object(placed, (placed.x + 10.0, placed.y, placed.width, placed.height))
        .unwrap();
    assert_eq!(document.object_format(target).unwrap().rotation.round(), 330.0);
}

#[test]
fn the_layout_dialog_positions_and_options() {
    let (mut document, target) = with_picture(png(40, 20));
    document
        .set_object_position(
            target,
            Position {
                relative_to: RelativeTo::Margin,
                placement: Placement::Align("center".into()),
            },
            Position {
                relative_to: RelativeTo::Page,
                placement: Placement::Offset(96.0),
            },
        )
        .unwrap();
    let format = document.object_format(target).unwrap();
    assert_ne!(format.wrap, Wrap::Inline);
    assert_eq!(
        format.horizontal,
        Some(Position {
            relative_to: RelativeTo::Margin,
            placement: Placement::Align("center".into())
        })
    );
    assert!(!format.moves_with_text);
    document
        .set_object_options(
            target,
            ObjectOptions {
                moves_with_text: Some(true),
                locked: Some(true),
                allow_overlap: Some(false),
                layout_in_cell: Some(false),
                paragraph_y: Some(12.0),
                ..Default::default()
            },
        )
        .unwrap();
    document.set_object_distances(target, [6.0, 7.0, 8.0, 9.0]).unwrap();
    let format = document.object_format(target).unwrap();
    assert!(format.moves_with_text && format.locked);
    assert!(!format.allow_overlap && !format.layout_in_cell);
    assert_eq!(format.distances.map(f64::round), [6.0, 7.0, 8.0, 9.0]);
    let xml = document_xml(&mut document);
    assert!(xml.contains(r#"locked="1""#), "{xml}");
    assert!(xml.contains(r#"allowOverlap="0""#), "{xml}");
    assert!(xml.contains("<wp:align>center</wp:align>"), "{xml}");
    // The page centre is where it is drawn.
    let placed = document.object_region(target).unwrap();
    let setup = jade_web::word::page::WordPage::page_setup(&document);
    let left = f64::from(setup.margins.left) * 96.0 / 72.0;
    let right = f64::from(setup.width - setup.margins.right) * 96.0 / 72.0;
    assert!((placed.x + placed.width / 2.0 - (left + right) / 2.0).abs() < 1.0, "{placed:?}");
}

#[test]
fn stacking_names_visibility_and_alignment() {
    let (mut document, first) = with_picture(png(40, 20));
    document.set_object_wrap(first, Wrap::InFrontOfText).unwrap();
    let paragraph = first.paragraph_index;
    document.place_caret(paragraph, 0);
    document.insert_picture(png(20, 20)).unwrap();
    // The new picture went in before the first one.
    let first = FloatingObjectTarget {
        run_index: first.run_index + 1,
        ..first
    };
    let second = FloatingObjectTarget {
        run_index: first.run_index - 1,
        ..first
    };
    document.set_object_wrap(second, Wrap::InFrontOfText).unwrap();
    let z = |document: &EditableDocument, target| {
        jade_web::word::objects::stacking_index(document.word_document(), target).unwrap()
    };
    document.arrange_object(second, Arrange::ToFront).unwrap();
    assert!(z(&document, second) > z(&document, first));
    document.arrange_object(second, Arrange::Backward).unwrap();
    assert!(z(&document, second) < z(&document, first));
    document.arrange_object(first, Arrange::ToBack).unwrap();
    assert!(z(&document, second) > z(&document, first));
    let xml = document_xml(&mut document);
    assert!(xml.contains(r#"relativeHeight="251660288""#), "{xml}");

    document.rename_object(first, "ロゴ").unwrap();
    document.set_object_hidden(second, true).unwrap();
    let names: Vec<_> = document
        .body_objects()
        .into_iter()
        .filter(|object| object.target.paragraph_index == paragraph)
        .map(|object| (object.name, object.hidden))
        .collect();
    assert!(names.contains(&("ロゴ".to_string(), false)), "{names:?}");
    assert!(names.iter().any(|(_, hidden)| *hidden));
    assert!(document.object_region(second).is_none(), "a hidden object is not on the page");
    document.set_object_hidden(second, false).unwrap();

    let a = document.object_region(first).unwrap();
    let b = document.object_region(second).unwrap();
    document.align_objects(&[a, b], Align::Left, AlignTo::Selection).unwrap();
    let a = document.object_region(first).unwrap();
    let b = document.object_region(second).unwrap();
    assert!((a.x - b.x).abs() < 0.5, "{a:?} {b:?}");
    document.align_objects(&[a, b], Align::Top, AlignTo::Page).unwrap();
    let top = document.object_region(first).unwrap();
    assert!(top.y.abs() < 0.5, "{top:?} {:?}", document.object_format(first));
    document.undo().unwrap();
    assert!((document.object_region(first).unwrap().y - a.y).abs() < 0.5);
}
