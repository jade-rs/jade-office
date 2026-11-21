//! Copy and paste that keep the formatting (元の書式を保持), over any
//! [`WordSession`].
//!
//! A copy takes the selection's runs with their character formatting and
//! each paragraph's style and alignment; a paste puts them back as one undo
//! step — text, paragraph breaks, then each run's format where it differs
//! from what the text would take on where it lands. The plain text goes on
//! the system clipboard as before; a shell keeps the copy beside it and
//! pastes it whenever the clipboard still holds that text.

use jade_docx_headless_api::{
    HeadlessCommand, HeadlessQuery, HeadlessQueryResult, HeadlessRuby, HeadlessRunStyle,
    HeadlessTableCellSelection,
};

use jade_docx_editor::commands::hyperlink::InsertHyperlinkCommand;
use jade_docx_editor::commands::insert_field::InsertFieldCommand;
use jade_docx_editor::commands::insert_image::InsertImageCommand;
use jade_docx_editor::document::RunFieldKind;
use jade_selection::{SelectionRange, TextPosition};

use super::asian::ruby_alignment;
use super::references::edit_document;
use super::{query, Position, WordFormatting, WordSession};

/// A run of copied text and its character formatting.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipRun {
    pub text: String,
    pub style: HeadlessRunStyle,
}

/// A copied paragraph, or the part of one the selection held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipParagraph {
    pub style_id: Option<String>,
    pub alignment: Option<String>,
    pub runs: Vec<ClipRun>,
}

/// A footnote or endnote whose reference mark a copy took: Word pastes a
/// new note of the same kind and text where the mark lands.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipNote {
    /// Which of the copy's paragraphs the mark is in.
    pub paragraph: usize,
    /// The mark's byte offset in that paragraph's copied text.
    pub offset: usize,
    pub footnote: bool,
    pub text: String,
    /// Where the page drew it, which orders it among objects at the same
    /// offset.
    pub shown: usize,
    /// The note's text with its formatting, when it is one paragraph; the
    /// paste gives the new note that formatting.
    pub runs: Vec<ClipRun>,
}

/// A picture in line with the text that a copy took, with its bytes, so
/// the paste puts in one of its own as Word does.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipPicture {
    /// Which of the copy's paragraphs it is in.
    pub paragraph: usize,
    /// Its byte offset in that paragraph's copied text.
    pub offset: usize,
    pub bytes: Vec<u8>,
    pub mime: String,
    /// Its size on the page, in CSS px.
    pub width: f64,
    pub height: f64,
    pub title: Option<String>,
    pub description: Option<String>,
    /// Where the page drew it.
    pub shown: usize,
}

/// A field whose result a copy took whole: Word pastes it as a field of
/// the same kind, which then shows its own value where it lands.
#[derive(Clone, Debug, PartialEq)]
pub struct ClipField {
    /// Which of the copy's paragraphs it is in.
    pub paragraph: usize,
    /// Its result's byte offset in that paragraph's copied text.
    pub offset: usize,
    /// Its saved result, as the copied text holds it.
    pub text: String,
    pub kind: RunFieldKind,
}

/// A hyperlink on text a copy took: Word pastes the text with its link.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipLink {
    /// Which of the copy's paragraphs it is in.
    pub paragraph: usize,
    /// The byte range of that paragraph's copied text it covers.
    pub offset: usize,
    pub len: usize,
    /// A web or mail address, or `#name` for a bookmark.
    pub address: String,
    pub tooltip: Option<String>,
}

/// A ルビ (phonetic guide) over text a copy took whole: Word pastes the
/// text with its guide.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipRuby {
    /// Which of the copy's paragraphs it is in.
    pub paragraph: usize,
    /// The byte range of that paragraph's copied text its base covers.
    pub offset: usize,
    pub len: usize,
    pub ruby: HeadlessRuby,
}

/// What a copy took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichClip {
    pub paragraphs: Vec<ClipParagraph>,
    /// Guides over the copied text, in document order.
    pub rubies: Vec<ClipRuby>,
    /// Links on the copied text, in document order.
    pub links: Vec<ClipLink>,
    /// Fields the copy took whole, in document order.
    pub fields: Vec<ClipField>,
    /// Reference marks the copy took, in document order.
    pub notes: Vec<ClipNote>,
    /// Pictures in line the copy took, in document order.
    pub pictures: Vec<ClipPicture>,
    /// Whole rows of tables the copy took, in document order.
    pub tables: Vec<ClipTable>,
}

/// Whole rows of a table a copy took, a paragraph a cell: the paste makes
/// them a table again, as Word pastes a table as a table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipTable {
    /// The copy's paragraph the first cell is.
    pub first: usize,
    /// How many of the copy's paragraphs — cells — it takes.
    pub cells: usize,
    pub columns: usize,
    /// The table style it had, which the pasted table takes too.
    pub style_id: Option<String>,
    /// How many paragraphs each cell holds, in order; empty when each holds
    /// one. A paste joins a cell's paragraphs to make the table and splits
    /// them again in it.
    pub cell_paragraphs: Vec<usize>,
    /// Where a paste splits the joined cells again: a copy's paragraph and a
    /// byte offset in its text, in order.
    pub splits: Vec<(usize, usize)>,
    /// Its grid columns' widths in points; empty when the table names none.
    pub widths: Vec<f32>,
    /// The cells' own shading (`#RRGGBB`): row (from the table's first
    /// copied row), grid column, colour.
    pub shading: Vec<(usize, usize, String)>,
    /// How many grid columns each cell spans, in order; empty when each
    /// spans one.
    pub cell_spans: Vec<usize>,
    /// The merged cells: row, grid column, columns and rows they cover.
    pub merges: Vec<(usize, usize, usize, usize)>,
}

/// One of the things a paste puts in after the text: a note or a picture.
enum ClipObject<'a> {
    Note(&'a ClipNote),
    Picture(&'a ClipPicture),
}

impl ClipObject<'_> {
    fn at(&self) -> (usize, usize, usize) {
        match self {
            ClipObject::Note(note) => (note.paragraph, note.offset, note.shown),
            ClipObject::Picture(picture) => (picture.paragraph, picture.offset, picture.shown),
        }
    }
}

impl RichClip {
    /// The notes and pictures, in the order the copy met them.
    fn objects(&self) -> Vec<ClipObject<'_>> {
        let mut objects: Vec<ClipObject<'_>> = self
            .notes
            .iter()
            .map(ClipObject::Note)
            .chain(self.pictures.iter().map(ClipObject::Picture))
            .collect();
        objects.sort_by_key(ClipObject::at);
        objects
    }
}

/// Whether a copied field of `kind` pastes as a field: a hyperlink goes
/// back as a link on its text instead.
fn pastes_as_field(kind: &RunFieldKind) -> bool {
    !matches!(kind.base_kind(), RunFieldKind::Hyperlink { .. })
}

/// Whether `text` can be a pasted field's result, as Word's field
/// insertion takes one.
fn fits_a_field(text: &str) -> bool {
    !text.is_empty() && text.chars().count() <= 1024 && !text.chars().any(char::is_control)
}

/// A field a copy took at what it shows, its run holding no text: its
/// offset in the copied paragraph, the text it shows, where on the page
/// that text ends, and which of the copy's fields it is.
type Splice = (usize, String, usize, usize);

