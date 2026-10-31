//! Excel's Paste Special (⌘⌃V, Ctrl+Alt+V): the copied cells pasted as
//! All, Formulas, Values or Formats, with an Operation, Skip blanks and
//! Transpose.
//!
//! As in Excel, what pastes is what the copied cells hold now, not when they
//! were copied, and a cut has no Paste Special. Text copied from elsewhere
//! pastes its values, transposed if asked.
//!
//! - All: the formulas (moved as a paste moves them), values and formats.
//! - Formulas: the formulas and values; the cells pasted on keep their
//!   formats.
//! - Values: what the cells show as values — a formula's result, not the
//!   formula — keeping the formats pasted on.
//! - Formats: the formats only; the values pasted on stay.
//! - Column widths: the copied columns' widths given to the columns pasted
//!   on, and nothing else.
//!
//! Transpose turns the block: the copied rows become columns, and formulas
//! turn their references with it (`transpose_formula`). Skip blanks leaves
//! the cells under an empty copied cell as they are. One write, one undo.
//!
//! An operation — Add, Subtract, Multiply, Divide — combines each copied
//! number with the one pasted on rather than replacing it, as Excel's do: a
//! blank counts as 0, a formula on either side makes the result a formula
//! (`=(B2*2)+5`), copied text pastes as it is, and text pasted on stays.

use jade_xlsx_headless_api::CellInput;
use jade_xlsx_headless_editor::formula_text;
use jade_xlsx_reader::{translate_formula, transpose_formula, CellRange, CellRef, CellValue};

use crate::sheet::{reference_text, Workbook};
use crate::{clipboard, typing, Backend};

/// What Paste Special pastes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PasteWhat {
    #[default]
    All,
    Formulas,
    Values,
    Formats,
    ColumnWidths,
}

impl PasteWhat {
    /// The dialog's choices, in Excel's order.
    pub const ALL: [PasteWhat; 5] = [
        PasteWhat::All,
        PasteWhat::Formulas,
        PasteWhat::Values,
        PasteWhat::Formats,
        PasteWhat::ColumnWidths,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            PasteWhat::All => "sheet.paste_special.all",
            PasteWhat::Formulas => "sheet.paste_special.formulas",
            PasteWhat::Values => "sheet.paste_special.values",
            PasteWhat::Formats => "sheet.paste_special.formats",
            PasteWhat::ColumnWidths => "sheet.paste_special.column_widths",
        }
    }
}

/// Paste Special's Operation: how a copied number meets the one pasted on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PasteOperation {
    #[default]
    None,
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl PasteOperation {
    /// The dialog's choices, in Excel's order.
    pub const ALL: [PasteOperation; 5] = [
        PasteOperation::None,
        PasteOperation::Add,
        PasteOperation::Subtract,
        PasteOperation::Multiply,
        PasteOperation::Divide,
    ];

    /// The i18n key of its name.
    pub fn label(self) -> &'static str {
        match self {
            PasteOperation::None => "sheet.paste_special.op_none",
            PasteOperation::Add => "sheet.paste_special.op_add",
            PasteOperation::Subtract => "sheet.paste_special.op_subtract",
            PasteOperation::Multiply => "sheet.paste_special.op_multiply",
            PasteOperation::Divide => "sheet.paste_special.op_divide",
        }
    }

    fn sign(self) -> &'static str {
        match self {
            PasteOperation::None => "",
            PasteOperation::Add => "+",
            PasteOperation::Subtract => "-",
            PasteOperation::Multiply => "*",
            PasteOperation::Divide => "/",
        }
    }

    /// What the cell pasted on (`under`) becomes with `copied` combined
    /// into it.
    fn combine(self, under: CellInput, copied: CellInput) -> CellInput {
        let operand = |input: &CellInput| match input {
            CellInput::Number(number) => Some(number.to_string()),
            CellInput::Blank => Some("0".to_string()),
            CellInput::Formula(formula) => Some(format!("({formula})")),
            _ => None,
        };
        // Copied text (or a truth value) pastes as it is.
        let Some(right) = operand(&copied) else {
            return copied;
        };
        // Text pasted on stays.
        let Some(left) = operand(&under) else {
            return under;
        };
        let number = |input: &CellInput| match input {
            CellInput::Number(number) => Some(*number),
            CellInput::Blank => Some(0.0),
            _ => None,
        };
        if let (Some(a), Some(b)) = (number(&under), number(&copied)) {
            let result = match self {
                PasteOperation::None => b,
                PasteOperation::Add => a + b,
                PasteOperation::Subtract => a - b,
                PasteOperation::Multiply => a * b,
                // ÷0 is Excel's #DIV/0!, which a formula gives.
                PasteOperation::Divide if b == 0.0 => {
                    return CellInput::Formula(format!("{left}/0"));
                }
                PasteOperation::Divide => a / b,
            };
            return CellInput::Number(result);
        }
        CellInput::Formula(format!("{left}{}{right}", self.sign()))
    }
}

