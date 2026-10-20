//! Copy and paste that keep the formatting (元の書式を保持), over any
//! [`WordSession`].
//!
//! A copy takes the selection's runs with their character formatting and
//! each paragraph's style and alignment; a paste puts them back as one undo
//! step — text, paragraph breaks, then each run's format where it differs
//! from what the text would take on where it lands. The plain text goes on
//! the system clipboard as before; a shell keeps the copy beside it and
//! pastes it whenever the clipboard still holds that text.

use jade_docx_headless_api::{
    HeadlessCommand, HeadlessQuery, HeadlessQueryResult, HeadlessRunStyle,
};

use super::{query, Position, WordFormatting, WordSession};

/// A run of copied text and its character formatting.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipRun {
    pub text: String,
    pub style: HeadlessRunStyle,
}

/// A copied paragraph, or the part of one the selection held.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ClipParagraph {
    pub style_id: Option<String>,
    pub alignment: Option<String>,
    pub runs: Vec<ClipRun>,
}

/// What a copy took.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RichClip {
    pub paragraphs: Vec<ClipParagraph>,
}

impl RichClip {
    /// The text the system clipboard gets: paragraphs joined by newlines,
    /// which a plain paste turns back into paragraph breaks.
    pub fn plain_text(&self) -> String {
        self.paragraphs
            .iter()
            .map(|paragraph| {
                paragraph
                    .runs
                    .iter()
                    .map(|run| run.text.as_str())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Whether `text` — what the system clipboard holds now — is this copy's,
    /// so pasting it should keep the formatting. Line endings may have been
    /// rewritten on the way.
    pub fn matches(&self, text: &str) -> bool {
        text.replace("\r\n", "\n").replace('\r', "\n") == self.plain_text()
    }
}

/// The formatting commands that give `start..end` of a paragraph `style`,
/// for each property `style` states and `inherited` does not already have.
fn format_commands(
    paragraph: usize,
    (start, end): (usize, usize),
    style: &HeadlessRunStyle,
    inherited: &HeadlessRunStyle,
) -> Vec<HeadlessCommand> {
    let (start_paragraph, start_offset, end_paragraph, end_offset) =
        (paragraph, start, paragraph, end);
    let mut commands = Vec::new();
    let differs = |wanted: Option<bool>, had: Option<bool>| {
        let wanted = wanted.unwrap_or(false);
        (wanted != had.unwrap_or(false)).then_some(wanted)
    };
    if let Some(enabled) = differs(style.bold, inherited.bold) {
        commands.push(HeadlessCommand::ApplyBold {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.italic, inherited.italic) {
        commands.push(HeadlessCommand::ApplyItalic {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.underline, inherited.underline) {
        commands.push(HeadlessCommand::ApplyUnderline {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            kind: None,
            enabled,
        });
    }
    if let Some(enabled) = differs(style.strike, inherited.strike) {
        commands.push(HeadlessCommand::ApplyStrike {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            double: false,
            enabled,
        });
    }
    if style.baseline != inherited.baseline {
        match style.baseline.as_deref() {
            Some("superscript") => commands.push(HeadlessCommand::ApplySuperscript {
                start_paragraph,
                start_offset,
                end_paragraph,
                end_offset,
                enabled: true,
            }),
            Some("subscript") => commands.push(HeadlessCommand::ApplySubscript {
                start_paragraph,
                start_offset,
                end_paragraph,
                end_offset,
                enabled: true,
            }),
            _ => {
                let off = if inherited.baseline.as_deref() == Some("subscript") {
                    HeadlessCommand::ApplySubscript {
                        start_paragraph,
                        start_offset,
                        end_paragraph,
                        end_offset,
                        enabled: false,
                    }
                } else {
                    HeadlessCommand::ApplySuperscript {
                        start_paragraph,
                        start_offset,
                        end_paragraph,
                        end_offset,
                        enabled: false,
                    }
                };
                commands.push(off);
            }
        }
    }
    if let Some(font_size) = style
        .font_size
        .filter(|size| Some(*size) != inherited.font_size)
    {
        commands.push(HeadlessCommand::SetFontSize {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            font_size,
        });
    }
    if let Some(family) = style
        .font_family
        .as_ref()
        .filter(|family| Some(*family) != inherited.font_family.as_ref())
    {
        commands.push(HeadlessCommand::SetFontFamily {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            font_family: family.clone(),
        });
    }
    if style.color != inherited.color {
        let color = match style.color.as_deref() {
            Some(color) if color.eq_ignore_ascii_case("auto") => "auto".to_string(),
            Some(color) => format!("#{}", color.trim_start_matches('#')),
            None => "auto".to_string(),
        };
        commands.push(HeadlessCommand::SetFontColor {
            start_paragraph,
            start_offset,
            end_paragraph,
            end_offset,
            color,
        });
    }
    commands
}

/// Formatted copy and paste for every session.
pub trait WordClipboard: WordSession {
    /// The selection's text with its formatting; `None` for a caret.
    fn copy_rich(&self) -> Option<RichClip> {
        let range = self
            .selection_range()
            .filter(|range| !range.is_collapsed())?;
        let (start, end) = range.normalized();
        let mut paragraphs = Vec::new();
        for index in start.paragraph..=end.paragraph {
            let Some(HeadlessQueryResult::Paragraph {
                value: Some(snapshot),
                ..
            }) = query(
                self.word_document(),
                HeadlessQuery::Paragraph { paragraph: index },
            )
            else {
                continue;
            };
            let from = if index == start.paragraph {
                start.offset
            } else {
                0
            };
            let to = if index == end.paragraph {
                end.offset
            } else {
                snapshot.text.len()
            };
            let mut runs: Vec<ClipRun> = snapshot
                .runs
                .iter()
                .filter_map(|run| {
                    let (run_start, run_end) = (run.start_offset.max(from), run.end_offset.min(to));
                    let text = snapshot.text.get(run_start..run_end)?;
                    (!text.is_empty()).then(|| ClipRun {
                        text: text.to_string(),
                        style: run.style.clone(),
                    })
                })
                .collect();
            // A paragraph whose runs the snapshot does not cover still copies
            // its text.
            let covered: usize = runs.iter().map(|run| run.text.len()).sum();
            if covered == 0 {
                if let Some(text) = snapshot.text.get(from..to.max(from)) {
                    if !text.is_empty() {
                        runs.push(ClipRun {
                            text: text.to_string(),
                            style: HeadlessRunStyle::default(),
                        });
                    }
                }
            }
            paragraphs.push(ClipParagraph {
                style_id: snapshot.style_id.clone(),
                alignment: snapshot.alignment.clone(),
                runs,
            });
        }
        Some(RichClip { paragraphs })
    }

    /// Pastes `clip` in place of the selection, or at the caret, as one undo
    /// step. Returns where the caret goes: after what was pasted.
    fn paste_rich(&mut self, clip: &RichClip) -> Result<Position, String> {
        let range = self.selection_range().ok_or("no caret")?;
        let (start, _) = range.normalized();
        let mut commands = Vec::new();
        if !range.is_collapsed() {
            let (start, end) = range.normalized();
            commands.push(HeadlessCommand::ReplaceRange {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                text: String::new(),
            });
        }
        // What typed text would take on where the paste lands: the character
        // before the caret, as typing continues it.
        let inherited = self
            .run_style_at((start.paragraph, start.offset.saturating_sub(1)))
            .unwrap_or_default();
        let mut at = (start.paragraph, start.offset);
        let mut formats = Vec::new();
        let count = clip.paragraphs.len();
        for (index, paragraph) in clip.paragraphs.iter().enumerate() {
            if index > 0 {
                commands.push(HeadlessCommand::InsertParagraphBreak {
                    paragraph: at.0,
                    offset: at.1,
                    count: 1,
                });
                at = (at.0 + 1, 0);
            }
            // Every copied paragraph but the last came with its mark, and the
            // mark carries the paragraph's style to the paragraph it ends
            // here; the last ends in the destination's own mark.
            if index + 1 < count {
                if let Some(style_id) = &paragraph.style_id {
                    formats.push(HeadlessCommand::SetParagraphStyle {
                        start_paragraph: at.0,
                        end_paragraph: at.0,
                        style_id: Some(style_id.clone()),
                    });
                }
                if let Some(alignment) = &paragraph.alignment {
                    formats.push(HeadlessCommand::SetParagraphAlignment {
                        start_paragraph: at.0,
                        end_paragraph: at.0,
                        alignment: alignment.clone(),
                    });
                }
            }
            for run in &paragraph.runs {
                commands.push(HeadlessCommand::InsertText {
                    paragraph: at.0,
                    offset: at.1,
                    text: run.text.clone(),
                });
                let end = at.1 + run.text.len();
                formats.extend(format_commands(at.0, (at.1, end), &run.style, &inherited));
                at.1 = end;
            }
        }
        // Formats after all the text: a split carries a paragraph's direct
        // formatting into the next, so formatting first would be undone.
        commands.extend(formats);
        if commands.is_empty() {
            return Ok(at);
        }
        self.apply_commands(commands, false)?;
        Ok(at)
    }
}

impl<T: WordSession + ?Sized> WordClipboard for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plain_text_joins_paragraphs_with_newlines() {
        let run = |text: &str| ClipRun {
            text: text.to_string(),
            style: HeadlessRunStyle::default(),
        };
        let clip = RichClip {
            paragraphs: vec![
                ClipParagraph {
                    runs: vec![run("Bold"), run(" plain")],
                    ..Default::default()
                },
                ClipParagraph {
                    runs: vec![run("next")],
                    ..Default::default()
                },
            ],
        };
        assert_eq!(clip.plain_text(), "Bold plain\nnext");
        assert!(clip.matches("Bold plain\r\nnext"));
        assert!(!clip.matches("Bold plain"));
    }

    #[test]
    fn only_what_differs_is_formatted() {
        let bold = HeadlessRunStyle {
            bold: Some(true),
            font_size: Some(14.0),
            ..Default::default()
        };
        let plain = HeadlessRunStyle {
            font_size: Some(14.0),
            ..Default::default()
        };
        let commands = format_commands(3, (0, 4), &bold, &plain);
        assert_eq!(commands.len(), 1, "{commands:?}");
        assert!(matches!(
            commands[0],
            HeadlessCommand::ApplyBold { enabled: true, .. }
        ));
        // Into bold text, plain text has bold taken off.
        let commands = format_commands(3, (0, 4), &plain, &bold);
        assert!(matches!(
            commands[0],
            HeadlessCommand::ApplyBold { enabled: false, .. }
        ));
    }
}
