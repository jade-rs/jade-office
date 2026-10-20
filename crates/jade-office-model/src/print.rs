//! Printing a sheet, and exporting it as PDF, as Excel does by the sheet's
//! Page Setup: the print area if the sheet has one, else the cells in use;
//! its paper and orientation, margins, and scale or Fit to pages (A4
//! portrait at 100% with Normal margins when the file says nothing); pages
//! cut where the columns and rows stop fitting and at the sheet's own page
//! breaks, numbered down and then over (or over then down); the print
//! titles repeated on every page; no gridlines; the sheet's header and
//! footer, with their page numbers, sheet name and date.
//!
//! Lengths are CSS px, 96 to the inch, as the sheet's layout measures.

use jade_painter::Painter;
use jade_xlsx_layout::{Rect, SheetLayout};
use jade_xlsx_reader::{CellRange, CellRef, PageMargins, PageOrientation, PageSetup};
use jade_xlsx_render::RenderOptions;

use crate::sheet::Workbook;
use crate::Backend;

/// A4, in px.
pub const A4: (f64, f64) = (210.0 / 25.4 * 96.0, 297.0 / 25.4 * 96.0);

/// Paper `code` (`pageSetup/@paperSize`) portrait, in px; A4 for a code
/// this does not know, as for none.
pub fn paper_size(code: u32) -> (f64, f64) {
    let inches = |width: f64, height: f64| (width * 96.0, height * 96.0);
    let mm = |width: f64, height: f64| (width / 25.4 * 96.0, height / 25.4 * 96.0);
    match code {
        1 | 2 | 18 => inches(8.5, 11.0),
        3 | 17 => inches(11.0, 17.0),
        4 => inches(17.0, 11.0),
        5 => inches(8.5, 14.0),
        6 => inches(5.5, 8.5),
        7 => inches(7.25, 10.5),
        8 => mm(297.0, 420.0),
        11 => mm(148.0, 210.0),
        12 => mm(257.0, 364.0),
        13 => mm(182.0, 257.0),
        14 => inches(8.5, 13.0),
        15 => mm(215.0, 275.0),
        16 => inches(10.0, 14.0),
        20 => inches(4.125, 9.5),
        27 => mm(110.0, 220.0),
        28 => mm(162.0, 229.0),
        34 => mm(176.0, 250.0),
        43 => mm(100.0, 148.0),
        66 => mm(420.0, 594.0),
        70 => mm(105.0, 148.0),
        _ => A4,
    }
}

/// How the active sheet's pages are laid out: the paper they print on, its
/// margins, and the scale the cells print at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PageFormat {
    /// The paper, in px, landscape already turned.
    pub paper: (f64, f64),
    /// The margins, in px.
    pub margins: PageMargins,
    /// 1.0 prints at 100%.
    pub scale: f64,
}

impl PageFormat {
    /// The room the cells have on a page, in the layout's px: the paper
    /// inside the margins, at the scale.
    pub fn room(&self) -> (f64, f64) {
        (
            (self.paper.0 - self.margins.left - self.margins.right) / self.scale,
            (self.paper.1 - self.margins.top - self.margins.bottom) / self.scale,
        )
    }
}

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

/// One printed page: the cells on it, and the print titles it repeats
/// above and to the left of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PrintPage {
    pub range: CellRange,
    /// The title rows printed above `range`, first and last.
    pub title_rows: Option<(u32, u32)>,
    /// The title columns printed left of `range`, first and last.
    pub title_columns: Option<(u32, u32)>,
}

impl<B: Backend> Workbook<B> {
    /// The active sheet's pages, in the order they print: down the first
    /// columns that fit, then the next — or across first, if the sheet says.
    pub fn print_pages(&self) -> Result<Vec<PrintPage>, String> {
        Ok(self.plan()?.map(|plan| plan.pages).unwrap_or_default())
    }

    /// The active sheet's paper, margins and scale, as its Page Setup says:
    /// a Fit to setting shrinks the scale until the pages fit.
    pub fn page_format(&self) -> Result<PageFormat, String> {
        Ok(match self.plan()? {
            Some(plan) => plan.format,
            None => self.page_format_at(None),
        })
    }

    fn setup(&self) -> &PageSetup {
        &self.engine().workbook().sheets[self.sheet_index()].page_setup
    }

    /// The paper and margins, at `percent` or the sheet's own scale.
    fn page_format_at(&self, percent: Option<u32>) -> PageFormat {
        let setup = self.setup();
        let (width, height) = paper_size(setup.paper_size.unwrap_or(9));
        let paper = if setup.orientation == Some(PageOrientation::Landscape) {
            (height, width)
        } else {
            (width, height)
        };
        let inches = setup.margins.unwrap_or_default();
        let px = |inches: f64| inches * 96.0;
        let percent = percent.or(setup.scale).unwrap_or(100).clamp(10, 400);
        PageFormat {
            paper,
            margins: PageMargins {
                left: px(inches.left),
                right: px(inches.right),
                top: px(inches.top),
                bottom: px(inches.bottom),
                header: px(inches.header),
                footer: px(inches.footer),
            },
            scale: f64::from(percent) / 100.0,
        }
    }

