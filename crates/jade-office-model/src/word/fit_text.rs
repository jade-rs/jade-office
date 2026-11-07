//! Word's 文字の均等割り付け (ホーム › 均等割り付け with text selected inside
//! a paragraph): the dialog that spreads the selected characters over a
//! width given in 字 — 現在の文字列の幅, 新しい文字列の幅 and 解除. Word keeps it
//! as `w:fitText` on the runs (the width in twips and one `w:id` for the
//! group); the engine writes it (`set_run_format`'s `fit_text`) and lays it
//! out, and this is what both shells do with it. With no text selected the
//! button stays the paragraph's 均等割り付け.

use jade_docx_headless_api::{HeadlessCommand, HeadlessRunFormat};

use super::{Helpers, WordSession};

/// The width of `text` in 字: a full-width character is one, a half-width
/// one a half.
pub fn text_width_in_characters(text: &str) -> f32 {
    text.chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| if is_half_width(ch) { 0.5 } else { 1.0 })
        .sum()
}

fn is_half_width(ch: char) -> bool {
    ch.is_ascii() || ('\u{FF61}'..='\u{FF9F}').contains(&ch)
}

/// A number of 字 as the dialog shows it: `4`, `4.5`.
pub fn characters_label(value: f32) -> String {
    let rounded = (value * 10.0).round() / 10.0;
    if (rounded - rounded.round()).abs() < 0.05 {
        format!("{}", rounded.round() as i64)
    } else {
        format!("{rounded:.1}")
    }
}

/// The 文字の均等割り付け dialog, on one paragraph's selected text.
#[derive(Clone, Debug, PartialEq)]
pub struct FitTextDialog {
    pub paragraph: usize,
    pub start: usize,
    pub end: usize,
    /// The selected text.
    pub text: String,
    /// The font size the 字 are measured in, in points.
    pub font_size: f32,
    /// 現在の文字列の幅, in 字: the text's own width, or the width it is
    /// already spread over.
    pub current: f32,
    /// The text is already spread (解除 has something to take off).
    pub fitted: bool,
    /// 新しい文字列の幅, as typed.
    pub new_width: String,
}

impl FitTextDialog {
    /// What OK sets: the selection spread over 新しい文字列の幅. A width
    /// it cannot take comes back as its message key.
    pub fn command(&self) -> Result<HeadlessCommand, &'static str> {
        let typed = self
            .new_width
            .trim()
            .trim_end_matches('字')
            .trim()
            .parse::<f32>()
            .map_err(|_| "fit_text.bad_width")?;
        if !typed.is_finite() || !(0.5..=1584.0).contains(&typed) {
            return Err("fit_text.bad_width");
        }
        Ok(self.with_width(typed * self.font_size))
    }

    /// 解除: the text back to its own width.
    pub fn remove_command(&self) -> HeadlessCommand {
        self.with_width(0.0)
    }

    fn with_width(&self, points: f32) -> HeadlessCommand {
        HeadlessCommand::SetRunFormat {
            start_paragraph: self.paragraph,
            start_offset: self.start,
            end_paragraph: self.paragraph,
            end_offset: self.end,
            format: Box::new(HeadlessRunFormat {
                fit_text: Some(points),
                ..Default::default()
            }),
        }
    }
}

/// 文字の均等割り付け on a Word session.
pub trait WordFitText: WordSession {
    /// The dialog on the selection, when it is text inside one paragraph —
    /// `None` keeps the button the paragraph's 均等割り付け.
    fn fit_text_dialog(&self) -> Option<FitTextDialog> {
        let range = self.selection_range()?;
        if range.is_collapsed() {
            return None;
        }
        let (start, end) = range.normalized();
        if start.paragraph != end.paragraph {
            return None;
        }
        let paragraph = start.paragraph;
        let full = self.text_of(paragraph)?;
        let text = full.get(start.offset..end.offset)?.to_string();
        if text.trim().is_empty() {
            return None;
        }
        let (font_size, fitted) = self.fit_text_at(paragraph, start.offset);
        let current = fitted.map_or_else(
            || text_width_in_characters(&text),
            |points| points / font_size,
        );
        Some(FitTextDialog {
            paragraph,
            start: start.offset,
            end: end.offset,
            text,
            font_size,
            current,
            fitted: fitted.is_some(),
            new_width: characters_label(current),
        })
    }

    /// The font size (points) of the run at `offset`, and the width it is
    /// spread over (points), if it is.
    fn fit_text_at(&self, paragraph: usize, offset: usize) -> (f32, Option<f32>) {
        let Some(paragraph) = self.word_document().paragraph(paragraph) else {
            return (10.5, None);
        };
        let mut start = 0;
        let mut found = None;
        for run in &paragraph.runs {
            let end = start + run.text.len();
            if !run.text.is_empty() {
                found = Some(run);
                if offset < end {
                    break;
                }
            }
            start = end;
        }
        let properties = found.and_then(|run| run.properties.as_ref());
        let size = properties
            .and_then(|properties| properties.font_size)
            .filter(|size| *size > 0.0)
            .unwrap_or_else(|| {
                // Inherited: the document's default size the grid measures 字 in.
                let size = self.word_document().section_layout(None).default_font_size;
                if size > 0.0 {
                    size
                } else {
                    10.5
                }
            });
        let fitted = properties
            .and_then(|properties| properties.fit_text.as_ref())
            .map(|fit| fit.val as f32 / 20.0);
        (size, fitted)
    }

    /// OK, as one undo step.
    fn apply_fit_text(&mut self, dialog: &FitTextDialog) -> Result<(), String> {
        let command = dialog.command().map_err(str::to_string)?;
        self.apply_command(command, true)
    }

    /// 解除, as one undo step.
    fn remove_fit_text(&mut self, dialog: &FitTextDialog) -> Result<(), String> {
        self.apply_command(dialog.remove_command(), true)
    }
}

impl<T: WordSession + ?Sized> WordFitText for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_count_full_and_half_width_characters() {
        assert_eq!(text_width_in_characters("氏名"), 2.0);
        assert_eq!(text_width_in_characters("ID番号"), 3.0);
        assert_eq!(characters_label(4.0), "4");
        assert_eq!(characters_label(4.5), "4.5");
    }

    #[test]
    fn the_new_width_is_in_characters_of_the_font_size() {
        let dialog = FitTextDialog {
            paragraph: 2,
            start: 0,
            end: 6,
            text: "氏名".into(),
            font_size: 10.5,
            current: 2.0,
            fitted: false,
            new_width: "4字".into(),
        };
        let HeadlessCommand::SetRunFormat { format, .. } = dialog.command().unwrap() else {
            panic!("a run format");
        };
        assert_eq!(format.fit_text, Some(42.0));
        let bad = FitTextDialog {
            new_width: "wide".into(),
            ..dialog.clone()
        };
        assert_eq!(bad.command().unwrap_err(), "fit_text.bad_width");
        let HeadlessCommand::SetRunFormat { format, .. } = dialog.remove_command() else {
            panic!("a run format");
        };
        assert_eq!(format.fit_text, Some(0.0));
    }
}
