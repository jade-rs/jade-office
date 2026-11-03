//! ホーム › 選択 › 類似した書式の文字列を選択: every run of the body whose
//! character formatting is the selection's, selected together — a
//! multi-range selection the shells draw and the font commands act on as one
//! undo step ([`format`]).

use jade_docx_editor::canvas_renderer::HyperlinkRegion;
use jade_docx_editor::commands::Command;
use jade_docx_editor::word_document::WordDocument;
use jade_docx_headless_api::{
    HeadlessCommand, HeadlessQuery, HeadlessQueryResult, HeadlessRunStyle,
};

use jade_selection::{SelectionRange, TextPosition};

use super::{query, CellLayout, CharacterFormat, Position, WordFormatting, WordSession};

/// A range of the multi-range selection, start and end.
pub type Range = (Position, Position);

/// The runs of body paragraph `paragraph`, with their formatting, as the
/// engine reports them.
fn runs<S: WordSession + ?Sized>(
    session: &S,
    paragraph: usize,
) -> Vec<(usize, usize, HeadlessRunStyle)> {
    match query(
        session.word_document(),
        HeadlessQuery::Paragraph { paragraph },
    ) {
        Some(HeadlessQueryResult::Paragraph {
            value: Some(snapshot),
            ..
        }) => snapshot
            .runs
            .into_iter()
            .map(|run| (run.start_offset, run.end_offset, run.style))
            .collect(),
        _ => Vec::new(),
    }
}

/// The character formatting the selection starts with: the run after the
/// selection's start, or the one before a caret — read as the paragraph's
/// runs report it, so it compares with every other run.
fn target_style<S: WordSession + ?Sized>(session: &S) -> Option<HeadlessRunStyle> {
    let range = session.selection_range()?;
    let (start, _) = range.normalized();
    let runs = runs(session, start.paragraph);
    let inside = |&&(from, to, _): &&(usize, usize, HeadlessRunStyle)| {
        if range.is_collapsed() {
            from < start.offset && start.offset <= to
        } else {
            from <= start.offset && start.offset < to
        }
    };
    runs.iter()
        .find(inside)
        .or_else(|| runs.iter().find(|(from, to, _)| from < to))
        .map(|(_, _, style)| style.clone())
}

/// 類似した書式の文字列を選択 over every session.
pub trait WordSimilar: WordSession {
    /// The body's ranges formatted as the selection is, in document order;
    /// adjacent runs make one range.
    fn similar_formatting(&self) -> Vec<Range> {
        let Some(target) = target_style(self) else {
            return Vec::new();
        };
        let document = self.word_document();
        let mut ranges: Vec<Range> = Vec::new();
        for paragraph in 0..document.paragraph_count() {
            if !document.is_body_paragraph_surface(paragraph) {
                continue;
            }
            for (from, to, style) in runs(self, paragraph) {
                if from >= to || style != target {
                    continue;
                }
                let start = (paragraph, from);
                match ranges.last_mut() {
                    Some((_, end)) if *end == start => *end = (paragraph, to),
                    _ => ranges.push((start, (paragraph, to))),
                }
            }
        }
        ranges
    }

    /// Selects them: the first becomes the selection; all are returned for
    /// the shell to keep, draw and format.
    fn select_similar_formatting(&mut self) -> Vec<Range> {
        let ranges = self.similar_formatting();
        if let Some(&(start, end)) = ranges.first() {
            self.set_selection_range(Some(SelectionRange::new(
                TextPosition::new(start.0, start.1),
                TextPosition::new(end.0, end.1),
            )));
        }
        ranges
    }
}

impl<T: WordSession + ?Sized> WordSimilar for T {}

/// Whether `ranges` is still the selection: its first range is what is
/// selected. Clicking or typing elsewhere ends a multi-range selection.
pub fn still_selected<S: WordSession + ?Sized>(session: &S, ranges: &[Range]) -> bool {
    let (Some(&(start, end)), Some(range)) = (ranges.first(), session.selection_range()) else {
        return false;
    };
    let (from, to) = range.normalized();
    ranges.len() > 1 && (from.paragraph, from.offset) == start && (to.paragraph, to.offset) == end
}