/// The Paste Special dialog's choices, which is also its state while open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PasteSpecial {
    pub what: PasteWhat,
    /// Operation: None replaces what is pasted on, as a paste does.
    pub operation: PasteOperation,
    pub skip_blanks: bool,
    pub transpose: bool,
}

impl PasteSpecial {
    /// The dialog's keys: arrows move between the choices, as a radio group
    /// does; Return pastes and Escape leaves are the shells'. Returns
    /// whether the key was the dialog's.
    pub fn key(&mut self, key: &str) -> bool {
        let at = PasteWhat::ALL
            .iter()
            .position(|what| *what == self.what)
            .unwrap_or(0);
        match key {
            "up" => self.what = PasteWhat::ALL[at.saturating_sub(1)],
            "down" => self.what = PasteWhat::ALL[(at + 1).min(PasteWhat::ALL.len() - 1)],
            _ => return false,
        }
        true
    }
}

/// One copied cell: what it holds, its formula moved for where it lands,
/// and its format.
struct Copied {
    /// The formula, without `=`, moved to its landing cell.
    formula: Option<String>,
    value: CellInput,
    style: Option<u32>,
    blank: bool,
}

impl<B: Backend> Workbook<B> {
    /// Whether Paste Special has something to paste: a copy, not a cut, or
    /// text from elsewhere.
    pub fn can_paste_special(&self, text: Option<&str>) -> bool {
        match self.copied_cells() {
            Some((_, _, cut)) => !cut,
            None => text.is_some_and(|text| !text.is_empty()),
        }
    }

