//! Hyperlink authoring shared by the two worksheet shells.
use crate::{
    sheet::{CellRange, CellRef, Workbook},
    text::TextEdit,
    Backend,
};
use jade_xlsx_reader::Hyperlink;
impl<B: Backend> Workbook<B> {
    /// Notes take precedence over the saved hyperlink ScreenTip.
    pub fn hover_text(&self, at: CellRef) -> Option<&str> {
        self.note(at).or_else(|| {
            let link = self.hyperlink(at)?;
            link.tooltip
                .as_deref()
                .filter(|s| !s.is_empty())
                .or(link.target.as_deref())
                .or(link.location.as_deref())
        })
    }
    pub fn hyperlink(&self, at: CellRef) -> Option<&Hyperlink> {
        self.engine().workbook().sheets[self.sheet_index()].hyperlink_at(at)
    }
    pub fn insert_hyperlink(
        &mut self,
        link: &Hyperlink,
        label: Option<&str>,
    ) -> Result<bool, String> {
        let name = self.sheet_name();
        self.engine_mut()
            .insert_hyperlink(&name, link, label)
            .map_err(|e| e.to_string())?;
        self.wrote(1)?;
        Ok(true)
    }
    pub fn clear_hyperlinks(&mut self, ranges: &[CellRange]) -> Result<bool, String> {
        let name = self.sheet_name();
        if !self
            .engine_mut()
            .clear_hyperlinks(&name, ranges)
            .map_err(|e| e.to_string())?
        {
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum LinkField {
    Address,
    Label,
    Tooltip,
}
impl LinkField {
    pub const ALL: [Self; 3] = [Self::Address, Self::Label, Self::Tooltip];
    pub fn key(self) -> &'static str {
        match self {
            Self::Address => "link.address",
            Self::Label => "link.label",
            Self::Tooltip => "link.tooltip",
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Address => 0,
            Self::Label => 1,
            Self::Tooltip => 2,
        }
    }
}
pub struct LinkDialog {
    pub field: LinkField,
    fields: [TextEdit; 3],
    pub places: Vec<(String, String)>,
    pub problem: Option<&'static str>,
    pub input_bounds: std::rc::Rc<std::cell::Cell<Option<[f32; 4]>>>,
    range: CellRange,
    original: Option<Hyperlink>,
    original_address: String,
    formula: bool,
    original_label: String,
}
fn address(link: &Hyperlink) -> String {
    match (&link.target, &link.location) {
        (Some(t), Some(l)) => format!("{t}#{l}"),
        (Some(t), None) => t.clone(),
        (None, Some(l)) => format!("#{l}"),
        _ => String::new(),
    }
}
impl LinkDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, range: CellRange, at: CellRef) -> Self {
        let original = book.hyperlink(at).cloned();
        let original_address = original.as_ref().map(address).unwrap_or_default();
        let formula = book.edit_text(range.start).starts_with('=');
        let label = book.display_text(range.start);
        let mut fields = [
            TextEdit::new(original_address.clone()),
            TextEdit::new(label.clone()),
            TextEdit::new(
                original
                    .as_ref()
                    .and_then(|l| l.tooltip.clone())
                    .unwrap_or_default(),
            ),
        ];
        fields[0].whole = true;
        let mut places = Vec::new();
        for sheet in &book.engine().workbook().sheets {
            if sheet.visibility == jade_xlsx_reader::SheetVisibility::Visible {
                places.push((
                    sheet.name.clone(),
                    format!("#'{}'!A1", sheet.name.replace('\'', "''")),
                ));
            }
        }
        for name in &book.engine().workbook().defined_names {
            if !name.hidden
                && !name.name.starts_with("_xlnm.")
                && name
                    .local_sheet_index
                    .is_none_or(|i| i == book.sheet_index())
                && book.resolve_reference(&name.name).is_some()
            {
                places.push((name.name.clone(), format!("#{}", name.name)));
            }
        }
        Self {
            field: LinkField::Address,
            fields,
            places,
            problem: None,
            input_bounds: std::rc::Rc::new(std::cell::Cell::new(None)),
            range,
            original,
            original_address,
            formula,
            original_label: label,
        }
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.field.index()]
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.field.index()]
    }
    pub fn value(&self, field: LinkField) -> &str {
        &self.fields[field.index()].text
    }
    pub fn focus(&mut self, field: LinkField) {
        self.field = field;
        self.field_mut().whole = true;
        self.input_bounds.set(None);
    }
    pub fn choose_place(&mut self, index: usize) {
        if let Some((_, value)) = self.places.get(index) {
            self.fields[0] = TextEdit::new(value.clone());
            self.focus(LinkField::Address);
        }
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        if key == "tab" {
            self.focus(LinkField::ALL[(self.field.index() + if shift { 2 } else { 1 }) % 3]);
            true
        } else if command && key == "a" {
            self.field_mut().whole = true;
            true
        } else {
            self.field_mut().edit_key(key)
        }
    }
    pub fn label(&self) -> Option<&str> {
        if self.formula
            || self.value(LinkField::Label).is_empty()
            || self.value(LinkField::Label) == self.original_label
        {
            None
        } else {
            Some(self.value(LinkField::Label))
        }
    }
    pub fn rule(&self) -> Result<Hyperlink, &'static str> {
        let value = self.value(LinkField::Address).trim();
        if value.is_empty() || value == "#" {
            return Err("link.missing_address");
        }
        let (target, location) = if value == self.original_address {
            let old = self.original.as_ref().ok_or("link.missing_address")?;
            (old.target.clone(), old.location.clone())
        } else if let Some(location) = value.strip_prefix('#') {
            (None, Some(location.to_string()))
        } else {
            (Some(value.to_string()), None)
        };
        Ok(Hyperlink {
            reference: self.range,
            target,
            location,
            display: self
                .label()
                .map(str::to_string)
                .or_else(|| self.original.as_ref().and_then(|l| l.display.clone())),
            tooltip: (!self.value(LinkField::Tooltip).is_empty())
                .then(|| self.value(LinkField::Tooltip).to_string()),
        })
    }
}
