//! 挿入 › オブジェクト › ファイルから for every shell: the file embedded as
//! an OLE package at the caret (jade-core's `ole_objects`), shown as Word
//! shows one — its icon with its name under it (アイコンで表示), or a
//! Word document's first page — and the file a double-click on it opens.
//!
//! The icon is a small page the shell renders with its own fonts from
//! [`object_icon_docx`], as the signature line's picture is.

use jade_docx_editor::commands::insert_image::InsertImageCommand;
use jade_docx_editor::commands::insert_ole_object::InsertOleObjectCommand;
pub use jade_docx_editor::word_document::ole_objects::{embedded_file, ole_package, EmbeddedFile};
use jade_selection::{SelectionRange, TextPosition};

use super::WordSession;

/// The icon's size, CSS pixels.
pub const OBJECT_ICON_SIZE: (f64, f64) = (112.0, 96.0);

/// How wide a first page shown as the object is drawn, CSS pixels.
pub const CONTENT_WIDTH: f64 = 204.0;

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// What the icon's document card says: the file's extension, upper case.
pub fn icon_badge(name: &str) -> String {
    name.rsplit_once('.')
        .map(|(_, extension)| extension.to_uppercase())
        .filter(|extension| !extension.is_empty() && extension.len() <= 5)
        .unwrap_or_else(|| "FILE".into())
}

/// The name under the icon: as long as it fits, an ellipsis in the middle
/// of one that does not, as the Finder shortens one.
pub fn icon_label(name: &str) -> String {
    const MAX: usize = 22;
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= MAX {
        return name.to_string();
    }
    let head: String = chars[..MAX / 2 - 1].iter().collect();
    let tail: String = chars[chars.len() - (MAX / 2 - 1)..].iter().collect();
    format!("{head}…{tail}")
}

/// A one-page document drawing the icon an embedded file is shown as: a
/// document card with the file's type on it, its name below.
pub fn object_icon_docx(name: &str) -> Vec<u8> {
    use std::io::Write;
    let (width, height) = OBJECT_ICON_SIZE;
    let (page_w, page_h) = ((width * 15.0) as u32, (height * 15.0) as u32);
    let card = |text: &str, size: u32, bold: bool| {
        format!(
            r#"<w:p><w:pPr><w:pBdr><w:top w:val="single" w:sz="8" w:space="1" w:color="7F7F7F"/><w:left w:val="single" w:sz="8" w:space="4" w:color="7F7F7F"/><w:bottom w:val="single" w:sz="8" w:space="1" w:color="7F7F7F"/><w:right w:val="single" w:sz="8" w:space="4" w:color="7F7F7F"/></w:pBdr><w:shd w:val="clear" w:color="auto" w:fill="F2F2F2"/><w:spacing w:before="0" w:after="0" w:line="240" w:lineRule="auto"/><w:ind w:left="{indent}" w:right="{indent}"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:rFonts w:ascii="Arial" w:hAnsi="Arial"/>{bold}<w:color w:val="2B579A"/><w:sz w:val="{size}"/></w:rPr><w:t xml:space="preserve">{text}</w:t></w:r></w:p>"#,
            indent = page_w / 2 - 330,
            bold = if bold { "<w:b/>" } else { "" },
            text = escape(text),
        )
    };
    let document = format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body><w:p><w:pPr><w:spacing w:before="0" w:after="0" w:line="120" w:lineRule="exact"/></w:pPr></w:p>{top}{badge}{bottom}<w:p><w:pPr><w:spacing w:before="80" w:after="0" w:line="240" w:lineRule="auto"/><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:sz w:val="16"/></w:rPr><w:t xml:space="preserve">{label}</w:t></w:r></w:p><w:sectPr><w:pgSz w:w="{page_w}" w:h="{page_h}"/><w:pgMar w:top="0" w:right="0" w:bottom="0" w:left="0" w:header="0" w:footer="0" w:gutter="0"/></w:sectPr></w:body></w:document>"#,
        top = card(" ", 16, false),
        badge = card(&icon_badge(name), 22, true),
        bottom = card(" ", 16, false),
        label = escape(&icon_label(name)),
    );
    let parts = [
        ("[Content_Types].xml", r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>"#.to_string()),
        ("_rels/.rels", r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#.to_string()),
        ("word/document.xml", document),
    ];
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, content) in parts {
        if zip
            .start_file(name, zip::write::SimpleFileOptions::default())
            .is_ok()
        {
            zip.write_all(content.as_bytes()).ok();
        }
    }
    zip.finish()
        .map(|cursor| cursor.into_inner())
        .unwrap_or_default()
}

