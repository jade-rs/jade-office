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
    HeadlessCommand, HeadlessQuery, HeadlessQueryResult, HeadlessRunStyle,
};

use jade_docx_editor::commands::insert_image::InsertImageCommand;
use jade_selection::{SelectionRange, TextPosition};

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

/// What a copy took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichClip {
    pub paragraphs: Vec<ClipParagraph>,
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
                });
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
            paragraphs.push(ClipParagraph {
                style_id: snapshot.style_id.clone(),
                alignment: snapshot.alignment.clone(),
                runs,
            });
        }
        let tables = self.copied_tables(start.paragraph, end.paragraph);
        Some(RichClip {
            paragraphs,
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
        let cell_of = |paragraph: usize| {
            self.cell_id_for_paragraph(paragraph)
                .and_then(|id| self.cell_layout(id))
        };
        let mut tables = Vec::new();
        let mut index = first;
        while index <= last {
            let Some(cell) = cell_of(index) else {
                index += 1;
                continue;
            };
            let table_id = cell.table_id;
            let mut end = index;
            while end < last && cell_of(end + 1).is_some_and(|next| next.table_id == table_id) {
                end += 1;
            }
            let columns = self
                .cell_layouts()
                .iter()
                .filter(|other| other.table_id == table_id && other.row == cell.row)
                .count();
            let cells: Vec<_> = (index..=end).filter_map(cell_of).collect();
            let whole_rows = columns > 0
                && cells.len() == end - index + 1
                && cells.len() % columns == 0
                && cells.iter().enumerate().all(|(at, cell)| {
                    cell.column == at % columns
                        && cell.column_span <= 1
                        && cell.paragraphs.len() == 1
                })
                && cells
                    .chunks(columns)
                    .all(|row| row.iter().all(|cell| cell.row == row[0].row));
            if whole_rows {
                tables.push(ClipTable {
                    first: index - first,
                    cells: cells.len(),
                    columns,
                });
            }
            index = end + 1;
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
        if objects.is_empty() {
            return self.apply_commands(commands, false);
        }
        self.begin_edit_group();
        let result = (|| {
            self.apply_commands(commands, false)?;
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
            Ok(())
        })();
        self.end_edit_group();
        result
    }

    /// Where the caret goes after a paste of `clip` that started at model
    /// position `model_start` and ends at `at` as the text alone would put
    /// it: past the notes' numbers and the pictures it brought into that
    /// paragraph too. Neither adds text to the model, so the end is found
    /// there. The caret is left there.
    fn after_pasted_objects(
        &mut self,
        clip: &RichClip,
        model_start: Position,
        at: Position,
    ) -> Position {
        let last = clip.paragraphs.len().saturating_sub(1);
        if !clip.objects().iter().any(|object| object.at().0 == last) {
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
        clip
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
