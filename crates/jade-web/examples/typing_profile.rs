//! Types into a Word document the way the browser shell does and reports,
//! per keystroke, how long the edit took and which rows of which pages it
//! left damaged — what the shell re-reads and uploads to the GPU.
//!
//!   cargo run --release --example typing_profile -- [file.docx] [scale] [paragraph]

use std::time::Instant;

use jade_web::editing::EditableDocument;

fn main() {
    let core = std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../jade-core"));
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| core.join("contract.docx"));
    let scale: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(2.0);
    let paragraph: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(3);

    let fonts_dir = core.join("fonts/Noto_Sans_JP/static");
    let regular = std::fs::read(fonts_dir.join("NotoSansJP-Regular.ttf")).unwrap();
    let bold = std::fs::read(fonts_dir.join("NotoSansJP-Bold.ttf")).unwrap();
    let fonts = jade_web::backend(regular.clone(), Some(bold.clone()))
        .unwrap()
        .fonts();
    let measurer = jade_web::document::measurer(&regular, Some(&bold)).unwrap();
    let bytes = std::fs::read(&path).unwrap();

    let started = Instant::now();
    let mut document = EditableDocument::open(&bytes, scale, fonts, measurer).unwrap();
    println!(
        "open: {:.1} ms, {} pages, {} paragraphs",
        started.elapsed().as_secs_f64() * 1e3,
        document.page_count(),
        document.paragraph_count()
    );
    let take = |document: &EditableDocument| {
        (0..document.page_count())
            .filter_map(|index| {
                let page = document.page(index)?;
                let height = page.height();
                page.take_damaged_rows().map(|rows| {
                    format!(
                        "p{index}:{}..{} ({} of {height})",
                        rows.start,
                        rows.end,
                        rows.len()
                    )
                })
            })
            .collect::<Vec<_>>()
    };
    take(&document);
    let text = document.paragraph_text(paragraph).unwrap_or_default();
    let mut middle = text.len() / 2;
    while !text.is_char_boundary(middle) {
        middle -= 1;
    }
    document.place_caret(paragraph, middle);
    println!(
        "caret: {:?}, damage after placing: {:?}",
        document.caret(),
        take(&document)
    );

    let mut total = 0.0;
    for (index, ch) in "abcdefghij".chars().enumerate() {
        let started = Instant::now();
        document.insert_text(&ch.to_string()).unwrap();
        let elapsed = started.elapsed().as_secs_f64() * 1e3;
        total += elapsed;
        println!("key {index}: {elapsed:.2} ms, damage {:?}", take(&document));
    }
    println!("typing mean: {:.2} ms", total / 10.0);
    println!("band stats: {:?}", document.band_redraw_stats());
    let started = Instant::now();
    document.delete(true).unwrap();
    println!(
        "backspace: {:.2} ms, damage {:?}",
        started.elapsed().as_secs_f64() * 1e3,
        take(&document)
    );
    let started = Instant::now();
    document.insert_paragraph_break().unwrap();
    println!(
        "enter: {:.2} ms, damage {:?}",
        started.elapsed().as_secs_f64() * 1e3,
        take(&document)
    );
}