/// Whether a file can be shown by its first page rather than an icon: a
/// Word document the shells can lay out.
pub fn shows_content(name: &str) -> bool {
    let lower = name.to_lowercase();
    [".docx", ".docm", ".dotx", ".dotm"]
        .iter()
        .any(|extension| lower.ends_with(extension))
}

/// The size a picture of `width` × `height` pixels is drawn at as the
/// object: the icon as it is, a page at [`CONTENT_WIDTH`].
pub fn object_size(icon: bool, width: f64, height: f64) -> (f64, f64) {
    if icon || width <= 0.0 {
        return OBJECT_ICON_SIZE;
    }
    (CONTENT_WIDTH, CONTENT_WIDTH * height / width)
}

/// What an object is shown as.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectPicture {
    pub png: Vec<u8>,
    /// CSS pixels.
    pub size: (f64, f64),
    /// The icon, rather than the file's first page.
    pub icon: bool,
}

/// The picture an object is shown as, from a shell's way of drawing the
/// first page of a document as a PNG (with its size in CSS pixels): a Word
/// document's first page when not shown as an icon, the icon otherwise.
pub fn object_picture(
    name: &str,
    bytes: &[u8],
    icon: bool,
    first_page: impl Fn(&[u8]) -> Result<(Vec<u8>, f64, f64), String>,
) -> Result<ObjectPicture, String> {
    if !icon && shows_content(name) {
        if let Ok((png, width, height)) = first_page(bytes) {
            return Ok(ObjectPicture {
                png,
                size: object_size(false, width, height),
                icon: false,
            });
        }
    }
    let (png, _, _) = first_page(&object_icon_docx(name))?;
    Ok(ObjectPicture {
        png,
        size: OBJECT_ICON_SIZE,
        icon: true,
    })
}

/// オブジェクト's choice as the dialog holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObjectDialog {
    /// アイコンで表示.
    pub icon: bool,
}

impl Default for ObjectDialog {
    fn default() -> Self {
        Self { icon: true }
    }
}

/// Embedded objects for every session.
pub trait WordOleObjects: WordSession {
    /// The file `name` (`bytes`) embedded at the caret, shown as `picture`
    /// (a PNG of `size` CSS pixels): its icon when `icon`, its first page
    /// otherwise.
    fn insert_ole_object(
        &mut self,
        name: &str,
        bytes: Vec<u8>,
        picture: Vec<u8>,
        size: (f64, f64),
        icon: bool,
    ) -> Result<(), String> {
        let range = self.selection_range().ok_or("no caret")?;
        let (start, _) = range.normalized();
        let caret = SelectionRange::collapsed(TextPosition::new(start.paragraph, start.offset));
        self.set_selection_range(Some(caret));
        self.execute_command(
            Box::new(InsertOleObjectCommand::new(
                InsertImageCommand::new(
                    caret,
                    picture,
                    "image/png".to_string(),
                    size.0,
                    size.1,
                    None,
                    None,
                ),
                name.to_string(),
                bytes,
                icon,
            )),
            false,
        )
    }

    /// The file the object in run `run` of `paragraph` holds, which a
    /// double-click opens.
    fn embedded_file_at(&self, paragraph: usize, run: usize) -> Option<EmbeddedFile> {
        self.word_document().embedded_object_file(paragraph, run)
    }
}

impl<T: WordSession + ?Sized> WordOleObjects for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_icon_names_the_file() {
        assert_eq!(icon_badge("見積書.pdf"), "PDF");
        assert_eq!(icon_badge("README"), "FILE");
        assert_eq!(icon_label("short.txt"), "short.txt");
        let long = icon_label("an extremely long file name for a report.xlsx");
        assert!(long.contains('…') && long.ends_with("port.xlsx"), "{long}");
        let docx = object_icon_docx("a<b>.csv");
        assert!(docx.starts_with(b"PK"));
        assert!(shows_content("Plan.DOCX"));
        assert!(!shows_content("plan.pdf"));
        assert_eq!(object_size(true, 800.0, 1000.0), OBJECT_ICON_SIZE);
        assert_eq!(
            object_size(false, 800.0, 1000.0),
            (CONTENT_WIDTH, CONTENT_WIDTH * 1.25)
        );
    }
}
