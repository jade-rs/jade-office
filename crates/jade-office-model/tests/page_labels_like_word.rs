//! Page labels on a long document: a table of contents' update and F9
//! write the number each target page displays, and read the session's
//! labels once per command, not once per entry or per page.

use std::cell::Cell;
use std::collections::HashMap;
use std::future::Future;
use std::io::Write;

use jade_docx_editor::commands::{Command, CommandContext, CommandExecutor};
use jade_docx_editor::field_datetime::FieldDateTime;
use jade_docx_editor::fields::{FieldScope, PageLabel};
use jade_docx_editor::word_document::WordDocument;
use jade_docx_editor::Options;
use jade_docx_headless_api::HeadlessCommand;
use jade_office_model::word::fields::{FieldHost, WordFields};
use jade_office_model::word::references::toc::WordToc;
use jade_office_model::word::{CellLayout, WordSession};
use jade_selection::{SelectionRange, TextPosition};

const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Targets, and so pages past the first: enough that a per-lookup rebuild
/// of the labels would be tens of thousands of label walks.
const TARGETS: usize = 200;

fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        if let std::task::Poll::Ready(value) = future.as_mut().poll(&mut context) {
            return value;
        }
    }
}

fn package(body: &str) -> Vec<u8> {
    let parts = [
        (
            "[Content_Types].xml",
            r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="xml" ContentType="application/xml"/><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string(),
        ),
        (
            "_rels/.rels",
            format!(
                r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="{R}/officeDocument" Target="word/document.xml"/></Relationships>"#
            ),
        ),
        (
            "word/document.xml",
            format!(
                r#"<w:document xmlns:w="{W}" xmlns:r="{R}"><w:body>{body}</w:body></w:document>"#
            ),
        ),
    ];
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, xml) in parts {
        zip.start_file(name, zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(xml.as_bytes()).unwrap();
    }
    zip.finish().unwrap().into_inner()
}

fn page_ref(name: &str, switches: &str) -> String {
    format!(
        r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> PAGEREF {name} {switches}</w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>9</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#
    )
}

/// `TARGETS` bookmarked paragraphs, each on its own page after the
/// references that come first.
fn targets() -> String {
    (0..TARGETS)
        .map(|k| {
            format!(
                r#"<w:p><w:bookmarkStart w:id="{k}" w:name="_Toc{k}"/><w:r><w:t>Target {k}</w:t></w:r><w:bookmarkEnd w:id="{k}"/></w:p>"#
            )
        })
        .collect()
}

/// A layout's facts without a renderer: target `k` (paragraph
/// `first_target + k`) is on page `k + 2`, and page `n` displays "p-n",
/// as a section with its own numbering would. Counts the reads.
struct Session {
    document: WordDocument,
    selection: Option<SelectionRange>,
    executor: CommandExecutor,
    first_target: usize,
    label_lists: Cell<usize>,
    label_lookups: Cell<usize>,
}

impl Session {
    fn open(body: &str, first_target: usize) -> Self {
        let document = block_on(WordDocument::load(&package(body), &Options::default()))
            .expect("the document loads");
        Self {
            document,
            selection: Some(SelectionRange::collapsed(TextPosition::new(0, 0))),
            executor: CommandExecutor::new(),
            first_target,
            label_lists: Cell::new(0),
            label_lookups: Cell::new(0),
        }
    }

    fn label(page: usize) -> PageLabel {
        PageLabel {
            value: page,
            text: format!("p-{page}"),
        }
    }

    fn text(&self, paragraph: usize) -> String {
        self.document
            .paragraph(paragraph)
            .expect("paragraph")
            .runs
            .iter()
            .map(|run| run.text.as_str())
            .collect()
    }
}

