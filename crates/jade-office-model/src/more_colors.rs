//! The colour galleries' More Colors…: any colour by its `RRGGBB` code
//! (or `#RGB`), for the fill or the font.

use crate::text::TextEdit;

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoreColorsKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// The dialog: the code typed, and whether it is for the fill.
pub struct MoreColorsDialog {
    pub fill: bool,
    pub code: TextEdit,
}

impl MoreColorsDialog {
    /// Opened on `current`, the colour the selection has, if any.
    pub fn new(fill: bool, current: Option<&str>) -> Self {
        Self {
            fill,
            code: TextEdit::selected(current.unwrap_or_default().to_string()),
        }
    }

    /// The colour typed as `RRGGBB`: six hex digits, or three as `#RGB` is
    /// short for them, with or without the `#`.
    pub fn color(&self) -> Option<String> {
        let typed = self.code.text.trim().trim_start_matches('#');
        if !typed.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        match typed.len() {
            6 => Some(typed.to_ascii_uppercase()),
            3 => Some(
                typed
                    .chars()
                    .flat_map(|c| [c, c])
                    .collect::<String>()
                    .to_ascii_uppercase(),
            ),
            _ => None,
        }
    }

    /// The dialog's keys: Enter takes the colour, Escape closes, the rest
    /// edit the code.
    pub fn key(&mut self, key: &str, command: bool) -> MoreColorsKey {
        if self.code.marked.is_some() {
            return MoreColorsKey::Unhandled;
        }
        match key {
            "enter" => MoreColorsKey::Take,
            "escape" => MoreColorsKey::Close,
            "a" if command => {
                self.code.whole = true;
                MoreColorsKey::Handled
            }
            key if self.code.edit_key(key) => MoreColorsKey::Handled,
            _ => MoreColorsKey::Unhandled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_read_as_excel_takes_them() {
        let mut dialog = MoreColorsDialog::new(true, Some("FF0000"));
        assert_eq!(dialog.color().as_deref(), Some("FF0000"));
        dialog.code = TextEdit::new("#1a2b3c".into());
        assert_eq!(dialog.color().as_deref(), Some("1A2B3C"));
        dialog.code = TextEdit::new("f80".into());
        assert_eq!(dialog.color().as_deref(), Some("FF8800"));
        dialog.code = TextEdit::new("blue".into());
        assert_eq!(dialog.color(), None);
        dialog.code = TextEdit::new("12345".into());
        assert_eq!(dialog.color(), None);
        assert_eq!(dialog.key("enter", false), MoreColorsKey::Take);
    }
}
