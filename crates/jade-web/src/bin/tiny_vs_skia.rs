//! Renders a deck's slides with the tiny-skia painter and compares them with
//! PNGs the Skia backend wrote, reporting how far apart they are.
//!
//! ```text
//! cd crates/jade-pptx-native-render
//! cargo run --bin pptx_to_png -- deck.pptx /tmp/skia 960
//! cd ../jade-web
//! cargo run --release --bin tiny_vs_skia -- deck.pptx /tmp/skia /tmp/tiny 960
//! ```
//!
//! A `.docx` works too: render the Skia pages with jade-docx-native-render's
//! `docx_to_png <doc> <skia-dir>/page.png` (the width argument is then
//! unused; pages are 1 px per CSS px).
//!
//! For each slide or page: the share of pixels that differ by more than a small
//! tolerance, and the mean absolute difference per channel. Antialiasing
//! alone keeps the first above zero; a missing shape or a text run in the
//! wrong place shows as a jump.

use std::path::{Path, PathBuf};
use std::rc::Rc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [deck, skia_dir, out_dir, width] = args.as_slice() else {
        eprintln!("usage: tiny_vs_skia <deck.pptx|doc.docx> <skia-png-dir> <out-dir> <width>");
        std::process::exit(2);
    };
    let width: u32 = width.parse().expect("a width in px");
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jade-core");
    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let fonts = jade_web::font_book(
        std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap(),
        Some(std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap()),
    )
    .unwrap();
    let bytes = std::fs::read(deck).expect("the document");
    std::fs::create_dir_all(out_dir).unwrap();
    // (file name, painted surface, ms) per slide or page. A document's pages
    // are named as jade-docx-native-render's docx_to_png names them, given
    // `<skia-dir>/page.png` as its output.
    let mut rendered = Vec::new();
    if deck.to_lowercase().ends_with(".docx") {
        let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
        let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
        let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
        let started = std::time::Instant::now();
        let document = jade_web::document::Document::open(&bytes, 1.0, Rc::clone(&fonts), measurer)
            .expect("a document");
        let elapsed =
            started.elapsed().as_secs_f64() * 1000.0 / document.page_count().max(1) as f64;
        for index in 0..document.page_count() {
            let name = if index == 0 {
                "page.png".to_string()
            } else {
                format!("page-{}.png", index + 1)
            };
            rendered.push((name, document.page(index).unwrap(), elapsed));
        }
    } else {
        let mut deck = jade_web::Deck::open(&bytes, Rc::clone(&fonts)).expect("a deck");
        for index in 0..deck.slide_count() {
            let started = std::time::Instant::now();
            let painter = deck.render_slide(index, width).expect("a painted slide");
            let elapsed = started.elapsed().as_secs_f64() * 1000.0;
            rendered.push((format!("slide-{:03}.png", index + 1), painter, elapsed));
        }
    }
    for (name, painter, elapsed) in rendered {
        let png = painter.encode_png().unwrap();
        std::fs::write(PathBuf::from(out_dir).join(&name), &png).unwrap();
        let ours = image::load_from_memory(&png).unwrap().to_rgba8();
        match image::open(PathBuf::from(skia_dir).join(&name)) {
            Ok(theirs) => {
                let theirs = theirs.to_rgba8();
                if theirs.dimensions() != ours.dimensions() {
                    println!(
                        "{name}: size differs {:?} vs {:?}",
                        ours.dimensions(),
                        theirs.dimensions()
                    );
                    continue;
                }
                let (mut differing, mut total) = (0usize, 0u64);
                for (a, b) in ours.pixels().zip(theirs.pixels()) {
                    let delta: u32 = (0..3)
                        .map(|c| u32::from(a[c].abs_diff(b[c])))
                        .max()
                        .unwrap();
                    total += u64::from(delta);
                    if delta > 48 {
                        differing += 1;
                    }
                }
                let pixels = ours.pixels().len();
                println!(
                    "{name}: {:.2}% of pixels differ, mean Δ {:.2}, {:.1} ms",
                    100.0 * differing as f64 / pixels as f64,
                    total as f64 / pixels as f64,
                    elapsed
                );
            }
            Err(_) => println!("{name}: no Skia render to compare with"),
        }
    }
}
