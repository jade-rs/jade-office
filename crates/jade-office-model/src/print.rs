//! Printing a sheet, and exporting it as PDF, as Excel does at its
//! defaults: the print area if the sheet has one, else the cells in use;
//! A4 portrait at 100%, Excel's Normal margins; pages cut where the columns
//! and rows stop fitting and at the sheet's own page breaks, numbered down
//! and then over; no gridlines; the sheet's header and footer, with their
//! page numbers, sheet name and date.
//!
//! Lengths are CSS px, 96 to the inch, as the sheet's layout measures.

use jade_painter::Painter;
use jade_xlsx_layout::{Rect, SheetLayout};
use jade_xlsx_reader::{CellRange, CellRef};
use jade_xlsx_render::RenderOptions;

use crate::sheet::Workbook;
use crate::Backend;

/// A4, in px.
pub const A4: (f64, f64) = (210.0 / 25.4 * 96.0, 297.0 / 25.4 * 96.0);

/// Excel's Normal margins, in px: 0.7" left and right, 0.75" top and
/// bottom, the header and footer 0.3" from the paper's edge.
const MARGIN_SIDE: f64 = 0.7 * 96.0;
const MARGIN_TOP: f64 = 0.75 * 96.0;
const MARGIN_HEADER: f64 = 0.3 * 96.0;

/// What a header or footer's fields read on: the date and time printed,
/// as the shell's clock and language write them, and the file's name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PrintFields {
    pub date: String,
    pub time: String,
    pub file: String,
}

/// Whether `name` is a PDF's, by its extension: Save As to one exports.
pub fn is_pdf(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("pdf"))
}

/// What printing an empty sheet says; the shells show Excel's words.
pub const NOTHING_TO_PRINT: &str = "there is nothing on the sheet to print";

/// One printed page: the cells on it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintPage {
    pub range: CellRange,
}

impl<B: Backend> Workbook<B> {
    /// The active sheet's pages, in the order they print: down the first
    /// columns that fit, then the next.
    pub fn print_pages(&self) -> Result<Vec<PrintPage>, String> {
        let Some(area) = self.print_area() else {
            return Ok(Vec::new());
        };
        let layout = self.print_layout(area.end)?;
        let width = A4.0 - 2.0 * MARGIN_SIDE;
        let height = A4.1 - 2.0 * MARGIN_TOP;
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let column_breaks: Vec<u32> = sheet.column_breaks.iter().map(|b| b.before).collect();
        let row_breaks: Vec<u32> = sheet.row_breaks.iter().map(|b| b.before).collect();
        let columns = bands(
            &layout.columns,
            area.start.column,
            area.end.column,
            width,
            &column_breaks,
        );
        let rows = bands(
            &layout.rows,
            area.start.row,
            area.end.row,
            height,
            &row_breaks,
        );
        Ok(columns
            .iter()
            .flat_map(|&(left, right)| {
                rows.iter().map(move |&(top, bottom)| PrintPage {
                    range: CellRange {
                        start: CellRef::new(left, top),
                        end: CellRef::new(right, bottom),
                    },
                })
            })
            .collect())
    }

    /// The print area: the sheet's defined one, else from the first cell in
    /// use to the last. `None` for an empty sheet.
    pub fn print_area(&self) -> Option<CellRange> {
        let workbook = self.engine().workbook();
        let index = self.sheet_index();
        let defined = workbook.defined_names.iter().find(|name| {
            name.name.eq_ignore_ascii_case("_xlnm.Print_Area")
                && name.local_sheet_index == Some(index)
        });
        if let Some(range) = defined.and_then(|name| area_of(&name.formula)) {
            return Some(range);
        }
        let sheet = &workbook.sheets[index];
        let mut cells = sheet.rows.iter().flat_map(|row| row.cells.iter());
        let first = cells.next()?.reference;
        let mut area = CellRange {
            start: first,
            end: first,
        };
        for cell in sheet.rows.iter().flat_map(|row| row.cells.iter()) {
            let at = cell.reference;
            area.start = CellRef::new(area.start.column.min(at.column), area.start.row.min(at.row));
            area.end = CellRef::new(area.end.column.max(at.column), area.end.row.max(at.row));
        }
        Some(area)
    }

