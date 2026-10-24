//! Shared bulk commands and deferred authentication of their actual writes.
use super::*;
use crate::protection::ProtectionDialog;
use jade_xlsx_headless_editor::{
    RangeAuthContext, RangeAuthentication, RangeEditRequirement, RangeUnlockCandidate,
    RangeUnlockStatus,
};

#[derive(Clone, Debug, PartialEq)]
pub enum ProtectedEdit {
    Paste {
        target: CellRange,
        text: String,
    },
    Clear {
        range: CellRange,
    },
    FillWith {
        range: CellRange,
        active: CellRef,
        text: String,
    },
    FillDown {
        range: CellRange,
    },
    FillRight {
        range: CellRange,
    },
    FillSeries {
        source: CellRange,
        target: CellRange,
    },
    MoveRange {
        source: CellRange,
        to: CellRef,
        copy: bool,
    },
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EditOutcome {
    pub changed: bool,
    pub selection: Option<CellRange>,
    active_origin: Option<CellRef>,
}
impl EditOutcome {
    pub fn active_cell(&self, current: CellRef) -> CellRef {
        let Some(range) = self.selection else {
            return current;
        };
        if let Some(origin) = self.active_origin {
            let column = (i64::from(range.start.column) + i64::from(current.column)
                - i64::from(origin.column))
            .clamp(i64::from(range.start.column), i64::from(range.end.column))
                as u32;
            let row = (i64::from(range.start.row) + i64::from(current.row) - i64::from(origin.row))
                .clamp(i64::from(range.start.row), i64::from(range.end.row))
                as u32;
            CellRef::new(column, row)
        } else if range.contains(current) {
            current
        } else {
            range.start
        }
    }

