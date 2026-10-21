//! Lays every fixture that has a Word glyph reference out natively and
//! reports how far jade-office's glyphs are from Word's, row by row.
//!
//! ```text
//! cargo run --release --bin word_glyph_sweep -- \
//!     <reference-dir> <fixture-dir> [--platform mac|windows] [--fonts word|bundled] \
//!     [--out report.json] [--snapshots dir] [fixture ...]
//! ```
//!
//! The native counterpart of jlsi/jade's `tests/playwright/word-glyph-sweep.mjs`,
//! which does the same in the browser playground. `<reference-dir>` holds the
//! `<fixture>.json` files `scripts/word-pdf-glyph-reference.py` wrote from a
//! PDF Word itself printed (`tests/word-oracle/mac-glyph-corpus-20261019` in
//! jlsi/jade), and `<fixture-dir>` the documents they were printed from
//! (jlsi/jade's `public/fixtures`). The report and its summary have the
//! playground sweep's fields and rules, so the two numbers sit side by side:
//! rows are aligned by text, and a row counts as exact when its page, glyphs,
//! baseline and every glyph origin agree with Word's within 0.375pt.
//!
//! The documents are laid out as `jade-web`'s `Document` lays them out — the
//! docx renderer on the tiny-skia host, measured by the shaping measurer with
//! `ShapingOptions::CHROMIUM_PARITY` — in the final view without markup, as
//! the playground sweep's `--changes false` does. `--platform` picks the Word
//! the layout reproduces (Mac by default, the corpus's).
//!
//! `--fonts` picks what the measurer knows:
//!
//! - `word` (the default) — what the playground sweep measures with on a Mac:
//!   the faces Word ships (its `DFonts`, under the families and Japanese
//!   names the playground registers them as), then any installed font a
//!   document or jade's fallback stacks name, as Chromium finds local fonts.
//! - `bundled` — what the jade-office shells ship: Noto Sans JP and Noto
//!   Sans Thai for every family.
//!
//! `--snapshots <dir>` also writes each fixture's full layout capture, the
//! JSON the playground's `captureFullLayoutDebug()` returns, so the
//! playground's own comparison can be run over it.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::rc::Rc;

use jade_docx_editor::canvas_renderer::FullLayoutDebugSnapshot;
use jade_docx_layout::font_measurer::RegisteredFontStyle;
use jade_docx_layout::font_resolver::{
    resolve_font_families, DEFAULT_COMPLEX_FONT_STACK, DEFAULT_EAST_ASIA_FONT_STACK,
    DEFAULT_LATIN_FONT_STACK,
};
use jade_docx_layout::line_metrics::{set_word_layout_platform, WordLayoutPlatform};
use jade_docx_layout::shaping_measurer::{ShapingMeasurer, ShapingOptions};
use jade_docx_reader::document::{RunFonts, RunProperties};
use serde::Serialize;
use sha2::{Digest, Sha256};

const PX_TO_PT: f64 = 72.0 / 96.0;
/// Glyphs whose baselines differ by less than this belong to one row.
const ROW_MERGE_PT: f64 = 1.0;
/// How far a baseline or glyph origin may sit from Word's.
const TOLERANCE_PT: f64 = 0.375;

/// Where the playground looks for the faces Word ships, in order.
fn word_font_directories() -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Ok(directory) = std::env::var("JADE_WORD_FONT_DIR") {
        if !directory.trim().is_empty() {
            directories.push(PathBuf::from(directory.trim()));
        }
    }
    directories.extend(
        [
            "/Applications/Microsoft Word.app/Contents/Resources/DFonts",
            "/Library/Fonts/Microsoft",
            "/Library/Fonts",
        ]
        .map(PathBuf::from),
    );
    directories
}

