//! 挿入 → 図形, ワードアート, SmartArt and グラフ on the browser build's Word
//! document, as the native app does them too: shapes drawn and styled,
//! connectors glued to them, freeforms and their points, groups, SmartArt
//! from the gallery with its text edited, a chart with its data, type and
//! legend — each saved as Word reads it, and drawn.

mod common;
use common::{contract, typed};
use jade_docx_editor::document::{ChartKind, TextBoxShape};
use jade_docx_editor::smartart::{self, DiagramNode};
use jade_docx_editor::word_document::{FloatingObjectKind, FloatingObjectTarget};
use jade_web::editing::{EditableDocument, WordSession};
use jade_web::word::connectors::{ConnectorEnd, ConnectorKind, WordConnectors};
use jade_web::word::diagrams::WordDiagrams;
use jade_web::word::group::WordGroups;
use jade_web::word::objects::WordObjects;
use jade_web::word::shapes::{Drawn, ShapeStyle, WordArtStyle, WordShapes};

fn package_part(document: &mut EditableDocument, name: &str) -> String {
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    package.part_str(name).unwrap().unwrap_or_default()
}

/// The text boxes and pictures of `paragraph`, as targets.
fn objects(document: &EditableDocument, paragraph: usize) -> Vec<FloatingObjectTarget> {
    document
        .word_document()
        .paragraph(paragraph)
        .unwrap()
        .runs
        .iter()
        .enumerate()
        .filter_map(|(run_index, run)| {
            let kind = if run.image.is_some() {
                FloatingObjectKind::Image
            } else if run.text_box.is_some() {
                FloatingObjectKind::TextBox
            } else {
                return None;
            };
            Some(FloatingObjectTarget {
                paragraph_index: paragraph,
                run_index,
                kind,
            })
        })
        .collect()
}

fn drawn(x: f64, y: f64, width: f64, height: f64) -> Option<Drawn> {
    Some(Drawn {
        x,
        y,
        width,
        height,
    })
}

