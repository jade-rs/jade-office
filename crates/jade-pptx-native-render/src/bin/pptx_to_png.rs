//! Rasterises the slides of a `.pptx` to PNGs, one file per slide.
//!
//! ```text
//! cd crates/jade-pptx-native-render
//! cargo run --bin pptx_to_png -- deck.pptx out-dir [width]
//! ```

use std::path::{Path, PathBuf};

use jade_pptx_native_render::{aliased_faces, bundled_font_paths, NativeDeck};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: pptx_to_png <in.pptx> <out-dir> [width]");
        std::process::exit(2);
    };
    let width = args
        .next()
        .and_then(|width| width.parse().ok())
        .unwrap_or(1280);

    let bytes = std::fs::read(&input).unwrap_or_else(|error| {
        eprintln!("cannot read {input}: {error}");
        std::process::exit(1);
    });
    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jade-core");
    let (regular, bold) = bundled_font_paths(&core);
    let deck = NativeDeck::open(&bytes, &aliased_faces(&regular, &bold)).unwrap_or_else(|error| {
        eprintln!("{error}");
        std::process::exit(1);
    });

    let output = PathBuf::from(output);
    if let Err(error) = std::fs::create_dir_all(&output) {
        eprintln!("cannot create {}: {error}", output.display());
        std::process::exit(1);
    }
    for index in 0..deck.slide_count() {
        let Some(png) = deck.slide_png(index, width) else {
            eprintln!("slide {} did not paint", index + 1);
            std::process::exit(1);
        };
        let path = output.join(format!("slide-{:03}.png", index + 1));
        if let Err(error) = std::fs::write(&path, png) {
            eprintln!("cannot write {}: {error}", path.display());
            std::process::exit(1);
        }
        println!("{}", path.display());
    }
}