/// A face Word ships: family, file, face in the collection, weight, italic.
/// The playground's `LOCAL_WORD_FONT_FACES`, with the collection faces its
/// font server cuts out.
const WORD_FACES: &[(&str, &str, u32, u16, bool)] = &[
    ("MS Mincho", "msmincho.ttc", 0, 400, false),
    ("MS Gothic", "msgothic.ttc", 0, 400, false),
    ("MS PMincho", "msmincho.ttc", 1, 400, false),
    ("MS PGothic", "msgothic.ttc", 2, 400, false),
    ("MS UI Gothic", "msgothic.ttc", 1, 400, false),
    ("Yu Mincho", "yumin.ttf", 0, 400, false),
    ("Yu Mincho", "yumindb.ttf", 0, 700, false),
    ("Yu Gothic", "YuGothR.ttc", 0, 400, false),
    ("Yu Gothic", "YuGothB.ttc", 0, 700, false),
    ("Yu Gothic Light", "YuGothL.ttc", 0, 400, false),
    ("Yu Gothic Medium", "YuGothM.ttc", 0, 400, false),
    ("Century", "Century.ttf", 0, 400, false),
    ("Calibri", "Calibri.ttf", 0, 400, false),
    ("Calibri", "Calibrib.ttf", 0, 700, false),
    ("Calibri", "Calibrii.ttf", 0, 400, true),
    ("Calibri", "Calibriz.ttf", 0, 700, true),
    ("Cambria", "Cambria.ttc", 0, 400, false),
    ("Cambria", "Cambriab.ttf", 0, 700, false),
    ("Cambria", "Cambriai.ttf", 0, 400, true),
    ("Cambria", "Cambriaz.ttf", 0, 700, true),
    ("Cambria Math", "Cambria.ttc", 1, 400, false),
    ("Bookman Old Style", "Bookman Old Style.ttf", 0, 400, false),
    (
        "Bookman Old Style",
        "Bookman Old Style Bold.ttf",
        0,
        700,
        false,
    ),
    (
        "Bookman Old Style",
        "Bookman Old Style Italic.ttf",
        0,
        400,
        true,
    ),
    (
        "Bookman Old Style",
        "Bookman Old Style Bold Italic.ttf",
        0,
        700,
        true,
    ),
    ("HGMaruGothicMPRO", "HGRSMP.ttf", 0, 400, false),
    ("Meiryo", "meiryo.ttc", 0, 400, false),
    ("Meiryo", "meiryob.ttc", 0, 700, false),
];

/// The Japanese names documents use for the faces above, registered too
/// (the playground's `LOCAL_WORD_FONT_ALIASES`).
const WORD_FACE_ALIASES: &[(&str, &str)] = &[
    ("MS Mincho", "ＭＳ 明朝"),
    ("MS Gothic", "ＭＳ ゴシック"),
    ("MS PMincho", "ＭＳ Ｐ明朝"),
    ("MS PGothic", "ＭＳ Ｐゴシック"),
    ("Yu Mincho", "游明朝"),
    ("Yu Gothic", "游ゴシック"),
    ("Yu Gothic Light", "游ゴシック Light"),
    ("Yu Gothic Medium", "游ゴシック Medium"),
    ("HGMaruGothicMPRO", "HG丸ｺﾞｼｯｸM-PRO"),
    ("Meiryo", "メイリオ"),
];

/// The families Chromium on a Mac gives the generic names, by default.
const GENERIC_FAMILIES: &[(&str, &str)] = &[
    ("serif", "Times"),
    ("sans-serif", "Helvetica"),
    ("monospace", "Courier"),
];

/// An installed face, found by any of its names.
struct InstalledFace {
    path: PathBuf,
    index: u32,
    weight: u16,
    style: RegisteredFontStyle,
    /// The family the face belongs to, as CoreText groups faces: the
    /// typographic family name, else the family name.
    family: String,
}

/// Installed fonts by name, as Chromium on a Mac finds a family: by family
/// name first, in any language; then a face by its full, PostScript or
/// legacy per-weight family name (`ヒラギノ角ゴ ProN W3`), whose family gives
/// its bold.
struct InstalledFonts {
    by_family: HashMap<String, Vec<usize>>,
    by_face_name: HashMap<String, usize>,
    faces: Vec<InstalledFace>,
}