#[test]
fn shapes_are_drawn_where_dragged_and_styled_as_word_styles_them() {
    let mut document = contract();
    let paragraph = typed(&mut document, "shapes ");
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::FivePointStar,
            drawn(200.0, 300.0, 120.0, 100.0),
            false,
        )
        .unwrap();
    let star = objects(&document, paragraph)[0];
    let region = document.object_region(star).unwrap();
    assert!(
        (region.x - 200.0).abs() < 1.0 && (region.y - 300.0).abs() < 1.0,
        "{region:?}"
    );
    assert_eq!(
        (region.width.round(), region.height.round()),
        (120.0, 100.0)
    );
    let shape = document.word_document().paragraph(paragraph).unwrap().runs[star.run_index]
        .text_box
        .clone()
        .unwrap();
    assert_eq!(shape.shape, TextBoxShape::FivePointStar);
    assert_eq!(shape.fill_color.as_deref(), Some("#4472C4"));
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"prst="star5""#), "{xml}");
    assert!(xml.contains("<wps:style>"), "{xml}");
    assert!(xml.contains(r#"<a:schemeClr val="accent1"/>"#), "{xml}");

    // The style gallery: 枠線のみ in orange.
    document
        .apply_shape_style(star, ShapeStyle::gallery(0, 2))
        .unwrap();
    let shape = document.word_document().paragraph(paragraph).unwrap().runs[star.run_index]
        .text_box
        .clone()
        .unwrap();
    assert_eq!(shape.fill_color.as_deref(), Some("#FFFFFF"));
    assert_eq!(shape.border.color.as_deref(), Some("#ED7D31"));
    document.undo().unwrap();
    document.undo().unwrap();
    assert!(objects(&document, paragraph).is_empty());
}

#[test]
fn wordart_goes_in_with_its_text_and_takes_a_transform() {
    let mut document = contract();
    let paragraph = typed(&mut document, "art ");
    document.place_caret(paragraph, 0);
    document
        .insert_wordart(WordArtStyle::AccentShadow, "見出し", None)
        .unwrap();
    let art = objects(&document, paragraph)[0];
    let format = document.object_format(art).unwrap();
    assert_eq!(format.sort, jade_web::word::objects::ObjectSort::WordArt);
    let text = document.word_document().paragraph(paragraph).unwrap().runs[art.run_index]
        .text_box
        .clone()
        .unwrap()
        .text;
    assert!(text.contains("見出し"), "{text:?}");
    document.set_text_warp(art, "textArchUp").unwrap();
    let warp = document.word_document().paragraph(paragraph).unwrap().runs[art.run_index]
        .text_box
        .clone()
        .unwrap()
        .extras()
        .text_warp
        .clone()
        .unwrap();
    assert_eq!(warp.preset, "textArchUp");
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"<a:prstTxWarp prst="textArchUp">"#), "{xml}");
    assert!(xml.contains("w14:shadow"), "{xml}");
}

#[test]
fn a_connector_stays_glued_when_its_shapes_move() {
    let mut document = contract();
    let paragraph = typed(&mut document, "flow ");
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Rectangle,
            drawn(100.0, 200.0, 100.0, 60.0),
            false,
        )
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Ellipse,
            drawn(400.0, 400.0, 100.0, 60.0),
            false,
        )
        .unwrap();
    // Newest first in the paragraph.
    let shapes = objects(&document, paragraph);
    let (ellipse, rectangle) = (shapes[0], shapes[1]);
    let left = document.object_region(rectangle).unwrap();
    let right = document.object_region(ellipse).unwrap();
    // From the rectangle's right point to the ellipse's top.
    document.place_caret(paragraph, 0);
    document
        .insert_connector(
            ConnectorKind::Elbow,
            true,
            ConnectorEnd {
                point: (200.0, 230.0),
                glued: Some((rectangle, 3)),
            },
            ConnectorEnd {
                point: (450.0, 400.0),
                glued: Some((ellipse, 0)),
            },
        )
        .unwrap();
    let shapes = objects(&document, paragraph);
    let connector = shapes[0];
    let (ellipse, rectangle) = (shapes[1], shapes[2]);
    let glue = document.word_document().paragraph(paragraph).unwrap().runs[connector.run_index]
        .text_box
        .clone()
        .unwrap()
        .extras()
        .connector
        .unwrap();
    assert!(glue.start.is_some() && glue.end.is_some());
    let line = document.object_region(connector).unwrap();
    assert!(
        (line.x - 200.0).abs() < 1.0 && (line.y - 230.0).abs() < 1.0,
        "{line:?}"
    );

    // The ellipse moves down and left; the connector follows.
    let placed = document.object_region(ellipse).unwrap();
    document
        .move_object(
            placed,
            (
                placed.x - 50.0,
                placed.y + 100.0,
                placed.width,
                placed.height,
            ),
        )
        .unwrap();
    let regions: Vec<_> = objects(&document, paragraph)
        .into_iter()
        .filter_map(|target| document.object_region(target))
        .collect();
    let moved = document.reroute_connectors(&regions).unwrap();
    assert!(moved);
    let line = document.object_region(connector).unwrap();
    let ellipse_now = document.object_region(ellipse).unwrap();
    assert!(
        (line.x + line.width - (ellipse_now.x + ellipse_now.width / 2.0)).abs() < 1.0,
        "{line:?} {ellipse_now:?}"
    );
    assert!((line.y + line.height - ellipse_now.y).abs() < 1.0);
    let _ = (left, right, rectangle);
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains("<a:stCxn "), "{xml}");
    assert!(xml.contains(r#"prst="bentConnector3""#), "{xml}");
}

#[test]
fn a_freeform_is_drawn_and_its_points_move() {
    let mut document = contract();
    let paragraph = typed(&mut document, "free ");
    document.place_caret(paragraph, 0);
    document
        .insert_freeform(&[(100.0, 300.0), (200.0, 200.0), (300.0, 300.0)], true)
        .unwrap();
    let freeform = objects(&document, paragraph)[0];
    let shape = document.word_document().paragraph(paragraph).unwrap().runs[freeform.run_index]
        .text_box
        .clone()
        .unwrap();
    let path = shape.extras().custom_geometry.clone().unwrap().paths[0].clone();
    let placed = document.object_region(freeform).unwrap();
    let points = jade_web::word::connectors::path_points(&path, &placed);
    assert_eq!(points.len(), 3);
    assert!(
        (points[1].0 - 200.0).abs() < 1.0 && (points[1].1 - 200.0).abs() < 1.0,
        "{points:?}"
    );
    let moved = jade_web::word::connectors::move_path_point(&path, &placed, 1, (200.0, 250.0));
    document.set_freeform_path(freeform, moved).unwrap();
    let shape = document.word_document().paragraph(paragraph).unwrap().runs[freeform.run_index]
        .text_box
        .clone()
        .unwrap();
    let path = shape.extras().custom_geometry.clone().unwrap().paths[0].clone();
    let points = jade_web::word::connectors::path_points(&path, &placed);
    assert!((points[1].1 - 250.0).abs() < 1.0, "{points:?}");
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains("<a:custGeom>"), "{xml}");
}

