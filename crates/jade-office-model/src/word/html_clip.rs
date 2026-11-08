//! HTML from the clipboard as a [`RichClip`]: what 形式を選択して貼り付け's
//! HTML 形式 pastes when the copy came from another application — a web
//! page, another editor — whose clipboard carries `text/html` beside the
//! plain text.
//!
//! The markup is read the way a paste into Word reads it: block elements
//! (`p`, `div`, headings, list items, table rows) end paragraphs; `br` inserts
//! a line break inside the current paragraph;
//! `b`/`strong`, `i`/`em`, `u`, `s`/`strike`/`del`, `sup` and `sub` and the
//! inline `style` declarations Word and the browsers write (`font-weight`,
//! `font-style`, `text-decoration`, `vertical-align`, `font-size`,
//! `font-family`, `color`, `text-align`) give the runs and paragraphs their
//! formatting. Scripts, styles and comments are dropped; entities are
//! decoded; ordinary HTML whitespace collapses while Unicode space separators
//! retain their original characters.

use jade_docx_headless_api::HeadlessRunStyle;

use super::clipboard::{ClipParagraph, ClipRun, RichClip};

/// Elements whose start and end break the paragraph.
const BLOCKS: [&str; 20] = [
    "p",
    "div",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "li",
    "tr",
    "blockquote",
    "pre",
    "ul",
    "ol",
    "table",
    "section",
    "article",
    "header",
    "footer",
    "address",
];

/// Elements whose content is not text.
const SKIPPED: [&str; 5] = ["script", "style", "head", "title", "template"];

/// The run style and paragraph alignment an open element gives.
#[derive(Clone, Default)]
struct Frame {
    tag: String,
    style: HeadlessRunStyle,
    alignment: Option<String>,
    preserve_whitespace: bool,
    /// Paragraphs already emitted when this element opened. An empty paragraph
    /// element still counts, but a wrapper around emitted paragraphs does not.
    paragraphs_at_open: usize,
}

/// A colour as CSS writes it (`#c00`, `#C00000`, `rgb(192, 0, 0)`, a few
/// names), as `RRGGBB`.
fn css_color(value: &str) -> Option<String> {
    let value = value.trim().to_ascii_lowercase();
    if let Some(hex) = value.strip_prefix('#') {
        let hex: String = match hex.len() {
            3 => hex.chars().flat_map(|c| [c, c]).collect(),
            6 => hex.to_string(),
            _ => return None,
        };
        return u32::from_str_radix(&hex, 16)
            .ok()
            .map(|_| hex.to_ascii_uppercase());
    }
    if let Some(inner) = value
        .strip_prefix("rgb(")
        .or_else(|| value.strip_prefix("rgba("))
        .and_then(|rest| rest.strip_suffix(')'))
    {
        let parts: Vec<u8> = inner
            .split([',', ' ', '/'])
            .filter(|part| !part.is_empty())
            .take(3)
            .filter_map(|part| part.trim().parse::<f32>().ok())
            .map(|channel| channel.round().clamp(0.0, 255.0) as u8)
            .collect();
        return (parts.len() == 3)
            .then(|| format!("{:02X}{:02X}{:02X}", parts[0], parts[1], parts[2]));
    }
    Some(
        match value.as_str() {
            "black" => "000000",
            "white" => "FFFFFF",
            "red" => "FF0000",
            "green" => "008000",
            "blue" => "0000FF",
            "yellow" => "FFFF00",
            "gray" | "grey" => "808080",
            "navy" => "000080",
            "maroon" => "800000",
            "purple" => "800080",
            "orange" => "FFA500",
            _ => return None,
        }
        .to_string(),
    )
}

/// A CSS length as points: `12pt`, `16px`, `1.5em` of `base`.
fn css_points(value: &str, base: f32) -> Option<f32> {
    let value = value.trim().to_ascii_lowercase();
    let number = |suffix: &str| value.strip_suffix(suffix)?.trim().parse::<f32>().ok();
    let points = if let Some(points) = number("pt") {
        points
    } else if let Some(pixels) = number("px") {
        pixels * 0.75
    } else if let Some(ems) = number("em").or_else(|| number("rem")) {
        ems * base
    } else if let Some(percent) = number("%") {
        percent / 100.0 * base
    } else {
        match value.as_str() {
            "x-small" => 7.5,
            "small" => 10.0,
            "medium" => 12.0,
            "large" => 13.5,
            "x-large" => 18.0,
            "xx-large" => 24.0,
            _ => return None,
        }
    };
    (points > 0.0 && points < 1640.0).then(|| (points * 2.0).round() / 2.0)
}

