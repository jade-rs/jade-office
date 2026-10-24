//! Table creation/design dialog and authoring shared by both worksheet shells.
use crate::{
    sheet::{range_text, CellRange, CellRef, Workbook},
    text::TextEdit,
    Backend,
};
pub use jade_xlsx_headless_editor::{
    TableOptions, TableStyle, TableTotal, TableUpdate, TotalFunction,
};
use jade_xlsx_reader::Table;
#[derive(Debug, Clone)]
pub enum TableCommand {
    Create(CellRange, TableOptions),
    Edit(String, TableUpdate),
}
impl<B: Backend> Workbook<B> {
    pub fn table(&self, at: CellRef) -> Option<&Table> {
        self.engine().workbook().sheets[self.sheet_index()].table_at(at)
    }
    pub fn apply_table_command(&mut self, command: TableCommand) -> Result<bool, String> {
        let sheet = self.sheet_name();
        match command {
            TableCommand::Create(range, options) => {
                self.engine_mut().create_table(&sheet, range, &options)
            }
            TableCommand::Edit(name, change) => {
                self.engine_mut().update_table(&sheet, &name, &change)
            }
        }
        .map_err(|e| e.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }
    pub fn convert_table_command(&mut self, command: TableCommand) -> Result<bool, String> {
        let TableCommand::Edit(name, change) = command else {
            return Err("select an existing table".into());
        };
        let sheet = self.sheet_name();
        self.engine_mut()
            .convert_table_with_update(&sheet, &name, &change)
            .map_err(|e| e.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableField {
    Range,
    Name,
    Formula,
    TotalValue,
}
impl TableField {
    pub fn key(self) -> &'static str {
        match self {
            Self::Range => "table.range",
            Self::Name => "table.name",
            Self::Formula => "table.calculated_formula",
            Self::TotalValue => "table.total_value",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Range => 0,
            Self::Name => 1,
            Self::Formula => 2,
            Self::TotalValue => 3,
        }
    }
}
pub const TOTALS: [(&str, Option<TotalFunction>); 11] = [
    ("table.total_none", None),
    ("table.total_sum", Some(TotalFunction::Sum)),
    ("table.total_average", Some(TotalFunction::Average)),
    ("table.total_count", Some(TotalFunction::Count)),
    ("table.total_numbers", Some(TotalFunction::CountNumbers)),
    ("table.total_max", Some(TotalFunction::Max)),
    ("table.total_min", Some(TotalFunction::Min)),
    ("table.total_stddev", Some(TotalFunction::StandardDeviation)),
    ("table.total_variance", Some(TotalFunction::Variance)),
    ("table.total_label", None),
    ("table.total_formula", None),
];
#[derive(Clone)]
struct ColumnDraft {
    formula: String,
    total: TableTotal,
}
fn column_draft(table: &Table, i: usize) -> ColumnDraft {
    let c = table.column_properties.get(i);
    ColumnDraft {
        formula: c
            .and_then(|c| c.calculated_formula.as_deref())
            .map(jade_xlsx_headless_editor::formula_text::to_display_formula)
            .unwrap_or_default(),
        total: if let Some(f) = c.and_then(|c| c.totals_formula.as_deref()) {
            TableTotal::Formula(jade_xlsx_headless_editor::formula_text::to_display_formula(
                f,
            ))
        } else if let Some(f) = c
            .and_then(|c| c.totals_function.as_deref())
            .and_then(TotalFunction::from_name)
        {
            TableTotal::Function(f)
        } else if let Some(s) = c.and_then(|c| c.totals_label.clone()) {
            TableTotal::Label(s)
        } else {
            TableTotal::None
        },
    }
}
pub struct TableDialog {
    pub field: TableField,
    fields: [TextEdit; 4],
    pub has_headers: bool,
    pub has_totals: bool,
    pub filter_buttons: bool,
    pub style: TableStyle,
    pub show_styles: bool,
    pub styles: Vec<Option<String>>,
    pub column: usize,
    pub total_kind: usize,
    pub problem: Option<String>,
    pub input_bounds: std::rc::Rc<std::cell::Cell<Option<[f32; 4]>>>,
    original: Option<Table>,
    columns: Vec<ColumnDraft>,
}
impl TableDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, selected: CellRange, at: CellRef) -> Self {
        let original = book.table(at).cloned();
        let range = original.as_ref().map(|t| t.reference).unwrap_or_else(|| {
            if selected.start == selected.end {
                book.current_region(at).unwrap_or(selected)
            } else {
                selected
            }
        });
        let all_names: Vec<_> = book
            .engine()
            .workbook()
            .sheets
            .iter()
            .flat_map(|s| s.tables.iter().map(|t| t.name.to_lowercase()))
            .chain(
                book.engine()
                    .workbook()
                    .defined_names
                    .iter()
                    .map(|n| n.name.to_lowercase()),
            )
            .collect();
        let name = original
            .as_ref()
            .map(|t| t.name.clone())
            .unwrap_or_else(|| {
                (1u32..)
                    .map(|i| format!("Table{i}"))
                    .find(|s| !all_names.contains(&s.to_lowercase()))
                    .unwrap()
            });
        let column = original
            .as_ref()
            .map(|t| at.column.saturating_sub(t.reference.start.column) as usize)
            .unwrap_or(0);
        let columns = original
            .as_ref()
            .map(|t| (0..t.columns.len()).map(|i| column_draft(t, i)).collect())
            .unwrap_or_default();
        let style = original
            .as_ref()
            .map(TableStyle::from_table)
            .unwrap_or_default();
        let mut styles = vec![None];
        for (prefix, count) in [
            ("TableStyleLight", 21),
            ("TableStyleMedium", 28),
            ("TableStyleDark", 11),
        ] {
            styles.extend((1..=count).map(|i| Some(format!("{prefix}{i}"))));
        }
        styles.extend(
            book.engine()
                .workbook()
                .styles
                .table_styles
                .iter()
                .map(|s| Some(s.name.clone())),
        );
        let sheet = &book.engine().workbook().sheets[book.sheet_index()];
        let all_text = |row| {
            (range.start.column..=range.end.column).all(|column| {
                sheet
                    .rows
                    .binary_search_by_key(&row, |r| r.index)
                    .ok()
                    .and_then(|i| {
                        sheet.rows[i]
                            .cells
                            .binary_search_by_key(&column, |c| c.reference.column)
                            .ok()
                            .map(|j| &sheet.rows[i].cells[j])
                    })
                    .is_some_and(|c| matches!(c.value, jade_xlsx_reader::CellValue::Text(_)))
            })
        };
        let guessed_headers = range.start.row < range.end.row
            && all_text(range.start.row)
            && !all_text(range.start.row + 1);
        let mut result = Self {
            field: TableField::Range,
            fields: [
                TextEdit::selected(range_text(range)),
                TextEdit::new(name),
                TextEdit::new(String::new()),
                TextEdit::new(String::new()),
            ],
            has_headers: original
                .as_ref()
                .map(|t| t.header_row_count > 0)
                .unwrap_or(guessed_headers),
            has_totals: original.as_ref().is_some_and(|t| t.totals_row_count > 0),
            filter_buttons: original.as_ref().is_none_or(|t| t.auto_filter),
            style,
            show_styles: false,
            styles,
            column,
            total_kind: 0,
            problem: None,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
            original,
            columns,
        };
        result.load_column();
        result
    }
    pub fn editing(&self) -> bool {
        self.original.is_some()
    }
    pub fn column_name(&self) -> Option<&str> {
        self.original
            .as_ref()?
            .columns
            .get(self.column)
            .map(String::as_str)
    }
    pub fn value(&self, f: TableField) -> &str {
        &self.fields[f.index()].text
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.field.index()]
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.field.index()]
    }
    pub fn focus(&mut self, f: TableField) {
        self.field = f;
        self.field_mut().whole = true;
        self.input_bounds.set(None);
    }
    pub fn visible_fields(&self) -> Vec<TableField> {
        let mut out = vec![TableField::Range, TableField::Name];
        if self.editing() {
            out.push(TableField::Formula);
            if self.has_totals && self.total_kind >= 9 {
                out.push(TableField::TotalValue);
            }
        }
        out
    }
    fn flush_column(&mut self) {
        let formula = self.value(TableField::Formula).to_string();
        let value = self.value(TableField::TotalValue).to_string();
        if let Some(d) = self.columns.get_mut(self.column) {
            d.formula = formula;
            d.total = match self.total_kind {
                0 => TableTotal::None,
                9 => TableTotal::Label(value),
                10 => TableTotal::Formula(value),
                i => TableTotal::Function(TOTALS[i].1.unwrap()),
            };
        }
    }
    fn load_column(&mut self) {
        if let Some(d) = self.columns.get(self.column) {
            self.fields[2] = TextEdit::new(d.formula.clone());
            let (kind, value) = match &d.total {
                TableTotal::None => (0, String::new()),
                TableTotal::Label(s) => (9, s.clone()),
                TableTotal::Formula(f) => (10, f.clone()),
                TableTotal::Function(f) => (
                    TOTALS.iter().position(|(_, x)| *x == Some(*f)).unwrap(),
                    String::new(),
                ),
            };
            self.total_kind = kind;
            self.fields[3] = TextEdit::new(value);
        }
    }
    pub fn next_column(&mut self) {
        if self.columns.is_empty() {
            return;
        }
        self.flush_column();
        self.column = (self.column + 1) % self.columns.len();
        self.load_column();
        self.focus(TableField::Formula);
    }
    pub fn next_total(&mut self) {
        self.total_kind = (self.total_kind + 1) % TOTALS.len();
        if self.total_kind >= 9 {
            self.focus(TableField::TotalValue);
        }
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        if key == "tab" {
            let fields = self.visible_fields();
            let i = fields.iter().position(|f| *f == self.field).unwrap_or(0);
            self.focus(fields[(i + if shift { fields.len() - 1 } else { 1 }) % fields.len()]);
            true
        } else if command && key == "a" {
            self.field_mut().whole = true;
            true
        } else {
            self.field_mut().edit_key(key)
        }
    }
    pub fn command(&mut self) -> Result<TableCommand, String> {
        self.flush_column();
        let range = self
            .value(TableField::Range)
            .trim()
            .trim_start_matches('=')
            .replace('$', "");
        let range = CellRange::parse(&range).map_err(|_| "table.invalid_range")?;
        let name = self.value(TableField::Name).trim().to_string();
        if name.is_empty() {
            return Err("table.missing_name".into());
        }
        if let Some(original) = &self.original {
            let mut update = TableUpdate {
                name: (name != original.name).then_some(name),
                reference: (range != original.reference).then_some(range),
                style: (self.style != TableStyle::from_table(original))
                    .then_some(self.style.clone()),
                header_row: (self.has_headers != (original.header_row_count > 0))
                    .then_some(self.has_headers),
                totals_row: (self.has_totals != (original.totals_row_count > 0))
                    .then_some(self.has_totals),
                filter_buttons: (self.filter_buttons != original.auto_filter
                    || self.has_headers != (original.header_row_count > 0))
                    .then_some(self.filter_buttons),
                ..Default::default()
            };
            for (i, draft) in self.columns.iter().enumerate() {
                let old = column_draft(original, i);
                if draft.formula != old.formula {
                    update.calculated_columns.push((
                        i,
                        (!draft.formula.trim().is_empty()).then(|| draft.formula.clone()),
                    ));
                }
                if self.has_totals && draft.total != old.total {
                    update.totals.push((i, draft.total.clone()));
                }
            }
            Ok(TableCommand::Edit(original.name.clone(), update))
        } else {
            Ok(TableCommand::Create(
                range,
                TableOptions {
                    name: Some(name),
                    has_headers: self.has_headers,
                    style: self.style.clone(),
                },
            ))
        }
    }
}
