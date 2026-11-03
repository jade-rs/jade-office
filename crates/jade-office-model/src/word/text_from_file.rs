//! 挿入 › テキスト › ファイルからテキスト: another file's text at the caret, in
//! place of the selection, as one undo step.
//!
//! A `.docx` brings its body — paragraphs, tables, pictures, lists — with
//! the styles it uses: a style the document already has keeps the
//! document's definition (the destination wins, as in Word), one it lacks
//! comes over with what it is based on. As Word inserts it, the file's last
//! paragraph mark is not: the first paragraph's text joins what was before
//! the caret (in that paragraph's formatting) and the last one's joins what
//! was after it (in the caret paragraph's).
//!
//! A text file (`.txt`, UTF-8 or UTF-16 with a byte-order mark) comes in a
//! paragraph a line, formatted as typing there would be.

use std::ops::Range;

use jade_docx_editor::commands::snapshot_edit::package_edit_command;
use jade_docx_headless_api::{HeadlessCommand, HeadlessEditRequest};
use jade_selection::TextPosition;
use quick_xml::events::Event;
use quick_xml::Reader;

use super::clipboard::{PasteMode, RichClip, WordClipboard};
use super::master::{self, MARK};
use super::package::load;
use super::{Position, WordSession};

/// What kind of file `bytes` is: a Word document when it is a ZIP package.
pub fn is_docx(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// The extensions the file picker offers.
pub const EXTENSIONS: [&str; 2] = ["docx", "txt"];

/// Inserts the file `bytes` at the caret. Returns where the caret goes:
/// after what came in.
pub fn insert_text_from_file<S: WordClipboard + ?Sized>(
    session: &mut S,
    bytes: &[u8],
) -> Result<Position, String> {
    if is_docx(bytes) {
        insert_docx(session, bytes)
    } else if bytes.starts_with(b"\xD0\xCF\x11\xE0") {
        Err("Word 97-2003 文書 (.doc) からは挿入できません".to_string())
    } else {
        let text = decode_text(bytes);
        session.paste_as(&RichClip::from_text(&text), PasteMode::TextOnly)
    }
}

/// A text file's characters: UTF-8 (with or without a byte-order mark) or
/// UTF-16 with one.
pub fn decode_text(bytes: &[u8]) -> String {
    let utf16 = |little: bool| {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|pair| {
                if little {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        String::from_utf16_lossy(&units)
    };
    let text = if bytes.starts_with(b"\xFF\xFE") {
        utf16(true)
    } else if bytes.starts_with(b"\xFE\xFF") {
        utf16(false)
    } else {
        String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).into_owned()
    };
    // The file's last line break is the end of the file, not an empty
    // paragraph.
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    text.strip_suffix('\n').unwrap_or(&text).to_string()
}

fn insert_docx<S: WordSession + ?Sized>(session: &mut S, source: &[u8]) -> Result<Position, String> {
    let range = session.selection_range().ok_or("no caret")?;
    let (start, end) = range.normalized();
    let before = session
        .word_document()
        .export_docx()
        .map_err(|error| error.to_string())?;
    let paragraph_length = |document: &jade_docx_editor::word_document::WordDocument,
                            index: usize| {
        document.paragraph(index).map_or(0, |paragraph| {
            paragraph.runs.iter().map(|run| run.text.len()).sum::<usize>()
        })
    };
    let after_length = paragraph_length(session.word_document(), end.paragraph)
        .saturating_sub(end.offset);
    let paragraphs_before = session.word_document().paragraph_count();

    // The document with the mark in place of the selection, made on a copy
    // so the session keeps its history.
    let mut copy = load(&before)?;
    jade_docx_editor::headless_api::apply_headless_commands_to_document(
        &mut copy,
        &HeadlessEditRequest {
            commands: vec![HeadlessCommand::ReplaceRange {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                text: MARK.to_string(),
            }],
        },
    )
    .map_err(|error| format!("{error:?}"))?;
    let marked = copy.export_docx().map_err(|error| error.to_string())?;

    let edited = splice(&marked, source)?;
    let replacement = load(&edited)?;
    let added = replacement.paragraph_count() as isize - paragraphs_before as isize
        + (end.paragraph - start.paragraph) as isize;
    let caret_paragraph = (start.paragraph as isize + added).max(0) as usize;
    let caret_offset =
        paragraph_length(&replacement, caret_paragraph).saturating_sub(after_length);
    let snapshot = replacement
        .snapshot_for_replacement()
        .ok_or("the edited document is empty")?;
    let caret = TextPosition::new(caret_paragraph, caret_offset);
    session.execute_command(package_edit_command(before, snapshot, Some(caret)), false)?;
    Ok((caret_paragraph, caret_offset))
}

/// `marked` with `source`'s body where its [`MARK`] is.
pub fn splice(marked: &[u8], source: &[u8]) -> Result<Vec<u8>, String> {
    let from = master::open(source)?;
    let from_xml = master::document(&from)?;
    let (_, blocks) = master::body(&from_xml).ok_or("the file has no body")?;
    let content: String = blocks
        .iter()
        .filter(|block| block.name != "w:sectPr")
        .map(|block| &from_xml[block.start..block.end])
        .collect();
    let content = strip_references(&content);

    let mut to = master::open(marked)?;
    let content = master::import(&content, &from, &mut to)?;
    let mut xml = master::document(&to)?;
    let at = xml.find(MARK).ok_or("no place is marked in the document")?;
    let paragraph = enclosing_paragraph(&xml, at).ok_or("the caret is not in a paragraph")?;
    let replacement = merge(&xml[paragraph.clone()], &content)?;
    xml.replace_range(paragraph, &replacement);
    to.replace(master::DOCUMENT, xml.replace(MARK, "").into_bytes());
    master::write(&to)
}

/// Note, endnote and comment marks name parts of the other file: they are
/// left behind with them.
fn strip_references(content: &str) -> String {
    let mut out = content.to_string();
    for tag in [
        "<w:footnoteReference",
        "<w:endnoteReference",
        "<w:commentReference",
        "<w:commentRangeStart",
        "<w:commentRangeEnd",
    ] {
        while let Some(at) = out.find(tag) {
            let Some(close) = out[at..].find("/>") else {
                break;
            };
            out.replace_range(at..at + close + 2, "");
        }
    }
    out
}

/// The innermost `w:p` around byte `at` of `xml`.
fn enclosing_paragraph(xml: &str, at: usize) -> Option<Range<usize>> {
    let mut reader = Reader::from_str(xml);
    let mut open: Vec<usize> = Vec::new();
    loop {
        let before = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        let after = reader.buffer_position() as usize;
        match event {
            Event::Start(start) if start.name().as_ref() == b"w:p" => open.push(before),
            Event::End(end) if end.name().as_ref() == b"w:p" => {
                let start = open.pop()?;
                if start <= at && at < after {
                    return Some(start..after);
                }
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// A paragraph cut in two at its [`MARK`]: its start tag, its `w:pPr`, what
/// is before the mark (every element open there closed) and what is after
/// it (each reopened, a run with its `w:rPr`).
struct Split {
    open_tag: String,
    properties: Option<String>,
    before: String,
    after: String,
}

fn split(paragraph: &str) -> Option<Split> {
    struct Open {
        name: String,
        tag: String,
        properties: Option<String>,
        properties_start: Option<usize>,
    }
    let mut reader = Reader::from_str(paragraph);
    let mut stack: Vec<Open> = Vec::new();
    let mut properties: Option<Range<usize>> = None;
    loop {
        let before = reader.buffer_position() as usize;
        let event = reader.read_event().ok()?;
        let after = reader.buffer_position() as usize;
        match event {
            Event::Start(start) => {
                let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
                if name == "w:rPr" || name == "w:pPr" {
                    if let Some(parent) = stack.last_mut() {
                        parent.properties_start = Some(before);
                    }
                }
                stack.push(Open {
                    name,
                    tag: paragraph[before..after].to_string(),
                    properties: None,
                    properties_start: None,
                });
            }
            Event::Empty(empty) => {
                let name = empty.name();
                if name.as_ref() == b"w:rPr" || name.as_ref() == b"w:pPr" {
                    if let Some(parent) = stack.last_mut() {
                        parent.properties = Some(paragraph[before..after].to_string());
                        if stack.len() == 1 {
                            properties = Some(before..after);
                        }
                    }
                }
            }
            Event::End(_) => {
                let closed = stack.pop()?;
                if closed.name == "w:rPr" || closed.name == "w:pPr" {
                    if let Some(parent) = stack.last_mut() {
                        if let Some(start) = parent.properties_start.take() {
                            parent.properties = Some(paragraph[start..after].to_string());
                            if stack.len() == 1 && closed.name == "w:pPr" {
                                properties = Some(start..after);
                            }
                        }
                    }
                }
            }
            Event::Text(_) => {
                let raw = &paragraph[before..after];
                let Some(offset) = raw.find(MARK) else {
                    continue;
                };
                let mark = before + offset;
                let first = stack.first()?;
                let inner_start = properties
                    .as_ref()
                    .map_or(first.tag.len(), |range| range.end);
                let closing = paragraph.rfind("</w:p>")?;
                let mut before_part = paragraph[inner_start..mark].to_string();
                for open in stack[1..].iter().rev() {
                    before_part.push_str(&format!("</{}>", open.name));
                }
                let mut after_part = String::new();
                for open in &stack[1..] {
                    after_part.push_str(&open.tag);
                    if let Some(properties) = &open.properties {
                        after_part.push_str(properties);
                    }
                }
                after_part.push_str(&paragraph[mark + MARK.len_utf8()..closing]);
                return Some(Split {
                    open_tag: first.tag.clone(),
                    properties: properties.map(|range| paragraph[range].to_string()),
                    before: before_part,
                    after: after_part,
                });
            }
            Event::Eof => return None,
            _ => {}
        }
    }
}

/// A paragraph's `w:pPr` and what follows it.
fn paragraph_parts(paragraph: &str) -> (Option<&str>, &str) {
    let blocks = master::scan(paragraph, 0..paragraph.len(), 0);
    let Some(element) = blocks.first() else {
        return (None, "");
    };
    let children = master::scan(paragraph, element.start..element.end, 1);
    match children.first() {
        Some(first) if first.name == "w:pPr" => (
            Some(&paragraph[first.start..first.end]),
            &paragraph[first.end..element.inner.end],
        ),
        _ => (None, &paragraph[element.inner.clone()]),
    }
}

/// A `w:pPr` without its section break, for a paragraph that is not the
/// last of its section.
fn without_section(properties: &str) -> String {
    match (properties.find("<w:sectPr"), properties.find("</w:sectPr>")) {
        (Some(start), Some(end)) => {
            let mut out = properties.to_string();
            out.replace_range(start..end + "</w:sectPr>".len(), "");
            out
        }
        _ => properties.to_string(),
    }
}

/// The marked `paragraph` with `content`'s blocks in it at the mark.
fn merge(paragraph: &str, content: &str) -> Result<String, String> {
    let split = split(paragraph).ok_or("the marked place is not in the text")?;
    let blocks = master::scan(content, 0..content.len(), 0);
    let own = split.properties.as_deref().unwrap_or("");
    let Some((first, rest)) = blocks.split_first() else {
        return Ok(format!(
            "{}{own}{}{}</w:p>",
            split.open_tag, split.before, split.after
        ));
    };
    let is_paragraph = |block: &master::Element| block.name == "w:p";
    let raw = |block: &master::Element| &content[block.start..block.end];
    let mut out = String::new();
    if rest.is_empty() && is_paragraph(first) {
        let (_, inner) = paragraph_parts(raw(first));
        out.push_str(&format!(
            "{}{own}{}{inner}{}</w:p>",
            split.open_tag, split.before, split.after
        ));
        return Ok(out);
    }
    let mut middle: &[master::Element] = &blocks;
    if is_paragraph(first) {
        let (properties, inner) = paragraph_parts(raw(first));
        out.push_str(&format!(
            "<w:p>{}{}{inner}</w:p>",
            properties.unwrap_or(""),
            split.before
        ));
        middle = rest;
    } else if !master::plain_text(&split.before).is_empty() {
        out.push_str(&format!("<w:p>{}{}</w:p>", without_section(own), split.before));
    }
    let last = middle.last().filter(|block| is_paragraph(block));
    let middle = if last.is_some() {
        &middle[..middle.len() - 1]
    } else {
        middle
    };
    for block in middle {
        out.push_str(raw(block));
    }
    let tail = last.map_or("", |block| paragraph_parts(raw(block)).1);
    out.push_str(&format!(
        "{}{own}{tail}{}</w:p>",
        split.open_tag, split.after
    ));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_paragraph_splits_at_the_mark_keeping_its_run_format() {
        let paragraph = format!(
            r#"<w:p w14:paraId="1"><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:b/></w:rPr><w:t>ab{MARK}cd</w:t></w:r></w:p>"#
        );
        let split = split(&paragraph).unwrap();
        assert_eq!(split.open_tag, r#"<w:p w14:paraId="1">"#);
        assert_eq!(split.properties.as_deref(), Some(r#"<w:pPr><w:jc w:val="center"/></w:pPr>"#));
        assert_eq!(split.before, "<w:r><w:rPr><w:b/></w:rPr><w:t>ab</w:t></w:r>");
        assert_eq!(split.after, "<w:r><w:rPr><w:b/></w:rPr><w:t>cd</w:t></w:r>");
    }

    #[test]
    fn the_first_and_last_paragraphs_join_the_text_around_the_caret() {
        let paragraph = format!(
            r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>前{MARK}後</w:t></w:r></w:p>"#
        );
        let content = r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>一</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p><w:r><w:t>三</w:t></w:r></w:p>"#;
        let merged = merge(&paragraph, content).unwrap();
        assert_eq!(
            merged,
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>前</w:t></w:r><w:r><w:t>一</w:t></w:r></w:p><w:tbl><w:tr><w:tc><w:p/></w:tc></w:tr></w:tbl><w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>三</w:t></w:r><w:r><w:t>後</w:t></w:r></w:p>"#
        );
        // One paragraph: it joins both sides in the caret's paragraph.
        let one = merge(&paragraph, r#"<w:p><w:r><w:t>中</w:t></w:r></w:p>"#).unwrap();
        assert_eq!(
            one,
            r#"<w:p><w:pPr><w:jc w:val="right"/></w:pPr><w:r><w:t>前</w:t></w:r><w:r><w:t>中</w:t></w:r><w:r><w:t>後</w:t></w:r></w:p>"#
        );
    }

    #[test]
    fn text_files_decode_by_their_byte_order_mark() {
        assert_eq!(decode_text("一\r\n二\n".as_bytes()), "一\n二");
        assert_eq!(decode_text(b"\xEF\xBB\xBFabc"), "abc");
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain("あ\n".encode_utf16().flat_map(u16::to_le_bytes))
            .collect();
        assert_eq!(decode_text(&utf16), "あ");
    }

    #[test]
    fn note_and_comment_marks_stay_behind() {
        assert_eq!(
            strip_references(r#"<w:r><w:footnoteReference w:id="2"/></w:r><w:commentRangeStart w:id="0"/>"#),
            "<w:r></w:r>"
        );
    }
}
