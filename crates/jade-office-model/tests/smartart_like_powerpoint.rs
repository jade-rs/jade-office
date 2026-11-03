//! SmartArt as PowerPoint for Mac makes and edits it, through the model
//! both shells share: 挿入 › SmartArt, the テキスト ウィンドウ, 図形の追加,
//! レベル上げ / 下げ, レイアウト, 色の変更, スタイル, one shape's format,
//! グラフィックのリセット, 図形に変換 and SmartArt グラフィックに変換 — each
//! one Undo, and the diagram drawn on the slide.

use std::path::PathBuf;
use std::sync::Arc;

use jade_office_model::aliased_faces;
use jade_office_model::deck::{
    Deck, Insert, ShapeRect, SmartArtEdit, SmartArtFormat, SmartArtPlace, SMARTART_LAYOUTS,
};
use jade_office_model::tiny::TinySkiaBackend;

fn core() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"))
}

fn deck() -> Deck<TinySkiaBackend> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let backend = TinySkiaBackend::new(&aliased_faces(
        Arc::new(std::fs::read(directory.join("NotoSansJP-Regular.ttf")).unwrap()),
        None,
    ))
    .unwrap();
    let bytes = std::fs::read(core().join("public/fixtures/sample-deck.pptx")).unwrap();
    Deck::open(&bytes, backend).unwrap()
}

#[test]
fn every_gallery_layout_inserts_and_draws() {
    let mut deck = deck();
    for layout in SMARTART_LAYOUTS.iter() {
        let id = deck.insert_smartart(0, &layout.id()).unwrap();
        let info = deck.smartart(0, id).expect("a diagram");
        assert!(info.layout.ends_with(layout.short), "{}", layout.short);
        assert!(!info.shapes.is_empty(), "{} draws shapes", layout.short);
        assert!(deck.render_slide(0, 320).is_some());
        assert!(deck.undo().unwrap());
    }
}

#[test]
fn the_text_pane_and_the_shape_commands_edit_the_bullets() {
    let mut deck = deck();
    let id = deck.insert_smartart(0, "process1").unwrap();
    deck.set_smartart_text_pane(
        0,
        id,
        &[
            (0, "企画".to_string()),
            (1, "範囲".to_string()),
            (0, "開発".to_string()),
            (0, "出荷".to_string()),
        ],
    )
    .unwrap();
    assert_eq!(
        deck.smartart_text_pane(0, id).unwrap(),
        vec![
            (0, "企画".to_string()),
            (1, "範囲".to_string()),
            (0, "開発".to_string()),
            (0, "出荷".to_string()),
        ]
    );
    let nodes = deck.smartart(0, id).unwrap().nodes;
    let develop = nodes[1].id.clone();
    let new = deck
        .smartart_add_shape(0, id, Some(&develop), SmartArtPlace::After)
        .unwrap()
        .expect("a new bullet");
    deck.smartart_demote(0, id, &new).unwrap();
    let pane = deck.smartart_text_pane(0, id).unwrap();
    assert_eq!(pane[3], (1, String::new()), "{pane:?}");
    deck.smartart_promote(0, id, &new).unwrap();
    deck.smartart_move(0, id, &new, true).unwrap();
    let pane = deck.smartart_text_pane(0, id).unwrap();
    assert_eq!(pane[2], (0, String::new()), "{pane:?}");
    deck.smartart_delete_shape(0, id, &new).unwrap();
    assert_eq!(deck.smartart_text_pane(0, id).unwrap().len(), 4);
    // A click on a bullet's shape finds it.
    let rect = deck.smartart_node_rect(0, id, &develop).expect("its shape");
    assert_eq!(
        deck.smartart_node_at(0, id, rect.x + rect.width / 2.0, rect.y + rect.height / 2.0),
        Some(develop)
    );
}

#[test]
fn layout_colours_style_and_one_shapes_format() {
    let mut deck = deck();
    let id = deck.insert_smartart(0, "default").unwrap();
    deck.edit_smartart(0, id, &SmartArtEdit::Layout("hierarchy1".into()))
        .unwrap();
    deck.edit_smartart(0, id, &SmartArtEdit::Colors("colorful2".into()))
        .unwrap();
    deck.edit_smartart(0, id, &SmartArtEdit::Style("simple3".into()))
        .unwrap();
    let info = deck.smartart(0, id).unwrap();
    assert!(info.layout.ends_with("hierarchy1"));
    assert!(info.colors.ends_with("colorful2"));
    assert!(info.style.ends_with("simple3"));
    let node = info.nodes[0].id.clone();
    let format = SmartArtFormat {
        fill: Some("00B050".into()),
        line: Some("000000".into()),
        ..SmartArtFormat::default()
    };
    deck.edit_smartart(
        0,
        id,
        &SmartArtEdit::Format {
            node: node.clone(),
            format: format.clone(),
        },
    )
    .unwrap();
    assert_eq!(deck.smartart(0, id).unwrap().nodes[0].format, format);
    deck.edit_smartart(0, id, &SmartArtEdit::ResetGraphic)
        .unwrap();
    assert!(deck.smartart(0, id).unwrap().nodes[0].format.is_empty());
}

#[test]
fn diagrams_convert_to_shapes_and_bulleted_text_to_diagrams() {
    let mut deck = deck();
    let id = deck.insert_smartart(0, "process1").unwrap();
    let group = deck.convert_smartart_to_shapes(0, id).unwrap();
    assert!(deck.smartart(0, group).is_none());
    assert_eq!(deck.shape(0, group).unwrap().kind, "group");

    let text = deck
        .insert(
            0,
            Insert::TextBox,
            ShapeRect {
                x: 40.0,
                y: 40.0,
                width: 300.0,
                height: 120.0,
            },
        )
        .unwrap();
    deck.set_text(
        0,
        jade_office_model::deck::TextTarget::Shape(text),
        "A\nB\nC",
    )
    .unwrap();
    let diagram = deck.convert_to_smartart(0, text, "vList2").unwrap();
    let pane = deck.smartart_text_pane(0, diagram).unwrap();
    let words: Vec<&str> = pane.iter().map(|(_, text)| text.as_str()).collect();
    assert_eq!(words, ["A", "B", "C"]);
}
