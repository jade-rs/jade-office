//! Page Setup ▸ Header/Footer, edited as Excel edits it: a left, centre
//! and right section for the header and the footer — of every page, or of
//! the first page and of even pages on their own — with buttons that put
//! in the page number (`&[Page]`), the number of pages, the date, the
//! time, the file and the sheet. Written to the sheet's `headerFooter` as
//! one undo step and printed on each page.

use jade_xlsx_headless_api::HeaderFooterChange;
use jade_xlsx_reader::HeaderFooter;

use crate::sheet::Workbook;
use crate::text::TextEdit;
use crate::Backend;

/// Which pages a header and footer are for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderPages {
    /// Every page, or every odd one when even pages have their own.
    Odd,
    First,
    Even,
}

impl HeaderPages {
    pub const ALL: [HeaderPages; 3] = [HeaderPages::Odd, HeaderPages::First, HeaderPages::Even];

    pub fn label(self) -> &'static str {
        match self {
            HeaderPages::Odd => "header_footer.pages.odd",
            HeaderPages::First => "header_footer.pages.first",
            HeaderPages::Even => "header_footer.pages.even",
        }
    }
}

/// The header (true) or footer, and its left, centre or right section.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeaderSlot {
    pub header: bool,
    /// 0 left, 1 centre, 2 right.
    pub section: usize,
}

/// A field a button puts in at the caret: what the box shows, and the code
/// the file keeps.
pub const FIELDS: [(&str, &str, &str); 6] = [
    ("&[Page]", "&P", "header_footer.field.page"),
    ("&[Pages]", "&N", "header_footer.field.pages"),
    ("&[Date]", "&D", "header_footer.field.date"),
    ("&[Time]", "&T", "header_footer.field.time"),
    ("&[File]", "&F", "header_footer.field.file"),
    ("&[Tab]", "&A", "header_footer.field.sheet"),
];

/// What a key did in the editor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeaderKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The header and footer editor for the active sheet.
pub struct HeaderFooterDialog {
    /// The sections, by pages, then header and footer, then left, centre
    /// and right, as the boxes show them.
    pub sections: [[[TextEdit; 3]; 2]; 3],
    pub pages: HeaderPages,
    pub slot: HeaderSlot,
    pub different_first: bool,
    pub different_odd_even: bool,
}

impl HeaderFooterDialog {
    pub fn open<B: Backend>(book: &Workbook<B>) -> Self {
        let current = book.header_footer().cloned().unwrap_or_default();
        let split_both = |header: &Option<String>, footer: &Option<String>| {
            [split(header.as_deref()), split(footer.as_deref())]
        };
        Self {
            sections: [
                split_both(&current.odd_header, &current.odd_footer),
                split_both(&current.first_header, &current.first_footer),
                split_both(&current.even_header, &current.even_footer),
            ],
            pages: HeaderPages::Odd,
            slot: HeaderSlot {
                header: true,
                section: 1,
            },
            different_first: current.different_first,
            different_odd_even: current.different_odd_even,
        }
    }

    fn pages_index(pages: HeaderPages) -> usize {
        match pages {
            HeaderPages::Odd => 0,
            HeaderPages::First => 1,
            HeaderPages::Even => 2,
        }
    }

    pub fn section(&self, pages: HeaderPages, slot: HeaderSlot) -> &TextEdit {
        &self.sections[Self::pages_index(pages)][usize::from(!slot.header)][slot.section]
    }

    /// The box with the keys.
    pub fn field(&self) -> &TextEdit {
        self.section(self.pages, self.slot)
    }

    pub fn field_mut(&mut self) -> &mut TextEdit {
        let slot = self.slot;
        &mut self.sections[Self::pages_index(self.pages)][usize::from(!slot.header)][slot.section]
    }

    /// The pages whose header and footer are shown: the first and even
    /// pages only once they are set to differ.
    pub fn show(&mut self, pages: HeaderPages) {
        let allowed = match pages {
            HeaderPages::Odd => true,
            HeaderPages::First => self.different_first,
            HeaderPages::Even => self.different_odd_even,
        };
        if allowed {
            self.pages = pages;
        }
    }

    pub fn focus(&mut self, slot: HeaderSlot) {
        self.slot = slot;
    }

    /// Different First Page or Different Odd and Even Pages turned on or
    /// off; off, its pages go back to the shared header and footer.
    pub fn toggle_first(&mut self) {
        self.different_first = !self.different_first;
        if !self.different_first && self.pages == HeaderPages::First {
            self.pages = HeaderPages::Odd;
        }
    }

    pub fn toggle_odd_even(&mut self) {
        self.different_odd_even = !self.different_odd_even;
        if !self.different_odd_even && self.pages == HeaderPages::Even {
            self.pages = HeaderPages::Odd;
        }
    }

    /// A field button: its code at the caret of the box with the keys.
    pub fn insert_field(&mut self, index: usize) {
        if let Some((shown, _, _)) = FIELDS.get(index) {
            self.field_mut().replace(None, shown, false);
        }
    }

