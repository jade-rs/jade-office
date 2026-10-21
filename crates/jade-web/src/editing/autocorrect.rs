//! オートコレクト and 入力オートフォーマット as the browser types: the shared
//! model's decisions (`jade_office_model::word::autocorrect`), each one undo
//! step after the typing that set it off, as in Word and the native app.

use jade_office_model::word::autocorrect::{
    corrections, heading_from, list_start, on_enter, position_after, AutoCorrectOptions,
    WordAutoCorrect,
};

use super::EditableDocument;

impl EditableDocument {
    /// After text went in before the caret: the corrections it sets off — a
    /// list started by `1. `, a word replaced, a capital, a quote.
    pub fn autocorrect_typed(&mut self, options: &AutoCorrectOptions) -> Result<(), String> {
        if self.selection().is_some() || self.composition().is_some() {
            return Ok(());
        }
        let Some((paragraph, caret)) = self.caret() else {
            return Ok(());
        };
        let text = self.paragraph_text(paragraph).unwrap_or_default();
        if let Some(list) = list_start(&text, caret, options) {
            if text[caret.min(text.len())..].trim().is_empty() {
                self.start_list(paragraph, &list)?;
                self.place_caret(paragraph, 0);
                return Ok(());
            }
        }
        let fixes = corrections(&text, caret, options);
        if fixes.is_empty() {
            return Ok(());
        }
        self.apply_fixes(paragraph, &fixes, caret)?;
        let (paragraph, offset) = position_after((paragraph, caret), paragraph, &fixes);
        self.place_caret(paragraph, offset);
        Ok(())
    }

    /// After Enter ended paragraph `ended`: its last word corrected, and a
    /// border, a 結語, 以上 or a heading as 入力オートフォーマット makes them.
    pub fn autocorrect_enter(
        &mut self,
        ended: usize,
        options: &AutoCorrectOptions,
    ) -> Result<(), String> {
        let caret = self.caret();
        let Some(text) = self.paragraph_text(ended) else {
            return Ok(());
        };
        let with_end = format!("{text}\n");
        let fixes: Vec<_> = corrections(&with_end, with_end.len(), options)
            .into_iter()
            .filter(|fix| fix.end <= text.len())
            .collect();
        if !fixes.is_empty() {
            self.apply_fixes(ended, &fixes, text.len())?;
        }
        let text = self.paragraph_text(ended).unwrap_or_default();
        let following: String = (ended + 1..ended + 40)
            .filter_map(|index| self.paragraph_text(index))
            .collect::<Vec<_>>()
            .join("\n");
        if let Some(format) = on_enter(&text, &following, options) {
            self.format_on_enter(ended, &format)?;
        } else if text.trim().is_empty() && ended > 0 {
            let previous = self.paragraph_text(ended - 1).unwrap_or_default();
            if heading_from(&previous, options) {
                self.make_heading(ended - 1)?;
            }
        }
        if let Some((paragraph, offset)) = caret {
            self.place_caret(paragraph, offset);
        }
        Ok(())
    }
}
