//! Field layout, value settings and member choices shared by both app shells.
use super::*;
use crate::text::TextEdit;
use jade_xlsx_reader::pivot::PivotScalar;
use std::collections::BTreeSet;
pub const PIVOT_AGGREGATIONS: &[&str] = &[
    "sum",
    "count",
    "average",
    "max",
    "min",
    "product",
    "countNums",
    "stdDev",
    "stdDevp",
    "var",
    "varp",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotDialogField {
    Source,
    Origin,
    MeasureCaption,
    FieldCaption,
    Search,
}
impl PivotDialogField {
    fn index(self) -> usize {
        self as usize
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PivotArea {
    None,
    Rows,
    Columns,
    Pages,
}
pub struct PivotMember {
    pub index: u32,
    pub label: String,
    pub blank: bool,
}
pub struct PivotDialog {
    pub name: String,
    pub selected_field: usize,
    pub selected_measure: usize,
    pub draft: PivotUpdate,
    pub field_names: Vec<String>,
    pub members: Vec<Vec<PivotMember>>,
    pub styles: Vec<Option<String>>,
    pub member_offset: usize,
    pub shared_reports: usize,
    pub problem: Option<String>,
    pub show_styles: bool,
    pub input_bounds: std::rc::Rc<std::cell::Cell<Option<[f32; 4]>>>,
    pub focus: PivotDialogField,
    fields: [TextEdit; 5],
    original: PivotUpdate,
    original_filters: Vec<Option<BTreeSet<u32>>>,
    filters: Vec<Option<BTreeSet<u32>>>,
    source_sheet: Option<String>,
    report_part: String,
    report_sheet: String,
    original_cache: std::sync::Arc<jade_xlsx_reader::pivot::PivotCache>,
}
impl PivotDialog {
    pub fn open<B: Backend>(book: &Workbook<B>, at: CellRef) -> Result<Self, String> {
        let table = book.pivot_at(at).ok_or("select a cell in a pivot table")?;
        let model = book.engine().workbook();
        let original = jade_xlsx_headless_editor::pivot_configuration(model, table)
            .map_err(|e| e.to_string())?;
        let cache = model
            .pivots
            .caches
            .iter()
            .find(|c| c.id == table.cache_id)
            .ok_or("missing pivot cache")?;
        if cache.fields.is_empty() {
            return Err("pivot has no source fields".into());
        }
        let source = match original.source.as_ref().unwrap() {
            PivotSource::Range { sheet, range } => format!(
                "{}!{}",
                jade_xlsx_reader::quote_sheet_name(sheet),
                range_text(*range)
            ),
            PivotSource::Name { name, .. } => name.clone(),
        };
        let origin =
            crate::sheet::reference_text(original.layout.as_ref().unwrap().origin.unwrap());
        let mut filters = vec![None; cache.fields.len()];
        for f in &original.member_filters {
            filters[f.field as usize] = f.visible.clone();
        }
        for p in &original.layout.as_ref().unwrap().pages {
            filters[p.field as usize] = match &p.selection {
                PivotFilterSelection::All => None,
                PivotFilterSelection::Item(i) => Some([*i].into_iter().collect()),
                PivotFilterSelection::Items(items) => Some(items.clone()),
            };
        }
        let members = cache
            .fields
            .iter()
            .enumerate()
            .map(|(i, f)| {
                f.shared_items
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let caption = table.fields[i]
                            .items
                            .iter()
                            .find(|m| m.shared_index == Some(index as u32))
                            .and_then(|m| m.caption.clone())
                            .or_else(|| value.caption.clone());
                        let label = caption.unwrap_or_else(|| match &value.value {
                            PivotScalar::Missing => String::new(),
                            PivotScalar::Text(s) | PivotScalar::Date(s) | PivotScalar::Error(s) => {
                                s.clone()
                            }
                            PivotScalar::Number(n) => n.to_string(),
                            PivotScalar::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
                            _ => String::new(),
                        });
                        PivotMember {
                            index: index as u32,
                            label,
                            blank: matches!(value.value, PivotScalar::Missing),
                        }
                    })
                    .collect()
            })
            .collect();
        let mut styles = vec![None];
        styles.extend(
            jade_xlsx_layout::pivot_style::style_names(&model.styles)
                .into_iter()
                .map(Some),
        );
        if let Some(name) = original.style.as_ref().and_then(|s| s.name.clone()) {
            if !styles.iter().any(|s| s.as_deref() == Some(name.as_str())) {
                styles.push(Some(name));
            }
        }
        let captions = original.measures.as_ref().unwrap();
        let field_caption = original
            .field_options
            .first()
            .and_then(|f| f.caption.clone())
            .unwrap_or_default();
        Ok(Self {
            name: table.name.clone(),
            selected_field: 0,
            selected_measure: 0,
            fields: [
                TextEdit::selected(source),
                TextEdit::selected(origin),
                TextEdit::selected(
                    captions
                        .first()
                        .map(|m| m.caption.clone())
                        .unwrap_or_default(),
                ),
                TextEdit::selected(field_caption),
                TextEdit::new(String::new()),
            ],
            draft: original.clone(),
            original,
            field_names: cache.fields.iter().map(|f| f.name.clone()).collect(),
            members,
            styles,
            member_offset: 0,
            shared_reports: model
                .pivots
                .tables
                .iter()
                .filter(|t| t.cache_id == table.cache_id)
                .count(),
            problem: None,
            show_styles: false,
            input_bounds: Default::default(),
            focus: PivotDialogField::Source,
            original_filters: filters.clone(),
            filters,
            source_sheet: cache.source.sheet.clone(),
            report_part: table.part_name.clone(),
            report_sheet: table.sheet_part.clone(),
            original_cache: cache.clone(),
        })
    }
    pub fn value(&self, f: PivotDialogField) -> &str {
        &self.fields[f.index()].text
    }
    pub fn field(&self) -> &TextEdit {
        &self.fields[self.focus.index()]
    }
    pub fn field_mut(&mut self) -> &mut TextEdit {
        &mut self.fields[self.focus.index()]
    }
    pub fn focus(&mut self, f: PivotDialogField) {
        self.focus = f;
        self.field_mut().whole = true;
        self.input_bounds.set(None);
    }
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> bool {
        if key == "tab" {
            let all = [
                PivotDialogField::Source,
                PivotDialogField::Origin,
                PivotDialogField::MeasureCaption,
                PivotDialogField::FieldCaption,
                PivotDialogField::Search,
            ];
            self.focus(all[(self.focus.index() + if shift { 4 } else { 1 }) % 5]);
            true
        } else if command && key == "a" {
            self.field_mut().whole = true;
            true
        } else {
            self.field_mut().edit_key(key)
        }
    }
    pub fn flush(&mut self) {
        if let Some(m) = self
            .draft
            .measures
            .as_mut()
            .and_then(|m| m.get_mut(self.selected_measure))
        {
            m.caption = self.fields[2].text.clone();
        }
        if let Some(f) = self.draft.field_options.get_mut(self.selected_field) {
            f.caption = Some(self.fields[3].text.clone());
        }
    }
    pub fn select_field(&mut self, i: usize) {
        if i >= self.field_names.len() {
            return;
        }
        self.flush();
        self.selected_field = i;
        self.fields[3] = TextEdit::selected(
            self.draft.field_options[i]
                .caption
                .clone()
                .unwrap_or_default(),
        );
        self.member_offset = 0;
        self.fields[4] = TextEdit::new(String::new());
    }
    pub fn select_measure(&mut self, i: usize) {
        if i >= self.draft.measures.as_ref().map_or(0, Vec::len) {
            return;
        }
        self.flush();
        self.selected_measure = i;
        self.fields[2] =
            TextEdit::selected(self.draft.measures.as_ref().unwrap()[i].caption.clone());
    }
    pub fn area(&self, field: usize) -> PivotArea {
        let l = self.draft.layout.as_ref().unwrap();
        let f = field as i32;
        if l.rows.contains(&f) {
            PivotArea::Rows
        } else if l.columns.contains(&f) {
            PivotArea::Columns
        } else if l.pages.iter().any(|p| p.field == field as u32) {
            PivotArea::Pages
        } else {
            PivotArea::None
        }
    }
    pub fn set_area(&mut self, area: PivotArea) {
        self.flush();
        let f = self.selected_field as i32;
        let l = self.draft.layout.as_mut().unwrap();
        l.rows.retain(|i| *i != f);
        l.columns.retain(|i| *i != f);
        l.pages.retain(|p| p.field != f as u32);
        match area {
            PivotArea::Rows => l.rows.push(f),
            PivotArea::Columns => l.columns.push(f),
            PivotArea::Pages => l.pages.push(PivotPage {
                field: f as u32,
                selection: PivotFilterSelection::All,
            }),
            PivotArea::None => {}
        }
    }
    pub fn move_field(&mut self, forward: bool) {
        let f = self.selected_field as i32;
        let l = self.draft.layout.as_mut().unwrap();
        for axis in [&mut l.rows, &mut l.columns] {
            if let Some(i) = axis.iter().position(|x| *x == f) {
                let target = if forward {
                    i.checked_add(1)
                } else {
                    i.checked_sub(1)
                };
                if let Some(j) = target.filter(|j| *j < axis.len()) {
                    axis.swap(i, j);
                }
                return;
            }
        }
        if let Some(i) = l.pages.iter().position(|p| p.field == f as u32) {
            let target = if forward {
                i.checked_add(1)
            } else {
                i.checked_sub(1)
            };
            if let Some(j) = target.filter(|j| *j < l.pages.len()) {
                l.pages.swap(i, j);
            }
        }
    }
    pub fn values_on_rows(&mut self, on_rows: bool) {
        let l = self.draft.layout.as_mut().unwrap();
        l.rows.retain(|i| *i != -2);
        l.columns.retain(|i| *i != -2);
        if on_rows {
            l.rows.push(-2)
        } else {
            l.columns.push(-2)
        }
    }
    pub fn add_measure(&mut self) {
        self.flush();
        let field = self.selected_field as u32;
        let measures = self.draft.measures.as_mut().unwrap();
        let base = format!("sum of {}", self.field_names[self.selected_field]);
        let mut caption = base.clone();
        let mut i = 2;
        while measures
            .iter()
            .any(|m| m.caption.eq_ignore_ascii_case(&caption))
        {
            caption = format!("{base} {i}");
            i += 1;
        }
        measures.push(PivotMeasure {
            existing: None,
            field,
            aggregation: "sum".into(),
            caption,
            number_format_id: None,
        });
        self.selected_measure = measures.len() - 1;
        self.fields[2] = TextEdit::selected(measures[self.selected_measure].caption.clone());
    }
    pub fn remove_measure(&mut self) {
        self.flush();
        let m = self.draft.measures.as_mut().unwrap();
        if m.len() > 1 {
            m.remove(self.selected_measure);
            self.selected_measure = self.selected_measure.min(m.len() - 1);
            self.fields[2] = TextEdit::selected(m[self.selected_measure].caption.clone());
        }
    }
    pub fn move_measure(&mut self, forward: bool) {
        self.flush();
        let m = self.draft.measures.as_mut().unwrap();
        let i = self.selected_measure;
        let target = if forward {
            i.checked_add(1)
        } else {
            i.checked_sub(1)
        };
        if let Some(j) = target.filter(|j| *j < m.len()) {
            m.swap(i, j);
            self.selected_measure = j;
        }
    }
    pub fn next_aggregation(&mut self) {
        let Some(m) = self
            .draft
            .measures
            .as_mut()
            .unwrap()
            .get_mut(self.selected_measure)
        else {
            return;
        };
        let i = PIVOT_AGGREGATIONS
            .iter()
            .position(|s| *s == m.aggregation)
            .unwrap_or(0);
        m.aggregation = PIVOT_AGGREGATIONS[(i + 1) % PIVOT_AGGREGATIONS.len()].into();
    }
    pub fn visible(&self, index: u32) -> bool {
        self.filters[self.selected_field]
            .as_ref()
            .is_none_or(|s| s.contains(&index))
    }
    pub fn toggle_member(&mut self, index: u32) {
        let field = self.selected_field;
        let chosen = self.filters[field]
            .get_or_insert_with(|| self.members[field].iter().map(|m| m.index).collect());
        if !chosen.remove(&index) {
            chosen.insert(index);
        }
    }
    pub fn all_members(&mut self, all: bool) {
        self.filters[self.selected_field] = if all { None } else { Some(BTreeSet::new()) };
    }
    pub fn shown_members(&self) -> Vec<&PivotMember> {
        let search = self.value(PivotDialogField::Search).to_lowercase();
        let count = self.members[self.selected_field]
            .iter()
            .filter(|m| search.is_empty() || m.label.to_lowercase().contains(&search))
            .count();
        let offset = if self.member_offset < count {
            self.member_offset
        } else {
            0
        };
        self.members[self.selected_field]
            .iter()
            .filter(|m| search.is_empty() || m.label.to_lowercase().contains(&search))
            .skip(offset)
            .take(100)
            .collect()
    }
    pub fn next_members(&mut self, forward: bool) {
        let search = self.value(PivotDialogField::Search).to_lowercase();
        let count = self.members[self.selected_field]
            .iter()
            .filter(|m| search.is_empty() || m.label.to_lowercase().contains(&search))
            .count();
        if self.member_offset >= count {
            self.member_offset = 0;
        }
        if forward {
            if self.member_offset.saturating_add(100) < count {
                self.member_offset = self.member_offset.saturating_add(100);
            }
        } else {
            self.member_offset = self.member_offset.saturating_sub(100);
        }
    }
    pub fn next_layout(&mut self) {
        let l = self.draft.layout.as_mut().unwrap();
        l.report_layout = match l.report_layout {
            PivotReportLayout::Compact => PivotReportLayout::Outline,
            PivotReportLayout::Outline => PivotReportLayout::Tabular,
            PivotReportLayout::Tabular => PivotReportLayout::Compact,
        };
    }
    pub fn next_style(&mut self) {
        let style = self.draft.style.get_or_insert(PivotStyle {
            name: None,
            row_headers: true,
            column_headers: true,
            row_stripes: false,
            column_stripes: false,
            last_column: false,
        });
        let at = self
            .styles
            .iter()
            .position(|s| *s == style.name)
            .unwrap_or(0);
        style.name = self.styles[(at + 1) % self.styles.len()].clone();
    }
    pub fn command(&mut self) -> Result<PivotUpdate, String> {
        self.flush();
        let mut update = PivotUpdate::default();
        let source = self
            .value(PivotDialogField::Source)
            .trim()
            .trim_start_matches('=');
        let original_source = match self.original.source.as_ref().unwrap() {
            PivotSource::Range { sheet, range } => format!(
                "{}!{}",
                jade_xlsx_reader::quote_sheet_name(sheet),
                range_text(*range)
            ),
            PivotSource::Name { name, .. } => name.clone(),
        };
        if source != original_source {
            if let Some((sheet, at)) = source.rsplit_once('!') {
                if let Ok(range) = jade_xlsx_reader::CellRange::parse(at) {
                    let sheet = jade_xlsx_reader::formula::unquote_sheet_name(sheet)
                        .ok_or("pivot.invalid_source")?;
                    update.source = Some(PivotSource::Range { sheet, range });
                } else {
                    return Err("pivot.invalid_source".into());
                }
            } else if let Ok(range) = jade_xlsx_reader::CellRange::parse(source) {
                update.source = Some(PivotSource::Range {
                    sheet: self.source_sheet.clone().ok_or("pivot.invalid_source")?,
                    range,
                });
            } else if !source.is_empty() {
                update.source = Some(PivotSource::Name {
                    sheet: self.source_sheet.clone(),
                    name: source.into(),
                });
            } else {
                return Err("pivot.invalid_source".into());
            }
        }
        let origin = CellRef::parse(self.value(PivotDialogField::Origin).trim())
            .map_err(|_| "pivot.invalid_origin")?;
        let mut l = self.draft.layout.clone().unwrap();
        l.origin = Some(origin);
        for page in &mut l.pages {
            let field = page.field as usize;
            let unchanged = self
                .original
                .layout
                .as_ref()
                .unwrap()
                .pages
                .iter()
                .find(|p| p.field == page.field)
                .filter(|_| self.filters[field] == self.original_filters[field]);
            page.selection = if let Some(old) = unchanged {
                old.selection.clone()
            } else {
                match &self.filters[field] {
                    None => PivotFilterSelection::All,
                    Some(items) if items.len() == 1 => {
                        PivotFilterSelection::Item(*items.first().unwrap())
                    }
                    Some(items) => PivotFilterSelection::Items(items.clone()),
                }
            };
        }
        if Some(&l) != self.original.layout.as_ref() {
            update.layout = Some(l.clone());
        }
        for (i, visible) in self
            .filters
            .iter()
            .enumerate()
            .filter(|(i, _)| !l.pages.iter().any(|p| p.field == *i as u32))
        {
            if *visible != self.original_filters[i]
                || self
                    .original
                    .layout
                    .as_ref()
                    .unwrap()
                    .pages
                    .iter()
                    .any(|p| p.field == i as u32)
            {
                update.member_filters.push(PivotMemberFilter {
                    field: i as u32,
                    visible: visible.clone(),
                });
            }
        }
        if self.draft.measures != self.original.measures {
            update.measures = self.draft.measures.clone();
        }
        for (now, old) in self
            .draft
            .field_options
            .iter()
            .zip(&self.original.field_options)
        {
            let change = PivotFieldOptions {
                field: now.field,
                caption: (now.caption != old.caption)
                    .then(|| now.caption.clone())
                    .flatten(),
                sort: (now.sort != old.sort).then(|| now.sort.clone()).flatten(),
                subtotals: if now.subtotals != old.subtotals {
                    now.subtotals
                } else {
                    None
                },
                subtotals_at_top: if now.subtotals_at_top != old.subtotals_at_top {
                    now.subtotals_at_top
                } else {
                    None
                },
            };
            if change.caption.is_some()
                || change.sort.is_some()
                || change.subtotals.is_some()
                || change.subtotals_at_top.is_some()
            {
                update.field_options.push(change);
            }
        }
        if self.draft.style != self.original.style {
            update.style = self.draft.style.clone();
        }
        if self.draft.fit_columns != self.original.fit_columns {
            update.fit_columns = self.draft.fit_columns;
        }
        Ok(update)
    }
    fn ensure_current<B: Backend>(&self, book: &Workbook<B>) -> Result<(), String> {
        let model = book.engine().workbook();
        let table = model
            .pivots
            .tables
            .iter()
            .find(|t| t.part_name == self.report_part && t.sheet_part == self.report_sheet)
            .ok_or("pivot.stale")?;
        let cache = model
            .pivots
            .caches
            .iter()
            .find(|c| c.id == table.cache_id)
            .ok_or("pivot.stale")?;
        let names = cache
            .fields
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<_>>();
        if model.sheets[book.sheet_index()].part_name != self.report_sheet
            || names != self.field_names
            || cache.id != self.original_cache.id
            || self
                .original_cache
                .fields
                .iter()
                .zip(&cache.fields)
                .any(|(old, new)| {
                    old.shared_items.len() > new.shared_items.len()
                        || old
                            .shared_items
                            .iter()
                            .zip(&new.shared_items)
                            .any(|(a, b)| a.value != b.value)
                })
            || model
                .pivots
                .tables
                .iter()
                .filter(|t| t.cache_id == table.cache_id)
                .count()
                != self.shared_reports
            || jade_xlsx_headless_editor::pivot_configuration(model, table)
                .map_err(|e| e.to_string())?
                != self.original
        {
            return Err("pivot.stale".into());
        }
        Ok(())
    }
    /// Explicitly refresh saved members from the current source without
    /// discarding the pending editor choices. Only real cache indices are ever
    /// offered; direct numeric records become selectable after this refresh.
    pub fn refresh_items<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, String> {
        self.ensure_current(book)?;
        let pending = self.command()?;
        if pending.source.is_some() {
            return Err("pivot.apply_source_first".into());
        }
        let fields = self.fields.each_ref().map(|t| TextEdit {
            text: t.text.clone(),
            marked: t.marked.clone(),
            caret: t.caret,
            whole: t.whole,
        });
        let filters = self.filters.clone();
        let original_filters = self.original_filters.clone();
        let focus = self.focus;
        let selected_field = self.selected_field;
        let selected_measure = self.selected_measure;
        book.refresh_pivot_named(&self.name)?;
        let at = book
            .engine()
            .workbook()
            .pivots
            .tables
            .iter()
            .find(|t| t.part_name == self.report_part)
            .ok_or("pivot.stale")?
            .location
            .reference
            .start;
        let mut fresh = Self::open(book, at)?;
        if let Some(l) = pending.layout {
            fresh.draft.layout = Some(l);
        }
        if let Some(m) = pending.measures {
            fresh.draft.measures = Some(m);
        }
        if let Some(s) = pending.style {
            fresh.draft.style = Some(s);
        }
        if let Some(f) = pending.fit_columns {
            fresh.draft.fit_columns = Some(f);
        }
        for change in pending.field_options {
            let field = &mut fresh.draft.field_options[change.field as usize];
            if change.caption.is_some() {
                field.caption = change.caption;
            }
            if change.sort.is_some() {
                field.sort = change.sort;
            }
            if change.subtotals.is_some() {
                field.subtotals = change.subtotals;
            }
            if change.subtotals_at_top.is_some() {
                field.subtotals_at_top = change.subtotals_at_top;
            }
        }
        for i in 0..filters.len() {
            if filters[i] != original_filters[i] {
                fresh.filters[i] = filters[i].clone();
            }
        }
        fresh.fields = fields;
        fresh.focus = focus;
        fresh.selected_field = selected_field;
        fresh.selected_measure = selected_measure.min(
            fresh
                .draft
                .measures
                .as_ref()
                .unwrap()
                .len()
                .saturating_sub(1),
        );
        *self = fresh;
        Ok(true)
    }
    pub fn apply<B: Backend>(&mut self, book: &mut Workbook<B>) -> Result<bool, String> {
        self.ensure_current(book)?;
        let update = self.command()?;
        if update == PivotUpdate::default() {
            return Ok(false);
        }
        book.update_pivot_named(&self.name, &update)
    }
}
fn range_text(range: jade_xlsx_reader::CellRange) -> String {
    if range.start == range.end {
        crate::sheet::reference_text(range.start)
    } else {
        format!(
            "{}:{}",
            crate::sheet::reference_text(range.start),
            crate::sheet::reference_text(range.end)
        )
    }
}
