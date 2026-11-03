//! 書式の詳細 (Reveal Formatting): the selection's formatting as Word's pane
//! lists it, under フォント, 段落 and セクション — each heading a link to the
//! dialog that changes it (フォント, 段落, ページ設定). What the pane shows
//! is the first character's, as Word's does for a mixed selection.

use super::page_setup::WordPageSetup;
use super::paragraph::{IndentUnit, LineSpacing, SpecialIndent, WordParagraphs, BODY_TEXT};
use super::WordFormatting;

const MM_PER_PT: f32 = 25.4 / 72.0;

/// A heading of the pane, and the dialog its link opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevealGroup {
    /// フォント → the フォント dialog.
    Font,
    /// 段落 → the 段落 dialog.
    Paragraph,
    /// セクション → ページ設定.
    Section,
}

impl RevealGroup {
    pub const ALL: [RevealGroup; 3] = [RevealGroup::Font, RevealGroup::Paragraph, RevealGroup::Section];

    /// The heading's i18n key.
    pub fn label(self) -> &'static str {
        match self {
            RevealGroup::Font => "reveal.font",
            RevealGroup::Paragraph => "reveal.paragraph",
            RevealGroup::Section => "reveal.section",
        }
    }
}

/// A value: text as it is, an i18n key, or a style's `w:name` (which the
/// shell shows as its language names it, `styles::display_name`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RevealValue {
    Text(String),
    Key(&'static str),
    Style(String),
}

/// One line of the pane: its label's i18n key and its value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevealRow {
    pub label: &'static str,
    pub value: RevealValue,
}

fn text(label: &'static str, value: impl Into<String>) -> RevealRow {
    RevealRow {
        label,
        value: RevealValue::Text(value.into()),
    }
}

fn key(label: &'static str, value: &'static str) -> RevealRow {
    RevealRow {
        label,
        value: RevealValue::Key(value),
    }
}

/// A number without trailing zeros: `10.5`, `12`.
fn number(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if rounded.fract() == 0.0 {
        format!("{}", rounded as i64)
    } else {
        format!("{rounded:.1}")
    }
}

fn millimetres(points: f32) -> String {
    format!("{} mm", number(points * MM_PER_PT))
}

/// 書式の詳細 over every session.
pub trait WordRevealFormatting: WordFormatting + WordParagraphs + WordPageSetup {
    /// The rows under `group`.
    fn reveal(&self, group: RevealGroup) -> Vec<RevealRow> {
        match group {
            RevealGroup::Font => self.reveal_font(),
            RevealGroup::Paragraph => self.reveal_paragraph(),
            RevealGroup::Section => self.reveal_section(),
        }
    }

