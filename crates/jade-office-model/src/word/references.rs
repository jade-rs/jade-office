//! The 参考資料 tab over any [`WordSession`]: 脚注の挿入 and 文末脚注の
//!挿入 (⌥⌘F and ⌥⌘D in Word for Mac) at the caret — the note's text typed
//! into a bar and put in with its reference as one undo step — and the table
//! of contents, inserted from the headings and updated after they change.

pub mod bookmarks;
pub mod captions;
pub mod citations;
pub mod crossref;
pub mod index;
pub mod navigation;
pub mod notes;
pub mod toc;

use jade_docx_headless_api::HeadlessCommand;

use jade_docx_editor::commands::document_edit::DocumentEditCommand;
use jade_docx_editor::word_document::WordDocument;
use jade_selection::SelectionRange;

use super::{Position, WordSession};
use crate::text::TextEdit;

/// Runs `edit` on the document as one undo step. It is handed the
/// selection to move; with `keep_selection` the selection stays as it was.
pub(crate) fn edit_document<S: WordSession + ?Sized>(
    session: &mut S,
    name: &'static str,
    keep_selection: bool,
    mut edit: impl FnMut(&mut WordDocument, &mut Option<SelectionRange>) -> Result<(), String> + 'static,
) -> Result<(), String> {
    session.execute_command(
        Box::new(DocumentEditCommand::new(
            name,
            Box::new(move |document, selection| edit(document, selection)),
        )),
        keep_selection,
    )
}

/// Which notes: at the foot of the page, or at the end of the document.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoteKind {
    Footnote,
    Endnote,
}

/// A note being typed: where its reference goes, and its text.
pub struct NoteForm {
    pub kind: NoteKind,
    pub at: Position,
    pub text: TextEdit,
    /// 任意の記号: the mark that stands in for the note's number, if one was
    /// typed into the 脚注と文末脚注 dialog.
    pub mark: Option<String>,
}

impl NoteForm {
    /// The note's reference goes where Word puts it: at the caret, or after
    /// the selection.
    pub fn new<S: WordSession + ?Sized>(session: &S, kind: NoteKind) -> Self {
        Self {
            kind,
            at: session.note_anchor(),
            text: TextEdit::new(String::new()),
            mark: None,
        }
    }

    /// The bar for a note whose reference is `mark` rather than a number.
    pub fn with_mark(mut self, mark: &str) -> Self {
        let mark = mark.trim();
        self.mark = (!mark.is_empty()).then(|| mark.to_string());
        self
    }

    /// OK: the reference and the note, one undo step. Returns where the caret
    /// goes: where the reference went.
    pub fn submit<S: WordSession + ?Sized>(&self, session: &mut S) -> Result<Position, String> {
        session.insert_note_marked(
            self.kind,
            self.at,
            self.text.text.trim(),
            self.mark.as_deref(),
        )
    }
}

/// 参考資料 over every session.
pub trait WordReferences: WordSession {
    /// Where a note's reference goes: the caret, or the end of the selection.
    fn note_anchor(&self) -> Position {
        match self.selection_range() {
            Some(range) => {
                let (_, end) = range.normalized();
                (end.paragraph, end.offset)
            }
            None => (0, 0),
        }
    }

    /// Whether paragraph `paragraph` takes a note: one of the main text's,
    /// not a header's, a footer's, a note's or a text box's, where Word
    /// greys out 脚注の挿入.
    fn takes_notes_at(&self, paragraph: usize) -> bool {
        self.is_main_story() && self.word_document().is_body_paragraph_surface(paragraph)
    }

    /// Whether the caret's paragraph takes a note.
    fn can_insert_note(&self) -> bool {
        self.selection_range()
            .is_some_and(|range| self.takes_notes_at(range.normalized().0.paragraph))
    }