    /// Paints the active sheet's pages, each on a page `page` makes at A4's
    /// size, for a PDF or a printer.
    pub fn paint_pages<P: Painter>(
        &self,
        fields: &PrintFields,
        page: impl Fn(f64, f64) -> Option<P>,
    ) -> Result<Vec<P>, String> {
        let pages = self.print_pages()?;
        let Some(area) = self.print_area() else {
            return Err(NOTHING_TO_PRINT.to_string());
        };
        let mut layout = self.print_layout(area.end)?;
        layout.show_grid_lines = false;
        let options = RenderOptions {
            background: Some("#ffffff".to_string()),
            origin: (MARGIN_SIDE, MARGIN_TOP),
            dark: false,
            ..RenderOptions::default()
        };
        let total = pages.len();
        let mut painted = Vec::with_capacity(total);
        for (number, printed) in pages.iter().enumerate() {
            let painter =
                page(A4.0, A4.1).ok_or_else(|| "cannot create a page to print on".to_string())?;
            painter.set_fill_style_str("#ffffff");
            painter.fill_rect(0.0, 0.0, A4.0, A4.1);
            let x = offset(&layout, true, printed.range.start.column);
            let y = offset(&layout, false, printed.range.start.row);
            let viewport = Rect {
                x,
                y,
                width: offset(&layout, true, printed.range.end.column + 1) - x,
                height: offset(&layout, false, printed.range.end.row + 1) - y,
            };
            painter.save();
            jade_xlsx_render::render_viewport(&painter, &layout, viewport, &options)
                .map_err(|_| format!("cannot paint page {}", number + 1))?;
            painter.restore();
            self.paint_header_footer(&painter, fields, number + 1, total);
            painted.push(painter);
        }
        Ok(painted)
    }

    /// The active sheet's pages as a PDF: vectors natively, pictures of
    /// the pages in the browser.
    pub fn pdf(&self, fields: &PrintFields) -> Result<Vec<u8>, String> {
        let backend = self.backend();
        let pages = self.paint_pages(fields, |width, height| backend.page(width, height))?;
        B::pdf(&pages, A4)
    }

    /// The sheet's header and footer on page `number` of `total`.
    fn paint_header_footer<P: Painter>(
        &self,
        painter: &P,
        fields: &PrintFields,
        number: usize,
        total: usize,
    ) {
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let Some(parts) = sheet.header_footer.as_ref() else {
            return;
        };
        let first = number == 1 && parts.different_first;
        let even = number.is_multiple_of(2) && parts.different_odd_even;
        let (header, footer) = if first {
            (&parts.first_header, &parts.first_footer)
        } else if even {
            (&parts.even_header, &parts.even_footer)
        } else {
            (&parts.odd_header, &parts.odd_footer)
        };
        let on = Page {
            number,
            total,
            sheet: &sheet.name,
            fields,
        };
        painter.save();
        painter.set_fill_style_str("#000000");
        painter.set_font("14.6667px Calibri");
        for (text, top) in [(header, true), (footer, false)] {
            let Some(text) = text.as_deref() else {
                continue;
            };
            painter.set_text_baseline(if top { "top" } else { "bottom" });
            let y = if top {
                MARGIN_HEADER
            } else {
                A4.1 - MARGIN_HEADER
            };
            for (section, line) in sections(text, &on) {
                if line.is_empty() {
                    continue;
                }
                let width = painter.measure_text_width(&line).unwrap_or(0.0);
                let x = match section {
                    Section::Left => MARGIN_SIDE,
                    Section::Center => (A4.0 - width) / 2.0,
                    Section::Right => A4.0 - MARGIN_SIDE - width,
                };
                let _ = painter.fill_text(&line, x, y);
            }
        }
        painter.restore();
    }
}

/// Where a header's or footer's part goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Section {
    Left,
    Center,
    Right,
}

/// What a page's header and footer fields read.
pub struct Page<'a> {
    pub number: usize,
    pub total: usize,
    pub sheet: &'a str,
    pub fields: &'a PrintFields,
}