impl InstalledFonts {
    fn scan() -> Self {
        let mut database = fontdb::Database::new();
        database.load_system_fonts();
        let mut fonts = Self {
            by_family: HashMap::new(),
            by_face_name: HashMap::new(),
            faces: Vec::new(),
        };
        for info in database.faces() {
            let fontdb::Source::File(path) = &info.source else {
                continue;
            };
            let names = database
                .with_face_data(info.id, |data, index| {
                    let face = ttf_parser::Face::parse(data, index).ok()?;
                    let named = |id: u16| -> BTreeSet<String> {
                        face.names()
                            .into_iter()
                            .filter(|name| name.name_id == id)
                            .filter_map(|name| name.to_string())
                            .map(|name| name.to_lowercase())
                            .collect()
                    };
                    // fontdb's families are the typographic family names,
                    // else the family names, and read the Mac Roman records
                    // ttf-parser leaves undecoded (Times has only those).
                    let mut families: BTreeSet<String> = info
                        .families
                        .iter()
                        .map(|(family, _)| family.to_lowercase())
                        .collect();
                    let typographic = named(ttf_parser::name_id::TYPOGRAPHIC_FAMILY);
                    let mut face_names = named(ttf_parser::name_id::FULL_NAME);
                    face_names.extend(named(ttf_parser::name_id::POST_SCRIPT_NAME));
                    face_names.insert(info.post_script_name.to_lowercase());
                    if typographic.is_empty() {
                        families.extend(named(ttf_parser::name_id::FAMILY));
                    } else {
                        families.extend(typographic);
                        face_names.extend(named(ttf_parser::name_id::FAMILY));
                    }
                    Some((families, face_names))
                })
                .flatten();
            let Some((families, face_names)) = names else {
                continue;
            };
            let Some(family) = info
                .families
                .first()
                .map(|(family, _)| family.to_lowercase())
            else {
                continue;
            };
            let id = fonts.faces.len();
            fonts.faces.push(InstalledFace {
                path: path.clone(),
                index: info.index,
                weight: info.weight.0,
                style: match info.style {
                    fontdb::Style::Normal => RegisteredFontStyle::Normal,
                    fontdb::Style::Italic | fontdb::Style::Oblique => RegisteredFontStyle::Italic,
                },
                family,
            });
            for name in families {
                fonts.by_family.entry(name).or_default().push(id);
            }
            for name in face_names {
                fonts.by_face_name.entry(name).or_insert(id);
            }
        }
        fonts
    }

    /// The faces `name` finds, each at the weight it answers for: every face
    /// of a family of that name at its own; else the one face of that name as
    /// the regular, with the heavier face of its family nearest bold as the
    /// bold.
    fn find(&self, name: &str) -> Vec<(&InstalledFace, u16)> {
        let key = name.to_lowercase();
        if let Some(family) = self.by_family.get(&key) {
            return family
                .iter()
                .map(|&id| (&self.faces[id], self.faces[id].weight))
                .collect();
        }
        let Some(&id) = self.by_face_name.get(&key) else {
            return Vec::new();
        };
        let face = &self.faces[id];
        let bold = self
            .faces
            .iter()
            .filter(|other| {
                other.family == face.family
                    && other.style == face.style
                    && other.weight > face.weight
            })
            .min_by_key(|other| (i32::from(other.weight) - 700).abs());
        std::iter::once((face, 400))
            .chain(bold.map(|bold| (bold, 700)))
            .collect()
    }
}

/// The measurer the playground sweep measures with: Word's faces first, then
/// the installed fonts `families` name.
fn word_measurer(families: &BTreeSet<String>) -> Result<(ShapingMeasurer, Vec<String>), String> {
    let mut measurer = ShapingMeasurer::with_options(12.0, ShapingOptions::CHROMIUM_PARITY);
    let mut registered = HashSet::new();
    let mut report = Vec::new();
    let directories = word_font_directories();
    for &(family, file, index, weight, italic) in WORD_FACES {
        let Some(path) = directories
            .iter()
            .map(|directory| directory.join(file))
            .find(|path| path.exists())
        else {
            report.push(format!("missing Word face {family} ({file})"));
            continue;
        };
        let bytes = std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))?;
        let style = if italic {
            RegisteredFontStyle::Italic
        } else {
            RegisteredFontStyle::Normal
        };
        let aliases = WORD_FACE_ALIASES
            .iter()
            .filter(|(english, _)| *english == family)
            .map(|(_, alias)| *alias);
        for name in std::iter::once(family).chain(aliases) {
            measurer
                .register_font_bytes_with_collection_index(name, weight, style, &bytes, index)
                .map_err(|error| format!("{}: {error}", path.display()))?;
            registered.insert(name.to_lowercase());
        }
    }
    // A family the page registers is the page's; the installed fonts only
    // answer for the names it does not.
    let installed = InstalledFonts::scan();
    let mut requests: Vec<(String, String)> = families
        .iter()
        .map(|family| (family.clone(), family.clone()))
        .collect();
    requests.extend(
        GENERIC_FAMILIES
            .iter()
            .map(|(generic, family)| (generic.to_string(), family.to_string())),
    );
    for (name, lookup) in requests {
        if registered.contains(&name.to_lowercase()) {
            continue;
        }
        let faces = installed.find(&lookup);
        if faces.is_empty() {
            report.push(format!("not installed: {name}"));
            continue;
        }
        for (face, weight) in faces {
            measurer
                .register_font_path_with_collection_index(
                    name.as_str(),
                    weight,
                    face.style,
                    &face.path,
                    face.index,
                )
                .map_err(|error| format!("{}: {error}", face.path.display()))?;
        }
        registered.insert(name.to_lowercase());
    }
    Ok((measurer, report))
}

