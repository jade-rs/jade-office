//! Opens each Word document given in the native app's document, saves it as
//! the app saves it — once untouched and once after typing into it — and
//! writes both to `<out-dir>`, to check that Word opens them cleanly.
//!
//! ```text
//! cargo run --example resave_docx --features skia -- <out-dir> <file>...
//! ```

use std::path::{Path, PathBuf};

use jade_docx_native_render::{FontFace, NativeDocument};

fn faces() -> Vec<FontFace> {
    let fonts = PathBuf::from(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../jade-core/fonts/Noto_Sans_JP/static"
    ));
    [
        (400, "NotoSansJP-Regular.ttf"),
        (700, "NotoSansJP-Bold.ttf"),
    ]
    .into_iter()
    .map(|(weight, file)| FontFace {
        family: "Noto Sans JP".to_string(),
        weight,
        italic: false,
        path: fonts.join(file),
        index: 0,
    })
    .collect()
}

fn save(bytes: &[u8], edit: bool) -> Result<Vec<u8>, String> {
    let mut document = NativeDocument::open_with_font_faces(bytes, 816.0, 1056.0, &faces())?;
    if edit {
        document.insert_text(0, 0, "jade 編集 ", 1)?;
        document.insert_paragraph_break(0, 4)?;
    }
    document.export_docx()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let out = PathBuf::from(args.next().expect("usage: resave_docx <out-dir> <file>..."));
    std::fs::create_dir_all(&out).unwrap();
    for input in args {
        let path = Path::new(&input);
        let bytes = std::fs::read(path).unwrap();
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        for edit in [false, true] {
            let tag = if edit { "edited" } else { "saved" };
            match std::panic::catch_unwind(|| save(&bytes, edit)) {
                Ok(Ok(saved)) => {
                    std::fs::write(out.join(format!("{stem}.{tag}.docx")), saved).unwrap();
                    println!("ok\t{input}\t{tag}");
                }
                Ok(Err(error)) => println!("error\t{input}\t{tag}\t{error}"),
                Err(_) => println!("panic\t{input}\t{tag}"),
            }
        }
    }
}