/// A header or footer (`&L…&C…&R…`) as the text of its parts, its fields
/// filled in: `&P` the page, `&N` the pages, `&D` the date, `&T` the time,
/// `&A` the sheet, `&F` the file, `&&` an ampersand. Font, size, colour
/// and style codes are dropped; text before any section code is centred.
pub fn sections(text: &str, page: &Page) -> Vec<(Section, String)> {
    let mut parts: Vec<(Section, String)> = Vec::new();
    let mut section = Section::Center;
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            current.push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            break;
        };
        let next = match code.to_ascii_uppercase() {
            'L' => Some(Section::Left),
            'C' => Some(Section::Center),
            'R' => Some(Section::Right),
            _ => None,
        };
        if let Some(next) = next {
            if !current.is_empty() {
                parts.push((section, std::mem::take(&mut current)));
            }
            section = next;
            continue;
        }
        match code {
            'P' | 'p' => current.push_str(&page.number.to_string()),
            'N' | 'n' => current.push_str(&page.total.to_string()),
            'D' | 'd' => current.push_str(&page.fields.date),
            'T' | 't' => current.push_str(&page.fields.time),
            'A' | 'a' => current.push_str(page.sheet),
            'F' | 'f' => current.push_str(&page.fields.file),
            '&' => current.push('&'),
            // &"Font,Style"
            '"' => {
                for c in chars.by_ref() {
                    if c == '"' {
                        break;
                    }
                }
            }
            // &K and its colour.
            'K' | 'k' => {
                for _ in 0..6 {
                    chars.next();
                }
            }
            // A font size.
            digit if digit.is_ascii_digit() => {
                while chars.peek().is_some_and(char::is_ascii_digit) {
                    chars.next();
                }
            }
            // &B, &I, &U, &E, &S, &X, &Y, &G, &Z: styles, pictures, the path.
            _ => {}
        }
    }
    if !current.is_empty() {
        parts.push((section, current));
    }
    parts
}

/// The bands of `entries` from `first` to `last` that each fit in
/// `length`, as (first, last), cut before the indices in `breaks` too. A
/// hidden column or row takes no room; one too long for a page has a page
/// of its own.
fn bands(
    entries: &[jade_xlsx_layout::AxisEntry],
    first: u32,
    last: u32,
    length: f64,
    breaks: &[u32],
) -> Vec<(u32, u32)> {
    let size = |index: u32| {
        entries
            .iter()
            .find(|entry| entry.index == index)
            .map_or(0.0, |entry| if entry.hidden { 0.0 } else { entry.size })
    };
    let mut bands = Vec::new();
    let mut start = first;
    let mut used = 0.0;
    for index in first..=last {
        let room = size(index);
        let full = used > 0.0 && used + room > length + 0.01;
        if index > start && (full || breaks.contains(&index)) {
            bands.push((start, index - 1));
            start = index;
            used = 0.0;
        }
        used += room;
    }
    bands.push((start, last));
    bands
}

/// Where column (or row) `index` starts, in the layout's px.
fn offset(layout: &SheetLayout, columns: bool, index: u32) -> f64 {
    let entries = if columns {
        &layout.columns
    } else {
        &layout.rows
    };
    match entries.iter().find(|entry| entry.index == index) {
        Some(entry) => entry.offset,
        None => entries
            .last()
            .map_or(0.0, |entry| entry.offset + entry.size),
    }
}

/// A print area's first range: `'Sheet 1'!$A$1:$D$20` as A1:D20.
fn area_of(formula: &str) -> Option<CellRange> {
    let first = formula.split(',').next()?;
    let reference = first
        .rsplit_once('!')
        .map_or(first, |(_, reference)| reference);
    let reference = reference.replace('$', "");
    let (start, end) = reference
        .split_once(':')
        .unwrap_or((&reference, &reference));
    let start = CellRef::parse(start).ok()?;
    let end = CellRef::parse(end).ok()?;
    Some(CellRange {
        start: CellRef::new(start.column.min(end.column), start.row.min(end.row)),
        end: CellRef::new(start.column.max(end.column), start.row.max(end.row)),
    })
}