/// Every family the documents name, and every family jade's fallback stacks
/// add to those: the names a measurer may be asked for.
fn named_families(documents: &[Vec<u8>]) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    for bytes in documents {
        let Ok(package) = jade_opc::OpcPackage::open(bytes) else {
            continue;
        };
        for part in package.part_names() {
            if !part.ends_with(".xml") {
                continue;
            }
            if let Ok(Some(xml)) = package.part_str(part) {
                font_attributes(&xml, part.ends_with("fontTable.xml"), &mut named);
            }
        }
    }
    let mut families = BTreeSet::new();
    for stack in [
        DEFAULT_LATIN_FONT_STACK,
        DEFAULT_EAST_ASIA_FONT_STACK,
        DEFAULT_COMPLEX_FONT_STACK,
    ] {
        stack_families(stack, &mut families);
    }
    for name in &named {
        let name: Rc<str> = Rc::from(name.as_str());
        for language in [None, Some("ja-JP")] {
            let properties = RunProperties {
                fonts: Some(RunFonts {
                    ascii: Some(Rc::clone(&name)),
                    h_ansi: Some(Rc::clone(&name)),
                    east_asia: Some(Rc::clone(&name)),
                    complex: Some(Rc::clone(&name)),
                    ..RunFonts::default()
                }),
                language: language.map(Rc::from),
                east_asia_language: language.map(Rc::from),
                ..RunProperties::default()
            };
            let resolved = resolve_font_families(Some(&properties));
            for stack in [
                resolved.latin_stack(),
                resolved.east_asia_stack(),
                resolved.complex_stack(),
            ] {
                stack_families(stack, &mut families);
            }
        }
        families.insert(name.to_string());
    }
    families.retain(|family| {
        !GENERIC_FAMILIES
            .iter()
            .any(|(generic, _)| generic == family)
    });
    families
}

/// The values of the attributes that name a font: `w:rFonts`' slots,
/// DrawingML's typefaces (theme fonts among them) and, in the font table,
/// the fonts' names.
fn font_attributes(xml: &str, font_table: bool, out: &mut BTreeSet<String>) {
    const ATTRIBUTES: &[&str] = &[
        "w:ascii=\"",
        "w:hAnsi=\"",
        "w:eastAsia=\"",
        "w:cs=\"",
        "typeface=\"",
    ];
    let names = font_table.then_some("w:name=\"");
    for attribute in ATTRIBUTES.iter().copied().chain(names) {
        let mut rest = xml;
        while let Some(start) = rest.find(attribute) {
            rest = &rest[start + attribute.len()..];
            let Some(end) = rest.find('"') else {
                break;
            };
            let value = rest[..end].trim();
            if !value.is_empty() && !value.starts_with('+') {
                out.insert(unescape_xml(value));
            }
            rest = &rest[end..];
        }
    }
}

