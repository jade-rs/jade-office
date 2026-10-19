//! `jade-office-perf <scenario> <fixture>`
//!
//! Runs one cost scenario of the browser build: the engine's renderers on the
//! tiny-skia painter, at 2 device px per CSS px as on a Retina screen. Meant
//! to be run under
//! `valgrind --tool=callgrind --collect-atstart=no --toggle-collect='*perf_measured_region*'`,
//! which counts only what runs inside [`perf_measured_region`]: opening a
//! fixture does not count towards the cost of typing into it. Run plainly it
//! prints the measured region's wall time, which is only a sanity check.
//!
//! Wall-clock time on a shared CI runner swings tens of percent on unchanged
//! code; instructions executed do not, so a pull request can be judged by
//! them.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use jade_web::document::ShapingMeasurer;
use jade_web::editing::EditableDocument;
use jade_web::TinySkiaBackend;

/// Device px per CSS px: a Retina screen, where the browser build paints
/// documents at twice their size.
const SCALE: f64 = 2.0;
/// Keystrokes the `docx-keystroke` scenario types. `JADE_PERF_KEYSTROKES`
/// overrides it for a sampling profiler that needs a longer run; the workflow
/// leaves it unset so both sides type the same.
const KEYSTROKES: usize = 20;

fn keystrokes() -> usize {
    std::env::var("JADE_PERF_KEYSTROKES")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(KEYSTROKES)
}
/// The sheet viewport, in device px.
const VIEWPORT: (u32, u32) = (2560, 1600);
/// A slide's width on screen, in device px.
const SLIDE_WIDTH: u32 = 1920;

/// The scenarios, in report order, by the kind of fixture they take.
const SCENARIOS: &[(&str, &str)] = &[
    ("docx-open", "docx"),
    ("docx-keystroke", "docx"),
    ("docx-select-all", "docx"),
    ("xlsx-paint", "xlsx"),
    ("pptx-paint", "pptx"),
];

/// Runs `body` as the measured part of a scenario.
///
/// Callgrind's `--toggle-collect=*perf_measured_region*` keys on this name, so
/// it must stay a real, uninlined function with this name.
#[inline(never)]
fn perf_measured_region(body: &mut dyn FnMut()) {
    body();
    std::hint::black_box(());
}

/// Runs `body` measured, and reports how long it took.
fn measure(mut body: impl FnMut() -> Result<(), String>) -> Result<Duration, String> {
    let mut result = Ok(());
    let started = Instant::now();
    perf_measured_region(&mut || result = body());
    let elapsed = started.elapsed();
    result.map(|()| elapsed)
}

fn core() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../jade-core")
}

struct Fonts {
    backend: TinySkiaBackend,
    measurer: Rc<ShapingMeasurer>,
}

fn fonts() -> Result<Fonts, String> {
    let directory = core().join("fonts/Noto_Sans_JP/static");
    let read = |name: &str| {
        std::fs::read(directory.join(name)).map_err(|error| format!("{name}: {error}"))
    };
    let (regular, bold) = (
        read("NotoSansJP-Regular.ttf")?,
        read("NotoSansJP-Bold.ttf")?,
    );
    Ok(Fonts {
        measurer: jade_web::document::measurer(&regular, Some(&bold))?,
        backend: jade_web::backend(regular, Some(bold))?,
    })
}

fn open_document(bytes: &[u8], fonts: &Fonts) -> Result<EditableDocument, String> {
    EditableDocument::open(
        bytes,
        SCALE,
        fonts.backend.fonts(),
        Rc::clone(&fonts.measurer),
    )
}

fn run(scenario: &str, fixture: &Path) -> Result<Duration, String> {
    let bytes =
        std::fs::read(fixture).map_err(|error| format!("{}: {error}", fixture.display()))?;
    let fonts = fonts()?;
    match scenario {
        // Reading, laying out and painting every page.
        "docx-open" => measure(|| open_document(&bytes, &fonts).map(drop)),
        // Typing into the first paragraph with text, one character at a
        // time, each repainting what it changed.
        "docx-keystroke" => {
            let mut document = open_document(&bytes, &fonts)?;
            let paragraph = (0..document.paragraph_count())
                .find(|&index| {
                    document
                        .paragraph_text(index)
                        .is_some_and(|text| !text.trim().is_empty())
                })
                .ok_or("no paragraph with text")?;
            document.place_caret(paragraph, 0);
            measure(|| {
                for _ in 0..keystrokes() {
                    document.insert_text("a")?;
                }
                Ok(())
            })
        }
        // A highlight over every page: the repaint a long selection costs.
        "docx-select-all" => {
            let mut document = open_document(&bytes, &fonts)?;
            measure(|| document.select_all())
        }
        "xlsx-paint" => {
            let book = jade_web::Workbook::open(&bytes, "UTC", fonts.backend.clone())?;
            measure(|| {
                book.render_viewport((0.0, 0.0), VIEWPORT.0, VIEWPORT.1, SCALE, None)
                    .map(drop)
                    .ok_or_else(|| "no viewport".to_string())
            })
        }
        "pptx-paint" => {
            let mut deck = jade_web::Deck::open(&bytes, fonts.backend.clone())?;
            measure(|| {
                for index in 0..deck.slide_count() {
                    deck.render_slide(index, SLIDE_WIDTH)
                        .ok_or_else(|| format!("slide {} did not paint", index + 1))?;
                }
                Ok(())
            })
        }
        _ => Err(format!("unknown scenario {scenario}")),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("--list") {
        for (scenario, kind) in SCENARIOS {
            println!("{scenario} {kind}");
        }
        return ExitCode::SUCCESS;
    }
    let [scenario, fixture] = args.as_slice() else {
        eprintln!("usage: jade-office-perf <scenario> <fixture>\n       jade-office-perf --list");
        return ExitCode::from(2);
    };
    let mut fixture = PathBuf::from(fixture);
    if !fixture.exists() {
        fixture = core().join("public/fixtures").join(&fixture);
    }
    match run(scenario, &fixture) {
        Ok(elapsed) => {
            eprintln!("{scenario} {}: {elapsed:?}", fixture.display());
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{scenario} {}: {error}", fixture.display());
            ExitCode::FAILURE
        }
    }
}
