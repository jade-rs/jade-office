//! Names shared by the name box, hyperlinks and validation lists.
use crate::{sheet::Workbook, Backend};
use jade_xlsx_reader::{quote_sheet_name, CellRange, DefinedName};

impl<B: Backend> Workbook<B> {
    /// Resolves an A1 address or a range name in the active sheet's scope.
    /// Sheet qualifiers and quoted sheet names are accepted. Expressions
    /// that do not denote a simple rectangular range cannot be navigated to.
    pub fn resolve_reference(&self, text: &str) -> Option<(usize, CellRange)> {
        resolve(
            self.engine().workbook(),
            self.sheet_index(),
            text,
            &mut Vec::new(),
        )
    }

    pub fn defined_names(&self) -> &[DefinedName] {
        &self.engine().workbook().defined_names
    }

    /// Defines the active selection as a workbook-scoped absolute range.
    pub fn define_range_name(&mut self, name: &str, range: CellRange) -> Result<bool, String> {
        if self.defined_names().iter().any(|n| {
            n.name.to_lowercase() == name.trim().to_lowercase()
                && (n.local_sheet_index.is_none()
                    || n.local_sheet_index == Some(self.sheet_index()))
        }) {
            return Err("this name already exists".into());
        }
        let absolute = |cell: jade_xlsx_reader::CellRef| {
            format!(
                "${}${}",
                jade_xlsx_layout::column_label(cell.column),
                cell.row + 1
            )
        };
        self.set_defined_name(&DefinedName {
            name: name.trim().into(),
            formula: format!(
                "{}!{}:{}",
                quote_sheet_name(&self.sheet_name()),
                absolute(range.start),
                absolute(range.end)
            ),
            local_sheet_index: None,
            hidden: false,
        })
    }

    pub fn set_defined_name(&mut self, definition: &DefinedName) -> Result<bool, String> {
        if !self
            .engine_mut()
            .set_defined_name(definition)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }

    pub fn delete_defined_name(
        &mut self,
        name: &str,
        scope: Option<usize>,
    ) -> Result<bool, String> {
        if !self
            .engine_mut()
            .delete_defined_name(name, scope)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
}

fn resolve(
    book: &jade_xlsx_reader::Workbook,
    context: usize,
    text: &str,
    seen: &mut Vec<(usize, String)>,
) -> Option<(usize, CellRange)> {
    if seen.len() >= 64 {
        return None;
    }
    let text = text.trim().trim_start_matches('=');
    let (sheet, reference) = match text.rsplit_once('!') {
        Some((name, reference)) => {
            let name = name.trim().trim_matches('\'').replace("''", "'");
            let sheet = book
                .sheets
                .iter()
                .position(|s| s.name.to_lowercase() == name.to_lowercase())?;
            (sheet, reference)
        }
        None => (context, text),
    };
    if let Ok(range) = CellRange::parse(reference) {
        return Some((
            sheet,
            CellRange {
                start: jade_xlsx_reader::CellRef::new(
                    range.start.column.min(range.end.column),
                    range.start.row.min(range.end.row),
                ),
                end: jade_xlsx_reader::CellRef::new(
                    range.start.column.max(range.end.column),
                    range.start.row.max(range.end.row),
                ),
            },
        ));
    }
    let key = (sheet, reference.to_lowercase());
    if seen.contains(&key) {
        return None;
    }
    seen.push(key);
    let names = &book.defined_names;
    let matches = |n: &&DefinedName| n.name.to_lowercase() == reference.to_lowercase();
    let name = names
        .iter()
        .filter(matches)
        .find(|n| n.local_sheet_index == Some(sheet))
        .or_else(|| {
            names
                .iter()
                .filter(matches)
                .find(|n| n.local_sheet_index.is_none())
        })?;
    resolve(book, sheet, &name.formula, seen)
}