fn unescape_xml(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// The family names of a CSS font stack.
fn stack_families(stack: &str, out: &mut BTreeSet<String>) {
    for family in stack.split(',') {
        let family = family.trim().trim_matches(|c| c == '\'' || c == '"').trim();
        if !family.is_empty() {
            out.insert(family.to_string());
        }
    }
}

#[derive(Clone, Debug)]
struct Glyph {
    page: usize,
    baseline_pt: f64,
    text: String,
    x_pt: f64,
}

#[derive(Clone, Debug)]
struct Row {
    page: usize,
    baseline_pt: f64,
    text: String,
    characters: Vec<Glyph>,
}

/// Whether JavaScript's `String.prototype.trim` leaves nothing of `text`.
fn js_blank(text: &str) -> bool {
    text.chars()
        .all(|c| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}')
}

/// Rows of a capture in page-local points, headers, notes and text boxes
/// included: `jadeGlyphRows(snapshot, { includeAuxiliary: true })`.
fn jade_rows(snapshot: &FullLayoutDebugSnapshot) -> Vec<Row> {
    let entries = snapshot
        .body_lines
        .iter()
        .map(|line| (line, line.page))
        .chain(
            snapshot
                .auxiliary_lines
                .iter()
                .map(|entry| (&entry.line, entry.page)),
        );
    let mut glyphs = Vec::new();
    for (line, physical_page) in entries {
        // Fragment x and each glyph x both carry a report-only shift for Word
        // COM traces; undo both to compare where the glyphs are drawn.
        let drawn_shift = 2.0 * line.report_x_offset;
        for fragment in &line.fragments {
            let baseline_pt = fragment.baseline * PX_TO_PT;
            for glyph in &fragment.glyphs {
                if js_blank(&glyph.text) {
                    continue;
                }
                glyphs.push(Glyph {
                    page: physical_page + 1,
                    baseline_pt,
                    text: glyph.text.clone(),
                    x_pt: (fragment.x + glyph.x - drawn_shift) * PX_TO_PT,
                });
            }
        }
    }
    glyphs.sort_by(|a, b| {
        a.page
            .cmp(&b.page)
            .then(js_order(a.baseline_pt, b.baseline_pt))
            .then(js_order(a.x_pt, b.x_pt))
    });
    let mut rows: Vec<Row> = Vec::new();
    for glyph in glyphs {
        match rows.last_mut() {
            Some(last)
                if last.page == glyph.page
                    && (last.baseline_pt - glyph.baseline_pt).abs() < ROW_MERGE_PT =>
            {
                last.characters.push(glyph);
            }
            _ => rows.push(Row {
                page: glyph.page,
                baseline_pt: glyph.baseline_pt,
                text: String::new(),
                characters: vec![glyph],
            }),
        }
    }
    for row in &mut rows {
        row.characters.sort_by(|a, b| js_order(a.x_pt, b.x_pt));
        row.text = row
            .characters
            .iter()
            .map(|glyph| glyph.text.as_str())
            .collect();
    }
    rows
}

/// A comparator's `a - b` as `Array.prototype.sort` reads it: NaN is a tie.
fn js_order(a: f64, b: f64) -> std::cmp::Ordering {
    a.partial_cmp(&b).unwrap_or(std::cmp::Ordering::Equal)
}

/// Rows of a Word glyph reference, flattened across pages.
fn word_rows(reference: &serde_json::Value) -> Vec<Row> {
    let number = |value: &serde_json::Value| value.as_f64().unwrap_or(f64::NAN);
    let mut rows = Vec::new();
    for page in reference["pages"].as_array().into_iter().flatten() {
        let page_number = page["page"].as_u64().unwrap_or(0) as usize;
        for row in page["rows"].as_array().into_iter().flatten() {
            let characters = row["characters"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|glyph| Glyph {
                    page: page_number,
                    baseline_pt: number(&row["baseline_pt"]),
                    text: glyph["text"].as_str().unwrap_or_default().to_string(),
                    x_pt: number(&glyph["x_pt"]),
                })
                .collect();
            rows.push(Row {
                page: page_number,
                baseline_pt: number(&row["baseline_pt"]),
                text: row["text"].as_str().unwrap_or_default().to_string(),
                characters,
            });
        }
    }
    rows
}

fn finite_positions(row: &Row) -> bool {
    row.baseline_pt.is_finite() && row.characters.iter().all(|glyph| glyph.x_pt.is_finite())
}

/// Longest common subsequence of two row lists by exact text.
fn align_rows<'a>(word: &'a [Row], jade: &'a [Row]) -> Vec<(usize, usize)> {
    let (n, m) = (word.len(), jade.len());
    let mut table = vec![0u32; (n + 1) * (m + 1)];
    let at = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            table[at(i, j)] = if word[i].text == jade[j].text {
                table[at(i + 1, j + 1)] + 1
            } else {
                table[at(i + 1, j)].max(table[at(i, j + 1)])
            };
        }
    }
    let mut pairs = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if word[i].text == jade[j].text {
            pairs.push((i, j));
            i += 1;
            j += 1;
        } else if table[at(i + 1, j)] >= table[at(i, j + 1)] {
            i += 1;
        } else {
            j += 1;
        }
    }
    pairs
}