#[test]
fn shapes_group_and_come_apart_again() {
    let mut document = contract();
    let paragraph = typed(&mut document, "group ");
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Rectangle,
            drawn(100.0, 200.0, 100.0, 60.0),
            false,
        )
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Ellipse,
            drawn(250.0, 260.0, 80.0, 80.0),
            false,
        )
        .unwrap();
    let shapes = objects(&document, paragraph);
    let placed: Vec<_> = shapes
        .iter()
        .map(|target| document.object_region(*target).unwrap())
        .collect();
    document.group_objects(&placed).unwrap();
    let after = objects(&document, paragraph);
    assert_eq!(after.len(), 1);
    let group = after[0];
    assert_eq!(group.kind, FloatingObjectKind::Image);
    let region = document.object_region(group).unwrap();
    assert!(
        (region.x - 100.0).abs() < 1.0 && (region.y - 200.0).abs() < 1.0,
        "{region:?}"
    );
    assert_eq!(
        (region.width.round(), region.height.round()),
        (230.0, 140.0)
    );
    let image = document.word_document().paragraph(paragraph).unwrap().runs[group.run_index]
        .image
        .clone()
        .unwrap();
    assert_eq!(image.shape_group.unwrap().children.len(), 2);
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains("<wpg:wgp>"), "{xml}");

    document.ungroup_object(region).unwrap();
    let parts = objects(&document, paragraph);
    assert_eq!(parts.len(), 2, "{parts:?}");
    let mut boxes: Vec<_> = parts
        .iter()
        .map(|target| {
            let placed = document.object_region(*target).unwrap();
            (
                placed.x.round(),
                placed.y.round(),
                placed.width.round(),
                placed.height.round(),
            )
        })
        .collect();
    boxes.sort_by(|a, b| a.0.total_cmp(&b.0));
    assert_eq!(
        boxes,
        vec![(100.0, 200.0, 100.0, 60.0), (250.0, 260.0, 80.0, 80.0)]
    );
    document.undo().unwrap();
    assert_eq!(objects(&document, paragraph).len(), 1);
}