    /// Enter takes, Escape closes, Tab moves to the next section; the rest
    /// edit the box with the keys.
    pub fn key(&mut self, key: &str, shift: bool, command: bool) -> HeaderKey {
        if self.field().marked.is_some() {
            return HeaderKey::Unhandled;
        }
        match key {
            "enter" => HeaderKey::Take,
            "escape" => HeaderKey::Close,
            "tab" => {
                let order = (usize::from(!self.slot.header)) * 3 + self.slot.section;
                let next = if shift { (order + 5) % 6 } else { (order + 1) % 6 };
                self.slot = HeaderSlot {
                    header: next < 3,
                    section: next % 3,
                };
                HeaderKey::Handled
            }
            "a" if command => {
                self.field_mut().whole = true;
                HeaderKey::Handled
            }
            key if self.field_mut().edit_key(key) => HeaderKey::Handled,
            _ => HeaderKey::Unhandled,
        }
    }

    /// The header or footer of `pages` as the file keeps it.
    fn joined(&self, pages: HeaderPages, header: bool) -> Option<String> {
        let row = &self.sections[Self::pages_index(pages)][usize::from(!header)];
        join([&row[0].text, &row[1].text, &row[2].text])
    }

    /// What OK writes.
    pub fn change(&self) -> HeaderFooterChange {
        HeaderFooterChange {
            odd_header: Some(self.joined(HeaderPages::Odd, true)),
            odd_footer: Some(self.joined(HeaderPages::Odd, false)),
            first_header: Some(self.joined(HeaderPages::First, true)),
            first_footer: Some(self.joined(HeaderPages::First, false)),
            even_header: Some(self.joined(HeaderPages::Even, true)),
            even_footer: Some(self.joined(HeaderPages::Even, false)),
            different_first: Some(self.different_first),
            different_odd_even: Some(self.different_odd_even),
            ..HeaderFooterChange::default()
        }
    }

    pub fn apply<B: Backend>(&self, book: &mut Workbook<B>) -> Result<bool, String> {
        book.set_header_footer(&self.change())
    }
}

/// `&L…&C…&R…` as the three boxes show it, the codes Excel's buttons put
/// in written as it shows them (`&P` as `&[Page]`).
pub fn split(text: Option<&str>) -> [TextEdit; 3] {
    let mut parts = [String::new(), String::new(), String::new()];
    let mut section = 1;
    let mut chars = text.unwrap_or_default().chars().peekable();
    while let Some(c) = chars.next() {
        if c != '&' {
            parts[section].push(c);
            continue;
        }
        match chars.peek().copied() {
            Some(code @ ('L' | 'l' | 'C' | 'c' | 'R' | 'r')) => {
                chars.next();
                section = match code.to_ascii_uppercase() {
                    'L' => 0,
                    'C' => 1,
                    _ => 2,
                };
            }
            Some(code) => {
                let shown = FIELDS
                    .iter()
                    .find(|(_, stored, _)| stored[1..].eq_ignore_ascii_case(&code.to_string()))
                    .map(|(shown, _, _)| *shown);
                match shown {
                    Some(shown) => {
                        chars.next();
                        parts[section].push_str(shown);
                    }
                    // `&&`, fonts and sizes stay as they are written.
                    None => parts[section].push('&'),
                }
            }
            None => parts[section].push('&'),
        }
    }
    parts.map(TextEdit::new)
}

/// The three boxes back as the file keeps them: each section that has
/// text with its code, `&[Page]` as `&P`; `None` when all are empty.
pub fn join(parts: [&str; 3]) -> Option<String> {
    let mut out = String::new();
    for (code, text) in ["&L", "&C", "&R"].into_iter().zip(parts) {
        if text.is_empty() {
            continue;
        }
        let mut text = text.to_string();
        for (shown, stored, _) in FIELDS {
            text = text.replace(shown, stored);
        }
        out.push_str(code);
        out.push_str(&text);
    }
    (!out.is_empty()).then_some(out)
}

impl<B: Backend> Workbook<B> {
    /// The active sheet's header and footer, if it has any.
    pub fn header_footer(&self) -> Option<&HeaderFooter> {
        self.engine().workbook().sheets[self.sheet_index()]
            .header_footer
            .as_ref()
    }

    /// The active sheet's header and footer changed as `change` says. One
    /// undo step; `false` when it already was so.
    pub fn set_header_footer(&mut self, change: &HeaderFooterChange) -> Result<bool, String> {
        let before = self.header_footer().cloned();
        let name = self.sheet_name();
        self.engine_mut()
            .set_header_footer(&name, change)
            .map_err(|error| error.to_string())?;
        if self.header_footer().cloned() == before {
            // Nothing changed, but the engine took a step: give it back.
            self.engine_mut().undo().map_err(|error| error.to_string())?;
            return Ok(false);
        }
        self.wrote(1)?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sections_split_and_join_with_their_fields() {
        let [left, centre, right] = split(Some("&LPage &P of &N&C&\"Arial,Bold\"Report&R&D &&"));
        assert_eq!(left.text, "Page &[Page] of &[Pages]");
        assert_eq!(centre.text, "&\"Arial,Bold\"Report");
        assert_eq!(right.text, "&[Date] &&");
        assert_eq!(
            join([&left.text, &centre.text, &right.text]).as_deref(),
            Some("&LPage &P of &N&C&\"Arial,Bold\"Report&R&D &&")
        );
        assert_eq!(join(["", "", ""]), None);
        // Text before any section code is centred, as Excel reads it.
        assert_eq!(split(Some("Title"))[1].text, "Title");
    }
}