/// Puts the text the `splices` fields show into copied paragraph
/// `paragraph`'s `runs`, in the style of the text before it, and moves the
/// notes, pictures, fields and spans (links and guides: offset and
/// length) after each along by it.
fn splice_shown_fields(
    runs: &mut Vec<ClipRun>,
    splices: &[Splice],
    paragraph: usize,
    fields: &mut [ClipField],
    spans: &mut [(&mut usize, &mut usize)],
    notes: &mut [ClipNote],
    pictures: &mut [ClipPicture],
) {
    // Last first, so each goes in at its offset in the text as copied.
    for (offset, shown, end, field) in splices.iter().rev() {
        let mut at = 0;
        let mut placed = false;
        for run in runs.iter_mut() {
            let len = run.text.len();
            if *offset <= at + len && run.text.is_char_boundary(offset - at) {
                run.text.insert_str(offset - at, shown);
                placed = true;
                break;
            }
            at += len;
        }
        if !placed {
            runs.push(ClipRun {
                text: shown.clone(),
                style: runs.last().map(|run| run.style.clone()).unwrap_or_default(),
            });
        }
        let after = |at: usize, drawn: usize| at > *offset || (at == *offset && drawn >= *end);
        for note in notes.iter_mut().filter(|note| note.paragraph == paragraph) {
            if after(note.offset, note.shown) {
                note.offset += shown.len();
            }
        }
        for picture in pictures
            .iter_mut()
            .filter(|picture| picture.paragraph == paragraph)
        {
            if after(picture.offset, picture.shown) {
                picture.offset += shown.len();
            }
        }
        for (start, len) in spans.iter_mut() {
            if **start >= *offset {
                **start += shown.len();
            } else if **start + **len > *offset {
                **len += shown.len();
            }
        }
        for (index, other) in fields.iter_mut().enumerate() {
            if other.paragraph == paragraph
                && index != *field
                && (other.offset > *offset || (other.offset == *offset && index > *field))
            {
                other.offset += shown.len();
            }
        }
    }
}

/// The surface of paragraph `paragraph` of a footnote (or endnote) `id`.
fn note_surface(
    document: &jade_docx_editor::word_document::WordDocument,
    footnote: bool,
    id: &str,
    paragraph: usize,
) -> Option<usize> {
    use jade_docx_editor::word_document::ParagraphStory;
    let note_id = id.to_string();
    document.paragraph_story_surface(&if footnote {
        ParagraphStory::Footnote { note_id, paragraph }
    } else {
        ParagraphStory::Endnote { note_id, paragraph }
    })
}

/// A note's text with its formatting, when the note is one paragraph.
fn note_runs(
    document: &jade_docx_editor::word_document::WordDocument,
    footnote: bool,
    id: &str,
) -> Vec<ClipRun> {
    if note_surface(document, footnote, id, 1).is_some() {
        return Vec::new();
    }
    let Some(surface) = note_surface(document, footnote, id, 0) else {
        return Vec::new();
    };
    let Some(HeadlessQueryResult::Paragraph {
        value: Some(snapshot),
        ..
    }) = query(document, HeadlessQuery::Paragraph { paragraph: surface })
    else {
        return Vec::new();
    };
    snapshot
        .runs
        .iter()
        .filter_map(|run| {
            let text = snapshot.text.get(run.start_offset..run.end_offset)?;
            (!text.is_empty()).then(|| ClipRun {
                text: text.to_string(),
                style: run.style.clone(),
            })
        })
        .collect()
}

/// The whole rows of `table` from `first_row` that `available` paragraphs
/// hold, as a copy takes them, and how many paragraphs they are. A row is
/// taken when its cells hold only paragraphs and cover its grid; vertically
/// merged cells continue in the rows below.
fn copied_rows(
    table: &jade_docx_reader::document::Table,
    first_row: usize,
    available: usize,
) -> Option<(ClipTable, usize)> {
    use jade_docx_reader::document::BodyElement;
    let columns = table.columns.len();
    if columns == 0 {
        return None;
    }
    let mut taken = 0;
    let mut rows: Vec<Vec<(usize, &jade_docx_reader::document::TableCell)>> = Vec::new();
    for row in table.rows.iter().skip(first_row) {
        if row.grid_before.unwrap_or(0) > 0 || row.grid_after.unwrap_or(0) > 0 {
            break;
        }
        let mut column = 0;
        let mut cells = Vec::new();
        let mut paragraphs = 0;
        for cell in &row.cells {
            if !cell
                .blocks
                .iter()
                .all(|block| matches!(block, BodyElement::Paragraph(_)))
            {
                return None;
            }
            paragraphs += cell.blocks.len().max(1);
            cells.push((column, cell));
            column += cell.span.unwrap_or(1).max(1) as usize;
        }
        if column != columns || taken + paragraphs > available {
            break;
        }
        taken += paragraphs;
        rows.push(cells);
    }
    if rows.is_empty() {
        return None;
    }
    let mut counts = Vec::new();
    let mut spans = Vec::new();
    let mut shading = Vec::new();
    let mut merges = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        for &(column, cell) in row {
            counts.push(cell.blocks.len().max(1));
            let span = cell.span.unwrap_or(1).max(1) as usize;
            spans.push(span);
            if let Some(color) = cell
                .style
                .get("background-color")
                .filter(|color| color.starts_with('#') && color.len() == 7)
            {
                shading.push((row_index, column, color.clone()));
            }
            let mut row_span = 1;
            if cell.vertical_merge.as_deref() == Some("restart") {
                for below in &rows[row_index + 1..] {
                    let continues = below.iter().any(|&(other_column, other)| {
                        other_column == column
                            && other.vertical_merge.as_deref() == Some("continue")
                    });
                    if !continues {
                        break;
                    }
                    row_span += 1;
                }
            }
            if span > 1 || row_span > 1 {
                merges.push((row_index, column, span, row_span));
            }
        }
    }
    let widths = table
        .columns
        .iter()
        .map(|column| column.width)
        .collect::<Option<Vec<f32>>>()
        .unwrap_or_default();
    Some((
        ClipTable {
            first: 0,
            cells: taken,
            columns,
            style_id: table.style_name.clone(),
            cell_paragraphs: if counts.iter().all(|&count| count == 1) {
                Vec::new()
            } else {
                counts
            },
            splits: Vec::new(),
            widths,
            shading,
            cell_spans: if spans.iter().all(|&span| span == 1) {
                Vec::new()
            } else {
                spans
            },
            merges,
        },
        taken,
    ))
}

impl RichClip {
    /// How many paragraphs a paste adds back by splitting joined cells.
    fn split_count(&self) -> usize {
        self.tables.iter().map(|table| table.splits.len()).sum()
    }
}

/// Joins the paragraphs of each of `clip`'s table cells that holds several
/// into one, so the cells make a table a paragraph a cell, and notes where
/// to split them again. Notes, pictures and the tables after it move with
/// the paragraphs.
fn join_cell_paragraphs(clip: &mut RichClip) {
    // Each copied paragraph's new index and the offset its text starts at.
    let mut map: Vec<(usize, usize)> = (0..clip.paragraphs.len()).map(|index| (index, 0)).collect();
    let mut joined: Vec<ClipParagraph> = Vec::new();
    let mut source = 0;
    let mut tables = clip.tables.clone();
    for table in &mut tables {
        while source < table.first {
            map[source] = (joined.len(), 0);
            joined.push(clip.paragraphs[source].clone());
            source += 1;
        }
        let first = joined.len();
        let counts = if table.cell_paragraphs.is_empty() {
            vec![1; table.cells]
        } else {
            table.cell_paragraphs.clone()
        };
        let mut grid_cells = 0;
        for (cell, count) in counts.iter().enumerate() {
            let index = joined.len();
            let mut joined_cell = clip.paragraphs[source].clone();
            map[source] = (index, 0);
            source += 1;
            for _ in 1..*count {
                let offset: usize = joined_cell.runs.iter().map(|run| run.text.len()).sum();
                table.splits.push((index, offset));
                map[source] = (index, offset);
                joined_cell
                    .runs
                    .extend(clip.paragraphs[source].runs.iter().cloned());
                source += 1;
            }
            joined.push(joined_cell);
            // A cell spanning columns takes empty ones beside it, which the
            // merge leaves out again.
            let span = table.cell_spans.get(cell).copied().unwrap_or(1);
            for _ in 1..span {
                joined.push(ClipParagraph::default());
            }
            grid_cells += span;
        }
        table.first = first;
        table.cells = grid_cells;
        table.cell_paragraphs.clear();
        table.cell_spans.clear();
    }
    while source < clip.paragraphs.len() {
        map[source] = (joined.len(), 0);
        joined.push(clip.paragraphs[source].clone());
        source += 1;
    }
    for note in &mut clip.notes {
        let (paragraph, offset) = map[note.paragraph];
        (note.paragraph, note.offset) = (paragraph, offset + note.offset);
    }
    for picture in &mut clip.pictures {
        let (paragraph, offset) = map[picture.paragraph];
        (picture.paragraph, picture.offset) = (paragraph, offset + picture.offset);
    }
    clip.paragraphs = joined;
    clip.tables = tables;
}