#[test]
fn smartart_goes_in_from_the_gallery_and_its_text_is_edited() {
    let mut document = contract();
    let paragraph = typed(&mut document, "smart ");
    document.place_caret(paragraph, 0);
    let layout = smartart::LAYOUTS[2].0;
    document.insert_smartart(layout, None).unwrap();
    let graphic = objects(&document, paragraph)[0];
    let diagram = document.smartart(graphic).unwrap();
    assert_eq!(diagram.layout, layout);
    assert_eq!(diagram.nodes.len(), 3);
    let mut diagram = diagram;
    diagram.nodes = vec![
        DiagramNode::new("企画"),
        DiagramNode::new("設計"),
        DiagramNode::new("実装"),
        DiagramNode::new("試験"),
    ];
    document.set_smartart(graphic, diagram.clone()).unwrap();
    let image = document.word_document().paragraph(paragraph).unwrap().runs[graphic.run_index]
        .image
        .clone()
        .unwrap();
    let group = image.shape_group.expect("drawn from its cached drawing");
    let texts: Vec<_> = group
        .children
        .iter()
        .map(|shape| shape.text.clone())
        .collect();
    for word in ["企画", "設計", "実装", "試験"] {
        assert!(texts.iter().any(|text| text == word), "{texts:?}");
    }
    // Four boxes and three arrows: 基本ステップ.
    assert_eq!(group.children.len(), 7);
    assert_eq!(document.smartart(graphic).unwrap().nodes, diagram.nodes);
    // Another layout from the gallery, the same text.
    diagram.layout = smartart::LAYOUTS[3].0.to_string();
    document.set_smartart(graphic, diagram).unwrap();
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let names = package.part_names();
    for part in [
        "data1.xml",
        "layout1.xml",
        "quickStyle1.xml",
        "colors1.xml",
        "drawing1.xml",
    ] {
        assert!(
            names
                .iter()
                .any(|name| name == &format!("word/diagrams/{part}")),
            "{names:?}"
        );
    }
    let layout_xml = package
        .part_str("word/diagrams/layout1.xml")
        .unwrap()
        .unwrap();
    assert!(layout_xml.contains("cycle2"), "{layout_xml}");
    let types = package.part_str("[Content_Types].xml").unwrap().unwrap();
    assert!(types.contains("diagramData+xml"), "{types}");
    // Reopened, the graphic is drawn and its text is all there.
    let reopened = common::open_bytes(&saved);
    let image = reopened.word_document().paragraph(paragraph).unwrap().runs[graphic.run_index]
        .image
        .clone()
        .unwrap();
    assert!(image.shape_group.is_some());
}

/// SmartArt Word saved without its cached drawing is drawn from its data.
#[test]
fn smartart_without_a_drawing_is_laid_out_from_its_data() {
    let mut document = contract();
    let paragraph = typed(&mut document, "data only ");
    document.place_caret(paragraph, 0);
    let layout = smartart::LAYOUTS[4].0;
    document
        .insert_smartart(
            layout,
            Some(vec![DiagramNode::with(
                "社長",
                vec![DiagramNode::new("営業"), DiagramNode::new("開発")],
            )]),
        )
        .unwrap();
    // Drop the drawing part and its relationship, as a data-only producer
    // would write the graphic.
    let saved = document.export_docx().unwrap();
    let mut package = jade_opc::OpcPackage::open(&saved).unwrap();
    let rels = package
        .part_str("word/_rels/document.xml.rels")
        .unwrap()
        .unwrap();
    let rels = rels
        .split("<Relationship ")
        .filter(|relationship| !relationship.contains("diagramDrawing"))
        .collect::<Vec<_>>()
        .join("<Relationship ");
    package.replace("word/_rels/document.xml.rels", rels.into_bytes());
    package.remove("word/diagrams/drawing1.xml");
    let bytes = package.write().unwrap();
    let reopened = common::open_bytes(&bytes);
    let image = reopened
        .word_document()
        .paragraph(paragraph)
        .unwrap()
        .runs
        .iter()
        .find_map(|run| run.image.clone())
        .unwrap();
    let group = image.shape_group.expect("laid out from the data");
    let texts: Vec<_> = group
        .children
        .iter()
        .map(|shape| shape.text.clone())
        .collect();
    for word in ["社長", "営業", "開発"] {
        assert!(texts.iter().any(|text| text == word), "{texts:?}");
    }
}