    /// Paste Special at `at` (the top-left of where it lands): this
    /// workbook's copied cells, or else `text` from the clipboard. Returns
    /// the range pasted on.
    pub fn paste_special(
        &mut self,
        at: CellRef,
        text: Option<&str>,
        special: PasteSpecial,
    ) -> Result<Option<CellRange>, String> {
        if special.what == PasteWhat::ColumnWidths {
            return self.paste_column_widths(at);
        }
        let grid = match self.copied_cells() {
            Some((_, _, true)) => return Ok(None),
            Some((sheet, range, false)) => self.copied_grid(sheet, range, at, special.transpose),
            None => match text {
                Some(text) => self.text_grid(text),
                None => return Ok(None),
            },
        };
        if grid.is_empty() {
            return Ok(None);
        }
        let rows = grid.len() as u32;
        let columns = grid.iter().map(Vec::len).max().unwrap_or(0) as u32;
        let (height, width) = if special.transpose {
            (columns, rows)
        } else {
            (rows, columns)
        };
        if width == 0 || height == 0 {
            return Ok(None);
        }
        let landed = CellRange {
            start: at,
            end: CellRef::new(at.column + width - 1, at.row + height - 1),
        };
        let mut writes: Vec<(String, CellInput, Option<u32>)> = Vec::new();
        for (row, cells) in grid.into_iter().enumerate() {
            for (column, cell) in cells.into_iter().enumerate() {
                let (down, across) = if special.transpose {
                    (column as u32, row as u32)
                } else {
                    (row as u32, column as u32)
                };
                let to = CellRef::new(at.column + across, at.row + down);
                if cell.blank && special.skip_blanks {
                    continue;
                }
                let write = match special.what {
                    PasteWhat::All => (cell.input(), cell.style),
                    PasteWhat::Formulas => (cell.input(), None),
                    PasteWhat::Values => (cell.value, None),
                    PasteWhat::Formats => match cell.style {
                        Some(style) => (self.input_at(to), Some(style)),
                        None => continue,
                    },
                    PasteWhat::ColumnWidths => continue,
                };
                let write = if special.operation == PasteOperation::None
                    || special.what == PasteWhat::Formats
                {
                    write
                } else {
                    (
                        special.operation.combine(self.input_at(to), write.0),
                        write.1,
                    )
                };
                writes.push((reference_text(to), write.0, write.1));
            }
        }
        if writes.is_empty() {
            return Ok(Some(landed));
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_cells_styled(&name, &writes)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(Some(landed))
    }

    /// Column widths at `at`: each copied column's width given to the
    /// column it lands on, a column of the sheet's standard width giving
    /// that. Returns the columns pasted on, the copied rows' height.
    fn paste_column_widths(&mut self, at: CellRef) -> Result<Option<CellRange>, String> {
        let Some((sheet, range, false)) = self.copied_cells() else {
            return Ok(None);
        };
        let Some(source) = self.engine().workbook().sheets.get(sheet) else {
            return Ok(None);
        };
        // Excel's standard 8.43 characters, as files store it.
        let standard = source.default_column_width_chars.unwrap_or(9.140625);
        let sizes: std::collections::BTreeMap<u32, f32> = (range.start.column..=range.end.column)
            .map(|column| {
                let width = source
                    .columns
                    .iter()
                    .find(|span| span.first <= column && column <= span.last)
                    .and_then(|span| span.width_chars)
                    .unwrap_or(standard);
                (at.column + (column - range.start.column), width)
            })
            .collect();
        let name = self.sheet_name();
        self.engine_mut()
            .set_axis_sizes(&name, jade_xlsx_reader::Axis::Columns, &sizes)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(Some(CellRange {
            start: at,
            end: CellRef::new(
                at.column + (range.end.column - range.start.column),
                at.row + (range.end.row - range.start.row),
            ),
        }))
    }

    /// Paste Link at `at`: a formula in each cell pasted on that reads the
    /// copied cell — `=$B$2` for one cell, `=B2` and on for several, with the
    /// sheet's name in front from another sheet — as Excel's button pastes.
    /// Not after a cut, nor from elsewhere. Returns the range pasted on.
    pub fn paste_link(&mut self, at: CellRef) -> Result<Option<CellRange>, String> {
        let Some((sheet, range, false)) = self.copied_cells() else {
            return Ok(None);
        };
        let prefix = if sheet == self.sheet_index() {
            String::new()
        } else {
            let names = self.sheet_names();
            format!("{}!", jade_xlsx_reader::quote_sheet_name(&names[sheet]))
        };
        let single = range.start == range.end;
        let (rows, columns) = (
            range.end.row - range.start.row,
            range.end.column - range.start.column,
        );
        let mut writes: Vec<(String, CellInput)> = Vec::new();
        for down in 0..=rows {
            for across in 0..=columns {
                let from = CellRef::new(range.start.column + across, range.start.row + down);
                let to = CellRef::new(at.column + across, at.row + down);
                let reference = if single {
                    format!(
                        "${}${}",
                        crate::sheet::column_label(from.column),
                        from.row + 1
                    )
                } else {
                    reference_text(from)
                };
                writes.push((
                    reference_text(to),
                    CellInput::Formula(format!("{prefix}{reference}")),
                ));
            }
        }
        let name = self.sheet_name();
        self.engine_mut()
            .set_cells(&name, &writes)
            .map_err(|error| error.to_string())?;
        self.wrote(1)?;
        Ok(Some(CellRange {
            start: at,
            end: CellRef::new(at.column + columns, at.row + rows),
        }))
    }

    /// The copied cells as they are now, row by row, each formula moved to
    /// where it lands from `at`, turned when transposed.
    fn copied_grid(
        &self,
        sheet: usize,
        range: CellRange,
        at: CellRef,
        transpose: bool,
    ) -> Vec<Vec<Copied>> {
        let Some(source) = self.engine().workbook().sheets.get(sheet) else {
            return Vec::new();
        };
        (range.start.row..=range.end.row)
            .map(|row| {
                let found = source.rows.iter().find(|candidate| candidate.index == row);
                (range.start.column..=range.end.column)
                    .map(|column| {
                        let from = CellRef::new(column, row);
                        let (down, across) = (row - range.start.row, column - range.start.column);
                        let to = if transpose {
                            CellRef::new(at.column + down, at.row + across)
                        } else {
                            CellRef::new(at.column + across, at.row + down)
                        };
                        let cell = found.and_then(|row| {
                            row.cells
                                .iter()
                                .find(|cell| cell.reference.column == column)
                        });
                        let style = Some(jade_xlsx_headless_editor::effective_style(source, from));
                        let Some(cell) = cell else {
                            return Copied {
                                formula: None,
                                value: CellInput::Blank,
                                style,
                                blank: true,
                            };
                        };
                        let formula = cell.formula.as_deref().map(|formula| {
                            let shown = formula_text::to_display_formula(formula);
                            if transpose {
                                transpose_formula(&shown, from, to)
                            } else {
                                translate_formula(&shown, from, to)
                            }
                        });
                        let value = match &cell.value {
                            CellValue::Empty => CellInput::Blank,
                            CellValue::Number(number) => CellInput::Number(*number),
                            CellValue::Text(text) => CellInput::Text(text.clone()),
                            CellValue::Bool(flag) => CellInput::Bool(*flag),
                            CellValue::Error(error) => CellInput::Text(error.clone()),
                        };
                        let blank = formula.is_none() && matches!(value, CellInput::Blank);
                        Copied {
                            formula,
                            value,
                            style,
                            blank,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// Text from elsewhere, read as typed, row by row.
    fn text_grid(&self, text: &str) -> Vec<Vec<Copied>> {
        let date1904 = matches!(self.date_system(), jade_xlsx_reader::DateSystem::Nineteen04);
        clipboard::parse(text)
            .iter()
            .map(|row| {
                row.iter()
                    .map(|value| {
                        let input = typing::read(value, date1904).input;
                        let formula = match &input {
                            CellInput::Formula(formula) => Some(formula.clone()),
                            _ => None,
                        };
                        Copied {
                            blank: value.is_empty(),
                            formula,
                            value: input,
                            style: None,
                        }
                    })
                    .collect()
            })
            .collect()
    }

    /// The cell at `to` as it is, to write back unchanged under a new
    /// format.
    fn input_at(&self, to: CellRef) -> CellInput {
        let typed = self.edit_text(to);
        if let Some(formula) = typed.strip_prefix('=') {
            return CellInput::Formula(formula.to_string());
        }
        let shown = self.engine().workbook().sheets[self.sheet_index()]
            .rows
            .iter()
            .find(|row| row.index == to.row)
            .and_then(|row| {
                row.cells
                    .iter()
                    .find(|cell| cell.reference.column == to.column)
            })
            .map(|cell| cell.value.clone());
        match shown {
            Some(CellValue::Number(number)) => CellInput::Number(number),
            Some(CellValue::Text(text)) => CellInput::Text(text),
            Some(CellValue::Bool(flag)) => CellInput::Bool(flag),
            Some(CellValue::Error(error)) => CellInput::Text(error),
            Some(CellValue::Empty) | None => CellInput::Blank,
        }
    }
}

impl Copied {
    /// The formula if it has one, else its value.
    fn input(&self) -> CellInput {
        match &self.formula {
            Some(formula) => CellInput::Formula(formula.clone()),
            None => self.value.clone(),
        }
    }
}