    /// The pages and the format they print at; `None` for an empty sheet.
    /// Fit to N pages wide by M tall tries the largest whole percentage,
    /// up to 100%, that the area fits in, down to Excel's least, 10%; it
    /// leaves out the manual page breaks, as Excel does.
    fn plan(&self) -> Result<Option<Plan>, String> {
        let Some(area) = self.print_area() else {
            return Ok(None);
        };
        let setup = self.setup().clone();
        // The titles may lie past the area's last cell.
        let end = CellRef::new(
            setup
                .title_columns
                .map_or(area.end.column, |(_, last)| last.max(area.end.column)),
            setup
                .title_rows
                .map_or(area.end.row, |(_, last)| last.max(area.end.row)),
        );
        let layout = self.print_layout(end)?;
        let sheet = &self.engine().workbook().sheets[self.sheet_index()];
        let column_breaks: Vec<u32> = sheet.column_breaks.iter().map(|b| b.before).collect();
        let row_breaks: Vec<u32> = sheet.row_breaks.iter().map(|b| b.before).collect();
        let columns = Axis::new(
            &layout.columns,
            area.start.column,
            area.end.column,
            setup.title_columns,
        );
        let rows = Axis::new(&layout.rows, area.start.row, area.end.row, setup.title_rows);
        let Some((wide, tall)) = setup.fit_to() else {
            let format = self.page_format_at(None);
            let (width, height) = format.room();
            let pages = pages(
                &columns.bands(width, &column_breaks),
                &rows.bands(height, &row_breaks),
                setup.over_then_down,
            );
            return Ok(Some(Plan {
                pages,
                format,
                layout,
            }));
        };
        let room = self.page_format_at(Some(100)).room();
        // A first guess from the lengths alone, then down until it fits.
        let guess = |pages: u32, room: f64, length: f64| {
            if pages == 0 || length <= 0.0 {
                100.0
            } else {
                f64::from(pages) * room / length * 100.0
            }
        };
        let mut percent = guess(wide, room.0, columns.length())
            .min(guess(tall, room.1, rows.length()))
            .floor()
            .clamp(10.0, 100.0) as u32;
        loop {
            let format = self.page_format_at(Some(percent));
            let (width, height) = format.room();
            let across = columns.bands(width, &[]);
            let down = rows.bands(height, &[]);
            let fits = |pages: u32, bands: usize| pages == 0 || bands <= pages as usize;
            if percent == 10 || (fits(wide, across.len()) && fits(tall, down.len())) {
                let pages = pages(&across, &down, setup.over_then_down);
                return Ok(Some(Plan {
                    pages,
                    format,
                    layout,
                }));
            }
            percent -= 1;
        }
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

    /// Paints the active sheet's pages, each on a page `page` makes at
    /// the paper's size, for a PDF or a printer.
    pub fn paint_pages<P: Painter>(
        &self,
        fields: &PrintFields,
        page: impl Fn(f64, f64) -> Option<P>,
    ) -> Result<Vec<P>, String> {
        let plan = self.plan()?.ok_or_else(|| NOTHING_TO_PRINT.to_string())?;
        self.paint_plan(plan, fields, page)
    }

    /// The active sheet's pages as a PDF: vectors natively, pictures of
    /// the pages in the browser.
    pub fn pdf(&self, fields: &PrintFields) -> Result<Vec<u8>, String> {
        let plan = self.plan()?.ok_or_else(|| NOTHING_TO_PRINT.to_string())?;
        let paper = plan.format.paper;
        let backend = self.backend();
        let pages = self.paint_plan(plan, fields, |width, height| backend.page(width, height))?;
        B::pdf(&pages, paper)
    }

    fn paint_plan<P: Painter>(
        &self,
        plan: Plan,
        fields: &PrintFields,
        page: impl Fn(f64, f64) -> Option<P>,
    ) -> Result<Vec<P>, String> {
        let Plan {
            pages,
            format,
            mut layout,
        } = plan;
        layout.show_grid_lines = false;
        let (width, height) = format.paper;
        let total = pages.len();
        let mut painted = Vec::with_capacity(total);
        for (number, printed) in pages.iter().enumerate() {
            let painter = page(width, height)
                .ok_or_else(|| "cannot create a page to print on".to_string())?;
            painter.set_fill_style_str("#ffffff");
            painter.fill_rect(0.0, 0.0, width, height);
            painter.save();
            let placed = painter
                .translate(format.margins.left, format.margins.top)
                .and_then(|()| painter.scale(format.scale, format.scale));
            if placed.is_err() {
                return Err(format!("cannot paint page {}", number + 1));
            }
            // The title columns, then the page's own; the title rows above.
            let columns = [
                printed.title_columns,
                Some((printed.range.start.column, printed.range.end.column)),
            ];
            let rows = [
                printed.title_rows,
                Some((printed.range.start.row, printed.range.end.row)),
            ];
            let mut y = 0.0;
            for (first_row, last_row) in rows.into_iter().flatten() {
                let top = offset(&layout, false, first_row);
                let tall = offset(&layout, false, last_row + 1) - top;
                let mut x = 0.0;
                for (first_column, last_column) in columns.into_iter().flatten() {
                    let left = offset(&layout, true, first_column);
                    let wide = offset(&layout, true, last_column + 1) - left;
                    let viewport = Rect {
                        x: left,
                        y: top,
                        width: wide,
                        height: tall,
                    };
                    let options = RenderOptions {
                        background: Some("#ffffff".to_string()),
                        origin: (x, y),
                        dark: false,
                        ..RenderOptions::default()
                    };
                    jade_xlsx_render::render_viewport(&painter, &layout, viewport, &options)
                        .map_err(|_| format!("cannot paint page {}", number + 1))?;
                    x += wide;
                }
                y += tall;
            }
            painter.restore();
            self.paint_header_footer(&painter, &format, fields, number + 1, total);
            painted.push(painter);
        }
        Ok(painted)
    }

    /// The sheet's header and footer on page `number` of `total`.
    fn paint_header_footer<P: Painter>(
        &self,
        painter: &P,
        format: &PageFormat,
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
                format.margins.header
            } else {
                format.paper.1 - format.margins.footer
            };
            for (section, line) in sections(text, &on) {
                if line.is_empty() {
                    continue;
                }
                let width = painter.measure_text_width(&line).unwrap_or(0.0);
                let x = match section {
                    Section::Left => format.margins.left,
                    Section::Center => (format.paper.0 - width) / 2.0,
                    Section::Right => format.paper.0 - format.margins.right - width,
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

/// What [`Workbook::plan`] decides: the pages, the format they print at,
/// and the layout they print from.
struct Plan {
    pages: Vec<PrintPage>,
    format: PageFormat,
    layout: SheetLayout,
}

/// The columns (or rows) of the print area, from `first` to `last`, with
/// their sizes, and the titles every page repeats ahead of them.
struct Axis {
    first: u32,
    sizes: Vec<f64>,
    titles: Option<(u32, u32)>,
    /// The titles' length; a hidden title takes no room.
    titles_length: f64,
}

impl Axis {
    fn new(
        entries: &[jade_xlsx_layout::AxisEntry],
        first: u32,
        last: u32,
        titles: Option<(u32, u32)>,
    ) -> Self {
        let size = |index: u32| {
            entries
                .iter()
                .find(|entry| entry.index == index)
                .map_or(0.0, |entry| if entry.hidden { 0.0 } else { entry.size })
        };
        Self {
            first,
            sizes: (first..=last).map(size).collect(),
            titles,
            titles_length: titles.map_or(0.0, |(start, end)| (start..=end).map(size).sum()),
        }
    }

    /// The area's whole length, with the titles once.
    fn length(&self) -> f64 {
        self.sizes.iter().sum::<f64>()
            + match self.titles {
                Some((_, end)) if end < self.first => self.titles_length,
                _ => 0.0,
            }
    }

    /// Whether a page starting at `start` repeats the titles: when they
    /// end above (or left of) it, rather than being on it.
    fn titled(&self, start: u32) -> Option<(u32, u32)> {
        self.titles.filter(|&(_, end)| end < start)
    }

    /// The bands that each fit in `length` with their titles, as (first,
    /// last, titles), cut before the indices in `breaks` too. One too long
    /// for a page has a page of its own.
    fn bands(&self, length: f64, breaks: &[u32]) -> Vec<Band> {
        let last = self.first + self.sizes.len() as u32 - 1;
        let room = |start: u32| {
            length
                - if self.titled(start).is_some() {
                    self.titles_length
                } else {
                    0.0
                }
        };
        let mut bands = Vec::new();
        let mut start = self.first;
        let mut used = 0.0;
        for (index, &size) in (self.first..).zip(&self.sizes) {
            let full = used > 0.0 && used + size > room(start) + 0.01;
            if index > start && (full || breaks.contains(&index)) {
                bands.push((start, index - 1, self.titled(start)));
                start = index;
                used = 0.0;
            }
            used += size;
        }
        bands.push((start, last, self.titled(start)));
        bands
    }
}

/// A page's columns (or rows), first and last, and the titles it repeats.
type Band = (u32, u32, Option<(u32, u32)>);

/// The pages from the bands across and down, numbered down then over, or
/// over then down.
fn pages(columns: &[Band], rows: &[Band], over_then_down: bool) -> Vec<PrintPage> {
    let page =
        |&(left, right, title_columns): &Band, &(top, bottom, title_rows): &Band| PrintPage {
            range: CellRange {
                start: CellRef::new(left, top),
                end: CellRef::new(right, bottom),
            },
            title_rows,
            title_columns,
        };
    if over_then_down {
        rows.iter()
            .flat_map(|row| columns.iter().map(move |column| page(column, row)))
            .collect()
    } else {
        columns
            .iter()
            .flat_map(|column| rows.iter().map(move |row| page(column, row)))
            .collect()
    }
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