    fn changed(changed: bool) -> Self {
        Self {
            changed,
            selection: None,
            active_origin: None,
        }
    }
}
/// Both shells use this path for the initial attempt and the same dialog for
/// its retry. Failure leaves all document, clipboard and history state intact.
pub fn attempt<B: Backend>(
    book: &mut Workbook<B>,
    dialog: &mut Option<ProtectionDialog>,
    edit: ProtectedEdit,
) -> Result<EditOutcome, String> {
    let result = book.execute_protected_edit(&edit);
    if result.as_ref().is_err_and(|e| protected_refusal(e)) {
        if let Ok(Some(prompt)) = ProtectionDialog::for_operation(book, edit) {
            *dialog = Some(prompt);
        }
    }
    result
}
impl<B: Backend> Workbook<B> {
    pub fn execute_protected_edit(&mut self, edit: &ProtectedEdit) -> Result<EditOutcome, String> {
        self.validate_protected_edit(edit)?;
        self.atomic_edit(|book| book.execute_protected_edit_inner(edit))
    }
    fn execute_protected_edit_inner(
        &mut self,
        edit: &ProtectedEdit,
    ) -> Result<EditOutcome, String> {
        Ok(match edit {
            ProtectedEdit::Paste { target, text } => {
                let selection = self.paste_over_inner(*target, text)?;
                EditOutcome {
                    changed: selection.is_some(),
                    selection,
                    active_origin: None,
                }
            }
            ProtectedEdit::Clear { range } => EditOutcome::changed(self.clear(*range)?),
            ProtectedEdit::FillWith {
                range,
                active,
                text,
            } => {
                let changed = self.fill_with_inner(*range, *active, text)?;
                self.forget_copy();
                EditOutcome::changed(changed)
            }
            ProtectedEdit::FillDown { range } => EditOutcome::changed(self.fill_down(*range)?),
            ProtectedEdit::FillRight { range } => EditOutcome::changed(self.fill_right(*range)?),
            ProtectedEdit::FillSeries { source, target } => {
                let changed = self.fill_series(*source, *target)?;
                EditOutcome {
                    changed,
                    selection: changed.then_some(*target),
                    active_origin: None,
                }
            }
            ProtectedEdit::MoveRange { source, to, copy } => EditOutcome {
                changed: *to != source.start,
                selection: Some(self.move_range(*source, *to, *copy)?),
                active_origin: Some(source.start),
            },
        })
    }
    fn validate_protected_edit(&self, edit: &ProtectedEdit) -> Result<(), String> {
        let ranges = match edit {
            ProtectedEdit::Paste { target, .. } => vec![*target],
            ProtectedEdit::Clear { range }
            | ProtectedEdit::FillDown { range }
            | ProtectedEdit::FillRight { range } => vec![*range],
            ProtectedEdit::FillWith { range, active, .. } => {
                if !range.contains(*active) {
                    return Err("invalid fill selection".into());
                }
                vec![*range]
            }
            ProtectedEdit::FillSeries { source, target } => vec![*source, *target],
            ProtectedEdit::MoveRange { source, to, .. } => vec![*source, translated(*source, *to)?],
        };
        if ranges.into_iter().all(Self::valid_edit_range) {
            Ok(())
        } else {
            Err("invalid edit selection".into())
        }
    }
    pub(super) fn directional_fill(
        &self,
        range: CellRange,
        down: bool,
    ) -> Option<(CellRange, CellRange)> {
        let mut target = self.in_use(range);
        if down && target.start.row == target.end.row {
            target.start.row = target.start.row.checked_sub(1)?;
        }
        if !down && target.start.column == target.end.column {
            target.start.column = target.start.column.checked_sub(1)?;
        }
        let source = CellRange {
            start: target.start,
            end: if down {
                CellRef::new(target.end.column, target.start.row)
            } else {
                CellRef::new(target.start.column, target.end.row)
            },
        };
        Some((source, target))
    }
    fn protected_edit_requirements(
        &self,
        edit: &ProtectedEdit,
    ) -> Result<Vec<RangeEditRequirement>, String> {
        self.validate_protected_edit(edit)?;
        let sheet = self.sheet_name();
        let mut ranges = Vec::new();
        let mut requirements = Vec::new();
        match edit {
            ProtectedEdit::Paste { target, text } => {
                if let Some(clip) = self
                    .clipboard
                    .as_ref()
                    .filter(|clip| clipboard_lines(&clip.text) == clipboard_lines(text))
                {
                    if !self.book.clipboard_is_current(&clip.snapshot) {
                        return Err("clipboard context changed".into());
                    }
                    let pasted = translated(clip.snapshot.source_range(), target.start)?;
                    let whole = (!clip.cut).then(|| tiled_over(pasted, *target)).flatten();
                    if let Some(whole) = whole {
                        tile_limit(whole)?;
                    }
                    ranges.push(whole.unwrap_or(pasted));
                    if clip.cut {
                        requirements.push(RangeEditRequirement {
                            sheet: clip.snapshot.source_sheet().into(),
                            ranges: vec![clip.snapshot.source_range()],
                        });
                    }
                } else {
                    let rows = clipboard::parse(text);
                    let mut far = target.start;
                    for (r, row) in rows.iter().enumerate().filter(|(_, row)| !row.is_empty()) {
                        let end = CellRef::new(
                            target
                                .start
                                .column
                                .checked_add(
                                    u32::try_from(row.len() - 1)
                                        .map_err(|_| "clipboard too large")?,
                                )
                                .ok_or("clipboard too large")?,
                            target
                                .start
                                .row
                                .checked_add(u32::try_from(r).map_err(|_| "clipboard too large")?)
                                .ok_or("clipboard too large")?,
                        );
                        ranges.push(CellRange {
                            start: CellRef::new(target.start.column, end.row),
                            end,
                        });
                        far.column = far.column.max(end.column);
                        far.row = far.row.max(end.row);
                    }
                    if !ranges.is_empty() {
                        let pasted = CellRange {
                            start: target.start,
                            end: far,
                        };
                        if let Some(whole) = tiled_over(pasted, *target) {
                            tile_limit(whole)?;
                            ranges.extend(difference(whole, pasted));
                        }
                    }
                }
            }
            ProtectedEdit::Clear { range } => {
                // Clearing a blank selection performs no writes, even if its
                // unused cells are locked. Coalesce stored cells into row runs.
                for cell in self
                    .cells_in(*range)
                    .filter(|c| c.formula.is_some() || !matches!(c.value, CellValue::Empty))
                {
                    let at = cell.reference;
                    if let Some(last) = ranges.last_mut().filter(|r: &&mut CellRange| {
                        r.end.row == at.row && r.end.column + 1 == at.column
                    }) {
                        last.end = at;
                    } else {
                        ranges.push(CellRange { start: at, end: at });
                    }
                }
            }
            ProtectedEdit::FillWith {
                range,
                active,
                text,
            } => {
                let target = self.in_use(*range);
                tile_limit(target)?;
                let source = CellRange {
                    start: *active,
                    end: *active,
                };
                if self.edit_text(*active) != *text {
                    ranges.push(source);
                }
                ranges.extend(difference(target, source));
            }
            ProtectedEdit::FillDown { range } | ProtectedEdit::FillRight { range } => {
                if let Some((source, target)) =
                    self.directional_fill(*range, matches!(edit, ProtectedEdit::FillDown { .. }))
                {
                    tile_limit(target)?;
                    ranges.extend(difference(target, source));
                }
            }
            ProtectedEdit::FillSeries { source, target } => {
                if jade_xlsx_headless_editor::fill::fill_axis(*source, *target).is_none() {
                    return Err("invalid fill selection".into());
                }
                tile_limit(*target)?;
                ranges.extend(difference(*target, *source));
            }
            ProtectedEdit::MoveRange { source, to, copy } => {
                if *to != source.start {
                    ranges.push(translated(*source, *to)?);
                    if !copy {
                        ranges.push(*source);
                    }
                }
            }
        }
        if !ranges.is_empty() {
            requirements.push(RangeEditRequirement { sheet, ranges });
        }
        Ok(requirements)
    }
}
fn translated(source: CellRange, to: CellRef) -> Result<CellRange, String> {
    let end = CellRef::new(
        to.column
            .checked_add(
                source
                    .end
                    .column
                    .checked_sub(source.start.column)
                    .ok_or("invalid source")?,
            )
            .ok_or("invalid destination")?,
        to.row
            .checked_add(
                source
                    .end
                    .row
                    .checked_sub(source.start.row)
                    .ok_or("invalid source")?,
            )
            .ok_or("invalid destination")?,
    );
    let r = CellRange { start: to, end };
    if end.column > MAX_COLUMN || end.row > MAX_ROW {
        Err("invalid destination".into())
    } else {
        Ok(r)
    }
}
fn tile_limit(range: CellRange) -> Result<(), String> {
    if u64::from(range.end.column - range.start.column + 1)
        * u64::from(range.end.row - range.start.row + 1)
        > CLIPBOARD_CELL_LIMIT
    {
        Err("selection too large to fill".into())
    } else {
        Ok(())
    }
}
/// At most four rectangles; never enumerates a whole column or worksheet.
fn difference(a: CellRange, b: CellRange) -> Vec<CellRange> {
    let c0 = a.start.column.max(b.start.column);
    let c1 = a.end.column.min(b.end.column);
    let r0 = a.start.row.max(b.start.row);
    let r1 = a.end.row.min(b.end.row);
    if c0 > c1 || r0 > r1 {
        return vec![a];
    }
    let mut out = Vec::new();
    let mut push = |c0, r0, c1, r1| {
        out.push(CellRange {
            start: CellRef::new(c0, r0),
            end: CellRef::new(c1, r1),
        })
    };
    if a.start.row < r0 {
        push(a.start.column, a.start.row, a.end.column, r0 - 1);
    }
    if r1 < a.end.row {
        push(a.start.column, r1 + 1, a.end.column, a.end.row);
    }
    if a.start.column < c0 {
        push(a.start.column, r0, c0 - 1, r1);
    }
    if c1 < a.end.column {
        push(c1 + 1, r0, a.end.column, r1);
    }
    out
}
pub(crate) struct PendingOperation {
    edit: ProtectedEdit,
    requirements: Vec<RangeEditRequirement>,
    context: RangeAuthContext,
    clipboard_revision: u64,
    tokens: Vec<RangeAuthentication>,
    pub candidates: Vec<RangeUnlockCandidate>,
    pub completed: Option<EditOutcome>,
}
impl PendingOperation {
    pub fn new<B: Backend>(
        book: &Workbook<B>,
        edit: ProtectedEdit,
    ) -> Result<Option<Self>, String> {
        let requirements = book.protected_edit_requirements(&edit)?;
        let RangeUnlockStatus::Prompt(candidates) = book
            .book
            .range_unlock_requirements(&requirements, &[])
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        Ok(Some(Self {
            edit,
            requirements,
            context: book.book.range_auth_context(),
            clipboard_revision: book.clipboard_revision,
            tokens: Vec::new(),
            candidates,
            completed: None,
        }))
    }
    pub fn matches<B: Backend>(&self, book: &Workbook<B>) -> bool {
        self.completed.is_none()
            && self.context == book.book.range_auth_context()
            && self.clipboard_revision == book.clipboard_revision
    }
    pub fn entry_matches(&self, entry: Option<(CellRef, &str)>) -> bool {
        match &self.edit {
            ProtectedEdit::FillWith { active, text, .. } => {
                entry.is_some_and(|(at, value)| at == *active && value == text)
            }
            _ => true,
        }
    }
    pub fn apply<B: Backend>(
        &mut self,
        book: &mut Workbook<B>,
        key: &str,
        password: &str,
    ) -> Result<bool, &'static str> {
        if !self.matches(book) {
            return Err("protection.range_stale");
        }
        let candidate = self
            .candidates
            .iter()
            .find(|c| c.key() == key)
            .ok_or("protection.range_stale")?;
        let token = book
            .book
            .authenticate_editable_range(&candidate.sheet, &candidate.definition.name, password)
            .map_err(|e| crate::editable_ranges::message(e.to_string()))?;
        let mut tokens = self.tokens.clone();
        tokens.push(token);
        let status = book
            .book
            .range_unlock_requirements(&self.requirements, &tokens)
            .map_err(|_| "protection.range_stale")?;
        match status {
            RangeUnlockStatus::Prompt(candidates) => {
                self.tokens = tokens;
                self.candidates = candidates;
                Ok(false)
            }
            RangeUnlockStatus::Blocked => Err("protection.range_stale"),
            RangeUnlockStatus::Ready => {
                let result = book
                    .atomic_edit(|book| {
                        book.book
                            .apply_range_authentications(&tokens)
                            .map_err(|e| e.to_string())?;
                        book.execute_protected_edit_inner(&self.edit)
                    })
                    .map_err(|e| crate::editable_ranges::message(e.to_string()))?;
                self.completed = Some(result);
                self.tokens.clear();
                self.candidates.clear();
                Ok(result.changed)
            }
        }
    }
}
