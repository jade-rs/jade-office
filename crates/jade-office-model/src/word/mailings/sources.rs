//! A merge's data source and its recipients: a CSV or text file (UTF-8 or
//! Shift_JIS, commas or tabs) or an Excel workbook's first sheet read into
//! columns and records, the list made here written as CSV, and which
//! records are merged — 宛先リストの編集's check boxes, its sort by a
//! column and its filters.

use jade_docx_editor::word_document::mail_merge::MergeData;

/// The data in `bytes`, a file named `name`: the first row is the header.
pub fn read_data_source(name: &str, bytes: &[u8]) -> Result<MergeData, String> {
    let lower = name.to_ascii_lowercase();
    let rows = if lower.ends_with(".xlsx") || lower.ends_with(".xlsm") || bytes.starts_with(b"PK") {
        workbook_rows(bytes)?
    } else {
        let text = crate::csv::decode(bytes);
        let separator = if lower.ends_with(".tsv")
            || (!lower.ends_with(".csv")
                && text.lines().next().is_some_and(|line| line.contains('\t')))
        {
            '\t'
        } else {
            ','
        };
        crate::clipboard::parse_separated(&text, separator)
    };
    let data = MergeData::from_rows(rows);
    if data.columns.is_empty() {
        return Err("データ ソースに列がありません".to_string());
    }
    Ok(data)
}

/// An Excel workbook's first visible sheet as rows of text.
fn workbook_rows(bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    use jade_xlsx_reader::CellValue;
    let workbook = jade_xlsx_reader::read_xlsx(bytes)
        .map_err(|error| format!("ブックを読めません: {error}"))?;
    let sheet = workbook
        .sheets
        .iter()
        .find(|sheet| !sheet.rows.is_empty())
        .ok_or("ブックにデータがありません")?;
    let width = sheet
        .rows
        .iter()
        .flat_map(|row| row.cells.iter().map(|cell| cell.reference.column + 1))
        .max()
        .unwrap_or(0) as usize;
    let first_row = sheet.rows.first().map_or(0, |row| row.index);
    let mut rows: Vec<Vec<String>> = Vec::new();
    for row in &sheet.rows {
        let at = (row.index - first_row) as usize;
        if rows.len() <= at {
            rows.resize(at + 1, vec![String::new(); width]);
        }
        for cell in &row.cells {
            let text = match &cell.value {
                CellValue::Empty => String::new(),
                CellValue::Text(text) => text.clone(),
                CellValue::Bool(value) => if *value { "TRUE" } else { "FALSE" }.to_string(),
                CellValue::Error(error) => error.clone(),
                CellValue::Number(number) => {
                    if number.fract() == 0.0 && number.abs() < 1e15 {
                        format!("{}", *number as i64)
                    } else {
                        format!("{number}")
                    }
                }
            };
            if let Some(slot) = rows[at].get_mut(cell.reference.column as usize) {
                *slot = text;
            }
        }
    }
    Ok(rows)
}

/// The list as CSV: UTF-8 with a BOM, every field quoted, CR LF.
pub fn csv_bytes(data: &MergeData) -> Vec<u8> {
    let quote = |value: &str| format!("\"{}\"", value.replace('"', "\"\""));
    let mut text = String::from("\u{feff}");
    for row in std::iter::once(&data.columns).chain(data.records.iter()) {
        text.push_str(
            &row.iter()
                .map(|value| quote(value))
                .collect::<Vec<_>>()
                .join(","),
        );
        text.push_str("\r\n");
    }
    text.into_bytes()
}

/// How a filter compares a column with its value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FilterOp {
    Equal,
    NotEqual,
    Contains,
    NotContains,
    Empty,
    NotEmpty,
    Less,
    Greater,
}

impl FilterOp {
    pub const ALL: [FilterOp; 8] = [
        FilterOp::Equal,
        FilterOp::NotEqual,
        FilterOp::Contains,
        FilterOp::NotContains,
        FilterOp::Empty,
        FilterOp::NotEmpty,
        FilterOp::Less,
        FilterOp::Greater,
    ];

    pub fn label_key(self) -> &'static str {
        match self {
            FilterOp::Equal => "mail.filter.equal",
            FilterOp::NotEqual => "mail.filter.not_equal",
            FilterOp::Contains => "mail.filter.contains",
            FilterOp::NotContains => "mail.filter.not_contains",
            FilterOp::Empty => "mail.filter.empty",
            FilterOp::NotEmpty => "mail.filter.not_empty",
            FilterOp::Less => "mail.filter.less",
            FilterOp::Greater => "mail.filter.greater",
        }
    }

    fn holds(self, value: &str, against: &str) -> bool {
        let numbers = (
            value.trim().replace(',', "").parse::<f64>(),
            against.trim().replace(',', "").parse::<f64>(),
        );
        let ordering = match numbers {
            (Ok(a), Ok(b)) => a.partial_cmp(&b),
            _ => Some(value.cmp(against)),
        };
        match self {
            FilterOp::Equal => value == against,
            FilterOp::NotEqual => value != against,
            FilterOp::Contains => value.contains(against),
            FilterOp::NotContains => !value.contains(against),
            FilterOp::Empty => value.trim().is_empty(),
            FilterOp::NotEmpty => !value.trim().is_empty(),
            FilterOp::Less => ordering == Some(std::cmp::Ordering::Less),
            FilterOp::Greater => ordering == Some(std::cmp::Ordering::Greater),
        }
    }
}

/// A recipient filter: a column, a comparison and what it compares with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Filter {
    pub column: usize,
    pub op: FilterOp,
    pub value: String,
}

/// The recipient list: the data, and which records are merged in which
/// order.
#[derive(Clone, Debug, PartialEq)]
pub struct Recipients {
    pub data: MergeData,
    /// The source's file name, and where it is when that is known.
    pub name: String,
    pub path: Option<String>,
    /// Whether the list was made here, so 編集 can change it.
    pub editable: bool,
    /// Each record's check box.
    pub included: Vec<bool>,
    /// The column sorted on and whether upwards.
    pub sort: Option<(usize, bool)>,
    /// Every filter must hold.
    pub filters: Vec<Filter>,
}

impl Recipients {
    pub fn new(data: MergeData, name: &str, path: Option<String>, editable: bool) -> Self {
        Self {
            included: vec![true; data.records.len()],
            data,
            name: name.to_string(),
            path,
            editable,
            sort: None,
            filters: Vec::new(),
        }
    }

    /// The records the filters let through, in the sort's order.
    pub fn shown(&self) -> Vec<usize> {
        let mut records: Vec<usize> = (0..self.data.records.len())
            .filter(|&record| {
                self.filters.iter().all(|filter| {
                    filter
                        .op
                        .holds(self.data.value(record, filter.column), &filter.value)
                })
            })
            .collect();
        if let Some((column, upwards)) = self.sort {
            records.sort_by(|&a, &b| {
                let (a, b) = (self.data.value(a, column), self.data.value(b, column));
                let ordering = match (
                    a.trim().replace(',', "").parse::<f64>(),
                    b.trim().replace(',', "").parse::<f64>(),
                ) {
                    (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
                    _ => a.cmp(b),
                };
                if upwards {
                    ordering
                } else {
                    ordering.reverse()
                }
            });
        }
        records
    }

    /// The records merged: shown and ticked.
    pub fn chosen(&self) -> Vec<usize> {
        self.shown()
            .into_iter()
            .filter(|&record| self.included.get(record).copied().unwrap_or(true))
            .collect()
    }
}
