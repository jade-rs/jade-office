#![cfg(any(feature = "skia", feature = "tiny-skia"))]

use std::{path::Path, sync::Arc};

use jade_office_model::{korean_face, Backend, Face, KOREAN_FONT};
use jade_painter::Painter;

fn checks_hangul_fallback<B: Backend>(backend: impl Fn(&[Face]) -> B) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |font| Arc::new(std::fs::read(root.join(font)).unwrap());
    let japanese = Face {
        family: "Noto Sans JP".to_string(),
        weight: 400,
        data: read("jade-core/fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf"),
    };
    let korean = korean_face(Arc::new(
        std::fs::read(root.join("fonts").join(KOREAN_FONT)).unwrap(),
    ));
    let width = |faces: &[Face], font: &str, text: &str| {
        let painter = backend(faces).surface(1, 1).unwrap();
        painter.set_font(font);
        painter.measure_text_width(text).ok().unwrap()
    };
    let jp_face = std::slice::from_ref(&japanese);
    let latin = width(jp_face, "16px Noto Sans JP", "Text ");
    let boxes = width(jp_face, "16px Noto Sans JP", "Text 한글");
    let hangul = width(std::slice::from_ref(&korean), "16px Noto Sans KR", "한글");
    let fallback = width(&[japanese, korean], "16px Noto Sans JP", "Text 한글");
    assert!((fallback - (latin + hangul)).abs() < 0.5);
    assert!((fallback - boxes).abs() > 1.0);
}

#[cfg(feature = "tiny-skia")]
#[test]
fn tiny_skia_falls_back_to_the_hangul_face() {
    checks_hangul_fallback(|faces| jade_office_model::tiny::TinySkiaBackend::new(faces).unwrap());
}

#[cfg(feature = "skia")]
#[test]
fn skia_falls_back_to_the_hangul_face() {
    checks_hangul_fallback(|faces| jade_office_model::skia::SkiaBackend::new(faces).unwrap());
}