/// A session that records the commands a formatting call would apply, for
/// one range at a time.
struct Recorder<'a, S: WordSession + ?Sized> {
    inner: &'a S,
    selection: Option<SelectionRange>,
    commands: Vec<HeadlessCommand>,
}

impl<S: WordSession + ?Sized> WordSession for Recorder<'_, S> {
    fn word_document(&self) -> &WordDocument {
        self.inner.word_document()
    }
    fn selection_range(&self) -> Option<SelectionRange> {
        self.selection
    }
    fn set_selection_range(&mut self, range: Option<SelectionRange>) {
        self.selection = range;
    }
    fn apply_command(&mut self, command: HeadlessCommand, _keep: bool) -> Result<(), String> {
        self.commands.push(command);
        Ok(())
    }
    fn apply_commands(
        &mut self,
        commands: Vec<HeadlessCommand>,
        _keep: bool,
    ) -> Result<(), String> {
        self.commands.extend(commands);
        Ok(())
    }
    fn execute_command(&mut self, _command: Box<dyn Command>, _keep: bool) -> Result<(), String> {
        Err("this cannot be done to several ranges at once".to_string())
    }
    fn cell_id_for_paragraph(&self, paragraph: usize) -> Option<usize> {
        self.inner.cell_id_for_paragraph(paragraph)
    }
    fn cell_layout(&self, cell_id: usize) -> Option<&CellLayout> {
        self.inner.cell_layout(cell_id)
    }
    fn cell_layouts(&self) -> &[CellLayout] {
        self.inner.cell_layouts()
    }
    fn table_order_for(&self, table_id: usize) -> Option<usize> {
        self.inner.table_order_for(table_id)
    }
    fn hyperlink_regions(&self) -> Vec<HyperlinkRegion> {
        self.inner.hyperlink_regions()
    }
    fn select_cells(&mut self, _anchor: usize, _focus: usize) -> Result<(), String> {
        Err("this cannot be done to several ranges at once".to_string())
    }
}

/// A font command the multi-range selection takes.
#[derive(Clone, Debug, PartialEq)]
pub enum RangeFormat {
    Toggle(CharacterFormat),
    ClearFormatting,
    FontSize(f32),
    StepFontSize(bool),
    FontFamily(String),
    FontColor(String),
    Highlight(String),
}

impl RangeFormat {
    fn run<S: WordSession + ?Sized>(&self, session: &mut S) -> Result<(), String> {
        match self {
            RangeFormat::Toggle(format) => session.toggle_format(*format),
            RangeFormat::ClearFormatting => session.clear_formatting(),
            RangeFormat::FontSize(points) => session.set_font_size(*points),
            RangeFormat::StepFontSize(grow) => session.step_font_size(*grow),
            RangeFormat::FontFamily(family) => session.set_font_family(family),
            RangeFormat::FontColor(color) => session.set_font_color(color),
            RangeFormat::Highlight(color) => session.set_highlight(color),
        }
    }
}

/// `format` on the multi-range selection `ranges` — every range, as one
/// undo step, each formatted as it would be selected on its own — or, when
/// `ranges` is no longer the selection, on the selection.
pub fn format<S: WordSession + ?Sized>(
    session: &mut S,
    ranges: &[Range],
    format: RangeFormat,
) -> Result<(), String> {
    if !still_selected(session, ranges) {
        return format.run(session);
    }
    let mut commands = Vec::new();
    for &(start, end) in ranges {
        let mut recorder = Recorder {
            inner: &*session,
            selection: Some(SelectionRange::new(
                TextPosition::new(start.0, start.1),
                TextPosition::new(end.0, end.1),
            )),
            commands: Vec::new(),
        };
        format.run(&mut recorder)?;
        commands.extend(recorder.commands);
    }
    if commands.is_empty() {
        return Ok(());
    }
    session.apply_commands(commands, true)
}