    /// Puts a note's reference mark at `at` and the note, holding `text`, in
    /// `footnotes.xml` or `endnotes.xml` (ECMA-376 Part 1 §17.11), one undo
    /// step. Returns where the caret goes: where the reference went.
    fn insert_note(
        &mut self,
        kind: NoteKind,
        at: Position,
        text: &str,
    ) -> Result<Position, String> {
        if !self.takes_notes_at(at.0) {
            return Err("notes go in the main text, not a header, footer, note or text box".into());
        }
        let (paragraph, offset) = at;
        let text = text.to_string();
        let command = match kind {
            NoteKind::Footnote => HeadlessCommand::InsertFootnote {
                paragraph,
                offset,
                text,
            },
            NoteKind::Endnote => HeadlessCommand::InsertEndnote {
                paragraph,
                offset,
                text,
            },
        };
        // The caret goes after the reference mark, as Word leaves it when
        // the note's text is done; the engine leaves its selection in the
        // note.
        self.apply_command(command, false)?;
        let caret = notes::WordNotes::note_marks(self)
            .into_iter()
            .find(|mark| mark.at == at)
            .map_or(at, |mark| mark.after());
        self.set_selection_range(Some(jade_selection::SelectionRange::collapsed(
            jade_selection::TextPosition::new(caret.0, caret.1),
        )));
        Ok(caret)
    }

    /// [`Self::insert_note`], with `mark` (Word's 任意の記号) as the reference
    /// in place of a number when there is one. The caret goes after the mark.
    fn insert_note_marked(
        &mut self,
        kind: NoteKind,
        at: Position,
        text: &str,
        mark: Option<&str>,
    ) -> Result<Position, String> {
        let Some(mark) = mark.filter(|mark| !mark.is_empty()) else {
            return self.insert_note(kind, at, text);
        };
        if !self.takes_notes_at(at.0) {
            return Err("notes go in the main text, not a header, footer, note or text box".into());
        }
        let range = jade_selection::SelectionRange::collapsed(jade_selection::TextPosition::new(
            at.0, at.1,
        ));
        let command: Box<dyn jade_docx_editor::commands::Command> = match kind {
            NoteKind::Footnote => Box::new(
                jade_docx_editor::commands::insert_footnote::InsertFootnoteCommand::new(
                    range,
                    text.to_string(),
                )
                .with_custom_mark(mark.to_string()),
            ),
            NoteKind::Endnote => Box::new(
                jade_docx_editor::commands::insert_endnote::InsertEndnoteCommand::new(
                    range,
                    text.to_string(),
                )
                .with_custom_mark(mark.to_string()),
            ),
        };
        self.execute_command(command, false)?;
        let caret = (at.0, at.1 + mark.len());
        self.set_selection_range(Some(jade_selection::SelectionRange::collapsed(
            jade_selection::TextPosition::new(caret.0, caret.1),
        )));
        Ok(caret)
    }

    /// The notes of a kind the document has, in the order they are first
    /// referred to: each one's text.
    fn note_texts(&self, kind: NoteKind) -> Vec<String> {
        let notes = match kind {
            NoteKind::Footnote => self.word_document().footnote_infos(),
            NoteKind::Endnote => self.word_document().endnote_infos(),
        };
        notes.into_iter().map(|note| note.text).collect()
    }

    /// 目次: a table of contents from headings 1–3, after the caret's
    /// paragraph.
    fn insert_table_of_contents(&mut self) -> Result<(), String> {
        let after_paragraph = self
            .selection_range()
            .map(|range| range.normalized().0.paragraph)
            .unwrap_or(0);
        self.apply_command(
            HeadlessCommand::InsertTableOfContents {
                after_paragraph,
                outline_min: 1,
                outline_max: 3,
            },
            true,
        )
    }

    /// 目次の更新: the document's table of contents made again from its
    /// headings as they now are.
    fn update_table_of_contents(&mut self) -> Result<(), String> {
        self.apply_command(HeadlessCommand::UpdateTableOfContents, true)
    }
}

impl<T: WordSession + ?Sized> WordReferences for T {}