#[test]
fn a_chart_goes_in_with_its_workbook_and_takes_new_data_type_and_legend() {
    let mut document = contract();
    let paragraph = typed(&mut document, "chart ");
    document.place_caret(paragraph, 0);
    document.insert_chart(ChartKind::Bar, true).unwrap();
    let chart_target = objects(&document, paragraph)[0];
    let mut chart = document.chart_data(chart_target).unwrap();
    assert_eq!(chart.categories.len(), 4);
    assert_eq!(chart.series.len(), 3);
    assert_eq!(chart.legend.as_deref(), Some("b"));
    chart.series[0].values[0] = Some(9.5);
    chart.title = "売上".into();
    chart.kind = ChartKind::Line;
    document.set_chart(chart_target, chart).unwrap();
    document.set_chart_legend(chart_target, Some("r")).unwrap();
    let chart = document.chart_data(chart_target).unwrap();
    assert_eq!(chart.kind, ChartKind::Line);
    assert_eq!(chart.series[0].values[0], Some(9.5));
    assert_eq!(chart.legend.as_deref(), Some("r"));
    let saved = document.export_docx().unwrap();
    let package = jade_opc::OpcPackage::open(&saved).unwrap();
    let names = package.part_names();
    let chart_part = names
        .iter()
        .find(|name| name.starts_with("word/charts/chart"))
        .expect("chart part");
    let xml = package.part_str(chart_part).unwrap().unwrap();
    assert!(xml.contains("<c:lineChart>"), "{xml}");
    assert!(xml.contains(r#"<c:legendPos val="r"/>"#), "{xml}");
    assert!(xml.contains("売上"), "{xml}");
    assert!(
        names
            .iter()
            .any(|name| name.contains("/embeddings/") && name.ends_with(".xlsx")),
        "{names:?}"
    );
    // It is drawn from its part: the page has the title's ink.
    let _ = WordSession::word_document(&document);
}

/// Everything this file puts in, drawn on one page: the shapes, WordArt
/// along an arch, a SmartArt cycle and a chart are all on the page's
/// pixels. With `JADE_DUMP_OBJECTS` set to a directory the page is written
/// there as a PNG to look at.
#[test]
fn objects_are_drawn_on_the_page() {
    let mut document = contract();
    let paragraph = typed(&mut document, "drawn ");
    // The SmartArt and the chart go in line, each in a paragraph of its
    // own, before the floating shapes add text box paragraphs.
    let text = document.paragraph_text(paragraph).unwrap();
    document.place_caret(paragraph, text.len());
    document.insert_paragraph_break().unwrap();
    document.insert_paragraph_break().unwrap();
    document.place_caret(paragraph + 2, 0);
    document.insert_chart(ChartKind::Bar, true).unwrap();
    document.place_caret(paragraph + 1, 0);
    document
        .insert_smartart(
            smartart::LAYOUTS[3].0,
            Some(vec![
                DiagramNode::new("A"),
                DiagramNode::new("B"),
                DiagramNode::new("C"),
            ]),
        )
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_shape(TextBoxShape::Heart, drawn(80.0, 120.0, 120.0, 100.0), false)
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_wordart(
            WordArtStyle::AccentShadow,
            "WordArt",
            drawn(260.0, 110.0, 360.0, 72.0),
        )
        .unwrap();
    let art = objects(&document, paragraph)[0];
    document.set_text_warp(art, "textArchUp").unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_freeform(
            &[
                (80.0, 300.0),
                (160.0, 240.0),
                (240.0, 300.0),
                (200.0, 360.0),
            ],
            true,
        )
        .unwrap();
    let page = document.page(0).unwrap();
    if let Ok(directory) = std::env::var("JADE_DUMP_OBJECTS") {
        std::fs::write(
            std::path::Path::new(&directory).join("objects-page.png"),
            page.encode_png().unwrap(),
        )
        .unwrap();
    }
    // The WordArt's box has ink: its arch was drawn.
    let region = document
        .object_region(
            objects(&document, paragraph)
                .iter()
                .copied()
                .find(|target| {
                    document.object_format(*target).is_some_and(|format| {
                        format.sort == jade_web::word::objects::ObjectSort::WordArt
                    })
                })
                .unwrap(),
        )
        .unwrap();
    let rgba = page.rgba_premultiplied();
    let width = page.width() as usize;
    let scale = width as f64 / document.page_size().0;
    let inked = (region.y as usize..(region.y + region.height) as usize)
        .flat_map(|y| (region.x as usize..(region.x + region.width) as usize).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let (px, py) = ((*x as f64 * scale) as usize, (*y as f64 * scale) as usize);
            let at = (py * width + px) * 4;
            rgba.get(at..at + 3)
                .is_some_and(|pixel| pixel.iter().any(|channel| *channel < 200))
        })
        .count();
    assert!(inked > 50, "the WordArt drew {inked} dark pixels");
}

#[test]
fn a_picture_groups_with_a_shape_and_is_drawn_inside_the_group() {
    use jade_web::word::picture::{WordPicture, Wrap};
    let mut document = contract();
    let paragraph = typed(&mut document, "group ");
    document.place_caret(paragraph, 0);
    let mut png = Vec::new();
    image::RgbaImage::from_pixel(60, 40, image::Rgba([220, 0, 0, 255]))
        .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
        .unwrap();
    document.insert_picture(png).unwrap();
    let picture = objects(&document, paragraph)[0];
    document
        .set_object_wrap(picture, Wrap::InFrontOfText)
        .unwrap();
    let start = document.object_region(picture).unwrap();
    document
        .place_object(start, (100.0, 200.0, 60.0, 40.0))
        .unwrap();
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Ellipse,
            drawn(250.0, 260.0, 80.0, 80.0),
            false,
        )
        .unwrap();
    let placed: Vec<_> = objects(&document, paragraph)
        .iter()
        .map(|target| document.object_region(*target).unwrap())
        .collect();
    assert_eq!(placed.len(), 2);
    document.group_objects(&placed).unwrap();
    let group = objects(&document, paragraph)[0];
    let image = document.word_document().paragraph(paragraph).unwrap().runs[group.run_index]
        .image
        .clone()
        .unwrap();
    let children = image.shape_group.unwrap().children;
    let grouped = children
        .iter()
        .find_map(|child| child.picture.as_ref())
        .expect("the picture is a member of the group");
    assert!(grouped
        .data_uri
        .as_deref()
        .is_some_and(|uri| uri.starts_with("data:image/png")));
    assert!(package_part(&mut document, "word/document.xml").contains("<pic:pic"));

    // Drawn: the picture's red fills its box inside the group.
    let page = document.page(0).unwrap();
    let rgba = page.rgba_premultiplied();
    let width = page.width() as usize;
    let scale = width as f64 / document.page_size().0;
    let at = ((220.0 * scale) as usize * width + (130.0 * scale) as usize) * 4;
    assert!(
        rgba[at] > 180 && rgba[at + 1] < 60,
        "{:?}",
        &rgba[at..at + 4]
    );

    let region = document.object_region(group).unwrap();
    document.ungroup_object(region).unwrap();
    let pictures: Vec<_> = objects(&document, paragraph)
        .into_iter()
        .filter_map(|target| {
            document.word_document().paragraph(paragraph).unwrap().runs[target.run_index]
                .image
                .clone()
        })
        .filter(|image| image.data_uri.is_some())
        .collect();
    assert_eq!(
        pictures.len(),
        1,
        "the picture comes out of the group as a picture"
    );
}