/// The bytes of a `data:` URI's base64 payload, and its type.
fn decode_data_uri(uri: &str) -> Option<(String, Vec<u8>)> {
    let rest = uri.strip_prefix("data:")?;
    let (header, payload) = rest.split_once(',')?;
    let mime = header.strip_suffix(";base64")?.to_string();
    let mut bytes = Vec::with_capacity(payload.len() * 3 / 4);
    let (mut buffer, mut bits) = (0u32, 0u32);
    for byte in payload.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' | b'\n' | b'\r' | b' ' => continue,
            _ => return None,
        };
        buffer = (buffer << 6) | u32::from(value);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some((mime, bytes))
}

impl RichClip {
    /// The text the system clipboard gets: paragraphs joined by newlines,
    /// which a plain paste turns back into paragraph breaks.
    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|paragraph| {
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether `text` — what the system clipboard holds now — is this copy's,
    /// so pasting it should keep the formatting. Line endings may have been
    /// rewritten on the way.
    pub fn matches(&self, text: &str) -> bool {
        text.replace("\r\n", "\n").replace('\r', "\n") == self.plain_text()
    }
}

/// The formatting commands that give `start..end` of a paragraph `style`,
/// for each property `style` states and `inherited` does not already have.
fn format_commands(
    paragraph: usize,
    (start, end): (usize, usize),
    style: &HeadlessRunStyle,
    inherited: &HeadlessRunStyle,
) -> Vec<HeadlessCommand> {
    let (start_paragraph, start_offset, end_paragraph, end_offset) =
        (paragraph, start, paragraph, end);
    let mut commands = Vec::new();
    let differs = |wanted: Option<bool>, had: Option<bool>| {
        let wanted = wanted.unwrap_or(false);
        (wanted != had.unwrap_or(false)).then_some(wanted)
    };
    if let Some(enabled) = differs(style.bold, inherited.bold) {
        commands.push(HeadlessCommand::ApplyBold {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.italic, inherited.italic) {
        commands.push(HeadlessCommand::ApplyItalic {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.underline, inherited.underline) {
        commands.push(HeadlessCommand::ApplyUnderline {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            kind: None,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.strike, inherited.strike) {
        commands.push(HeadlessCommand::ApplyStrike {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            double: false,
            enabled,
        });
    }
    if style.baseline != inherited.baseline {
        match style.baseline.as_deref() {
            Some("superscript") => commands.push(HeadlessCommand::ApplySuperscript {
                start_paragraph,
                start_offset,
                end_paragraph,
                end_offset,
                enabled: true,
            }),
            Some("subscript") => commands.push(HeadlessCommand::ApplySubscript {
                start_paragraph,
                start_offset,
                end_paragraph,
                end_offset,
                enabled: true,
            }),
            _ => {
                let off = if inherited.baseline.as_deref() == Some("subscript") {
                    HeadlessCommand::ApplySubscript {
                        start_paragraph,
                        start_offset,
                        end_paragraph,
                        end_offset,
                        enabled: false,
                    }
                } else {
                    HeadlessCommand::ApplySuperscript {
                        start_paragraph,
                        start_offset,
                        end_paragraph,
                        end_offset,
                        enabled: false,
                    }
                };
                commands.push(off);
            }
        }
    }
    if let Some(font_size) = style
        .font_size
        .filter(|size| Some(*size) != inherited.font_size)
    {
        commands.push(HeadlessCommand::SetFontSize {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            font_size,
        });
    }
    if let Some(family) = style
        .font_family
        .as_ref()
        .filter(|family| Some(*family) != inherited.font_family.as_ref())
    {
        commands.push(HeadlessCommand::SetFontFamily {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            font_family: family.clone(),
        });
    }
    if style.color != inherited.color {
        let color = match style.color.as_deref() {
            Some(color) if color.eq_ignore_ascii_case("auto") => "auto".to_string(),
            Some(color) => format!("#{}", color.trim_start_matches('#')),
            None => "auto".to_string(),
        };
        commands.push(HeadlessCommand::SetFontColor {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            color,
        });
    }
    commands
}

/// Word's 貼り付けのオプション: how a paste treats the copy's formatting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteMode {
    /// 元の書式を保持: the copy's character and paragraph formatting.
    KeepSource,
    /// 書式を結合 (貼り付け先の書式に合わせる, ⌥⇧⌘V): the destination's
    /// formatting, keeping the copy's bold, italic, underline, strikethrough
    /// and super/subscript.
    MergeFormatting,
    /// テキストのみ保持: the text alone, formatted as typing there would be.
    TextOnly,
}

impl PasteMode {
    pub const ALL: [PasteMode; 3] = [
        PasteMode::KeepSource,
        PasteMode::MergeFormatting,
        PasteMode::TextOnly,
    ];
}

/// The formats 形式を選択して貼り付け lists, and the paste each stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasteFormat {
    /// Microsoft Word 文書.
    WordDocument,
    /// 書式付きテキスト (RTF).
    FormattedText,
    /// HTML 形式.
    Html,
    /// 書式なしテキスト.
    UnformattedText,
    /// 書式なしの Unicode テキスト.
    UnicodeText,
}

impl PasteFormat {
    pub const ALL: [PasteFormat; 5] = [
        PasteFormat::WordDocument,
        PasteFormat::FormattedText,
        PasteFormat::Html,
        PasteFormat::UnformattedText,
        PasteFormat::UnicodeText,
    ];

    /// The formats a clipboard offers: all of them for a copy made here,
    /// only the text ones for text from elsewhere.
    pub fn available(rich: bool) -> Vec<PasteFormat> {
        Self::ALL
            .into_iter()
            .filter(|format| rich || format.mode() == PasteMode::TextOnly)
            .collect()
    }

    pub fn mode(self) -> PasteMode {
        match self {
            PasteFormat::WordDocument | PasteFormat::FormattedText | PasteFormat::Html => {
                PasteMode::KeepSource
            }
            PasteFormat::UnformattedText | PasteFormat::UnicodeText => PasteMode::TextOnly,
        }
    }
}

impl RichClip {
    /// The copy as a paste in `mode` would put it in, given what typing
    /// at the destination would take on (`inherited`).
    pub fn for_mode(&self, mode: PasteMode, inherited: &HeadlessRunStyle) -> RichClip {
        if mode == PasteMode::KeepSource {
            return self.clone();
        }
        let paragraphs = self
            .paragraphs
            .iter()
            .map(|paragraph| ClipParagraph {
                style_id: None,
                alignment: None,
                runs: paragraph
                    .runs
                    .iter()
                    .map(|run| {
                        let mut style = inherited.clone();
                        if mode == PasteMode::MergeFormatting {
                            let keep = |source: Option<bool>, had: Option<bool>| {
                                if source == Some(true) {
                                    Some(true)
                                } else {
                                    had
                                }
                            };
                            style.bold = keep(run.style.bold, style.bold);
                            style.italic = keep(run.style.italic, style.italic);
                            style.underline = keep(run.style.underline, style.underline);
                            style.strike = keep(run.style.strike, style.strike);
                            if run.style.baseline.is_some() {
                                style.baseline = run.style.baseline.clone();
                            }
                        }
                        ClipRun {
                            text: run.text.clone(),
                            style,
                        }
                    })
                    .collect(),
            })
            .collect();
        RichClip {
            paragraphs,
            // テキストのみ保持 takes a field's result as text.
            fields: if mode == PasteMode::TextOnly {
                Vec::new()
            } else {
                self.fields.clone()
            },
            links: if mode == PasteMode::TextOnly {
                Vec::new()
            } else {
                self.links.clone()
            },
            rubies: if mode == PasteMode::TextOnly {
                Vec::new()
            } else {
                self.rubies.clone()
            },
            notes: self.notes.clone(),
            pictures: self.pictures.clone(),
            tables: self.tables.clone(),
        }
    }

    /// Plain text from elsewhere as a copy: one paragraph per line.
    pub fn from_text(text: &str) -> RichClip {
        RichClip {
            paragraphs: text
                .replace("\r\n", "\n")
                .replace('\r', "\n")
                .split('\n')
                .map(|line| ClipParagraph {
                    runs: (!line.is_empty())
                        .then(|| ClipRun {
                            text: line.to_string(),
                            style: HeadlessRunStyle::default(),
                        })
                        .into_iter()
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            notes: Vec::new(),
            pictures: Vec::new(),
            tables: Vec::new(),
            fields: Vec::new(),
            links: Vec::new(),
            rubies: Vec::new(),
        }
    }
}

/// Formatted copy and paste for every session.
pub trait WordClipboard: WordSession {
    /// The selection's text with its formatting; `None` for a caret.
    fn copy_rich(&self) -> Option<RichClip> {
        let range = self
            .selection_range()
            .filter(|range| !range.is_collapsed())?;
        // The selection counts what the page draws — note numbers, fields
        // as they show — and the paragraph's text does not.
        let document = self.word_document();
        let (rendered_start, rendered_end) = range.normalized();
        let (start, end) = (
            document.rendered_to_model_text_position(rendered_start),
            document.rendered_to_model_text_position(rendered_end),
        );
        let footnotes = document.footnote_infos();
        let endnotes = document.endnote_infos();
        let mut notes = Vec::new();
        let mut pictures = Vec::new();
        let mut fields = Vec::new();
        let mut links: Vec<ClipLink> = Vec::new();
        let mut rubies: Vec<ClipRuby> = Vec::new();
        let mut paragraphs = Vec::new();
        for index in start.paragraph..=end.paragraph {
            let Some(HeadlessQueryResult::Paragraph {
                value: Some(snapshot),
                ..
            }) = query(
                self.word_document(),
                HeadlessQuery::Paragraph { paragraph: index },
            )
            else {
                continue;
            };
            let from = if index == start.paragraph {
                start.offset
            } else {
                0
            };
            let to = if index == end.paragraph {
                end.offset
            } else {
                snapshot.text.len()
            };
            // The reference marks drawn wholly inside the selection.
            let (shown_from, shown_to) = (
                if index == rendered_start.paragraph {
                    rendered_start.offset
                } else {
                    0
                },
                if index == rendered_end.paragraph {
                    rendered_end.offset
                } else {
                    usize::MAX
                },
            );
            let mut shown = 0;
            for span in document.reference_marker_spans(index) {
                let at = span.model_offset + shown;
                shown += span.rendered_len;
                if at < shown_from || at + span.rendered_len > shown_to {
                    continue;
                }
                let infos = if span.is_footnote {
                    &footnotes
                } else {
                    &endnotes
                };
                notes.push(ClipNote {
                    paragraph: paragraphs.len(),
                    offset: span.model_offset.saturating_sub(from),
                    shown: at,
                    footnote: span.is_footnote,
                    text: infos
                        .iter()
                        .find(|note| note.id == span.note_id)
                        .map(|note| note.text.clone())
                        .unwrap_or_default(),
                    runs: note_runs(document, span.is_footnote, &span.note_id),
                });
            }
            // The fields whose results it holds whole. A field that keeps no
            // text of its own (a header's PAGE, its digits drawn by layout)
            // is taken at what it shows, and that goes into the copied text.
            let mut splices: Vec<Splice> = Vec::new();
            if let Some(model) = document.paragraph(index) {
                // Taken out of the lock: positions below take it again.
                let lengths = document
                    .rendered_field_lengths()
                    .lock()
                    .map(|lengths| lengths.clone())
                    .unwrap_or_default();
                let mut run_start = 0;
                for run in &model.runs {
                    let run_end = run_start + run.text.len();
                    let Some(field) = run
                        .field
                        .as_ref()
                        .filter(|field| pastes_as_field(&field.kind))
                    else {
                        run_start = run_end;
                        continue;
                    };
                    let text = if run.text.is_empty() {
                        let shown = run
                            .node_id
                            .and_then(|id| lengths.get(&id).cloned())
                            .unwrap_or_default();
                        let at = document
                            .model_to_rendered_text_position(TextPosition::new(index, run_start))
                            .offset;
                        let end = at + shown.len();
                        let within = from <= run_start
                            && run_start <= to
                            && shown_from <= at
                            && end <= shown_to;
                        // An XE, TA or RD shows nothing: it goes as its code.
                        if shown.is_empty()
                            && within
                            && matches!(field.kind.base_kind(), RunFieldKind::NonDisplay { .. })
                        {
                            fields.push(ClipField {
                                paragraph: paragraphs.len(),
                                offset: run_start - from,
                                text: String::new(),
                                kind: field.kind.clone(),
                            });
                            run_start = run_end;
                            continue;
                        }
                        if !fits_a_field(&shown)
                            || run_start < from
                            || to < run_start
                            || at < shown_from
                            || end > shown_to
                        {
                            run_start = run_end;
                            continue;
                        }
                        splices.push((run_start - from, shown.clone(), end, fields.len()));
                        shown
                    } else if from <= run_start && run_end <= to {
                        run.text.clone()
                    } else {
                        run_start = run_end;
                        continue;
                    };
                    if fits_a_field(&text) {
                        fields.push(ClipField {
                            paragraph: paragraphs.len(),
                            offset: run_start - from,
                            text,
                            kind: field.kind.clone(),
                        });
                    }
                    run_start = run_end;
                }
            }
            // The guides over bases it holds whole: a base is the runs a
            // guide continues over.
            if let Some(model) = document.paragraph(index) {
                let mut run_start = 0;
                let mut open: Option<(usize, usize, HeadlessRuby)> = None;
                let mut bases = Vec::new();
                for run in &model.runs {
                    let run_end = run_start + run.text.len();
                    let ruby = run
                        .properties
                        .as_ref()
                        .and_then(|properties| properties.ruby.as_ref());
                    match (ruby, open.as_mut()) {
                        (Some(ruby), Some((_, end, _)))
                            if ruby.continuation && *end == run_start =>
                        {
                            *end = run_end;
                        }
                        (ruby, _) => {
                            bases.extend(open.take());
                            open = ruby.map(|ruby| {
                                (
                                    run_start,
                                    run_end,
                                    HeadlessRuby {
                                        text: ruby.text.clone(),
                                        align: ruby.align.as_deref().map(ruby_alignment),
                                        size: ruby.size_pt,
                                        raise: ruby.raise_half_points.map(|half| half as f32 / 2.0),
                                        font: ruby.fonts.as_ref().and_then(|fonts| {
                                            fonts.east_asia.as_deref().map(str::to_string)
                                        }),
                                    },
                                )
                            });
                        }
                    }
                    run_start = run_end;
                }
                bases.extend(open);
                for (start, end, ruby) in bases {
                    if from <= start && end <= to && start < end && !ruby.text.is_empty() {
                        rubies.push(ClipRuby {
                            paragraph: paragraphs.len(),
                            offset: start - from,
                            len: end - start,
                            ruby,
                        });
                    }
                }
            }
            // The links on it, a link the runs that touch and go to the
            // same place.
            if let Some(model) = document.paragraph(index) {
                let mut run_start = 0;
                for run in &model.runs {
                    let run_end = run_start + run.text.len();
                    let (start, end) = (run_start.max(from), run_end.min(to));
                    run_start = run_end;
                    let Some(link) = run.hyperlink.as_ref() else {
                        continue;
                    };
                    let address = match (&link.target, &link.anchor) {
                        (Some(target), _) if !target.is_empty() => target.clone(),
                        (_, Some(anchor)) if !anchor.is_empty() => format!("#{anchor}"),
                        _ => continue,
                    };
                    if start >= end {
                        continue;
                    }
                    let (offset, len) = (start - from, end - start);
                    match links.last_mut() {
                        Some(last)
                            if last.paragraph == paragraphs.len()
                                && last.address == address
                                && last.offset + last.len == offset =>
                        {
                            last.len += len;
                        }
                        _ => links.push(ClipLink {
                            paragraph: paragraphs.len(),
                            offset,
                            len,
                            address,
                            tooltip: link.tooltip.clone(),
                        }),
                    }
                }
            }
            // The pictures in line drawn inside it, with their bytes.
            if let Some(model) = document.paragraph(index) {
                for (target, model_offset, at) in document.inline_object_rendered_starts(index) {
                    if at < shown_from || at + '\u{fffc}'.len_utf8() > shown_to {
                        continue;
                    }
                    let Some(image) = model
                        .runs
                        .get(target.run_index)
                        .and_then(|run| run.image.as_ref())
                        .filter(|image| {
                            image.chart.is_none()
                                && image.shape_group.is_none()
                                && image.ink.is_none()
                                && image.ole_relationship_id.is_none()
                        })
                    else {
                        continue;
                    };
                    let Some((mime, bytes)) = image.data_uri.as_deref().and_then(decode_data_uri)
                    else {
                        continue;
                    };
                    pictures.push(ClipPicture {
                        paragraph: paragraphs.len(),
                        offset: model_offset.saturating_sub(from),
                        bytes,
                        mime,
                        width: image.width_px.unwrap_or(96.0),
                        height: image.height_px.unwrap_or(96.0),
                        title: image.title.clone(),
                        description: image.description.clone(),
                        shown: at,
                    });
                }
            }
            let mut runs: Vec<ClipRun> = snapshot
                .runs
                .iter()
                .filter_map(|run| {
                    let (run_start, run_end) = (run.start_offset.max(from), run.end_offset.min(to));
                    let text = snapshot.text.get(run_start..run_end)?;
                    (!text.is_empty()).then(|| ClipRun {
                        text: text.to_string(),
                        style: run.style.clone(),
                    })
                })
                .collect();
            // A paragraph whose runs the snapshot does not cover still copies
            // its text.
            let covered: usize = runs.iter().map(|run| run.text.len()).sum();
            if covered == 0 {
                if let Some(text) = snapshot.text.get(from..to.max(from)) {
                    if !text.is_empty() {
                        runs.push(ClipRun {
                            text: text.to_string(),
                            style: HeadlessRunStyle::default(),
                        });
                    }
                }
            }
            let here = paragraphs.len();
            let mut spans: Vec<(&mut usize, &mut usize)> = links
                .iter_mut()
                .filter(|link| link.paragraph == here)
                .map(|link| (&mut link.offset, &mut link.len))
                .chain(
                    rubies
                        .iter_mut()
                        .filter(|ruby| ruby.paragraph == here)
                        .map(|ruby| (&mut ruby.offset, &mut ruby.len)),
                )
                .collect();
            splice_shown_fields(
                &mut runs,
                &splices,
                here,
                &mut fields,
                &mut spans,
                &mut notes,
                &mut pictures,
            );
            paragraphs.push(ClipParagraph {
                style_id: snapshot.style_id.clone(),
                alignment: snapshot.alignment.clone(),
                runs,
            });
        }
        let tables = self.copied_tables(start.paragraph, end.paragraph);
        Some(RichClip {
            paragraphs,
            fields,
            links,
            rubies,
            notes,
            pictures,
            tables,
        })
    }

    /// The runs of whole table rows among paragraphs `first..=last` — what
    /// a copy of them takes as tables. A row is taken only when every cell
    /// of it is one paragraph and spans one column, so the cells come back
    /// a paragraph a cell; anything else is copied as its paragraphs.
    fn copied_tables(&self, first: usize, last: usize) -> Vec<ClipTable> {
        let mut tables = Vec::new();
        let mut index = first;
        while index <= last {
            let Some(cell) = self
                .cell_id_for_paragraph(index)
                .and_then(|id| self.cell_layout(id))
            else {
                index += 1;
                continue;
            };
            // The table from the row the copy starts in, if it starts there.
            let source = self
                .table_order_for(cell.table_id)
                .and_then(|order| self.word_document().table_by_order(order));
            let starts_row = cell.column == 0 && cell.paragraphs.first() == Some(&index);
            match source.filter(|_| starts_row) {
                Some(source) => match copied_rows(source, cell.row, last - index + 1) {
                    Some((mut table, paragraphs)) => {
                        table.first = index - first;
                        index += paragraphs;
                        tables.push(table);
                    }
                    None => index += 1,
                },
                None => index += 1,
            }
            // Past the rest of this table's paragraphs in the copy.
            while index <= last
                && self
                    .cell_id_for_paragraph(index)
                    .and_then(|id| self.cell_layout(id))
                    .is_some_and(|other| other.table_id == cell.table_id)
            {
                index += 1;
            }
        }
        tables
    }

    /// Pastes `clip` the way `mode` says — 元の書式を保持, 書式を結合 or
    /// テキストのみ保持 — as one undo step. Returns where the caret goes.
    fn paste_as(&mut self, clip: &RichClip, mode: PasteMode) -> Result<Position, String> {
        let range = self.selection_range().ok_or("no caret")?;
        let (start, _) = range.normalized();
        let inherited = self
            .run_style_at((start.paragraph, start.offset.saturating_sub(1)))
            .unwrap_or_default();
        self.paste_rich(&clip.for_mode(mode, &inherited))
    }

    /// Pastes `clip` in place of the selection, or at the caret, as one undo
    /// step. Returns where the caret goes: after what was pasted.
    fn paste_rich(&mut self, clip: &RichClip) -> Result<Position, String> {
        let range = self.selection_range().ok_or("no caret")?;
        let (start, _) = range.normalized();
        let mut commands = Vec::new();
        if !range.is_collapsed() {
            let (start, end) = range.normalized();
            commands.push(HeadlessCommand::ReplaceRange {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                text: String::new(),
            });
        }
        // What typed text would take on where the paste lands: the character
        // before the caret, as typing continues it.
        let inherited = self
            .run_style_at((start.paragraph, start.offset.saturating_sub(1)))
            .unwrap_or_default();
        let model_start = self.model_position((start.paragraph, start.offset));
        let clip = &self.with_room_for_tables(clip, (start.paragraph, start.offset));
        let (at, starts) = push_paste_commands(
            clip,
            (start.paragraph, start.offset),
            &inherited,
            &mut commands,
        );
        if commands.is_empty() && clip.notes.is_empty() && clip.pictures.is_empty() {
            return Ok(at);
        }
        self.paste_commands_and_objects(clip, commands, &starts)?;
        let at = (at.0 + clip.split_count(), at.1);
        Ok(self.after_pasted_objects(clip, model_start, at))
    }

    /// Applies a paste's text `commands`, then puts in the copy's notes and
    /// pictures where the text alone put them (`starts`: where each copied
    /// paragraph went), all as one undo step. They go in last first, so
    /// each lands where the text left it: one moves only what follows it.
    fn paste_commands_and_objects(
        &mut self,
        clip: &RichClip,
        commands: Vec<HeadlessCommand>,
        starts: &[Position],
    ) -> Result<(), String> {
        let objects = clip.objects();
        let styled = clip.tables.iter().any(|table| {
            table.style_id.is_some()
                || !table.widths.is_empty()
                || !table.shading.is_empty()
                || !table.merges.is_empty()
        });
        let splits: Vec<(usize, usize)> = clip
            .tables
            .iter()
            .flat_map(|table| table.splits.iter().copied())
            .collect();
        if objects.is_empty()
            && !styled
            && splits.is_empty()
            && clip.fields.is_empty()
            && clip.links.is_empty()
            && clip.rubies.is_empty()
        {
            return self.apply_commands(commands, false);
        }
        let mark = self.begin_edit_group();
        let result = (|| {
            self.apply_commands(commands, false)?;
            // Each pasted table takes the style, column widths and cell
            // shading the copied one had.
            for table in &clip.tables {
                let Some(&(paragraph, _)) = starts.get(table.first) else {
                    continue;
                };
                let Some(order) = self
                    .cell_id_for_paragraph(paragraph)
                    .and_then(|id| self.cell_layout(id))
                    .and_then(|cell| self.table_order_for(cell.table_id))
                else {
                    continue;
                };
                let mut formats = Vec::new();
                if let Some(style_id) = table.style_id.clone() {
                    formats.push(HeadlessCommand::ApplyTableNamedStyle {
                        table_order_index: order,
                        style_id: Some(style_id),
                    });
                }
                // Widths fix the layout, so only when the new table's differ.
                let made: Vec<Option<f32>> = self
                    .word_document()
                    .table_by_order(order)
                    .map(|made| made.columns.iter().map(|column| column.width).collect())
                    .unwrap_or_default();
                let same = made.len() == table.widths.len()
                    && made
                        .iter()
                        .zip(&table.widths)
                        .all(|(made, width)| made.is_some_and(|made| (made - width).abs() < 0.5));
                if !table.widths.is_empty() && !same {
                    formats.push(HeadlessCommand::SetTableColumnWidths {
                        table_order_index: order,
                        widths: table.widths.clone(),
                    });
                }
                for &(row, column, columns, rows) in &table.merges {
                    formats.push(HeadlessCommand::MergeTableCells {
                        table_order_index: order,
                        selected_cells: (row..row + rows)
                            .flat_map(|row| {
                                (column..column + columns).map(move |column| {
                                    HeadlessTableCellSelection {
                                        row,
                                        column,
                                        column_span: 1,
                                    }
                                })
                            })
                            .collect(),
                    });
                }
                for (row, column, color) in &table.shading {
                    formats.push(HeadlessCommand::SetTableCellShading {
                        table_order_index: order,
                        row: *row,
                        column: *column,
                        color: Some(color.clone()),
                    });
                }
                if !formats.is_empty() {
                    self.apply_commands(formats, false)?;
                }
            }
            // The links go back on their text, then the fields' results
            // become fields again, each last first so the ones before keep
            // their places. Where a field cannot go its result stays text.
            let caret = self.selection_range();
            let mut guides = Vec::new();
            for ruby in clip.rubies.iter().rev() {
                let Some(range) = self.pasted_range(starts, ruby.paragraph, ruby.offset, ruby.len)
                else {
                    continue;
                };
                let (start, end) = range.normalized();
                guides.push(HeadlessCommand::SetRuby {
                    paragraph: start.paragraph,
                    start_offset: start.offset,
                    end_offset: end.offset,
                    ruby: Some(ruby.ruby.clone()),
                });
            }
            if !guides.is_empty() {
                self.apply_commands(guides, false)?;
            }
            for link in clip.links.iter().rev() {
                let Some(range) = self.pasted_range(starts, link.paragraph, link.offset, link.len)
                else {
                    continue;
                };
                self.set_selection_range(Some(range));
                let _ = self.execute_command(
                    Box::new(InsertHyperlinkCommand::new(
                        range,
                        None,
                        link.address.clone(),
                        link.tooltip.clone(),
                    )),
                    false,
                );
            }
            for field in clip.fields.iter().rev() {
                if field.text.is_empty() {
                    self.paste_code_field(starts, field)?;
                    continue;
                }
                let Some(range) =
                    self.pasted_range(starts, field.paragraph, field.offset, field.text.len())
                else {
                    continue;
                };
                self.set_selection_range(Some(range));
                let _ = self.execute_command(
                    Box::new(InsertFieldCommand::of_kind(
                        range,
                        field.kind.clone(),
                        field.text.clone(),
                    )),
                    false,
                );
            }
            if !clip.fields.is_empty() || !clip.links.is_empty() {
                self.set_selection_range(caret);
            }
            for object in objects.iter().rev() {
                let (paragraph, offset, _) = object.at();
                let Some(&(at_paragraph, at_offset)) = starts.get(paragraph) else {
                    continue;
                };
                let (paragraph, offset) = (at_paragraph, at_offset + offset);
                match object {
                    ClipObject::Note(note) => {
                        let text = note.text.clone();
                        self.apply_command(
                            if note.footnote {
                                HeadlessCommand::InsertFootnote {
                                    paragraph,
                                    offset,
                                    text,
                                }
                            } else {
                                HeadlessCommand::InsertEndnote {
                                    paragraph,
                                    offset,
                                    text,
                                }
                            },
                            false,
                        )?;
                        self.format_pasted_note(note, (paragraph, offset))?;
                    }
                    ClipObject::Picture(picture) => {
                        let caret = SelectionRange::collapsed(TextPosition::new(paragraph, offset));
                        self.set_selection_range(Some(caret));
                        self.execute_command(
                            Box::new(InsertImageCommand::new(
                                caret,
                                picture.bytes.clone(),
                                picture.mime.clone(),
                                picture.width,
                                picture.height,
                                picture.title.clone(),
                                picture.description.clone(),
                            )),
                            false,
                        )?;
                    }
                }
            }
            // Last, the cells that held several paragraphs are split again,
            // last first so the ones before keep their places.
            let mut breaks = Vec::new();
            for &(paragraph, offset) in splits.iter().rev() {
                let Some(&(at_paragraph, _)) = starts.get(paragraph) else {
                    continue;
                };
                let at = self
                    .word_document()
                    .model_to_rendered_text_position(TextPosition::new(at_paragraph, offset));
                breaks.push(HeadlessCommand::InsertParagraphBreak {
                    paragraph: at.paragraph,
                    offset: at.offset,
                    count: 1,
                });
            }
            if !breaks.is_empty() {
                self.apply_commands(breaks, false)?;
            }
            Ok(())
        })();
        self.end_edit_group(mark);
        result
    }

    /// Where the caret goes after a paste of `clip` that started at model
    /// position `model_start` and ends at `at` as the text alone would put
    /// it: past the notes' numbers and the pictures it brought into that
    /// paragraph too, and the fields there shown at their own values.
    /// Neither changes the model's text, so the end is found there. The
    /// caret is left there.
    fn after_pasted_objects(
        &mut self,
        clip: &RichClip,
        model_start: Position,
        at: Position,
    ) -> Position {
        let last = clip.paragraphs.len().saturating_sub(1);
        if !clip.objects().iter().any(|object| object.at().0 == last)
            && !clip.fields.iter().any(|field| field.paragraph == last)
        {
            return at;
        }
        let pasted: usize = clip.paragraphs.last().map_or(0, |paragraph| {
            paragraph.runs.iter().map(|run| run.text.len()).sum()
        });
        let at_end = |object: &ClipObject<'_>| {
            let (paragraph, offset, _) = object.at();
            paragraph == last && offset == pasted
        };
        let notes_at_end = clip
            .notes
            .iter()
            .filter(|note| at_end(&ClipObject::Note(note)))
            .count();
        let pictures_at_end = clip
            .pictures
            .iter()
            .filter(|picture| at_end(&ClipObject::Picture(picture)))
            .count();
        let model_end = if last == 0 { model_start.1 } else { 0 } + pasted;
        let document = self.word_document();
        let mut caret = document
            .model_to_rendered_text_position(TextPosition::new(at.0, model_end))
            .offset;
        // Objects at the end itself are drawn before the caret.
        let marks: Vec<usize> = document
            .reference_marker_spans(at.0)
            .into_iter()
            .filter(|span| span.model_offset == model_end)
            .map(|span| span.rendered_len)
            .collect();
        caret += marks.iter().rev().take(notes_at_end).sum::<usize>();
        caret += pictures_at_end * '\u{fffc}'.len_utf8();
        self.set_selection_range(Some(SelectionRange::collapsed(TextPosition::new(
            at.0, caret,
        ))));
        (at.0, caret)
    }

    /// Gives the note just pasted with its mark at `at` the formatting the
    /// copied one had.
    fn format_pasted_note(&mut self, note: &ClipNote, at: Position) -> Result<(), String> {
        if note.runs.is_empty() {
            return Ok(());
        }
        let document = self.word_document();
        let mut shown = 0;
        let Some(id) = document
            .reference_marker_spans(at.0)
            .into_iter()
            .find_map(|span| {
                let start = span.model_offset + shown;
                shown += span.rendered_len;
                (start == at.1).then_some(span.note_id)
            })
        else {
            return Ok(());
        };
        let Some(surface) = note_surface(document, note.footnote, &id, 0) else {
            return Ok(());
        };
        let Some(HeadlessQueryResult::Paragraph {
            value: Some(snapshot),
            ..
        }) = query(document, HeadlessQuery::Paragraph { paragraph: surface })
        else {
            return Ok(());
        };
        let joined: String = note.runs.iter().map(|run| run.text.as_str()).collect();
        let Some(mut model) = snapshot.text.find(&joined) else {
            return Ok(());
        };
        let mut commands = Vec::new();
        for run in &note.runs {
            let rendered = |offset: usize| {
                document
                    .model_to_rendered_text_position(TextPosition::new(surface, offset))
                    .offset
            };
            let (start, end) = (rendered(model), rendered(model + run.text.len()));
            model += run.text.len();
            if start == end {
                continue;
            }
            let inherited = self.run_style_at((surface, start)).unwrap_or_default();
            commands.extend(format_commands(
                surface,
                (start, end),
                &run.style,
                &inherited,
            ));
        }
        if commands.is_empty() {
            return Ok(());
        }
        self.apply_commands(commands, false)
    }

    /// `clip` as it can be pasted at `at`: a table that would share its
    /// first cell with the text before the caret, or its last with the text
    /// after it, gets a paragraph of its own there, as Word puts a pasted
    /// table on lines of its own. In a table cell, where Word would nest
    /// it, the cells come in as paragraphs.
    fn with_room_for_tables(&self, clip: &RichClip, at: Position) -> RichClip {
        let mut clip = clip.clone();
        if clip.tables.is_empty() {
            return clip;
        }
        if self.cell_id_for_paragraph(at.0).is_some() {
            clip.tables.clear();
            return clip;
        }
        if clip.tables[0].first == 0 && at.1 > 0 {
            clip.paragraphs.insert(0, ClipParagraph::default());
            for table in &mut clip.tables {
                table.first += 1;
            }
            for note in &mut clip.notes {
                note.paragraph += 1;
            }
            for picture in &mut clip.pictures {
                picture.paragraph += 1;
            }
        }
        let ends_in_table = clip
            .tables
            .last()
            .is_some_and(|table| table.first + table.cells == clip.paragraphs.len());
        if ends_in_table {
            clip.paragraphs.push(ClipParagraph::default());
        }
        join_cell_paragraphs(&mut clip);
        clip
    }

    /// The range a paste put `len` bytes of the copy's paragraph
    /// `paragraph` at `offset` in, as the selection counts it (`starts`:
    /// where each copied paragraph went).
    fn pasted_range(
        &self,
        starts: &[Position],
        paragraph: usize,
        offset: usize,
        len: usize,
    ) -> Option<SelectionRange> {
        let &(at_paragraph, at_offset) = starts.get(paragraph)?;
        let document = self.word_document();
        let start =
            document.rendered_to_model_text_position(TextPosition::new(at_paragraph, at_offset));
        let from = start.offset + offset;
        Some(SelectionRange::new(
            document.model_to_rendered_text_position(TextPosition::new(start.paragraph, from)),
            document
                .model_to_rendered_text_position(TextPosition::new(start.paragraph, from + len)),
        ))
    }

    /// Puts in a field that shows nothing (an XE, TA or RD) where the
    /// paste put the text it was copied in.
    fn paste_code_field(&mut self, starts: &[Position], field: &ClipField) -> Result<(), String> {
        let RunFieldKind::NonDisplay { instruction } = field.kind.base_kind() else {
            return Ok(());
        };
        let Some(range) = self.pasted_range(starts, field.paragraph, field.offset, 0) else {
            return Ok(());
        };
        let (at, instruction) = (range.normalized().0, instruction.clone());
        edit_document(self, "paste_field", true, move |document, _| {
            document
                .insert_inline_field(at, &instruction, "")
                .map(|_| ())
        })
    }

    /// `position` in model offsets, which pasted notes do not move.
    fn model_position(&self, position: Position) -> Position {
        let model = self
            .word_document()
            .rendered_to_model_text_position(TextPosition::new(position.0, position.1));
        (model.paragraph, model.offset)
    }

    /// Drag and drop: `clip` dropped at `target`, and — for a move — the
    /// range it was dragged from taken out, as one undo step. `target` is
    /// where the pointer let go in the document as it was; returns where
    /// the dropped text ends.
    fn drop_rich(
        &mut self,
        clip: &RichClip,
        moved_from: Option<(Position, Position)>,
        target: Position,
    ) -> Result<Position, String> {
        let inherited = self
            .run_style_at((target.0, target.1.saturating_sub(1)))
            .unwrap_or_default();
        let mut commands = Vec::new();
        let mut at = target;
        let mut model_at = self.model_position(target);
        if let Some((start, end)) = moved_from {
            let (start, end) = if start <= end {
                (start, end)
            } else {
                (end, start)
            };
            if target > start && target < end {
                return Err("dropped inside what was dragged".to_string());
            }
            commands.push(HeadlessCommand::ReplaceRange {
                start_paragraph: start.0,
                start_offset: start.1,
                end_paragraph: end.0,
                end_offset: end.1,
                text: String::new(),
            });
            at = position_after_removal(target, start, end);
            model_at = position_after_removal(
                model_at,
                self.model_position(start),
                self.model_position(end),
            );
        }
        let clip = &self.with_room_for_tables(clip, target);
        let (end, starts) = push_paste_commands(clip, at, &inherited, &mut commands);
        self.paste_commands_and_objects(clip, commands, &starts)?;
        let end = (end.0 + clip.split_count(), end.1);
        Ok(self.after_pasted_objects(clip, model_at, end))
    }
}

/// Where `target` is once `start..end` has been taken out before it.
pub fn position_after_removal(target: Position, start: Position, end: Position) -> Position {
    if target <= start {
        target
    } else if target.0 == end.0 {
        (start.0, start.1 + target.1.saturating_sub(end.1))
    } else {
        (target.0 - (end.0 - start.0), target.1)
    }
}

/// The commands that put `clip`'s text in at `start`, appended to
/// `commands`; answers where the pasted text ends and where each of the
/// copy's paragraphs starts.
fn push_paste_commands(
    clip: &RichClip,
    start: Position,
    inherited: &HeadlessRunStyle,
    commands: &mut Vec<HeadlessCommand>,
) -> (Position, Vec<Position>) {
    let inherited = inherited.clone();
    let mut at = start;
    let mut formats = Vec::new();
    let mut starts = Vec::new();
    let count = clip.paragraphs.len();
    for (index, paragraph) in clip.paragraphs.iter().enumerate() {
        if index > 0 {
            commands.push(HeadlessCommand::InsertParagraphBreak {
                paragraph: at.0,
                offset: at.1,
                count: 1,
            });
            at = (at.0 + 1, 0);
        }
        starts.push(at);
        // Every copied paragraph but the last came with its mark, and the
        // mark carries the paragraph's style to the paragraph it ends
        // here; the last ends in the destination's own mark.
        if index + 1 < count {
            if let Some(style_id) = &paragraph.style_id {
                formats.push(HeadlessCommand::SetParagraphStyle {
                    start_paragraph: at.0,
                    end_paragraph: at.0,
                    style_id: Some(style_id.clone()),
                });
            }
            if let Some(alignment) = &paragraph.alignment {
                formats.push(HeadlessCommand::SetParagraphAlignment {
                    start_paragraph: at.0,
                    end_paragraph: at.0,
                    alignment: alignment.clone(),
                });
            }
        }
        for run in &paragraph.runs {
            commands.push(HeadlessCommand::InsertText {
                paragraph: at.0,
                offset: at.1,
                text: run.text.clone(),
            });
            let end = at.1 + run.text.len();
            formats.extend(format_commands(at.0, (at.1, end), &run.style, &inherited));
            at.1 = end;
        }
    }
    // Formats after all the text: a split carries a paragraph's direct
    // formatting into the next, so formatting first would be undone.
    commands.extend(formats);
    // Then the tables, from their cells' paragraphs; that keeps the
    // paragraphs where they are, so what follows still finds them.
    for table in &clip.tables {
        let (Some(first), Some(last)) = (
            starts.get(table.first),
            starts.get(table.first + table.cells - 1),
        ) else {
            continue;
        };
        commands.push(HeadlessCommand::ConvertTextToTable {
            start_paragraph: first.0,
            end_paragraph: last.0,
            separator: "paragraph".to_string(),
            columns: Some(table.columns),
        });
    }
    (at, starts)
}

impl<T: WordSession + ?Sized> WordClipboard for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plain_text_joins_paragraphs_with_newlines() {
        let run = |text: &str| ClipRun {
            text: text.to_string(),
            style: HeadlessRunStyle::default(),
        };
        let clip = RichClip {
            notes: Vec::new(),
            pictures: Vec::new(),
            tables: Vec::new(),
            fields: Vec::new(),
            links: Vec::new(),
            rubies: Vec::new(),
            paragraphs: vec![
                ClipParagraph {
                    runs: vec![run("Bold"), run(" plain")],
                    ..Default::default()
                },
                ClipParagraph {
                    runs: vec![run("next")],
                    ..Default::default()
                },
            ],
        };
        assert_eq!(clip.plain_text(), "Bold plain\nnext");
        assert!(clip.matches("Bold plain\r\nnext"));
        assert!(!clip.matches("Bold plain"));
    }

    #[test]
    fn merging_keeps_emphasis_and_text_only_keeps_nothing() {
        let clip = RichClip {
            notes: Vec::new(),
            pictures: Vec::new(),
            tables: Vec::new(),
            fields: Vec::new(),
            links: Vec::new(),
            rubies: Vec::new(),
            paragraphs: vec![ClipParagraph {
                style_id: Some("Heading1".into()),
                alignment: Some("center".into()),
                runs: vec![ClipRun {
                    text: "Bold red".into(),
                    style: HeadlessRunStyle {
                        bold: Some(true),
                        font_size: Some(20.0),
                        color: Some("FF0000".into()),
                        font_family: Some("Arial".into()),
                        ..Default::default()
                    },
                }],
            }],
        };
        let destination = HeadlessRunStyle {
            font_size: Some(10.5),
            font_family: Some("游明朝".into()),
            ..Default::default()
        };
        let merged = clip.for_mode(PasteMode::MergeFormatting, &destination);
        let run = &merged.paragraphs[0].runs[0];
        assert_eq!(run.style.bold, Some(true));
        assert_eq!(run.style.font_size, Some(10.5));
        assert_eq!(run.style.font_family.as_deref(), Some("游明朝"));
        assert_eq!(run.style.color, None);
        assert_eq!(merged.paragraphs[0].style_id, None);
        let plain = clip.for_mode(PasteMode::TextOnly, &destination);
        assert_eq!(plain.paragraphs[0].runs[0].style, destination);
        assert_eq!(clip.for_mode(PasteMode::KeepSource, &destination), clip);
        // Text from elsewhere offers only the text formats.
        assert_eq!(
            PasteFormat::available(false),
            [PasteFormat::UnformattedText, PasteFormat::UnicodeText]
        );
        assert_eq!(RichClip::from_text("a\r\nb").plain_text(), "a\nb");
    }

    #[test]
    fn only_what_differs_is_formatted() {
        let bold = HeadlessRunStyle {
            bold: Some(true),
            font_size: Some(14.0),
            ..Default::default()
        };
        let plain = HeadlessRunStyle {
            font_size: Some(14.0),
            ..Default::default()
        };
        let commands = format_commands(3, (0, 4), &bold, &plain);
        assert_eq!(commands.len(), 1, "{commands:?}");
        assert!(matches!(
            commands[0],
            HeadlessCommand::ApplyBold { enabled: true, .. }
        ));
        // Into bold text, plain text has bold taken off.
        let commands = format_commands(3, (0, 4), &plain, &bold);
        assert!(matches!(
            commands[0],
            HeadlessCommand::ApplyBold { enabled: false, .. }
        ));
    }
}

#[cfg(all(test, feature = "skia"))]
mod native_tests {
    use super::*;
    use crate::test_support::fixture;
    use crate::word::picture::WordPicture;
    use jade_docx_native_render::{FaceSpec, NativeDocument};

    fn open(name: &str) -> NativeDocument {
        let font =
            crate::test_support::core().join("fonts/Noto_Sans_JP/static/NotoSansJP-Regular.ttf");
        let faces = ["Noto Sans JP", "Times New Roman", "serif"].map(|family| FaceSpec {
            family,
            weight: 400,
            path: &font,
        });
        NativeDocument::open(&fixture(name), 816.0, 1056.0, &faces).unwrap()
    }

    fn png() -> Vec<u8> {
        let mut bytes = Vec::new();
        image::RgbaImage::from_raw(2, 1, vec![255, 0, 0, 255, 0, 0, 255, 255])
            .unwrap()
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn pictures(document: &NativeDocument) -> usize {
        let document = WordSession::word_document(document);
        (0..document.paragraph_count())
            .filter_map(|index| document.paragraph(index))
            .flat_map(|paragraph| paragraph.runs.iter())
            .filter(|run| run.image.is_some())
            .count()
    }

    #[test]
    fn a_paste_with_a_picture_undoes_in_one_step_in_the_native_document() {
        let mut document = open("hello.docx");
        let caret = |offset| Some(SelectionRange::collapsed(TextPosition::new(0, offset)));
        WordSession::set_selection_range(&mut document, caret(1));
        document.insert_picture(png()).unwrap();
        assert_eq!(pictures(&document), 1);
        WordSession::set_selection_range(
            &mut document,
            Some(SelectionRange::new(
                TextPosition::new(0, 0),
                TextPosition::new(0, 5),
            )),
        );
        let clip = document.copy_rich().unwrap();
        assert_eq!(clip.pictures.len(), 1);
        let text = WordSession::word_document(&document).rendered_paragraph_text(0);
        WordSession::set_selection_range(&mut document, caret(text.len()));
        let before = document.history_len();
        document.paste_rich(&clip).unwrap();
        assert_eq!(pictures(&document), 2);
        assert_eq!(document.history_len(), before + 1);
        NativeDocument::undo(&mut document).unwrap();
        assert_eq!(pictures(&document), 1);
        assert_eq!(
            WordSession::word_document(&document).rendered_paragraph_text(0),
            text
        );
    }
}
