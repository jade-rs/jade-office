//! What the analysis commands write: a table of values and formulas put at
//! a cell of the active sheet or on a sheet of its own — a scenario
//! summary, Solver's answer report, an Analysis ToolPak tool's output — in
//! one undo step.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_reader::CellRef;

use crate::sheet::{reference_text, CellFormat, Workbook};
use crate::Backend;

/// Where a report goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Output {
    /// Its top left corner on the active sheet.
    At(CellRef),
    /// A new sheet after the active one, by the name it is given — `Sheet
    /// Name (2)` when one has it — shown once written.
    NewSheet(String),
}

/// A report's cells, from its top left corner, and how some of them look.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    pub cells: Vec<(CellRef, CellInput)>,
    /// Cells drawn bold: headings.
    pub bold: Vec<CellRef>,
    /// Cells filled grey, as a scenario summary marks its changing cells.
    pub shaded: Vec<CellRef>,
    /// Number formats, by cell.
    pub formats: Vec<(CellRef, String)>,
}

impl Report {
    pub fn put(&mut self, at: CellRef, value: CellInput) {
        self.cells.push((at, value));
    }

    pub fn text(&mut self, at: CellRef, text: &str) {
        self.put(at, CellInput::Text(text.to_string()));
    }

    pub fn number(&mut self, at: CellRef, value: f64) {
        self.put(
            at,
            if value.is_finite() {
                CellInput::Number(value)
            } else {
                CellInput::Text("#NUM!".to_string())
            },
        );
    }

    pub fn heading(&mut self, at: CellRef, text: &str) {
        self.text(at, text);
        self.bold.push(at);
    }
}

impl<B: Backend> Workbook<B> {
    /// Writes `report` where `output` says, in one undo step; returns the
    /// sheet it went on.
    pub fn write_report(&mut self, output: &Output, report: &Report) -> Result<usize, String> {
        self.atomic_edit(|book| {
            let (index, steps) = book.write_report_steps(output, report)?;
            book.wrote(steps)?;
            if index != book.sheet_index() {
                book.select_sheet(index)?;
            }
            Ok(index)
        })
    }

    /// [`Self::write_report`] without noting the write: the sheet it went
    /// on and the engine steps it took, for a command that writes more in
    /// the same undo step.
    pub(crate) fn write_report_steps(
        &mut self,
        output: &Output,
        report: &Report,
    ) -> Result<(usize, usize), String> {
        let book = self;
        {
            let mut steps = 0;
            let (index, name, origin) = match output {
                Output::At(origin) => (book.sheet_index(), book.sheet_name(), *origin),
                Output::NewSheet(wanted) => {
                    let names = book.sheet_names();
                    let name = std::iter::once(wanted.clone())
                        .chain((2..).map(|n| format!("{wanted} ({n})")))
                        .find(|name| !names.iter().any(|taken| taken.eq_ignore_ascii_case(name)))
                        .unwrap_or_default();
                    let after = book.sheet_name();
                    let index = book
                        .engine_mut()
                        .add_sheet(&name, Some(&after))
                        .map_err(|error| error.to_string())?;
                    steps += 1;
                    (index, name, CellRef::new(0, 0))
                }
            };
            let at = |cell: CellRef| {
                reference_text(CellRef::new(
                    origin.column + cell.column,
                    origin.row + cell.row,
                ))
            };
            let values: Vec<(String, CellInput)> = report
                .cells
                .iter()
                .map(|(cell, value)| (at(*cell), value.clone()))
                .collect();
            if !values.is_empty() {
                book.engine_mut()
                    .set_cells(&name, &values)
                    .map_err(|error| error.to_string())?;
                steps += 1;
            }
            let looks = report
                .bold
                .iter()
                .map(|cell| {
                    (
                        *cell,
                        CellFormat {
                            bold: Some(true),
                            ..Default::default()
                        },
                    )
                })
                .chain(report.shaded.iter().map(|cell| {
                    (
                        *cell,
                        CellFormat {
                            fill_color: Some("D9D9D9".to_string()),
                            ..Default::default()
                        },
                    )
                }))
                .chain(report.formats.iter().map(|(cell, code)| {
                    (
                        *cell,
                        CellFormat {
                            number_format: Some(code.clone()),
                            ..Default::default()
                        },
                    )
                }));
            for (cell, change) in looks {
                book.engine_mut()
                    .set_format(&name, &at(cell), &change)
                    .map_err(|error| error.to_string())?;
                steps += 1;
            }
            Ok((index, steps))
        }
    }
}