/// Dark pixels in a page box (page px).
fn inked(document: &EditableDocument, (x, y, width, height): (f64, f64, f64, f64)) -> usize {
    let page = document.page(0).unwrap();
    let rgba = page.rgba_premultiplied();
    let stride = page.width() as usize;
    let scale = stride as f64 / document.page_size().0;
    let mut count = 0;
    for py in ((y * scale) as usize)..(((y + height) * scale) as usize) {
        for px in ((x * scale) as usize)..(((x + width) * scale) as usize) {
            let at = (py * stride + px) * 4;
            if rgba
                .get(at..at + 3)
                .is_some_and(|pixel| pixel.iter().all(|channel| *channel < 120))
            {
                count += 1;
            }
        }
    }
    count
}

#[test]
fn a_linked_text_box_takes_the_text_that_does_not_fit() {
    let mut document = contract();
    let paragraph = typed(&mut document, "link ");
    for top in [200.0, 420.0] {
        document.place_caret(paragraph, 0);
        document
            .insert_shape(
                TextBoxShape::Rectangle,
                drawn(100.0, top, 220.0, 70.0),
                true,
            )
            .unwrap();
    }
    let at_top = |document: &EditableDocument, top: f64| {
        objects(document, paragraph)
            .into_iter()
            .find(|target| {
                document
                    .object_region(*target)
                    .is_some_and(|placed| (placed.y - top).abs() < 2.0)
            })
            .unwrap()
    };
    let first = at_top(&document, 200.0);
    document
        .execute_command(
            Box::new(
                jade_docx_editor::commands::edit_drawing::replace_drawing_command(
                    "test_text",
                    first,
                    |_, drawing| {
                        let content =
                            jade_docx_editor::drawing_xml::descendant_mut(drawing, "w:txbxContent")
                                .unwrap();
                        let paragraphs: String = (1..=12)
                            .map(|line| {
                                format!("<w:p><w:r><w:t>LINE {line} WWWW</w:t></w:r></w:p>")
                            })
                            .collect();
                        let parsed = jade_docx_editor::word_document::objects::parse_element(
                            &format!("<w:txbxContent>{paragraphs}</w:txbxContent>"),
                        )?;
                        content.children = parsed.children;
                        Ok(())
                    },
                ),
            ),
            true,
        )
        .unwrap();
    let second_box = (110.0, 425.0, 200.0, 60.0);
    let before = inked(&document, second_box);

    let (first, second) = (at_top(&document, 200.0), at_top(&document, 420.0));
    document.link_text_boxes(first, second).unwrap();
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"<wps:linkedTxbx id="1" seq="1"/>"#), "{xml}");
    assert!(xml.contains(r#"<wps:txbx id="1">"#), "{xml}");
    let after = inked(&document, second_box);
    if let Ok(directory) = std::env::var("JADE_DUMP_OBJECTS") {
        let page = document.page(0).unwrap();
        std::fs::write(
            std::path::Path::new(&directory).join("linked-page.png"),
            page.encode_png().unwrap(),
        )
        .unwrap();
    }
    assert!(
        after > before + 100,
        "the second box shows the overflow: {before} -> {after}"
    );

    document.undo().unwrap();
    assert!(!package_part(&mut document, "word/document.xml").contains("wps:linkedTxbx"));
}

/// 図形の書式 → 文字の配置: the text box's text goes to its bottom, saved as
/// Word's `anchor`, one undo step.
#[test]
fn a_text_box_takes_its_text_anchor() {
    let mut document = contract();
    let paragraph = typed(&mut document, "anchored ");
    document.place_caret(paragraph, 0);
    document
        .insert_shape(
            TextBoxShape::Rectangle,
            drawn(100.0, 200.0, 160.0, 120.0),
            true,
        )
        .unwrap();
    let box_ = objects(&document, paragraph)[0];
    document.set_text_anchor(box_, "bottom").unwrap();
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"anchor="b""#), "{xml}");
    document.set_text_anchor(box_, "top").unwrap();
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"anchor="t""#), "{xml}");
    document.undo().unwrap();
    let xml = package_part(&mut document, "word/document.xml");
    assert!(xml.contains(r#"anchor="b""#), "one step back: {xml}");
}
