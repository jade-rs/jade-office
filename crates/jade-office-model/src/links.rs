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
    /// The workbook's Hyperlink Base (its properties' `HyperlinkBase`),
    /// which link addresses that are paths are read against.
    pub fn hyperlink_base(&self) -> Option<String> {
        let package = jade_opc::OpcPackage::open(self.engine().bytes()).ok()?;
        let xml = package.part_str("docProps/app.xml").ok()??;
        let mut reader = quick_xml::Reader::from_str(&xml);
        let mut base: Option<String> = None;
        loop {
            match (reader.read_event().ok()?, base.as_mut()) {
                (quick_xml::events::Event::Start(element), None)
                    if element.local_name().as_ref() == b"HyperlinkBase" =>
                {
                    base = Some(String::new());
                }
                (quick_xml::events::Event::Text(text), Some(base)) => {
                    base.push_str(&text.xml_content().ok()?);
                }
                // `&amp;` and the like come apart from the text round them.
                (quick_xml::events::Event::GeneralRef(entity), Some(base)) => {
                    let name = entity.decode().ok()?;
                    match quick_xml::escape::resolve_predefined_entity(&name) {
                        Some(text) => base.push_str(text),
                        None => base.push_str(&format!("&{name};")),
                    }
                }
                (quick_xml::events::Event::End(_), Some(base)) => {
                    let base = base.trim();
                    return (!base.is_empty()).then(|| base.to_string());
                }
                (quick_xml::events::Event::Eof, _) => return None,
                _ => {}
            }
        }
    }
    /// A link address as a URL to open: a path read against the Hyperlink
    /// Base, else `folder` (the one the workbook is in), as
    /// [`link_address`] reads it.
    pub fn link_url(&self, address: &str, folder: Option<&str>) -> String {
        link_address(address, self.hyperlink_base().as_deref(), folder)
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

/// Where a hyperlink's address leads, as a URL to open: one with a scheme
/// (`https:`, `mailto:`, `file:`) as it is; a path — `report.docx`,
/// `..\data\q3.xlsx`, `C:\shared\plan.xlsx`, `\\server\share\a.xlsx` —
/// read as Excel reads it, against the workbook's Hyperlink Base when it
/// has one (a folder or a web address), else the folder the workbook is in,
/// and given as a `file://` URL (or joined onto a web base). A relative path
/// with nothing to read it against is left as it is.
pub fn link_address(address: &str, base: Option<&str>, folder: Option<&str>) -> String {
    let address = address.trim();
    if has_scheme(address) {
        return address.to_string();
    }
    let (path, fragment) = match address.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (address, None),
    };
    let with_fragment = |url: String| match fragment {
        Some(fragment) => format!("{url}#{fragment}"),
        None => url,
    };
    if path.is_empty() {
        return address.to_string();
    }
    if is_absolute(path) {
        return with_fragment(file_url(path));
    }
    let base = base
        .map(str::trim)
        .filter(|base| !base.is_empty())
        .or(folder.filter(|folder| !folder.is_empty()));
    let Some(base) = base else {
        return address.to_string();
    };
    if has_scheme(base) {
        let (scheme, rest) = base.split_once("://").unwrap_or(("", base));
        let (host, start) = match rest.split_once('/') {
            Some((host, start)) => (host, start),
            None => (rest, ""),
        };
        let joined = normalize(&format!("{start}/{}", path.replace('\\', "/")));
        let url = if scheme.is_empty() {
            format!("{base}/{joined}")
        } else {
            format!("{scheme}://{host}/{}", encode(&joined))
        };
        return with_fragment(url);
    }
    with_fragment(file_url(&format!(
        "{}/{path}",
        base.trim_end_matches(['/', '\\'])
    )))
}

/// `https:`, `mailto:`: a scheme of two or more letters, so a drive letter
/// (`C:`) is a path.
fn has_scheme(text: &str) -> bool {
    text.split_once(':').is_some_and(|(scheme, _)| {
        scheme.len() > 1
            && scheme.starts_with(|c: char| c.is_ascii_alphabetic())
            && scheme
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

fn is_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    path.starts_with('/')
        || path.starts_with('\\')
        || (bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'\\' | b'/'))
}

/// A path as a `file://` URL: `\\server\share` as `file://server/share`,
/// `C:\x` as `file:///C:/x`, `..` steps taken.
fn file_url(path: &str) -> String {
    let slashed = path.replace('\\', "/");
    if let Some(unc) = slashed.strip_prefix("//") {
        let (host, rest) = unc.split_once('/').unwrap_or((unc, ""));
        return format!("file://{host}/{}", encode(&normalize(rest)));
    }
    format!("file:///{}", encode(&normalize(&slashed)))
}

/// The path's segments with `.` dropped and `..` taking the one before it.
fn normalize(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                // A drive (`C:`) is never stepped out of.
                if segments.last().is_some_and(|last| !last.ends_with(':')) {
                    segments.pop();
                }
            }
            segment => segments.push(segment),
        }
    }
    segments.join("/")
}

/// Spaces and the few characters a URL path cannot hold, percent-encoded.
fn encode(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '%' => out.push_str("%25"),
            '?' => out.push_str("%3F"),
            '"' => out.push_str("%22"),
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::link_address;

    #[test]
    fn addresses_with_a_scheme_open_as_they_are() {
        for url in [
            "https://example.com/a b",
            "mailto:ann@example.com",
            "file:///C:/x.xlsx",
        ] {
            assert_eq!(link_address(url, Some("C:\\base"), Some("/docs")), url);
        }
    }

    #[test]
    fn paths_are_read_against_the_base_or_the_folder() {
        let folder = Some("/Users/ann/Documents");
        assert_eq!(
            link_address("report.docx", None, folder),
            "file:///Users/ann/Documents/report.docx"
        );
        assert_eq!(
            link_address("..\\data\\Q3 sales.xlsx#Sheet2!A1", None, folder),
            "file:///Users/ann/data/Q3%20sales.xlsx#Sheet2!A1"
        );
        assert_eq!(
            link_address("plan.xlsx", Some("C:\\Shared\\"), folder),
            "file:///C:/Shared/plan.xlsx"
        );
        assert_eq!(
            link_address("..\\..\\plan.xlsx", Some("C:\\Shared"), None),
            "file:///C:/plan.xlsx"
        );
        assert_eq!(
            link_address(
                "specs/a b.pdf",
                Some("https://intranet.example.com/docs/"),
                folder
            ),
            "https://intranet.example.com/docs/specs/a%20b.pdf"
        );
        assert_eq!(
            link_address("../up.pdf", Some("https://intranet.example.com/docs"), None),
            "https://intranet.example.com/up.pdf"
        );
        // Nothing to read it against: left as it is.
        assert_eq!(link_address("report.docx", None, None), "report.docx");
    }

    #[test]
    fn absolute_paths_become_file_urls() {
        assert_eq!(
            link_address("C:\\Shared\\plan.xlsx", Some("https://x.example"), None),
            "file:///C:/Shared/plan.xlsx"
        );
        assert_eq!(
            link_address("\\\\server\\share\\a.xlsx", None, None),
            "file://server/share/a.xlsx"
        );
        assert_eq!(
            link_address("/Volumes/X/a.xlsx", None, Some("/tmp")),
            "file:///Volumes/X/a.xlsx"
        );
    }
}
