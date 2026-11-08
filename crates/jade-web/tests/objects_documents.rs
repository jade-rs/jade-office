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
use jade_web::word::shapes::WordShapes;

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
    let run = document
        .word_document()
        .paragraph(paragraph)
        .unwrap()
        .runs
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

fn image(
    document: &EditableDocument,
    target: FloatingObjectTarget,
) -> jade_docx_editor::document::RunImage {
    document
        .word_document()
        .paragraph(target.paragraph_index)
        .unwrap()
        .runs[target.run_index]
        .image
        .clone()
        .unwrap()
}

#[test]
fn an_svg_goes_in_with_its_png_fallback() {
    let (mut document, target) = with_picture(SVG.as_bytes().to_vec());
    let picture = image(&document, target);
    assert_eq!(
        (
            picture.width_px.map(f64::round),
            picture.height_px.map(f64::round)
        ),
        (Some(40.0), Some(20.0))
    );
    assert!(picture.format().svg_relationship_id.is_some());
    assert!(
        picture
            .data_uri
            .as_deref()
            .unwrap()
            .starts_with("data:image/svg+xml"),
        "the SVG is what is drawn"
    );
    let xml = document_xml(&mut document);
    assert!(xml.contains("asvg:svgBlip"), "{xml}");
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let names = package.part_names();
    assert!(names.iter().any(|name| name.ends_with(".svg")), "{names:?}");
    assert!(names
        .iter()
        .any(|name| name.starts_with("word/media/") && name.ends_with(".png")));
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
    assert_eq!(
        image(&document, target).relationship_id,
        before.relationship_id
    );
}