/// `Math.round(value * 1000) / 1000`.
fn round(value: f64) -> f64 {
    (value * 1000.0 + 0.5).floor() / 1000.0
}

/// `text.slice(0, units)`: the first `units` UTF-16 code units.
fn prefix(text: &str, units: usize) -> String {
    let units: Vec<u16> = text.encode_utf16().take(units).collect();
    String::from_utf16_lossy(&units)
}

#[derive(Serialize)]
struct Worst {
    page: usize,
    text: String,
    dy: f64,
    dx: f64,
    #[serde(rename = "jadePage")]
    jade_page: usize,
    invalid_coordinates: bool,
}

#[derive(Serialize)]
struct Delta {
    page: usize,
    dy: f64,
    dx0: f64,
    dx: f64,
    text: String,
}

/// One fixture's comparison: `compareWordGlyphSweep`'s fields.
#[derive(Serialize)]
struct Comparison {
    word_pages: u64,
    jade_pages: usize,
    word_rows: usize,
    jade_rows: usize,
    matched_rows: usize,
    rows_off: usize,
    page_mismatches: usize,
    glyph_segmentation_mismatches: usize,
    invalid_coordinate_rows: usize,
    complete: bool,
    max_baseline_pt: f64,
    max_x_pt: f64,
    unmatched_word: Vec<String>,
    unmatched_jade: Vec<String>,
    worst: Vec<Worst>,
    deltas: Vec<Delta>,
}