    fn reveal_font(&self) -> Vec<RevealRow> {
        let state = self.format_state();
        let character = &state.character;
        let mut rows = Vec::new();
        let latin = character
            .latin_font
            .clone()
            .or_else(|| state.font_family.clone());
        if let Some(east_asia) = &character.east_asia_font {
            rows.push(text("reveal.east_asia_font", east_asia.clone()));
        }
        if let Some(latin) = latin {
            rows.push(text("reveal.latin_font", latin));
        }
        if let Some(size) = character.font_size.or(state.font_size) {
            rows.push(text("reveal.size", format!("{} pt", number(size))));
        }
        let effects: [(bool, &'static str); 8] = [
            (state.bold, "reveal.bold"),
            (state.italic, "reveal.italic"),
            (state.underline, "reveal.underline"),
            (state.strike, "reveal.strike"),
            (character.double_strike == Some(true), "reveal.double_strike"),
            (state.superscript, "reveal.superscript"),
            (state.subscript, "reveal.subscript"),
            (character.small_caps == Some(true), "reveal.small_caps"),
        ];
        for (on, name) in effects {
            if on {
                rows.push(key("reveal.effect", name));
            }
        }
        if character.hidden == Some(true) {
            rows.push(key("reveal.effect", "reveal.hidden"));
        }
        if let Some(color) = character.color.as_deref() {
            rows.push(match color {
                "auto" => key("reveal.color", "reveal.automatic"),
                color => text("reveal.color", format!("#{color}")),
            });
        }
        if let Some(spacing) = character.spacing.filter(|spacing| *spacing != 0.0) {
            rows.push(text("reveal.spacing", format!("{} pt", number(spacing))));
        }
        rows
    }

    fn reveal_paragraph(&self) -> Vec<RevealRow> {
        let state = self.format_state();
        let format = self.paragraph_format();
        let mut rows = Vec::new();
        let styles = self.paragraph_styles();
        let style = state
            .style_id
            .as_deref()
            .and_then(|id| styles.iter().find(|style| style.id == id))
            .or_else(|| styles.iter().find(|style| style.is_default));
        if let Some(style) = style {
            rows.push(RevealRow {
                label: "reveal.style",
                value: RevealValue::Style(style.name.clone()),
            });
        }
        rows.push(key(
            "reveal.alignment",
            match format.alignment.as_str() {
                "center" => "reveal.align.center",
                "right" => "reveal.align.right",
                "both" => "reveal.align.both",
                "distribute" => "reveal.align.distribute",
                _ => "reveal.align.left",
            },
        ));
        let indent = |value: f32| match format.unit {
            IndentUnit::Characters => format!("{} 字", number(value)),
            IndentUnit::Millimetres => format!("{} mm", number(value)),
        };
        rows.push(text("reveal.indent_left", indent(format.left)));
        rows.push(text("reveal.indent_right", indent(format.right)));
        match format.special {
            SpecialIndent::FirstLine => {
                rows.push(text("reveal.first_line", indent(format.special_by)))
            }
            SpecialIndent::Hanging => rows.push(text("reveal.hanging", indent(format.special_by))),
            SpecialIndent::None => {}
        }
        rows.push(text("reveal.before", format!("{} pt", number(format.before))));
        rows.push(text("reveal.after", format!("{} pt", number(format.after))));
        rows.push(match format.line {
            LineSpacing::Single => key("reveal.line", "reveal.line.single"),
            LineSpacing::OneAndHalf => key("reveal.line", "reveal.line.one_and_half"),
            LineSpacing::Double => key("reveal.line", "reveal.line.double"),
            LineSpacing::AtLeast(points) => {
                text("reveal.line_at_least", format!("{} pt", number(points)))
            }
            LineSpacing::Exactly(points) => {
                text("reveal.line_exactly", format!("{} pt", number(points)))
            }
            LineSpacing::Multiple(lines) => text("reveal.line_multiple", number(lines)),
        });
        if format.outline_level == BODY_TEXT {
            rows.push(key("reveal.outline", "paragraph.outline.body"));
        } else {
            rows.push(text(
                "reveal.outline_level",
                (format.outline_level + 1).to_string(),
            ));
        }
        for (on, name) in [
            (format.keep_next, "reveal.keep_next"),
            (format.keep_lines, "reveal.keep_lines"),
            (format.page_break_before, "reveal.page_break_before"),
        ] {
            if on {
                rows.push(key("reveal.pagination", name));
            }
        }
        rows
    }

    fn reveal_section(&self) -> Vec<RevealRow> {
        let layout = self.section_layout();
        let mut rows = vec![
            text(
                "reveal.paper",
                format!(
                    "{} × {} mm",
                    number(layout.width * MM_PER_PT),
                    number(layout.height * MM_PER_PT)
                ),
            ),
            key(
                "reveal.orientation",
                if layout.landscape {
                    "reveal.landscape"
                } else {
                    "reveal.portrait"
                },
            ),
            text("reveal.margin_top", millimetres(layout.margin_top)),
            text("reveal.margin_bottom", millimetres(layout.margin_bottom)),
            text("reveal.margin_left", millimetres(layout.margin_left)),
            text("reveal.margin_right", millimetres(layout.margin_right)),
            key(
                "reveal.section_start",
                match layout.start.as_str() {
                    "continuous" => "reveal.start.continuous",
                    "evenPage" => "reveal.start.even",
                    "oddPage" => "reveal.start.odd",
                    "nextColumn" => "reveal.start.column",
                    _ => "reveal.start.next_page",
                },
            ),
        ];
        if layout.columns.count > 1 {
            rows.push(text("reveal.columns", layout.columns.count.to_string()));
        }
        if layout.text_direction == "tbRl" {
            rows.push(key("reveal.direction", "reveal.vertical"));
        }
        rows
    }
}

impl<T: WordFormatting + WordParagraphs + WordPageSetup + ?Sized> WordRevealFormatting for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_drop_their_trailing_zeros() {
        assert_eq!(number(10.5), "10.5");
        assert_eq!(number(12.0), "12");
        assert_eq!(millimetres(72.0), "25.4 mm");
    }
}