#[test]
fn outline_effects_and_colour_are_written_and_drawn() {
    let (mut document, target) = with_picture(png(40, 20));
    document
        .set_picture_outline(target, OutlineEdit::Color("FF0000".into()))
        .unwrap();
    document
        .set_picture_outline(target, OutlineEdit::Width(3.0))
        .unwrap();
    document
        .set_object_effect(target, Effect::Reflection(Some(ReflectionPreset::Half)))
        .unwrap();
    document
        .set_object_effect(target, Effect::Glow(Some((5.0, "4472C4".into()))))
        .unwrap();
    document
        .set_object_effect(target, Effect::SoftEdge(Some(2.5)))
        .unwrap();
    document
        .set_object_effect(target, Effect::Bevel(Some("circle".into())))
        .unwrap();
    document.set_picture_corrections(target, 0.2, -0.2).unwrap();
    document
        .recolor_picture(target, Recolor::Grayscale)
        .unwrap();
    document.set_picture_transparency(target, 0.5).unwrap();

    let picture = image(&document, target);
    let format = picture.format();
    let outline = format.outline.as_ref().unwrap();
    assert_eq!(outline.color.as_deref(), Some("#FF0000"));
    assert_eq!(outline.width_px.map(|px| (px * 0.75).round()), Some(3.0));
    assert!(format.reflection.is_some());
    assert_eq!(
        format.glow.as_ref().unwrap().color.as_deref(),
        Some("#4472C4")
    );
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
    assert_ne!(
        picture.relationship_id, original,
        "the blip is the baked copy"
    );
    let corrections = picture.format().corrections.clone().unwrap();
    assert_eq!(corrections.original_relationship_id, original);
    assert_eq!(corrections.saturation, Some(0.0));
    assert!(corrections.background_removal.is_some());
    // The corner went transparent, the middle stayed (grey now).
    let bytes =
        jade_docx_editor::picture_effects::data_uri_bytes(picture.data_uri.as_deref().unwrap())
            .unwrap();
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
    document
        .crop_picture_to_shape(target, TextBoxShape::Ellipse)
        .unwrap();
    assert_eq!(
        image(&document, target).format().geometry,
        Some(TextBoxShape::Ellipse)
    );
    document.crop_picture_to_aspect(target, (1, 1)).unwrap();
    let picture = image(&document, target);
    let crop = picture.crop.unwrap();
    assert!((crop.left - 0.25).abs() < 1e-3 && (crop.right - 0.25).abs() < 1e-3);
    assert_eq!(crop.top, 0.0);
    assert_eq!(
        picture.width_px.map(f64::round),
        picture.height_px.map(f64::round)
    );
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
    assert!(
        xml.contains(r#"<a:srcRect l="10000" t="0" r="0" b="20000"/>"#),
        "{xml}"
    );
}

#[test]
fn rotating_and_flipping() {
    let (mut document, target) = with_picture(png(40, 20));
    document.rotate_object(target, Rotate::Right90).unwrap();
    document.rotate_object(target, Rotate::Right90).unwrap();
    document.rotate_object(target, Rotate::Left90).unwrap();
    document
        .rotate_object(target, Rotate::FlipHorizontal)
        .unwrap();
    let format = document.object_format(target).unwrap();
    assert_eq!(format.rotation.round(), 90.0);
    assert!(format.flip_horizontal && !format.flip_vertical);
    document.set_object_rotation(target, -30.0).unwrap();
    assert_eq!(
        document.object_format(target).unwrap().rotation.round(),
        330.0
    );
    // A drag keeps the rotation.
    let placed = document.object_region(target).unwrap();
    document
        .move_object(
            placed,
            (placed.x + 10.0, placed.y, placed.width, placed.height),
        )
        .unwrap();
    assert_eq!(
        document.object_format(target).unwrap().rotation.round(),
        330.0
    );
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
    document
        .set_object_distances(target, [6.0, 7.0, 8.0, 9.0])
        .unwrap();
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
    assert!(
        (placed.x + placed.width / 2.0 - (left + right) / 2.0).abs() < 1.0,
        "{placed:?}"
    );
}

#[test]
fn stacking_names_visibility_and_alignment() {
    let (mut document, first) = with_picture(png(40, 20));
    document
        .set_object_wrap(first, Wrap::InFrontOfText)
        .unwrap();
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
    document
        .set_object_wrap(second, Wrap::InFrontOfText)
        .unwrap();
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
    assert!(
        document.object_region(second).is_none(),
        "a hidden object is not on the page"
    );
    document.set_object_hidden(second, false).unwrap();

    let a = document.object_region(first).unwrap();
    let b = document.object_region(second).unwrap();
    document
        .align_objects(&[a, b], Align::Left, AlignTo::Selection)
        .unwrap();
    let a = document.object_region(first).unwrap();
    let b = document.object_region(second).unwrap();
    assert!((a.x - b.x).abs() < 0.5, "{a:?} {b:?}");
    document
        .align_objects(&[a, b], Align::Top, AlignTo::Page)
        .unwrap();
    let top = document.object_region(first).unwrap();
    assert!(
        top.y.abs() < 0.5,
        "{top:?} {:?}",
        document.object_format(first)
    );
    document.undo().unwrap();
    assert!((document.object_region(first).unwrap().y - a.y).abs() < 0.5);
}

#[test]
fn transforming_vml_text_boxes_exports_one_box_and_reopens_the_new_frame() {
    for floating in [false, true] {
        let mut package = jade_opc::OpcPackage::open(&jade_web::word::new::blank_document(
            jade_web::word::new::Blank::English,
        ))
        .unwrap();
        let position = if floating {
            "position:absolute;margin-left:72pt;margin-top:72pt;mso-position-horizontal-relative:page;mso-position-vertical-relative:page;"
        } else {
            ""
        };
        package.replace("word/document.xml", format!(r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:pict><v:shape id="Box1" style="{position}width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>日本語 box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p><w:p><w:r><w:t>Body after</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="12240" w:h="15840"/><w:pgMar w:top="1440" w:right="1440" w:bottom="1440" w:left="1440"/></w:sectPr></w:body></w:document>"#).into_bytes());
        let mut document = common::open_bytes(&package.write().unwrap());
        let target = FloatingObjectTarget {
            paragraph_index: 0,
            run_index: 0,
            kind: FloatingObjectKind::TextBox,
        };
        let before = document.export_docx().unwrap();
        let placed = document.object_region(target).unwrap();
        let rect = (
            placed.x + if floating { 20.0 } else { 0.0 },
            placed.y + if floating { 10.0 } else { 0.0 },
            placed.width + 20.0,
            placed.height + 10.0,
        );
        document.place_object(placed, rect).unwrap();
        let saved = document.export_docx().unwrap();
        let xml = jade_opc::OpcPackage::open(&saved)
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap();
        assert_eq!(
            xml.matches("<w:txbxContent>").count(),
            1,
            "one logical box must be exported: {xml}"
        );
        let reopened = common::open_bytes(&saved);
        let restored = reopened.object_region(target).unwrap();
        assert!((restored.width - rect.2).abs() < 0.1);
        assert!((restored.height - rect.3).abs() < 0.1);
        if floating {
            assert!((restored.x - rect.0).abs() < 0.1);
            assert!((restored.y - rect.1).abs() < 0.1);
        }
        let original_box = document.word_document().paragraph(0).unwrap().runs[0]
            .text_box
            .as_ref()
            .unwrap();
        let restored_box = reopened.word_document().paragraph(0).unwrap().runs[0]
            .text_box
            .as_ref()
            .unwrap();
        assert_eq!(restored_box.text, original_box.text);
        assert_eq!(restored_box.blocks.len(), original_box.blocks.len());
        let jade_docx_editor::document::BodyElement::Paragraph(paragraph) = &restored_box.blocks[0]
        else {
            panic!("text-box paragraph missing")
        };
        assert_eq!(
            paragraph.runs[0].properties.as_ref().unwrap().bold,
            Some(true),
            "text-box run formatting must survive"
        );
        document.undo().unwrap();
        let undone = document.export_docx().unwrap();
        let original = common::open_bytes(&before);
        let undone_document = common::open_bytes(&undone);
        let original_frame = original.object_region(target).unwrap();
        let undone_frame = undone_document.object_region(target).unwrap();
        for (actual, expected) in [
            (undone_frame.x, original_frame.x),
            (undone_frame.y, original_frame.y),
            (undone_frame.width, original_frame.width),
            (undone_frame.height, original_frame.height),
        ] {
            assert!(
                (actual - expected).abs() < 0.1,
                "undo must restore the original frame"
            );
        }
        let undone_xml = jade_opc::OpcPackage::open(&undone)
            .unwrap()
            .part_str("word/document.xml")
            .unwrap()
            .unwrap();
        assert_eq!(undone_xml.matches("<w:txbxContent>").count(), 1);
        assert!(undone_xml.contains("<w:t>Body after</w:t>"));
        document.redo().unwrap();
        let redone = common::open_bytes(&document.export_docx().unwrap());
        let redone_frame = redone.object_region(target).unwrap();
        assert!((redone_frame.width - rect.2).abs() < 0.1);
        assert!((redone_frame.height - rect.3).abs() < 0.1);
    }
}

#[test]
fn deleting_a_text_box_keeps_host_text_and_restores_the_box_on_undo() {
    let mut package = jade_opc::OpcPackage::open(&jade_web::word::new::blank_document(
        jade_web::word::new::Blank::English,
    ))
    .unwrap();
    package.replace("word/document.xml", br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:t>Host text</w:t><w:pict><v:shape id="Box1" style="width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:r><w:t>Inside box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p><w:p><w:r><w:t>Body after</w:t></w:r></w:p></w:body></w:document>"#.to_vec());
    let mut document = common::open_bytes(&package.write().unwrap());
    let target = FloatingObjectTarget {
        paragraph_index: 0,
        run_index: 0,
        kind: FloatingObjectKind::TextBox,
    };
    let before = document_xml(&mut document);
    document.select((1, 0), (1, 6)).unwrap();
    document.delete_object(target).unwrap();
    assert_eq!(document.caret(), Some((0, 0)));
    let saved = document.export_docx().unwrap();
    let xml = document_xml(&mut document);
    assert!(
        !xml.contains("w:txbxContent"),
        "deleted box survived: {xml}"
    );
    let reopened = common::open_bytes(&saved);
    assert_eq!(reopened.paragraph_text(0).as_deref(), Some("Host text"));
    assert_eq!(reopened.paragraph_text(1).as_deref(), Some("Body after"));
    assert!(document.undo().unwrap());
    assert_eq!(
        document_xml(&mut document),
        before,
        "undo restores original drawing XML"
    );
    assert_eq!(document.paragraph_text(1).as_deref(), Some("Inside box"));
    assert!(document.redo().unwrap());
    assert!(!document_xml(&mut document).contains("w:txbxContent"));
}

#[test]
fn deleting_a_picture_preserves_host_text_and_round_trips_history() {
    let (mut document, target) = with_picture(png(8, 8));
    let before = document_xml(&mut document);
    let text = document.paragraph_text(target.paragraph_index).unwrap();
    document.delete_object(target).unwrap();
    let deleted = document_xml(&mut document);
    assert!(!deleted.contains("<pic:pic"));
    let reopened = common::open_bytes(&document.export_docx().unwrap());
    assert_eq!(reopened.paragraph_text(target.paragraph_index), Some(text));
    assert!(document.undo().unwrap());
    assert_eq!(document_xml(&mut document), before);
    assert!(document.redo().unwrap());
    assert_eq!(document_xml(&mut document), deleted);
}

#[test]
fn copying_a_cut_vml_box_to_another_document_preserves_its_contents_and_history() {
    let mut package = jade_opc::OpcPackage::open(&jade_web::word::new::blank_document(
        jade_web::word::new::Blank::English,
    ))
    .unwrap();
    package.replace("word/document.xml", br#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:t>Host text</w:t><w:pict><v:shape id="Box1" style="width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:r><w:rPr><w:b/></w:rPr><w:t>Inside box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p></w:body></w:document>"#.to_vec());
    let mut source = common::open_bytes(&package.write().unwrap());
    let target = FloatingObjectTarget {
        paragraph_index: 0,
        run_index: 0,
        kind: FloatingObjectKind::TextBox,
    };
    let clip = source.copy_object(target).unwrap();
    source.delete_object(target).unwrap();
    assert_eq!(source.paragraph_text(0).as_deref(), Some("Host text"));
    let mut destination = contract();
    let paragraph = typed(&mut destination, "Paste here");
    destination.place_caret(paragraph, 5);
    let before = destination.export_docx().unwrap();
    let host_text = destination.paragraph_text(paragraph);
    destination.paste_object(&clip).unwrap();
    assert_eq!(destination.paragraph_text(paragraph), host_text);
    let after = destination.export_docx().unwrap();
    let xml = document_xml(&mut destination);
    assert!(xml.contains("Inside box"));
    assert!(xml.contains("width:200pt;height:100pt"));
    assert!(!xml.contains("Host text"));
    assert!(xml.contains("<w:b"));
    let reopened = common::open_bytes(&after);
    assert!((0..reopened.word_document().paragraph_count())
        .any(|p| reopened.paragraph_text(p).as_deref() == Some("Inside box")));
    assert!(destination.undo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), before);
    assert!(destination.redo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), after);
    let mut reopened = common::open_bytes(&after);
    reopened.place_caret(paragraph, 0);
    reopened.paste_object(&clip).unwrap();
    let xml = document_xml(&mut reopened);
    assert!(xml.contains("id=\"_jade_copy_1\""));
    assert!(xml.contains("id=\"_jade_copy_2\""));
    assert_eq!(xml.matches("id=\"_jade_copy_1\"").count(), 1);
}

#[test]
fn copied_svg_imports_fallback_and_original_without_overwriting_destination_media() {
    let (mut source, target) = with_picture(SVG.as_bytes().to_vec());
    let clip = source.copy_object(target).unwrap();
    source.delete_object(target).unwrap();
    let (mut destination, existing) = with_picture(png(11, 13));
    let original = image(&destination, existing);
    destination.place_caret(existing.paragraph_index, 0);
    let before = destination.export_docx().unwrap();
    destination.paste_object(&clip).unwrap();
    let after = destination.export_docx().unwrap();
    let reopened = common::open_bytes(&after);
    let pictures: Vec<_> = reopened
        .word_document()
        .paragraph(existing.paragraph_index)
        .unwrap()
        .runs
        .iter()
        .filter_map(|r| r.image.as_ref())
        .collect();
    assert_eq!(pictures.len(), 2);
    assert_eq!(pictures[0].width_px, Some(40.0));
    assert_eq!(pictures[0].height_px, Some(20.0));
    assert!(pictures[0].format().svg_relationship_id.is_some());
    assert_eq!(pictures[1].data_uri, original.data_uri);
    assert_eq!(
        reopened
            .word_document()
            .object_media(pictures[0].format().svg_relationship_id.as_deref().unwrap())
            .unwrap(),
        SVG.as_bytes()
    );
    assert!(destination.undo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), before);
    assert!(destination.redo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), after);
}

#[test]
fn copied_chart_keeps_its_editable_data_and_embedded_workbook_in_a_new_document() {
    use jade_web::word::diagrams::{ChartKind, WordDiagrams};
    let mut source = contract();
    let paragraph = typed(&mut source, "Chart source");
    source.place_caret(paragraph, 0);
    source.insert_chart(ChartKind::Bar, true).unwrap();
    let target = FloatingObjectTarget {
        paragraph_index: paragraph,
        run_index: 0,
        kind: FloatingObjectKind::Image,
    };
    let chart = source.chart_data(target).unwrap();
    let clip = source.copy_object(target).unwrap();
    source.delete_object(target).unwrap();
    let mut destination = contract();
    let paragraph = typed(&mut destination, "Chart destination");
    destination.place_caret(paragraph, 0);
    let before = destination.export_docx().unwrap();
    destination.paste_object(&clip).unwrap();
    let after = destination.export_docx().unwrap();
    let reopened = common::open_bytes(&after);
    let pasted = FloatingObjectTarget {
        paragraph_index: paragraph,
        ..target
    };
    assert_eq!(reopened.chart_data(pasted).unwrap().title, chart.title);
    assert_eq!(reopened.chart_data(pasted).unwrap().series, chart.series);
    let package = jade_opc::OpcPackage::open(&after).unwrap();
    assert!(package
        .part_names()
        .iter()
        .any(|path| path.starts_with("word/jade-paste-") && path.ends_with(".xlsx")));
    assert!(destination.undo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), before);
    assert!(destination.redo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), after);
}