impl WordSession for Session {
    fn word_document(&self) -> &WordDocument {
        &self.document
    }
    fn selection_range(&self) -> Option<SelectionRange> {
        self.selection
    }
    fn set_selection_range(&mut self, range: Option<SelectionRange>) {
        self.selection = range;
    }
    fn apply_command(&mut self, command: HeadlessCommand, keep: bool) -> Result<(), String> {
        self.apply_commands(vec![command], keep)
    }
    fn apply_commands(
        &mut self,
        commands: Vec<HeadlessCommand>,
        _keep: bool,
    ) -> Result<(), String> {
        jade_docx_editor::headless_api::apply_headless_commands_to_document(
            &mut self.document,
            &jade_docx_headless_api::HeadlessEditRequest { commands },
        )
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
    }
    fn execute_command(&mut self, command: Box<dyn Command>, keep: bool) -> Result<(), String> {
        let before = self.selection;
        let mut ctx = CommandContext::new(&mut self.document, &mut self.selection);
        self.executor
            .execute(command, &mut ctx)
            .map_err(|error| format!("{error:?}"))?;
        if keep {
            self.selection = before;
        }
        Ok(())
    }
    fn cell_id_for_paragraph(&self, _paragraph: usize) -> Option<usize> {
        None
    }
    fn cell_layout(&self, _cell_id: usize) -> Option<&CellLayout> {
        None
    }
    fn cell_layouts(&self) -> &[CellLayout] {
        &[]
    }
    fn table_order_for(&self, _table_id: usize) -> Option<usize> {
        None
    }
    fn hyperlink_regions(&self) -> Vec<jade_docx_editor::canvas_renderer::HyperlinkRegion> {
        Vec::new()
    }
    fn select_cells(&mut self, _anchor_cell: usize, _focus_cell: usize) -> Result<(), String> {
        Err("no tables in this session".into())
    }
    fn page_count(&self) -> Option<usize> {
        Some(TARGETS + 1)
    }
    fn paragraph_page(&self, paragraph: usize) -> Option<usize> {
        Some(
            paragraph
                .checked_sub(self.first_target)
                .map_or(1, |target| target + 2),
        )
    }
    fn page_label(&self, page: usize) -> Option<PageLabel> {
        self.label_lookups.set(self.label_lookups.get() + 1);
        (1..=TARGETS + 1).contains(&page).then(|| Self::label(page))
    }
    fn displayed_page_labels(&self) -> Vec<PageLabel> {
        self.label_lists.set(self.label_lists.get() + 1);
        (1..=TARGETS + 1).map(Self::label).collect()
    }
    fn has_page_labels(&self) -> bool {
        true
    }
}

fn host() -> FieldHost {
    FieldHost {
        now: FieldDateTime {
            year: 2026,
            month: 11,
            day: 26,
            hour: 9,
            minute: 0,
            second: 0,
            weekday: 4,
        },
        utc_offset_seconds: 9 * 3600,
        user_name: String::new(),
        user_initials: String::new(),
        user_address: String::new(),
        file: None,
        japanese: true,
        merge: None,
    }
}

/// 目次の更新 (page numbers only) over a table of `TARGETS` entries writes
/// each target page's label and reads the labels once.
#[test]
fn a_long_tables_page_numbers_read_the_labels_once() {
    let mut body = String::new();
    for k in 0..TARGETS {
        let begin = if k == 0 {
            r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText xml:space="preserve"> TOC \o "1-3" \h </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r>"#
        } else {
            ""
        };
        let end = if k + 1 == TARGETS {
            r#"<w:r><w:fldChar w:fldCharType="end"/></w:r>"#
        } else {
            ""
        };
        body.push_str(&format!(
            r#"<w:p>{begin}<w:r><w:t>Entry {k}</w:t></w:r><w:r><w:tab/></w:r>{}{end}</w:p>"#,
            page_ref(&format!("_Toc{k}"), r"\h ")
        ));
    }
    body.push_str(&targets());
    let mut session = Session::open(&body, TARGETS);
    assert_eq!(session.word_document().tables_of_contents().len(), 1);

    let skipped = session.update_toc(true).expect("update the page numbers");

    assert_eq!(skipped, 0);
    for k in [0, 1, TARGETS / 2, TARGETS - 1] {
        assert_eq!(session.text(k), format!("Entry {k}\tp-{}", k + 2));
    }
    assert_eq!(session.label_lists.get(), 1, "the labels are listed once");
    assert_eq!(session.label_lookups.get(), 0, "and not page by page");
}

/// F9 over `TARGETS` PAGEREFs writes each target page's label and reads
/// the labels once.
#[test]
fn f9_over_many_page_references_reads_the_labels_once() {
    let mut body: String = (0..TARGETS)
        .map(|k| {
            format!(
                r#"<w:p><w:r><w:t xml:space="preserve">See </w:t></w:r>{}</w:p>"#,
                page_ref(&format!("_Toc{k}"), "")
            )
        })
        .collect();
    body.push_str(&targets());
    let mut session = Session::open(&body, TARGETS);

    session
        .update_fields(FieldScope::Document, &host(), HashMap::new())
        .expect("F9");

    for k in [0, 1, TARGETS / 2, TARGETS - 1] {
        assert_eq!(session.text(k), format!("See p-{}", k + 2));
    }
    assert_eq!(session.label_lists.get(), 1, "the labels are listed once");
    assert_eq!(session.label_lookups.get(), 0, "and not page by page");
}