/// The first family of a CSS `font-family` list, unquoted, when it is a
/// face rather than a generic family.
fn css_family(value: &str) -> Option<String> {
    let first = value.split(',').next()?.trim().trim_matches(['"', '\'']);
    let generic = [
        "serif",
        "sans-serif",
        "monospace",
        "cursive",
        "fantasy",
        "system-ui",
        "inherit",
        "initial",
    ];
    (!first.is_empty() && !generic.contains(&first.to_ascii_lowercase().as_str()))
        .then(|| first.to_string())
}

/// `name="value"`, `name='value'` or `name=value` in a start tag.
fn attribute(tag: &str, name: &str) -> Option<String> {
    let bytes = tag.as_bytes();
    let mut at = bytes.iter().position(u8::is_ascii_whitespace)?;
    while at < bytes.len() {
        while at < bytes.len() && (bytes[at].is_ascii_whitespace() || bytes[at] == b'/') {
            at += 1;
        }
        let start = at;
        while at < bytes.len() && !bytes[at].is_ascii_whitespace() && bytes[at] != b'=' {
            at += 1;
        }
        if start == at {
            // Stray '=' is not an attribute name; always advance on malformed HTML.
            at += 1;
            continue;
        }
        let key = &tag[start..at];
        while at < bytes.len() && bytes[at].is_ascii_whitespace() {
            at += 1;
        }
        let mut value = "";
        if bytes.get(at) == Some(&b'=') {
            at += 1;
            while at < bytes.len() && bytes[at].is_ascii_whitespace() {
                at += 1;
            }
            if let Some(quote @ (b'"' | b'\'')) = bytes.get(at).copied() {
                at += 1;
                let start = at;
                while at < bytes.len() && bytes[at] != quote {
                    at += 1;
                }
                value = &tag[start..at];
                if at < bytes.len() {
                    at += 1;
                }
            } else {
                let start = at;
                while at < bytes.len() && !bytes[at].is_ascii_whitespace() {
                    at += 1;
                }
                value = &tag[start..at];
            }
        }
        if key.eq_ignore_ascii_case(name) {
            return Some(decode_entities(value));
        }
    }
    None
}

/// Tag terminator, excluding greater-than signs inside quoted attribute values.
/// Quotes inside an unquoted value remain literal, matching HTML error recovery.
fn tag_end(html: &str) -> Option<usize> {
    let mut quote = None;
    let mut value_pending = false;
    let mut unquoted = false;
    for (at, byte) in html.bytes().enumerate() {
        if let Some(delimiter) = quote {
            if byte == delimiter {
                quote = None;
            }
        } else if byte == b'>' {
            return Some(at);
        } else if unquoted {
            if byte.is_ascii_whitespace() {
                unquoted = false;
            }
        } else if value_pending {
            if !byte.is_ascii_whitespace() {
                value_pending = false;
                if matches!(byte, b'"' | b'\'') {
                    quote = Some(byte);
                } else {
                    unquoted = true;
                }
            }
        } else if byte == b'=' {
            value_pending = true;
        }
    }
    None
}