#[test]
fn object_paste_replaces_forward_and_backward_text_ranges_as_one_history_step() {
    let (source, target) = with_picture(png(9, 7));
    let clip = source.copy_object(target).unwrap();
    for (anchor, focus, expected) in [
        ((0, 3), (0, 12), vec!["前後", "second tail", "untouched"]),
        ((0, 12), (0, 3), vec!["前後", "second tail", "untouched"]),
        ((0, 3), (1, 6), vec!["前 tail", "untouched"]),
        ((1, 6), (0, 3), vec!["前 tail", "untouched"]),
    ] {
        let mut package = jade_opc::OpcPackage::open(&jade_web::word::new::blank_document(
            jade_web::word::new::Blank::English,
        ))
        .unwrap();
        package.replace("word/document.xml", r#"<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>前選択中後</w:t></w:r></w:p><w:p><w:r><w:t>second tail</w:t></w:r></w:p><w:p><w:r><w:t>untouched</w:t></w:r></w:p></w:body></w:document>"#.as_bytes().to_vec());
        let mut destination = common::open_bytes(&package.write().unwrap());
        destination.select(anchor, focus).unwrap();
        let selection = destination.selection();
        let before = destination.export_docx().unwrap();
        destination.paste_object(&clip).unwrap();
        let after = destination.export_docx().unwrap();
        let reopened = common::open_bytes(&after);
        assert_eq!(
            (0..reopened.word_document().paragraph_count())
                .map(|p| reopened.paragraph_text(p).unwrap())
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(reopened.body_objects().len(), 1);
        let runs = &reopened.word_document().paragraph(0).unwrap().runs;
        let object_index = runs.iter().position(|run| run.image.is_some()).unwrap();
        assert_eq!(
            runs[..object_index]
                .iter()
                .map(|run| run.text.as_str())
                .collect::<String>(),
            "前"
        );
        assert_eq!(destination.caret(), Some((0, 3)));
        assert!(destination.undo().unwrap());
        assert_eq!(destination.selection(), selection);
        assert_eq!(destination.export_docx().unwrap(), before);
        assert!(destination.redo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), after);
    }
}

#[test]
fn object_replacement_rejects_invalid_ranges_without_changing_text_or_package() {
    use jade_docx_editor::commands::paste_object::PasteObjectCommand;
    use jade_selection::{SelectionRange, TextPosition};
    let (source, target) = with_picture(png(9, 7));
    let clip = source.copy_object(target).unwrap();
    let mut destination = contract();
    let paragraph = typed(&mut destination, "日本語");
    destination.select((paragraph, 0), (paragraph, 3)).unwrap();
    let before = destination.export_docx().unwrap();
    let selection = destination.selection();
    for end in [1, usize::MAX] {
        let range = SelectionRange::new(
            TextPosition::new(paragraph, 0),
            TextPosition::new(paragraph, end),
        );
        assert!(destination
            .execute_command(
                Box::new(PasteObjectCommand::new(range, clip.clone())),
                false
            )
            .is_err());
        assert_eq!(destination.selection(), selection);
        assert_eq!(destination.export_docx().unwrap(), before);
    }
}

#[test]
fn copied_text_box_imports_style_dependencies_without_changing_destination_styles() {
    let host_format = |document: &EditableDocument| {
        let word = document.word_document();
        let paragraph = word.paragraph(0).unwrap();
        let run = paragraph
            .runs
            .iter()
            .find(|run| run.text == "Destination")
            .unwrap();
        word.styles_part
            .as_ref()
            .and_then(|styles| {
                styles.resolve_effective_run_properties(
                    paragraph
                        .properties
                        .as_ref()
                        .and_then(|props| props.style.as_deref()),
                    run.properties.as_ref(),
                )
            })
            .unwrap_or_default()
    };
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let blank = jade_web::word::new::blank_document(jade_web::word::new::Blank::English);
    let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
    package.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:pict><v:shape id="Box1" style="width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:pPr><w:pStyle w:val="BoxText"/></w:pPr><w:r><w:rPr><w:rStyle w:val="BoxEmphasis"/></w:rPr><w:t>Styled box</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p></w:body></w:document>"#).into_bytes());
    package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:default="1" w:styleId="Base"><w:name w:val="Source base"/><w:rPr><w:b/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="BoxText"><w:name w:val="Box text"/><w:basedOn w:val="Base"/><w:next w:val="BoxText"/><w:link w:val="BoxEmphasis"/></w:style><w:style w:type="character" w:styleId="BoxEmphasis"><w:name w:val="Box emphasis"/><w:link w:val="BoxText"/><w:rPr><w:i/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Unused"><w:name w:val="Unused"/></w:style></w:styles>"#).into_bytes());
    let source = common::open_bytes(&package.write().unwrap());
    let target = FloatingObjectTarget {
        paragraph_index: 0,
        run_index: 0,
        kind: FloatingObjectKind::TextBox,
    };
    let clip = source.copy_object(target).unwrap();
    for destination_kind in 0..3 {
        let without_styles = destination_kind != 0;
        let mut destination_package = jade_opc::OpcPackage::open(&blank).unwrap();
        destination_package.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}"><w:body><w:p><w:pPr><w:pStyle w:val="BoxText"/></w:pPr><w:r><w:t>Destination</w:t></w:r></w:p></w:body></w:document>"#).into_bytes());
        if without_styles {
            if destination_kind == 1 {
                destination_package.remove("word/styles.xml");
            } else {
                destination_package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="BoxText"><w:name w:val="Orphan style"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#).into_bytes());
            }
            let rels = destination_package
                .part_str("word/_rels/document.xml.rels")
                .unwrap()
                .unwrap();
            // Remove only the style relationship, retaining all other package parts.
            let rels = rels
                .split("<Relationship ")
                .enumerate()
                .filter_map(|(index, fragment)| {
                    if index == 0 {
                        return Some(fragment.to_owned());
                    }
                    let end = fragment.find("/>").unwrap() + 2;
                    if fragment[..end].contains("/styles\"") {
                        Some(fragment[end..].to_owned())
                    } else {
                        Some(format!("<Relationship {fragment}"))
                    }
                })
                .collect::<String>();
            destination_package.replace("word/_rels/document.xml.rels", rels.into_bytes());
        } else {
            destination_package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:default="1" w:styleId="Base"><w:name w:val="Destination base"/><w:rPr><w:b w:val="0"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="BoxText"><w:name w:val="Box text"/><w:basedOn w:val="Base"/></w:style><w:style w:type="character" w:styleId="BoxEmphasis"><w:name w:val="Box emphasis"/><w:rPr><w:i w:val="0"/></w:rPr></w:style></w:styles>"#).into_bytes());
        }
        if destination_kind == 0 {
            destination_package.replace("word/numbering.xml", format!(r#"<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="4"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:pStyle w:val="HostNumber"/><w:lvlText w:val="%1."/></w:lvl></w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="4"/></w:num></w:numbering>"#).into_bytes());
            let rels = destination_package
                .part_str("word/_rels/document.xml.rels")
                .unwrap()
                .unwrap();
            if !rels.contains("/numbering\"") {
                destination_package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", r#"<Relationship Id="rIdHostNumber" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#).into_bytes());
            }
            let types = destination_package
                .part_str("[Content_Types].xml")
                .unwrap()
                .unwrap();
            if !types.contains("/word/numbering.xml") {
                destination_package.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#).into_bytes());
            }
        }
        let mut destination = common::open_bytes(&destination_package.write().unwrap());
        destination.place_caret(0, 0);
        if destination_kind == 0 {
            assert_eq!(
                destination
                    .word_document()
                    .styles_part
                    .as_ref()
                    .unwrap()
                    .resolve_numbering("HostNumber")
                    .unwrap()
                    .list_id(),
                Some("7")
            );
        }
        let host_before = host_format(&destination);
        let before = destination.export_docx().unwrap();
        destination.paste_object(&clip).unwrap();
        if destination_kind == 0 {
            assert_eq!(
                destination
                    .word_document()
                    .styles_part
                    .as_ref()
                    .unwrap()
                    .resolve_numbering("HostNumber")
                    .unwrap()
                    .list_id(),
                Some("7")
            );
        }
        let pasted_box = destination
            .word_document()
            .paragraph(0)
            .unwrap()
            .runs
            .iter()
            .find_map(|run| run.text_box.as_ref())
            .unwrap();
        let jade_docx_editor::document::BodyElement::Paragraph(pasted_paragraph) =
            &pasted_box.blocks[0]
        else {
            panic!("pasted text-box paragraph missing");
        };
        let effective = destination
            .word_document()
            .styles_part
            .as_ref()
            .unwrap()
            .resolve_effective_run_properties(
                pasted_paragraph
                    .properties
                    .as_ref()
                    .and_then(|props| props.style.as_deref()),
                pasted_paragraph.runs[0].properties.as_ref(),
            )
            .unwrap();
        assert_eq!(effective.bold, Some(true));
        assert_eq!(effective.italic, Some(true));
        let after = destination.export_docx().unwrap();
        let saved = jade_opc::OpcPackage::open(&after).unwrap();
        let styles_path = if destination_kind == 2 {
            "word/jade-paste-styles-1.xml"
        } else {
            "word/styles.xml"
        };
        let styles = saved.part_str(styles_path).unwrap().unwrap();
        if destination_kind == 2 {
            assert!(saved
                .part_str("word/styles.xml")
                .unwrap()
                .unwrap()
                .contains("Orphan style"));
            assert!(!styles.contains("Orphan style"));
            assert!(saved
                .part_str("word/_rels/document.xml.rels")
                .unwrap()
                .unwrap()
                .contains("/word/jade-paste-styles-1.xml"));
        }
        let xml = saved.part_str("word/document.xml").unwrap().unwrap();
        assert!(
            !styles.contains("Unused"),
            "unreferenced styles must stay behind"
        );
        if without_styles {
            assert_eq!(
                styles.matches("w:default=").count(),
                3,
                "only the destination's three implicit defaults may become explicit"
            );
        } else {
            assert!(styles.contains("Destination base"));
            assert!(styles.contains(r#"w:b w:val="0""#));
            assert!(styles.contains(r#"w:basedOn w:val="Base_jade_copy_1""#));
            assert!(styles.contains(r#"w:next w:val="BoxText_jade_copy_1""#));
            assert!(styles.contains(r#"w:link w:val="BoxEmphasis_jade_copy_1""#));
            assert!(xml.contains(r#"w:pStyle w:val="BoxText_jade_copy_1""#));
            assert!(xml.contains(r#"w:rStyle w:val="BoxEmphasis_jade_copy_1""#));
            assert!(
                xml.contains(r#"w:pStyle w:val="BoxText""#),
                "host style was changed"
            );
        }
        assert!(xml.contains(r#"w:pStyle w:val="BoxText_jade_copy_1""#));
        assert!(xml.contains(r#"w:pStyle w:val="BoxText""#));
        let reopened = common::open_bytes(&after);
        let host_after = host_format(&reopened);
        assert_eq!(
            host_after, host_before,
            "pasting must preserve host formatting"
        );
        let paragraph = (0..reopened.word_document().paragraph_count())
            .find_map(|p| {
                let paragraph = reopened.word_document().paragraph(p)?;
                (reopened.paragraph_text(p).as_deref() == Some("Styled box")).then_some(paragraph)
            })
            .unwrap();
        let effective = reopened
            .word_document()
            .styles_part
            .as_ref()
            .unwrap()
            .resolve_effective_run_properties(
                paragraph
                    .properties
                    .as_ref()
                    .and_then(|props| props.style.as_deref()),
                paragraph.runs[0].properties.as_ref(),
            )
            .unwrap();
        assert_eq!(effective.bold, Some(true));
        assert_eq!(effective.italic, Some(true));
        assert!(destination.undo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), before);
        assert!(destination.redo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), after);
    }
}

#[test]
fn copied_text_box_preserves_numbered_lists_overrides_and_picture_bullets() {
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
    const PKG: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
    let blank = jade_web::word::new::blank_document(jade_web::word::new::Blank::English);
    let mut source = jade_opc::OpcPackage::open(&blank).unwrap();
    let attach_numbering = |package: &mut jade_opc::OpcPackage| {
        let rels = package
            .part_str("word/_rels/document.xml.rels")
            .unwrap()
            .unwrap();
        if !rels.contains(&format!("{R}/numbering")) {
            package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", &format!(r#"<Relationship Id="rIdNumbering" Type="{R}/numbering" Target="numbering.xml"/></Relationships>"#)).into_bytes());
        }
        let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
        package.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#).into_bytes());
    };
    attach_numbering(&mut source);
    source.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:pict><v:shape id="ListBox" style="width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:pPr><w:pStyle w:val="BoxList"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>Numbered item</w:t></w:r></w:p><w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="7"/></w:numPr></w:pPr><w:r><w:t>Picture item</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p></w:body></w:document>"#).into_bytes());
    source.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="BoxList"><w:name w:val="Box list"/><w:pPr><w:numPr><w:numId w:val="7"/></w:numPr></w:pPr></w:style><w:style w:type="numbering" w:styleId="ListTemplate"><w:name w:val="List template"/><w:pPr><w:numPr><w:numId w:val="8"/></w:numPr></w:pPr></w:style><w:style w:type="paragraph" w:styleId="LevelStyle"><w:name w:val="Level style"/><w:rPr><w:b/></w:rPr></w:style></w:styles>"#).into_bytes());
    source.replace("word/numbering.xml", format!(r#"<w:numbering xmlns:w="{W}" xmlns:r="{R}" xmlns:v="urn:schemas-microsoft-com:vml"><w:numPicBullet w:numPicBulletId="3"><w:pict><v:shape style="width:9pt;height:9pt"><v:imagedata r:id="rIdBullet"/></v:shape></w:pict></w:numPicBullet><w:abstractNum w:abstractNumId="4"><w:styleLink w:val="ListTemplate"/><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:pStyle w:val="LevelStyle"/><w:lvlText w:val="%1."/></w:lvl><w:lvl w:ilvl="1"><w:start w:val="1"/><w:numFmt w:val="bullet"/><w:lvlText w:val="•"/><w:lvlPicBulletId w:val="3"/></w:lvl></w:abstractNum><w:abstractNum w:abstractNumId="5"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="upperRoman"/><w:lvlText w:val="%1)"/></w:lvl></w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="4"/><w:lvlOverride w:ilvl="0"><w:startOverride w:val="9"/></w:lvlOverride></w:num><w:num w:numId="8"><w:abstractNumId w:val="5"/></w:num><w:num w:numId="999"><w:abstractNumId w:val="5"/></w:num></w:numbering>"#).into_bytes());
    source.replace("word/_rels/numbering.xml.rels", format!(r#"<Relationships xmlns="{PKG}"><Relationship Id="rIdBullet" Type="{R}/image" Target="media/list-bullet.png"/></Relationships>"#).into_bytes());
    let image = png(3, 3);
    source.replace("word/media/list-bullet.png", image.clone());
    let types = source.part_str("[Content_Types].xml").unwrap().unwrap();
    source.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/media/list-bullet.png" ContentType="image/png"/></Types>"#).into_bytes());
    let source = common::open_bytes(&source.write().unwrap());
    let clip = source
        .copy_object(FloatingObjectTarget {
            paragraph_index: 0,
            run_index: 0,
            kind: FloatingObjectKind::TextBox,
        })
        .unwrap();
    for kind in 0..3 {
        let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
        if kind == 0 {
            attach_numbering(&mut package);
        } else {
            let rels = package
                .part_str("word/_rels/document.xml.rels")
                .unwrap()
                .unwrap();
            let rels = rels
                .split("<Relationship ")
                .enumerate()
                .filter_map(|(i, part)| {
                    if i == 0 {
                        return Some(part.to_owned());
                    }
                    let end = part.find("/>").unwrap() + 2;
                    Some(if part[..end].contains(&format!("{R}/numbering")) {
                        part[end..].into()
                    } else {
                        format!("<Relationship {part}")
                    })
                })
                .collect::<String>();
            package.replace("word/_rels/document.xml.rels", rels.into_bytes());
        }
        let host_numbering = format!(
            r#"<w:numbering xmlns:w="{W}" xmlns:r="{R}" xmlns:v="urn:schemas-microsoft-com:vml"><w:numPicBullet w:numPicBulletId="1"><w:pict><v:shape style="width:6pt;height:6pt"><v:imagedata r:id="rId1"/></v:shape></w:pict></w:numPicBullet><w:abstractNum w:abstractNumId="4"><w:lvl w:ilvl="0"><w:start w:val="3"/><w:numFmt w:val="lowerLetter"/><w:lvlText w:val="%1)"/><w:lvlPicBulletId w:val="1"/></w:lvl></w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="4"/></w:num></w:numbering>"#
        );
        if kind == 1 {
            package.remove("word/numbering.xml");
        } else {
            package.replace("word/numbering.xml", host_numbering.clone().into_bytes());
        }
        package.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}"><w:body><w:p><w:pPr><w:numPr><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>Destination</w:t></w:r></w:p></w:body></w:document>"#).into_bytes());
        let host_image = png(2, 2);
        if kind == 0 {
            package.replace("word/_rels/numbering.xml.rels", format!(r#"<Relationships xmlns="{PKG}"><Relationship Id="rId1" Type="{R}/image" Target="media/host-bullet.png"/></Relationships>"#).into_bytes());
            package.replace("word/media/host-bullet.png", host_image.clone());
            let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
            package.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/media/host-bullet.png" ContentType="image/png"/></Types>"#).into_bytes());
        }
        // Force style collisions, including the style referenced only by a list level.
        package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="BoxList"><w:name w:val="Host list"/></w:style><w:style w:type="paragraph" w:styleId="LevelStyle"><w:name w:val="Host level"/></w:style><w:style w:type="numbering" w:styleId="ListTemplate"><w:name w:val="Host template"/></w:style></w:styles>"#).into_bytes());
        let mut destination = common::open_bytes(&package.write().unwrap());
        destination.place_caret(0, 0);
        let before = destination.export_docx().unwrap();
        destination.paste_object(&clip).unwrap();
        let check_numbering = |document: &EditableDocument| {
            let word = document.word_document();
            let text_box = word
                .paragraph(0)
                .unwrap()
                .runs
                .iter()
                .find_map(|r| r.text_box.as_ref())
                .unwrap();
            let jade_docx_editor::document::BodyElement::Paragraph(p) = &text_box.blocks[0] else {
                panic!("missing list item");
            };
            let id = p
                .properties
                .as_ref()
                .unwrap()
                .numbering
                .as_ref()
                .unwrap()
                .id
                .as_deref()
                .unwrap();
            assert_ne!(id, "1", "an unresolved host list ID must stay unresolved");
            assert_ne!(id, "7", "source list must have a separate instance");
            let styles = word.styles_part.as_ref().unwrap();
            let style_id = p.properties.as_ref().unwrap().style.as_deref().unwrap();
            assert_eq!(
                styles.resolve_numbering(style_id).unwrap().list_id(),
                Some(id)
            );
            let numbering = word.numbering_part.as_ref().unwrap();
            assert_eq!(numbering.start_value(id, 0), Some(9));
            assert_eq!(
                numbering.get_level_info(id, 0).unwrap().num_fmt.as_deref(),
                Some("decimal")
            );
            let picture = numbering.level_picture_bullet(id, 1).unwrap();
            assert_eq!(picture.width_pt, Some(9.0));
            assert!(picture
                .data_uri
                .as_deref()
                .unwrap()
                .starts_with("data:image/png;base64,"));
            if kind == 0 {
                let host_picture = numbering.level_picture_bullet("7", 0).unwrap();
                assert_eq!(host_picture.width_pt, Some(6.0));
                assert!(host_picture
                    .data_uri
                    .as_deref()
                    .unwrap()
                    .starts_with("data:image/png;base64,"));
                assert_eq!(
                    numbering.get_level_info("7", 0).unwrap().num_fmt.as_deref(),
                    Some("lowerLetter")
                );
            }
        };
        check_numbering(&destination);
        let after = destination.export_docx().unwrap();
        let saved = jade_opc::OpcPackage::open(&after).unwrap();
        let path = if kind == 2 {
            "word/jade-paste-numbering-1.xml"
        } else {
            "word/numbering.xml"
        };
        let xml = saved.part_str(path).unwrap().unwrap();
        assert!(!xml.contains(r#"w:numId="999""#));
        assert!(xml.contains(r#"w:pStyle w:val="LevelStyle_jade_copy_1""#));
        assert!(xml.contains(r#"w:styleLink w:val="ListTemplate_jade_copy_1""#));
        let styles = saved.part_str("word/styles.xml").unwrap().unwrap();
        assert!(
            styles.contains("List template"),
            "numbering's style dependency was not imported"
        );
        assert!(styles.contains("Host list"));
        assert!(styles.contains("BoxList_jade_copy_1"));
        if kind == 2 {
            assert_eq!(
                saved.part_str("word/numbering.xml").unwrap().unwrap(),
                host_numbering
            );
        }
        let rel_path = if kind == 2 {
            "word/_rels/jade-paste-numbering-1.xml.rels"
        } else {
            "word/_rels/numbering.xml.rels"
        };
        let rels = saved.part_str(rel_path).unwrap().unwrap();
        let imported_image = rels
            .split("Target=\"")
            .last()
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .trim_start_matches('/');
        assert_eq!(saved.part(imported_image).unwrap(), image.as_slice());
        if kind == 0 {
            assert_eq!(
                saved.part("word/media/host-bullet.png").unwrap(),
                host_image.as_slice()
            );
            assert!(rels.contains(r#"Id="rId2""#));
        }
        let reopened = common::open_bytes(&after);
        check_numbering(&reopened);
        assert!(destination.undo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), before);
        assert!(destination.redo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), after);
    }
}

fn object_theme(package: &mut jade_opc::OpcPackage, accent: &str, face: &str) {
    let theme = format!(
        r#"<a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="Clipboard"><a:themeElements><a:clrScheme name="Clipboard"><a:dk1><a:srgbClr val="000000"/></a:dk1><a:lt1><a:srgbClr val="FFFFFF"/></a:lt1><a:accent1><a:srgbClr val="{accent}"/></a:accent1><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="Clipboard"><a:majorFont><a:latin typeface="{face} Major"/><a:ea typeface="{face} East"/><a:cs typeface="{face} Complex"/></a:majorFont><a:minorFont><a:latin typeface="{face} Minor"/><a:ea typeface="{face} East"/><a:cs typeface="{face} Complex"/></a:minorFont></a:fontScheme><a:fmtScheme name="Clipboard"/></a:themeElements></a:theme>"#
    );
    package.replace("word/theme/theme1.xml", theme.into_bytes());
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    // Point at a known theme part, irrespective of the blank document's defaults.
    let rels = rels
        .split("<Relationship ")
        .filter_map(|fragment| {
            if fragment.contains("/theme\"") {
                Some(fragment.split_once("/>").unwrap().1.to_owned())
            } else if fragment.starts_with('<') {
                Some(fragment.to_owned())
            } else {
                Some(format!("<Relationship {fragment}"))
            }
        })
        .collect::<String>();
    package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", r#"<Relationship Id="rIdClipTheme" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="theme/theme1.xml"/></Relationships>"#).into_bytes());
    let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    if !types.contains("/word/theme/theme1.xml") {
        package.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/></Types>"#).into_bytes());
    }
}

#[test]
fn copied_text_box_freezes_source_theme_in_text_styles_and_numbering() {
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let blank = jade_web::word::new::blank_document(jade_web::word::new::Blank::English);
    let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
    object_theme(&mut package, "4472C4", "Source");
    package.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:pict><v:shape id="ThemedBox" style="width:200pt;height:100pt"><v:textbox><w:txbxContent><w:p><w:pPr><w:pStyle w:val="Themed"/><w:numPr><w:ilvl w:val="0"/><w:numId w:val="7"/></w:numPr><w:shd w:fill="auto" w:themeFill="accent1" w:themeFillShade="BF"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Fallback" w:asciiTheme="majorHAnsi" w:eastAsiaTheme="minorEastAsia" w:cstheme="majorBidi"/><w:color w:val="000000" w:themeColor="accent1" w:themeShade="BF"/><w:u w:val="single" w:themeColor="followedHyperlink"/></w:rPr><w:t>Themed text</w:t></w:r><w:r><w:rPr><w:rFonts w:ascii="+mn-lt"/><w:color w:val="112233" w:themeColor="unknown"/></w:rPr><w:t>Fallback text</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p></w:body></w:document>"#).into_bytes());
    package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}"><w:style w:type="paragraph" w:styleId="Themed"><w:name w:val="Themed"/><w:rPr><w:rFonts w:hAnsiTheme="minorHAnsi"/><w:color w:val="000000" w:themeColor="text1" w:themeTint="80"/></w:rPr></w:style></w:styles>"#).into_bytes());
    package.replace("word/numbering.xml", format!(r#"<w:numbering xmlns:w="{W}"><w:abstractNum w:abstractNumId="4"><w:lvl w:ilvl="0"><w:start w:val="1"/><w:numFmt w:val="decimal"/><w:lvlText w:val="%1."/><w:rPr><w:rFonts w:asciiTheme="majorHAnsi"/><w:color w:themeColor="accent1"/></w:rPr></w:lvl></w:abstractNum><w:num w:numId="7"><w:abstractNumId w:val="4"/></w:num></w:numbering>"#).into_bytes());
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    package.replace("word/_rels/document.xml.rels", rels.replace("</Relationships>", r#"<Relationship Id="rIdClipNum" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering" Target="numbering.xml"/></Relationships>"#).into_bytes());
    let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    package.replace("[Content_Types].xml", types.replace("</Types>", r#"<Override PartName="/word/numbering.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml"/></Types>"#).into_bytes());
    let mut source = common::open_bytes(&package.write().unwrap());
    let source_before = source.export_docx().unwrap();
    let clip = source
        .copy_object(FloatingObjectTarget {
            paragraph_index: 0,
            run_index: 0,
            kind: FloatingObjectKind::TextBox,
        })
        .unwrap();
    assert_eq!(source.export_docx().unwrap(), source_before);
    let mut host = jade_opc::OpcPackage::open(&blank).unwrap();
    object_theme(&mut host, "FF0000", "Destination");
    let host_theme = host.part("word/theme/theme1.xml").unwrap().to_vec();
    let mut destination = common::open_bytes(&host.write().unwrap());
    destination.place_caret(0, 0);
    let before = destination.export_docx().unwrap();
    destination.paste_object(&clip).unwrap();
    let check = |document: &EditableDocument| {
        let box_ = document
            .word_document()
            .paragraph(0)
            .unwrap()
            .runs
            .iter()
            .find_map(|run| run.text_box.as_ref())
            .unwrap();
        let jade_docx_editor::document::BodyElement::Paragraph(paragraph) = &box_.blocks[0] else {
            panic!("missing paragraph")
        };
        let props = paragraph.runs[0].properties.as_ref().unwrap();
        assert_eq!(props.color.as_deref(), Some("#2F5496"));
        let fonts = props.fonts.as_ref().unwrap();
        assert_eq!(fonts.ascii.as_deref(), Some("Source Major"));
        assert_eq!(fonts.east_asia.as_deref(), Some("Source East"));
        assert_eq!(fonts.complex.as_deref(), Some("Source Complex"));
        assert_eq!(
            paragraph.runs[1]
                .properties
                .as_ref()
                .unwrap()
                .fonts
                .as_ref()
                .unwrap()
                .ascii
                .as_deref(),
            Some("Source Minor")
        );
    };
    check(&destination);
    let after = destination.export_docx().unwrap();
    let saved = jade_opc::OpcPackage::open(&after).unwrap();
    assert_eq!(saved.part("word/theme/theme1.xml").unwrap(), host_theme);
    let xml = saved.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains("w:fill=\"2F5496\""));
    assert!(xml.contains("w:color=\"954F72\""));
    assert!(xml.contains("w:themeColor=\"unknown\""));
    assert!(!xml.contains("majorHAnsi"));
    let styles = saved.part_str("word/styles.xml").unwrap().unwrap();
    assert!(styles.contains("w:hAnsi=\"Source Minor\""));
    assert!(styles.contains("w:val=\"7F7F7F\""));
    let numbering = saved.part_str("word/numbering.xml").unwrap().unwrap();
    assert!(numbering.contains("w:ascii=\"Source Major\""));
    assert!(numbering.contains("w:val=\"4472C4\""));
    check(&common::open_bytes(&after));
    assert!(destination.undo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), before);
    assert!(destination.redo().unwrap());
    assert_eq!(destination.export_docx().unwrap(), after);
}

#[test]
fn copied_drawingml_shape_retains_source_scheme_colors_and_transforms() {
    let mut source = contract();
    let paragraph = typed(&mut source, "Shape ");
    source.place_caret(paragraph, 0);
    source
        .insert_shape(TextBoxShape::Rectangle, None, false)
        .unwrap();
    let mut package = jade_opc::OpcPackage::open(&source.export_docx().unwrap()).unwrap();
    object_theme(&mut package, "4472C4", "Source");
    let xml = package.part_str("word/document.xml").unwrap().unwrap();
    assert!(xml.contains(r#"<a:schemeClr val="accent1"/>"#));
    package.replace(
        "word/document.xml",
        xml.replace(
            r#"<a:schemeClr val="accent1"/>"#,
            r#"<a:schemeClr val="accent1"><a:alpha val="50000"/></a:schemeClr>"#,
        )
        .into_bytes(),
    );
    let source = common::open_bytes(&package.write().unwrap());
    let run_index = source
        .word_document()
        .paragraph(paragraph)
        .unwrap()
        .runs
        .iter()
        .position(|run| run.text_box.is_some())
        .unwrap();
    let clip = source
        .copy_object(FloatingObjectTarget {
            paragraph_index: paragraph,
            run_index,
            kind: FloatingObjectKind::TextBox,
        })
        .unwrap();
    let blank = jade_web::word::new::blank_document(jade_web::word::new::Blank::English);
    let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
    object_theme(&mut package, "FF0000", "Destination");
    let mut destination = common::open_bytes(&package.write().unwrap());
    destination.place_caret(0, 0);
    destination.paste_object(&clip).unwrap();
    let bytes = destination.export_docx().unwrap();
    let saved = jade_opc::OpcPackage::open(&bytes).unwrap();
    let xml = saved.part_str("word/document.xml").unwrap().unwrap();
    assert!(
        xml.contains(r#"<a:srgbClr val="4472C4"><a:alpha val="50000"/></a:srgbClr>"#),
        "{xml}"
    );
    assert!(!xml.contains(r#"<a:schemeClr val="accent1""#));
    let reopened = common::open_bytes(&bytes);
    let shape = reopened
        .word_document()
        .paragraph(0)
        .unwrap()
        .runs
        .iter()
        .find_map(|run| run.text_box.as_ref())
        .unwrap();
    assert_eq!(shape.fill_color.as_deref(), Some("#4472C4"));
}

#[test]
fn copied_text_box_retains_document_defaults_without_changing_host_defaults() {
    use jade_docx_editor::document::{BodyElement, ParagraphProperties};
    use jade_docx_editor::styles::merge_paragraph_properties_override;
    const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
    let blank = jade_web::word::new::blank_document(jade_web::word::new::Blank::English);
    let effective = |document: &EditableDocument| {
        let word = document.word_document();
        let styles = word.styles_part.as_ref().unwrap();
        let box_ = word
            .paragraph(0)
            .unwrap()
            .runs
            .iter()
            .find_map(|run| run.text_box.as_ref())
            .unwrap();
        box_.blocks
            .iter()
            .filter_map(|block| {
                let BodyElement::Paragraph(paragraph) = block else {
                    return None;
                };
                let id = paragraph
                    .properties
                    .as_ref()
                    .and_then(|props| props.style.as_deref());
                let mut properties = styles
                    .doc_default_paragraph_properties()
                    .cloned()
                    .unwrap_or_default();
                if let Some(props) = id
                    .or_else(|| styles.default_paragraph_style_id())
                    .and_then(|id| styles.resolve_paragraph_properties(id))
                {
                    merge_paragraph_properties_override(&mut properties, &props);
                }
                if let Some(props) = paragraph.properties.as_ref() {
                    merge_paragraph_properties_override(&mut properties, props);
                }
                let props = styles
                    .resolve_effective_run_properties(id, paragraph.runs[0].properties.as_ref())
                    .unwrap();
                // Compare rendered defaults: an absent color is black and
                // absent paragraph spacing is zero in the current editor.
                Some((
                    props.font_size,
                    props
                        .fonts
                        .as_ref()
                        .and_then(|fonts| fonts.ascii.as_deref())
                        .map(str::to_owned),
                    Some(
                        props
                            .color
                            .as_deref()
                            .filter(|color| *color != "#auto")
                            .unwrap_or("#000000")
                            .to_owned(),
                    ),
                    properties.alignment.as_deref().map(str::to_owned),
                    Some(
                        properties
                            .spacing
                            .and_then(|spacing| spacing.before)
                            .unwrap_or(0.0),
                    ),
                ))
            })
            .collect::<Vec<_>>()
    };
    for source_kind in 0..3 {
        let mut source_package = jade_opc::OpcPackage::open(&blank).unwrap();
        object_theme(&mut source_package, "4472C4", "Source");
        source_package.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}" xmlns:v="urn:schemas-microsoft-com:vml"><w:body><w:p><w:r><w:pict><v:shape id="DefaultsBox" style="width:200pt;height:180pt"><v:textbox><w:txbxContent><w:p><w:r><w:t>Implicit</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Derived"/></w:pPr><w:r><w:t>Named</w:t></w:r></w:p><w:p><w:pPr><w:pStyle w:val="Missing"/><w:jc w:val="right"/><w:spacing w:before="120"/></w:pPr><w:r><w:rPr><w:sz w:val="52"/><w:color w:val="123456"/></w:rPr><w:t>Direct</w:t></w:r></w:p></w:txbxContent></v:textbox></v:shape></w:pict></w:r></w:p></w:body></w:document>"#).into_bytes());
        if source_kind < 2 {
            let default_style = if source_kind == 0 {
                r#"<w:style w:type="paragraph" w:default="1" w:styleId="SourceNormal"><w:name w:val="Source normal"/><w:pPr><w:jc w:val="center"/></w:pPr><w:rPr><w:sz w:val="40"/></w:rPr></w:style>"#
            } else {
                ""
            };
            source_package.replace("word/styles.xml", format!(r#"<w:styles xmlns:w="{W}" xmlns:x="urn:clipboard-test"><w:docDefaults><w:pPrDefault><w:pPr><w:spacing w:before="240"/><w:jc w:val="left"/><x:future x:val="retain"/></w:pPr></w:pPrDefault><w:rPrDefault><w:rPr><w:rFonts w:asciiTheme="majorHAnsi"/><w:sz w:val="36"/><w:color w:themeColor="accent1"/></w:rPr></w:rPrDefault></w:docDefaults>{default_style}<w:style w:type="paragraph" w:styleId="Root"><w:name w:val="Root"/><w:rPr><w:sz w:val="44"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="Derived"><w:name w:val="Derived"/><w:basedOn w:val="Root"/><w:pPr><w:spacing w:before="480"/></w:pPr><w:rPr><w:color w:val="654321"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="JadeClipboardDefaults1"><w:name w:val="Unused collision"/></w:style></w:styles>"#).into_bytes());
        } else {
            source_package.remove("word/styles.xml");
            let rels = source_package
                .part_str("word/_rels/document.xml.rels")
                .unwrap()
                .unwrap();
            let rels = rels
                .split("<Relationship ")
                .map(|fragment| {
                    if fragment.contains("/styles\"") {
                        fragment.split_once("/>").unwrap().1.to_owned()
                    } else if fragment.starts_with('<') {
                        fragment.to_owned()
                    } else {
                        format!("<Relationship {fragment}")
                    }
                })
                .collect::<String>();
            source_package.replace("word/_rels/document.xml.rels", rels.into_bytes());
        }
        let mut source = common::open_bytes(&source_package.write().unwrap());
        let expected = effective(&source);
        let source_before = source.export_docx().unwrap();
        let clip = source
            .copy_object(FloatingObjectTarget {
                paragraph_index: 0,
                run_index: 0,
                kind: FloatingObjectKind::TextBox,
            })
            .unwrap();
        assert_eq!(source.export_docx().unwrap(), source_before);
        let mut host = jade_opc::OpcPackage::open(&blank).unwrap();
        object_theme(&mut host, "FF0000", "Destination");
        let host_styles = format!(
            r#"<w:styles xmlns:w="{W}"><w:docDefaults><w:rPrDefault><w:rPr><w:rFonts w:ascii="Destination"/><w:sz w:val="16"/><w:color w:val="FF0000"/></w:rPr></w:rPrDefault><w:pPrDefault><w:pPr><w:jc w:val="right"/><w:spacing w:before="1200"/></w:pPr></w:pPrDefault></w:docDefaults><w:style w:type="paragraph" w:default="1" w:styleId="SourceNormal"><w:name w:val="Host normal"/><w:rPr><w:sz w:val="12"/></w:rPr></w:style><w:style w:type="paragraph" w:styleId="JadeClipboardDefaults2"><w:name w:val="Host collision"/></w:style></w:styles>"#
        );
        host.replace("word/styles.xml", host_styles.as_bytes().to_vec());
        host.replace("word/document.xml", format!(r#"<w:document xmlns:w="{W}"><w:body><w:p><w:r><w:t>Host</w:t></w:r></w:p></w:body></w:document>"#).into_bytes());
        let mut destination = common::open_bytes(&host.write().unwrap());
        destination.place_caret(0, 0);
        let host_defaults = destination
            .word_document()
            .styles_part
            .as_ref()
            .unwrap()
            .doc_default_run_properties()
            .cloned();
        let host_paragraph_defaults: Option<ParagraphProperties> = destination
            .word_document()
            .styles_part
            .as_ref()
            .unwrap()
            .doc_default_paragraph_properties()
            .cloned();
        let before = destination.export_docx().unwrap();
        destination.paste_object(&clip).unwrap();
        assert_eq!(
            effective(&destination),
            expected,
            "source kind {source_kind}"
        );
        let styles = destination.word_document().styles_part.as_ref().unwrap();
        assert_eq!(styles.doc_default_run_properties(), host_defaults.as_ref());
        assert_eq!(
            serde_json::to_value(styles.doc_default_paragraph_properties()).unwrap(),
            serde_json::to_value(host_paragraph_defaults.as_ref()).unwrap()
        );
        let host_run = styles.resolve_effective_run_properties(None, None).unwrap();
        assert_eq!(host_run.font_size, Some(6.0));
        let after = destination.export_docx().unwrap();
        let saved = jade_opc::OpcPackage::open(&after).unwrap();
        let xml = saved.part_str("word/styles.xml").unwrap().unwrap();
        assert!(xml.contains("Copied document defaults"));
        if source_kind == 2 {
            assert!(xml.contains("w:color w:val=\"auto\""));
            assert_eq!(
                destination
                    .word_document()
                    .styles_part
                    .as_ref()
                    .unwrap()
                    .resolve_effective_run_properties(
                        destination
                            .word_document()
                            .paragraph(0)
                            .unwrap()
                            .runs
                            .iter()
                            .find_map(|run| run.text_box.as_ref())
                            .unwrap()
                            .blocks
                            .iter()
                            .find_map(|block| match block {
                                BodyElement::Paragraph(p) =>
                                    p.properties.as_ref().and_then(|p| p.style.as_deref()),
                                _ => None,
                            }),
                        None
                    )
                    .unwrap()
                    .color
                    .as_deref(),
                Some("#auto")
            );
        }
        if source_kind < 2 {
            assert!(xml.contains("JadeClipboardDefaults2_jade_copy_1"));
            assert!(xml.contains("xmlns:x=\"urn:clipboard-test\""));
            assert!(xml.contains("x:val=\"retain\""));
            assert!(!xml.contains("Unused collision"));
        }
        assert_eq!(xml.matches("w:default=\"1\"").count(), 1);
        assert_eq!(effective(&common::open_bytes(&after)), expected);
        assert!(destination.undo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), before);
        assert!(destination.redo().unwrap());
        assert_eq!(destination.export_docx().unwrap(), after);
    }
}
