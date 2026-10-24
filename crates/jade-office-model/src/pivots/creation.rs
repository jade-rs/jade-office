//! Source/destination entry and text focus shared by both application shells.
use super::*;
use crate::text::TextEdit;
use std::{cell::Cell, rc::Rc};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotCreationField {
    Source,
    Name,
    Destination,
    SheetName,
}
pub struct PivotCreation {
    pub focus: PivotCreationField,
    pub new_sheet: bool,
    pub problem: Option<String>,
    pub input_bounds: Rc<Cell<Option<[f32; 4]>>>,
    fields: [TextEdit; 4],
    base_sheet: String,
}
pub fn parse_source(text: &str, fallback: &str) -> Result<PivotSource, String> {
    let text = text.trim().trim_start_matches('=');
    if let Some((sheet, at)) = text.rsplit_once('!') {
        let sheet =
            jade_xlsx_reader::formula::unquote_sheet_name(sheet).ok_or("pivot.invalid_source")?;
        let range = jade_xlsx_reader::CellRange::parse(at).map_err(|_| "pivot.invalid_source")?;
        Ok(PivotSource::Range { sheet, range })
    } else if let Ok(range) = jade_xlsx_reader::CellRange::parse(text) {
        Ok(PivotSource::Range {
            sheet: fallback.into(),
            range,
        })
    } else if !text.is_empty() {
        Ok(PivotSource::Name {
            sheet: Some(fallback.into()),
            name: text.into(),
        })
    } else {
        Err("pivot.invalid_source".into())
    }
}
impl PivotCreation {
    pub fn open<B: Backend>(
        book: &Workbook<B>,
        selected: jade_xlsx_reader::CellRange,
        at: CellRef,
    ) -> Self {
        let selected = if selected.start == selected.end {
            book.current_region(at).unwrap_or(selected)
        } else {
            selected
        };
        let model = book.engine().workbook();
        let name = (1u32..)
            .map(|n| format!("PivotTable{n}"))
            .find(|name| {
                !model
                    .pivots
                    .tables
                    .iter()
                    .any(|p| p.name.eq_ignore_ascii_case(name))
            })
            .unwrap_or_default();
        let sheet = (1u32..)
            .map(|n| format!("Pivot{n}"))
            .find(|name| {
                !model
                    .sheets
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case(name))
            })
            .unwrap_or_default();
        let source = format!(
            "{}!{}",
            jade_xlsx_reader::quote_sheet_name(&book.sheet_name()),
            crate::sheet::range_text(selected)
        );
        Self {
            focus: PivotCreationField::Source,
            new_sheet: true,
            problem: None,
            input_bounds: Default::default(),
            fields: [
                TextEdit::selected(source),
                TextEdit::selected(name),
                TextEdit::selected("A3".into()),
                TextEdit::selected(sheet),
            ],
            base_sheet: book.sheet_name(),
        }
    }
    pub fn value(&self, f: PivotCreationField) -> &str {
        &self.fields[f as usize].text
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.focus as usize]
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.focus as usize]
    }
    pub fn focus(&mut self, f: PivotCreationField) {
        self.focus = f;
        self.field_mut().whole = true;
        self.input_bounds.set(None);
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        if key == "tab" {
            let fields = [
                PivotCreationField::Source,
                PivotCreationField::Name,
                PivotCreationField::Destination,
                PivotCreationField::SheetName,
            ];
            let count = if self.new_sheet { 4 } else { 3 };
            let at = self.focus as usize;
            self.focus(fields[(at + if shift { count - 1 } else { 1 }) % count]);
            true
        } else if command && key == "a" {
            self.field_mut().whole = true;
            true
        } else {
            self.field_mut().edit_key(key)
        }
    }
    pub fn command(&self) -> Result<PivotCreate, String> {
        let source = parse_source(self.value(PivotCreationField::Source), &self.base_sheet)?;
        let location = self.value(PivotCreationField::Destination).trim();
        let (sheet, at) = match location.rsplit_once('!') {
            Some((sheet, at)) => (
                Some(
                    jade_xlsx_reader::formula::unquote_sheet_name(sheet)
                        .ok_or("pivot.invalid_origin")?,
                ),
                at,
            ),
            None => (None, location),
        };
        let origin = CellRef::parse(at).map_err(|_| "pivot.invalid_origin")?;
        let destination = if self.new_sheet {
            let name = self.value(PivotCreationField::SheetName).trim();
            if sheet
                .as_deref()
                .is_some_and(|s| !jade_xlsx_reader::formula::same_sheet_name(s, name))
            {
                return Err("pivot.invalid_origin".into());
            }
            PivotDestination::NewSheet {
                name: name.into(),
                after: Some(self.base_sheet.clone()),
                origin,
            }
        } else {
            PivotDestination::Existing {
                sheet: sheet.unwrap_or_else(|| self.base_sheet.clone()),
                origin,
            }
        };
        let name = self.value(PivotCreationField::Name).trim();
        Ok(PivotCreate {
            name: (!name.is_empty()).then(|| name.to_string()),
            source,
            destination,
            settings: PivotUpdate::default(),
        })
    }
    pub fn apply<B: Backend>(
        &self,
        book: &mut Workbook<B>,
    ) -> Result<jade_xlsx_headless_editor::PivotCreationSummary, String> {
        book.create_pivot_command(&self.command()?)
    }
}
/// One modal state keeps grid guards, IME text and candidate bounds consistent
/// while switching between creating and configuring an existing report.
pub struct PivotPanel {
    pub input_bounds: Rc<Cell<Option<[f32; 4]>>>,
    state: Panel,
}
enum Panel {
    Create(Box<PivotCreation>),
    Edit(Box<PivotDialog>),
}
impl PivotPanel {
    pub fn create(d: PivotCreation) -> Self {
        Self {
            input_bounds: d.input_bounds.clone(),
            state: Panel::Create(Box::new(d)),
        }
    }
    pub fn edit(d: PivotDialog) -> Self {
        Self {
            input_bounds: d.input_bounds.clone(),
            state: Panel::Edit(Box::new(d)),
        }
    }
    pub fn editor(&self) -> Option<&PivotDialog> {
        match &self.state {
            Panel::Edit(d) => Some(d),
            _ => None,
        }
    }
    pub fn editor_mut(&mut self) -> Option<&mut PivotDialog> {
        match &mut self.state {
            Panel::Edit(d) => Some(d),
            _ => None,
        }
    }
    pub fn creator(&self) -> Option<&PivotCreation> {
        match &self.state {
            Panel::Create(d) => Some(d),
            _ => None,
        }
    }
    pub fn creator_mut(&mut self) -> Option<&mut PivotCreation> {
        match &mut self.state {
            Panel::Create(d) => Some(d),
            _ => None,
        }
    }
    pub fn field(&self) -> &TextEdit {
        match &self.state {
            Panel::Create(d) => d.field(),
            Panel::Edit(d) => d.field(),
        }
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        match &mut self.state {
            Panel::Create(d) => d.field_mut(),
            Panel::Edit(d) => d.field_mut(),
        }
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        match &mut self.state {
            Panel::Create(d) => d.key(key, shift, command),
            Panel::Edit(d) => d.key(key, shift, command),
        }
    }
}