/// `&amp;`, `&lt;`, `&#12354;`, `&#x3042;` and their kin as text.
fn decode_entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(end) = rest[1..]
            .find(';')
            .map(|end| end + 1)
            .filter(|end| *end <= 12)
        else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let name = &rest[1..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            "ensp" => Some('\u{2002}'),
            "emsp" => Some('\u{2003}'),
            "copy" => Some('©'),
            "reg" => Some('®'),
            "hellip" => Some('…'),
            "mdash" => Some('—'),
            "ndash" => Some('–'),
            _ => name
                .strip_prefix("#x")
                .or_else(|| name.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| name.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(character) => {
                out.push(character);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// What an element and its `style` attribute set, over what it inherits.
fn frame_for(tag_name: &str, tag: &str, parent: &Frame) -> Frame {
    let mut frame = Frame {
        tag: tag_name.to_string(),
        style: parent.style.clone(),
        alignment: parent.alignment.clone(),
        preserve_whitespace: parent.preserve_whitespace,
        paragraphs_at_open: 0,
    };
    let style = &mut frame.style;
    match tag_name {
        "b" | "strong" | "th" => style.bold = Some(true),
        "i" | "em" | "cite" | "var" => style.italic = Some(true),
        "u" | "ins" => style.underline = Some(true),
        "s" | "strike" | "del" => style.strike = Some(true),
        "sup" => style.baseline = Some("superscript".to_string()),
        "sub" => style.baseline = Some("subscript".to_string()),
        "h1" => {
            style.bold = Some(true);
            style.font_size = Some(24.0);
        }
        "h2" => {
            style.bold = Some(true);
            style.font_size = Some(18.0);
        }
        "h3" => {
            style.bold = Some(true);
            style.font_size = Some(14.0);
        }
        "h4" | "h5" | "h6" => style.bold = Some(true),
        "code" | "pre" | "tt" | "kbd" => style.font_family = Some("Courier New".to_string()),
        _ => {}
    }
    if tag_name == "font" {
        if let Some(color) = attribute(tag, "color").and_then(|value| css_color(&value)) {
            style.color = Some(color);
        }
        if let Some(face) = attribute(tag, "face").and_then(|value| css_family(&value)) {
            style.font_family = Some(face);
        }
    }
    if let Some(align) = attribute(tag, "align") {
        frame.alignment = alignment(&align);
    }
    if let Some(declarations) = attribute(tag, "style") {
        let base = frame.style.font_size.unwrap_or(12.0);
        for declaration in declarations.split(';') {
            let Some((property, value)) = declaration.split_once(':') else {
                continue;
            };
            let value = value.trim().trim_end_matches("!important").trim();
            let lower = value.to_ascii_lowercase();
            let style = &mut frame.style;
            match property.trim().to_ascii_lowercase().as_str() {
                "font-weight" => {
                    style.bold = Some(
                        lower == "bold"
                            || lower == "bolder"
                            || lower.parse::<u32>().is_ok_and(|weight| weight >= 600),
                    )
                }
                "font-style" => style.italic = Some(lower == "italic" || lower == "oblique"),
                "text-decoration" | "text-decoration-line" => {
                    if lower.contains("underline") {
                        style.underline = Some(true);
                    }
                    if lower.contains("line-through") {
                        style.strike = Some(true);
                    }
                    if lower == "none" {
                        style.underline = Some(false);
                        style.strike = Some(false);
                    }
                }
                "vertical-align" => match lower.as_str() {
                    "super" => style.baseline = Some("superscript".to_string()),
                    "sub" => style.baseline = Some("subscript".to_string()),
                    "baseline" => style.baseline = None,
                    _ => {}
                },
                "font-size" => {
                    if let Some(points) = css_points(&lower, base) {
                        style.font_size = Some(points);
                    }
                }
                "font-family" => {
                    if let Some(family) = css_family(value) {
                        style.font_family = Some(family);
                    }
                }
                "color" => {
                    if let Some(color) = css_color(value) {
                        style.color = Some(color);
                    }
                }
                "text-align" => frame.alignment = alignment(&lower),
                "white-space" => match lower.as_str() {
                    "pre" | "pre-wrap" | "break-spaces" => frame.preserve_whitespace = true,
                    "normal" | "nowrap" | "pre-line" => frame.preserve_whitespace = false,
                    _ => {}
                },
                _ => {}
            }
        }
    }
    frame
}

/// A CSS or HTML alignment as Word's `w:jc`.
fn alignment(value: &str) -> Option<String> {
    Some(
        match value.trim().to_ascii_lowercase().as_str() {
            "center" | "middle" => "center",
            "right" | "end" => "right",
            "justify" => "both",
            "left" | "start" => "left",
            _ => return None,
        }
        .to_string(),
    )
}

/// Builds the clip as the markup is read.
#[derive(Default)]
struct Builder {
    paragraphs: Vec<ClipParagraph>,
    current: ClipParagraph,
    /// White space is pending before the next text, collapsed to one, in
    /// the style of the text it was in.
    space: Option<HeadlessRunStyle>,
    /// Inside `pre`: white space is kept.
    preformatted: usize,
    /// A lone unmarked BR is the placeholder used for an empty HTML block.
    placeholder_break: bool,
}

impl Builder {
    /// `text` in `style` at the end of the paragraph.
    fn push(&mut self, text: &str, style: &HeadlessRunStyle) {
        if text.is_empty() {
            return;
        }
        self.placeholder_break = false;
        match self.current.runs.last_mut() {
            Some(run) if run.style == *style => run.text.push_str(text),
            _ => self.current.runs.push(ClipRun {
                text: text.to_string(),
                style: style.clone(),
            }),
        }
    }

    fn text(&mut self, text: &str, frame: &Frame) {
        if self.current.runs.is_empty() && self.current.alignment.is_none() {
            self.current.alignment = frame.alignment.clone();
        }
        if self.preformatted > 0 || frame.preserve_whitespace {
            if let Some(style) = self.space.take() {
                if self
                    .current
                    .runs
                    .last()
                    .is_some_and(|run| !run.text.ends_with('\n'))
                {
                    self.push(" ", &style);
                }
            }
            self.push(text, &frame.style);
            return;
        }
        let mut collapsed = String::new();
        for character in text.chars() {
            // CSS collapses document whitespace, not Unicode space separators.
            // Preserve NBSP, narrow NBSP and width-specific spaces verbatim.
            if matches!(character, ' ' | '\t' | '\n' | '\r' | '\u{c}') {
                if self.space.is_none() {
                    // The space keeps the style of the text it was in.
                    self.push(&std::mem::take(&mut collapsed), &frame.style);
                    self.space = Some(frame.style.clone());
                }
                continue;
            }
            if let Some(style) = self.space.take() {
                // None at a paragraph's start.
                if self
                    .current
                    .runs
                    .last()
                    .is_some_and(|run| !run.text.ends_with('\n'))
                {
                    self.push(" ", &style);
                }
            }
            collapsed.push(character);
        }
        self.push(&collapsed, &frame.style);
    }

    /// Ends a block, retaining explicit empty paragraphs and BR placeholders.
    fn break_paragraph(&mut self, forced: bool) {
        self.space = None;
        let placeholder = std::mem::take(&mut self.placeholder_break);
        if placeholder {
            self.current.runs.clear();
        }
        let forced = forced || placeholder;
        if self.current.runs.is_empty() && !forced {
            self.current = ClipParagraph::default();
            return;
        }
        self.paragraphs.push(std::mem::take(&mut self.current));
    }

    fn finish(mut self) -> RichClip {
        self.break_paragraph(false);
        RichClip {
            paragraphs: self.paragraphs,
        }
    }
}

impl RichClip {
    /// HTML for other applications, alongside `plain_text()` on the clipboard.
    /// Only text and formatting are emitted; copied text cannot become markup.
    pub fn to_html(&self) -> String {
        fn escaped(value: &str) -> String {
            value
                .replace('&', "&amp;")
                .replace('<', "&lt;")
                .replace('>', "&gt;")
                .replace('"', "&quot;")
                .replace('\'', "&#39;")
        }
        let mut html = String::from("<html><body><!--StartFragment-->");
        for paragraph in &self.paragraphs {
            let alignment = match paragraph.alignment.as_deref() {
                Some("left" | "start") => "left",
                Some("right" | "end") => "right",
                Some("center") => "center",
                Some("both" | "distribute") => "justify",
                _ => "left",
            };
            html.push_str(&format!(
                "<p style=\"margin:0;white-space:pre-wrap;text-align:{alignment}\">"
            ));
            for run in &paragraph.runs {
                let style = &run.style;
                let mut css = String::new();
                if let Some(bold) = style.bold {
                    css.push_str(if bold {
                        "font-weight:700;"
                    } else {
                        "font-weight:400;"
                    });
                }
                if let Some(italic) = style.italic {
                    css.push_str(if italic {
                        "font-style:italic;"
                    } else {
                        "font-style:normal;"
                    });
                }
                let mut decorations = Vec::new();
                if style.underline == Some(true) {
                    decorations.push("underline");
                }
                if style.strike == Some(true) {
                    decorations.push("line-through");
                }
                if !decorations.is_empty() {
                    css.push_str(&format!("text-decoration:{};", decorations.join(" ")));
                } else if style.underline.is_some() || style.strike.is_some() {
                    css.push_str("text-decoration:none;");
                }
                match style.baseline.as_deref() {
                    Some("superscript") => css.push_str("vertical-align:super;"),
                    Some("subscript") => css.push_str("vertical-align:sub;"),
                    _ => {}
                }
                if let Some(size) = style
                    .font_size
                    .filter(|size| size.is_finite() && *size > 0.0)
                {
                    css.push_str(&format!("font-size:{size}pt;"));
                }
                if let Some(family) = &style.font_family {
                    // Quote the CSS string before escaping the HTML attribute.
                    let family = family
                        .replace('\\', "\\\\")
                        .replace('\'', "\\'")
                        .replace(['\r', '\n', '\0'], " ");
                    css.push_str(&format!("font-family:'{family}';"));
                }
                if let Some(color) = &style.color {
                    let color = color.strip_prefix('#').unwrap_or(color);
                    if color.len() == 6 && color.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                        css.push_str(&format!("color:#{color};"));
                    }
                }
                html.push_str(&format!("<span style=\"{}\">", escaped(&css)));
                html.push_str(
                    &escaped(&run.text).replace('\n', "<br data-jade-line-break=\"true\">"),
                );
                html.push_str("</span>");
            }
            if paragraph.runs.is_empty() {
                html.push_str("<br>");
            }
            html.push_str("</p>");
        }
        html.push_str("<!--EndFragment--></body></html>");
        html
    }

    /// The clipboard's `text/html` as a copy with its formatting. Only the
    /// fragment between `<!--StartFragment-->` and `<!--EndFragment-->` is
    /// read when the markup marks one, as Windows' and Office's HTML
    /// clipboard formats do.
    pub fn from_html(html: &str) -> RichClip {
        let html = match (
            html.find("<!--StartFragment-->"),
            html.find("<!--EndFragment-->"),
        ) {
            (Some(start), Some(end)) if start < end => &html[start + 20..end],
            _ => html,
        };
        let mut builder = Builder::default();
        let mut stack: Vec<Frame> = vec![Frame::default()];
        let mut skipping: Option<String> = None;
        let mut rest = html;
        while !rest.is_empty() {
            let Some(open) = rest.find('<') else {
                if skipping.is_none() {
                    let frame = stack.last().cloned().unwrap_or_default();
                    builder.text(&decode_entities(rest), &frame);
                }
                break;
            };
            if open > 0 && skipping.is_none() {
                let frame = stack.last().cloned().unwrap_or_default();
                builder.text(&decode_entities(&rest[..open]), &frame);
            }
            rest = &rest[open..];
            if let Some(comment) = rest.strip_prefix("<!--") {
                rest = comment.find("-->").map_or("", |end| &comment[end + 3..]);
                continue;
            }
            let Some(close) = tag_end(rest) else {
                break;
            };
            let tag = &rest[1..close];
            rest = &rest[close + 1..];
            if tag.starts_with('!') || tag.starts_with('?') {
                continue;
            }
            let closing = tag.starts_with('/');
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == ':')
                .collect::<String>()
                .to_ascii_lowercase();
            // Keep Office namespace wrappers distinct: `o:p` inside an HTML
            // paragraph is a placeholder, not another paragraph element.
            if let Some(skipped) = &skipping {
                if closing && *skipped == name {
                    skipping = None;
                }
                continue;
            }
            if SKIPPED.contains(&name.as_str()) {
                if !closing && !tag.trim_end().ends_with('/') {
                    skipping = Some(name);
                }
                continue;
            }
            let block = BLOCKS.contains(&name.as_str());
            if name == "br" {
                if builder.current.runs.is_empty() {
                    builder.current.alignment =
                        stack.last().and_then(|frame| frame.alignment.clone());
                }
                // Keep the paragraph's formatting and write a text-wrapping
                // break. A lone browser placeholder BR stays an empty block;
                // exported genuine breaks are marked to disambiguate that case.
                let placeholder = builder.current.runs.is_empty()
                    && attribute(tag, "data-jade-line-break").as_deref() != Some("true");
                let frame = stack.last().cloned().unwrap_or_default();
                builder.space = None;
                builder.push("\n", &frame.style);
                builder.placeholder_break = placeholder;
                continue;
            }
            if closing {
                if block {
                    let empty_paragraph = stack
                        .iter()
                        .rfind(|frame| frame.tag == name)
                        .filter(|frame| frame.paragraphs_at_open == builder.paragraphs.len())
                        .filter(|_| {
                            matches!(
                                name.as_str(),
                                "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "pre"
                            )
                        });
                    if let Some(frame) = empty_paragraph {
                        if builder.current.runs.is_empty() {
                            builder.current.alignment = frame.alignment.clone();
                        }
                    }
                    builder.break_paragraph(empty_paragraph.is_some());
                }
                if name == "pre" {
                    builder.preformatted = builder.preformatted.saturating_sub(1);
                }
                // Back to the element's parent, closing what it left open.
                if let Some(at) = stack.iter().rposition(|frame| frame.tag == name) {
                    if at > 0 {
                        stack.truncate(at);
                    }
                }
                continue;
            }
            if block {
                builder.break_paragraph(false);
            }
            if matches!(name.as_str(), "td" | "th") && !builder.current.runs.is_empty() {
                // Cells of a row read as one line, a tab between them.
                let frame = stack.last().cloned().unwrap_or_default();
                builder.space = None;
                if let Some(run) = builder.current.runs.last_mut() {
                    run.text.push('\t');
                } else {
                    builder.text("\t", &frame);
                }
            }
            let void = matches!(
                name.as_str(),
                "img" | "hr" | "meta" | "link" | "input" | "wbr" | "col" | "area" | "source"
            ) || tag.trim_end().ends_with('/');
            if void {
                if name == "hr" {
                    builder.break_paragraph(false);
                }
                continue;
            }
            if name == "pre" {
                builder.preformatted += 1;
            }
            let parent = stack.last().cloned().unwrap_or_default();
            let mut frame = frame_for(&name, tag, &parent);
            frame.paragraphs_at_open = builder.paragraphs.len();
            stack.push(frame);
        }
        builder.finish()
    }
}

#[cfg(test)]
mod export_tests {
    use super::*;

    #[test]
    fn clipboard_html_accepts_document_and_imported_color_forms() {
        for color in ["123456", "#123456"] {
            let clip = RichClip {
                paragraphs: vec![ClipParagraph {
                    runs: vec![ClipRun {
                        text: "Color".into(),
                        style: HeadlessRunStyle {
                            color: Some(color.into()),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                }],
            };
            assert!(clip.to_html().contains("color:#123456;"));
        }
    }

    #[test]
    fn clipboard_html_roundtrips_text_and_character_formatting() {
        let style = HeadlessRunStyle {
            bold: Some(true),
            italic: Some(true),
            underline: Some(true),
            strike: Some(true),
            baseline: Some("superscript".into()),
            font_size: Some(18.5),
            font_family: Some("Noto Sans JP".into()),
            color: Some("123456".into()),
        };
        let clip = RichClip {
            paragraphs: vec![
                ClipParagraph {
                    alignment: Some("center".into()),
                    runs: vec![ClipRun {
                        text: "  A<&\"' 日本語\t  ".into(),
                        style: style.clone(),
                    }],
                    ..Default::default()
                },
                ClipParagraph {
                    alignment: Some("both".into()),
                    runs: vec![ClipRun {
                        text: "Second".into(),
                        style: HeadlessRunStyle {
                            bold: Some(false),
                            italic: Some(false),
                            underline: Some(false),
                            strike: Some(false),
                            ..Default::default()
                        },
                    }],
                    ..Default::default()
                },
            ],
        };
        let html = clip.to_html();
        assert!(html.contains("&lt;&amp;&quot;&#39;"));
        assert!(html.contains("<!--StartFragment-->"));
        assert_eq!(RichClip::from_html(&html), clip);
    }

    #[test]
    fn clipboard_html_cannot_turn_text_or_font_names_into_markup() {
        let clip = RichClip {
            paragraphs: vec![ClipParagraph {
                runs: vec![ClipRun {
                    text: "<script>alert('text')</script>".into(),
                    style: HeadlessRunStyle {
                        font_family: Some("'\" onmouseover=\"alert(1)\\\n".into()),
                        color: Some("red;position:fixed".into()),
                        font_size: Some(f32::NAN),
                        ..Default::default()
                    },
                }],
                ..Default::default()
            }],
        };
        let html = clip.to_html();
        assert!(!html.contains("<script>"));
        assert!(!html.contains("style=\"font-family:'\""));
        assert!(!html.contains("position:fixed"));
        assert!(!html.contains("NaN"));
        assert_eq!(RichClip::from_html(&html).plain_text(), clip.plain_text());
    }
}