fn compare(reference: &serde_json::Value, snapshot: &FullLayoutDebugSnapshot) -> Comparison {
    let word = word_rows(reference);
    let jade = jade_rows(snapshot);
    let pairs = align_rows(&word, &jade);
    let (mut max_baseline, mut max_x) = (0f64, 0f64);
    let (mut rows_off, mut page_mismatches, mut segmentation, mut invalid) = (0, 0, 0, 0);
    let mut worst = Vec::new();
    let mut deltas = Vec::new();
    for &(wi, ji) in &pairs {
        let (w, j) = (&word[wi], &jade[ji]);
        if w.page != j.page {
            page_mismatches += 1;
        }
        let dy = j.baseline_pt - w.baseline_pt;
        let mut row_max_x = 0f64;
        let same_glyphs = w.characters.len() == j.characters.len()
            && w.characters
                .iter()
                .zip(&j.characters)
                .all(|(a, b)| a.text == b.text);
        let valid = finite_positions(w) && finite_positions(j);
        if !valid {
            invalid += 1;
        }
        if same_glyphs && valid {
            for (a, b) in w.characters.iter().zip(&j.characters) {
                row_max_x = row_max_x.max((b.x_pt - a.x_pt).abs());
            }
        } else if !same_glyphs {
            segmentation += 1;
        }
        if dy.is_finite() {
            max_baseline = max_baseline.max(dy.abs());
        }
        max_x = max_x.max(row_max_x);
        deltas.push(Delta {
            page: w.page,
            dy: round(dy),
            dx0: round(j.characters[0].x_pt - w.characters[0].x_pt),
            dx: round(row_max_x),
            text: prefix(&w.text, 24),
        });
        if !same_glyphs
            || !valid
            || dy.abs() > TOLERANCE_PT
            || row_max_x > TOLERANCE_PT
            || w.page != j.page
        {
            rows_off += 1;
            worst.push(Worst {
                page: w.page,
                text: prefix(&w.text, 40),
                dy: round(dy),
                dx: round(row_max_x),
                jade_page: j.page,
                invalid_coordinates: !valid,
            });
        }
    }
    let matched_word: HashSet<usize> = pairs.iter().map(|&(w, _)| w).collect();
    let matched_jade: HashSet<usize> = pairs.iter().map(|&(_, j)| j).collect();
    let unmatched = |rows: &[Row], matched: &HashSet<usize>| {
        rows.iter()
            .enumerate()
            .filter(|(index, _)| !matched.contains(index))
            .take(6)
            .map(|(_, row)| format!("{}:{}", row.page, prefix(&row.text, 40)))
            .collect()
    };
    let severity = |entry: &Worst| entry.dy.abs().max(entry.dx);
    worst.sort_by(|a, b| js_order(severity(b), severity(a)));
    worst.truncate(6);
    Comparison {
        word_pages: reference["page_count"].as_u64().unwrap_or(0),
        jade_pages: snapshot.page_count,
        word_rows: word.len(),
        jade_rows: jade.len(),
        matched_rows: pairs.len(),
        rows_off,
        page_mismatches,
        glyph_segmentation_mismatches: segmentation,
        invalid_coordinate_rows: invalid,
        complete: snapshot.complete,
        max_baseline_pt: round(max_baseline),
        max_x_pt: round(max_x),
        unmatched_word: unmatched(&word, &matched_word),
        unmatched_jade: unmatched(&jade, &matched_jade),
        worst,
        deltas,
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum Entry {
    Compared(Comparison),
    Failed { error: String },
}

/// `summarizeWordGlyphSweep`: completion from exact counts.
#[derive(Serialize)]
struct Summary {
    fixtures: usize,
    error_fixtures: usize,
    word_rows: usize,
    matched_rows: usize,
    exact_rows: usize,
    matched_percent: Option<f64>,
    exact_percent: Option<f64>,
    platform: &'static str,
    fonts: String,
}

fn summarize(report: &[(String, Entry)], platform: &'static str, fonts: &str) -> Summary {
    let compared: Vec<&Comparison> = report
        .iter()
        .filter_map(|(_, entry)| match entry {
            Entry::Compared(comparison) => Some(comparison),
            Entry::Failed { .. } => None,
        })
        .collect();
    let word_rows: usize = compared.iter().map(|entry| entry.word_rows).sum();
    let matched_rows: usize = compared.iter().map(|entry| entry.matched_rows).sum();
    let exact_rows = matched_rows - compared.iter().map(|entry| entry.rows_off).sum::<usize>();
    let percent = |rows: usize| (word_rows > 0).then(|| 100.0 * rows as f64 / word_rows as f64);
    Summary {
        fixtures: report.len(),
        error_fixtures: report.len() - compared.len(),
        word_rows,
        matched_rows,
        exact_rows,
        matched_percent: percent(matched_rows),
        exact_percent: percent(exact_rows),
        platform,
        fonts: fonts.to_string(),
    }
}

struct Arguments {
    reference_dir: PathBuf,
    fixture_dir: PathBuf,
    platform: WordLayoutPlatform,
    fonts: String,
    out: PathBuf,
    snapshots: Option<PathBuf>,
    only: Vec<String>,
}

fn arguments() -> Result<Arguments, String> {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut option = |name: &str| -> Option<String> {
        let index = args.iter().position(|arg| arg == name)?;
        let value = args.get(index + 1).cloned();
        args.drain(index..(index + 2).min(args.len()));
        value
    };
    let platform = match option("--platform").as_deref() {
        None | Some("mac") => WordLayoutPlatform::Mac,
        Some("windows") => WordLayoutPlatform::Windows,
        Some(other) => return Err(format!("--platform takes mac or windows, not {other}")),
    };
    let fonts = option("--fonts").unwrap_or_else(|| "word".to_string());
    if fonts != "word" && fonts != "bundled" {
        return Err(format!("--fonts takes word or bundled, not {fonts}"));
    }
    let out = option("--out").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("target/word-glyph-sweep.json")
    });
    let snapshots = option("--snapshots").map(PathBuf::from);
    let mut positional = args.into_iter();
    let (Some(reference_dir), Some(fixture_dir)) = (positional.next(), positional.next()) else {
        return Err("usage: word_glyph_sweep <reference-dir> <fixture-dir> [--platform mac|windows] [--fonts word|bundled] [--out file] [--snapshots dir] [fixture ...]".to_string());
    };
    Ok(Arguments {
        reference_dir: reference_dir.into(),
        fixture_dir: fixture_dir.into(),
        platform,
        fonts,
        out,
        snapshots,
        only: positional.collect(),
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn run() -> Result<(), String> {
    let arguments = arguments()?;
    // Before anything is laid out, as a host does.
    set_word_layout_platform(arguments.platform);
    let platform = match arguments.platform {
        WordLayoutPlatform::Mac => "mac",
        WordLayoutPlatform::Windows => "windows",
    };

    let mut fixtures: Vec<String> = std::fs::read_dir(&arguments.reference_dir)
        .map_err(|error| format!("{}: {error}", arguments.reference_dir.display()))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            entry
                .file_name()
                .to_str()
                .and_then(|name| name.strip_suffix(".json"))
                .map(str::to_string)
        })
        .filter(|name| arguments.only.is_empty() || arguments.only.contains(name))
        .collect();
    fixtures.sort();
    let documents: Vec<Option<Vec<u8>>> = fixtures
        .iter()
        .map(|fixture| std::fs::read(arguments.fixture_dir.join(format!("{fixture}.docx"))).ok())
        .collect();

    let core = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../jade-core/fonts");
    let read = |path: PathBuf| {
        std::fs::read(&path).map_err(|error| format!("{}: {error}", path.display()))
    };
    let regular = read(core.join("Noto_Sans_JP/static/NotoSansJP-Regular.ttf"))?;
    let bold = read(core.join("Noto_Sans_JP/static/NotoSansJP-Bold.ttf"))?;
    let thai = read(core.join(jade_office_model::THAI_FONT))?;
    // The painter's faces only colour pixels; the measurer decides the layout.
    let painter_fonts = jade_web::backend(regular.clone(), Some(bold.clone()))?.fonts();
    let measurer = if arguments.fonts == "word" {
        let named = named_families(&documents.iter().flatten().cloned().collect::<Vec<_>>());
        let (measurer, notes) = word_measurer(&named)?;
        for note in notes {
            eprintln!("fonts: {note}");
        }
        Rc::new(measurer)
    } else {
        jade_web::document::measurer_with(&regular, Some(&bold), Some(&thai))?
    };

    if let Some(directory) = &arguments.snapshots {
        std::fs::create_dir_all(directory)
            .map_err(|error| format!("{}: {error}", directory.display()))?;
    }
    let mut report = Vec::new();
    for (fixture, document) in fixtures.iter().zip(&documents) {
        let entry = (|| -> Result<Comparison, String> {
            let path = arguments.reference_dir.join(format!("{fixture}.json"));
            let reference: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?,
            )
            .map_err(|error| format!("{}: {error}", path.display()))?;
            let bytes = document.as_ref().ok_or_else(|| {
                format!("no {fixture}.docx in {}", arguments.fixture_dir.display())
            })?;
            if reference["docx_sha256"].as_str() != Some(sha256_hex(bytes).as_str()) {
                return Err(format!(
                    "Word reference source hash does not match {fixture}.docx"
                ));
            }
            let snapshot = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                jade_web::document::full_layout_debug(
                    bytes,
                    Rc::clone(&painter_fonts),
                    Rc::clone(&measurer),
                )
            }))
            .map_err(|_| "the layout panicked".to_string())??;
            if let Some(directory) = &arguments.snapshots {
                let path = directory.join(format!("{fixture}.json"));
                let json = serde_json::to_string(&snapshot).map_err(|error| error.to_string())?;
                std::fs::write(&path, json)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
            }
            Ok(compare(&reference, &snapshot))
        })();
        match &entry {
            Ok(entry) => println!(
                "{fixture}: pages {}/{} rows {}/{} off {} maxY {} maxX {}",
                entry.jade_pages,
                entry.word_pages,
                entry.matched_rows,
                entry.word_rows,
                entry.rows_off,
                entry.max_baseline_pt,
                entry.max_x_pt
            ),
            Err(error) => println!("{fixture}: ERROR {error}"),
        }
        report.push((
            fixture.clone(),
            match entry {
                Ok(comparison) => Entry::Compared(comparison),
                Err(error) => Entry::Failed {
                    error: error.chars().take(300).collect(),
                },
            },
        ));
    }

    let write = |path: &Path, value: &serde_json::Value| -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        let json = serde_json::to_string_pretty(value).map_err(|error| error.to_string())?;
        std::fs::write(path, format!("{json}\n"))
            .map_err(|error| format!("{}: {error}", path.display()))
    };
    let report_map: serde_json::Map<String, serde_json::Value> = report
        .iter()
        .map(|(fixture, entry)| Ok((fixture.clone(), serde_json::to_value(entry)?)))
        .collect::<Result<_, serde_json::Error>>()
        .map_err(|error| error.to_string())?;
    write(&arguments.out, &serde_json::Value::Object(report_map))?;
    println!("report: {}", arguments.out.display());
    let summary = summarize(&report, platform, &arguments.fonts);
    let summary_path = PathBuf::from(format!("{}.summary.json", arguments.out.display()));
    write(
        &summary_path,
        &serde_json::to_value(&summary).map_err(|e| e.to_string())?,
    )?;
    println!(
        "summary: {}",
        serde_json::to_string(&summary).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
