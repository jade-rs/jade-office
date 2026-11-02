//! The dialogs of Data ▸ What-If Analysis and the analysis tools beside
//! it, each a [`Form`] whose buttons the model answers: both shells hold
//! one [`WhatIfDialog`], draw its form and hand it the presses.

use jade_xlsx_reader::{CellRange, CellRef};

use crate::data_table::DataTableDialog;
use crate::form::Form;
use crate::goal_seek::GoalSeekDialog;
use crate::sheet::Workbook;
use crate::Backend;

/// What a press did: whether the dialog is done, and whether it wrote to
/// the workbook (so the sheet is drawn again).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Outcome {
    pub close: bool,
    pub wrote: bool,
}

impl Outcome {
    pub fn stay() -> Self {
        Self {
            close: false,
            wrote: false,
        }
    }
    pub fn close() -> Self {
        Self {
            close: true,
            wrote: false,
        }
    }
    pub fn writing() -> Self {
        Self {
            close: false,
            wrote: true,
        }
    }
    pub fn closed_writing() -> Self {
        Self {
            close: true,
            wrote: true,
        }
    }
}

/// One of these dialogs: its form, and what its buttons do.
pub trait ToolDialog {
    fn form(&self) -> &Form;
    fn form_mut(&mut self) -> &mut Form;
    /// A button pressed, by its id.
    fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String>;
    /// A choice, tick box or list line of the form's, `id`, changed: what
    /// the dialog shows may follow it.
    fn changed<B: Backend>(
        &mut self,
        _id: &'static str,
        _book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        Ok(Outcome::stay())
    }
}

/// The commands that open one, as the grid's menu lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhatIfTool {
    GoalSeek,
    DataTable,
}

impl WhatIfTool {
    /// The grid menu's What-If Analysis group, with each one's i18n key.
    pub const MENU: [(WhatIfTool, &'static str); 2] = [
        (WhatIfTool::GoalSeek, "sheet.menu.goal_seek"),
        (WhatIfTool::DataTable, "sheet.menu.data_table"),
    ];
}

/// One of the dialogs, open.
pub enum WhatIfDialog {
    GoalSeek(GoalSeekDialog),
    DataTable(DataTableDialog),
}

/// `$body` with `$dialog` bound to whichever dialog is open.
macro_rules! each {
    ($self:expr, $dialog:ident => $body:expr) => {
        match $self {
            WhatIfDialog::GoalSeek($dialog) => $body,
            WhatIfDialog::DataTable($dialog) => $body,
        }
    };
}

impl WhatIfDialog {
    /// The dialog `tool` opens, on the selection.
    pub fn open<B: Backend>(
        tool: WhatIfTool,
        _book: &Workbook<B>,
        selected: CellRange,
        at: CellRef,
    ) -> Self {
        match tool {
            WhatIfTool::GoalSeek => WhatIfDialog::GoalSeek(GoalSeekDialog::open(at)),
            WhatIfTool::DataTable => WhatIfDialog::DataTable(DataTableDialog::open(selected)),
        }
    }

    pub fn form(&self) -> &Form {
        each!(self, dialog => dialog.form())
    }

    pub fn form_mut(&mut self) -> &mut Form {
        each!(self, dialog => dialog.form_mut())
    }

    /// A button pressed, by its id.
    pub fn press<B: Backend>(
        &mut self,
        button: &str,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        each!(self, dialog => dialog.press(button, book))
    }

    /// A click on the form's control at `index` (and its option or line
    /// `option`), which the dialog may follow.
    pub fn click<B: Backend>(
        &mut self,
        index: usize,
        option: usize,
        book: &mut Workbook<B>,
    ) -> Result<Outcome, String> {
        match self.form_mut().click(index, option) {
            Some(id) => each!(self, dialog => dialog.changed(id, book)),
            None => Ok(Outcome::stay()),
        }
    }
}

/// A cell as Excel writes it into these dialogs: `$B$3`.
pub fn absolute(at: CellRef) -> String {
    format!("${}${}", crate::sheet::column_label(at.column), at.row + 1)
}

/// A range as Excel writes it into these dialogs: `$A$1:$C$4`, or one cell.
pub fn absolute_range(range: CellRange) -> String {
    if range.start == range.end {
        absolute(range.start)
    } else {
        format!("{}:{}", absolute(range.start), absolute(range.end))
    }
}

/// A one-cell reference on the active sheet, as typed: `B3`, `$B$3`,
/// `Sheet1!B3`, a name.
pub fn single_cell<B: Backend>(book: &Workbook<B>, text: &str) -> Option<CellRef> {
    let (sheet, range) = book.resolve_reference(text)?;
    (sheet == book.sheet_index() && range.start == range.end).then_some(range.start)
}

/// A range on the active sheet, as typed.
pub fn range_on_sheet<B: Backend>(book: &Workbook<B>, text: &str) -> Option<CellRange> {
    let (sheet, range) = book.resolve_reference(text)?;
    (sheet == book.sheet_index()).then_some(range)
}
