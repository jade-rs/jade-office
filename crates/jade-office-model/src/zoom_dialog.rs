//! View ▸ Zoom, as Excel's dialog: 200%, 100%, 75%, 50% or 25%, Fit
//! selection, or a custom percentage from 10 to 400 typed in.

use crate::sheet_properties::zoom_percent;
use crate::text::TextEdit;

/// What a key did in the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomKey {
    Take,
    Close,
    Handled,
    Unhandled,
}

/// One of the dialog's choices.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZoomChoice {
    /// A preset percentage.
    Percent(u32),
    /// Fit selection: the zoom at which the selection fills the window.
    FitSelection,
    /// What is typed in the Custom box.
    Custom,
}

pub struct ZoomDialog {
    choice: ZoomChoice,
    /// The Custom box.
    pub custom: TextEdit,
    /// The box's text when a choice was last made: typing past it picks
    /// Custom, as typing in Excel's box does.
    chosen_with: String,
    /// The box held something that is not a zoom when OK was pressed.
    pub problem: bool,
}

impl ZoomDialog {
    /// Excel's presets, top to bottom.
    pub const PRESETS: [u32; 5] = [200, 100, 75, 50, 25];

    /// Opened on the zoom the sheet is at (a factor): its preset if it has
    /// one, else Custom, the box holding it either way.
    pub fn new(zoom: f32) -> Self {
        let percent = zoom_percent(zoom);
        let text = percent.to_string();
        Self {
            choice: if Self::PRESETS.contains(&percent) {
                ZoomChoice::Percent(percent)
            } else {
                ZoomChoice::Custom
            },
            custom: TextEdit::selected(text.clone()),
            chosen_with: text,
            problem: false,
        }
    }

    /// The choice that OK takes.
    pub fn choice(&self) -> ZoomChoice {
        if self.custom.text != self.chosen_with {
            ZoomChoice::Custom
        } else {
            self.choice
        }
    }

    /// A click on one of the choices.
    pub fn choose(&mut self, choice: ZoomChoice) {
        self.choice = choice;
        self.chosen_with = self.custom.text.clone();
        self.problem = false;
    }

    /// The percentage typed in the box (a trailing `%` allowed), when it is
    /// a whole number Excel takes: 10 to 400.
    pub fn custom_percent(&self) -> Option<u32> {
        let typed = self.custom.text.trim().trim_end_matches('%').trim();
        let percent: u32 = typed.parse().ok()?;
        (10..=400).contains(&percent).then_some(percent)
    }

    /// The zoom OK sets, as a factor, `fit` being the one that fits the
    /// selection; `None` when Custom holds something Excel refuses.
    pub fn zoom(&self, fit: impl FnOnce() -> f32) -> Option<f32> {
        match self.choice() {
            ZoomChoice::Percent(percent) => Some(percent as f32 / 100.0),
            ZoomChoice::FitSelection => Some(fit()),
            ZoomChoice::Custom => self.custom_percent().map(|percent| percent as f32 / 100.0),
        }
    }

    /// The dialog's keys: Enter takes, Escape closes, and the rest edit the
    /// Custom box.
    pub fn key(&mut self, key: &str, command: bool) -> ZoomKey {
        if self.custom.marked.is_some() {
            return ZoomKey::Unhandled;
        }
        match key {
            "enter" => ZoomKey::Take,
            "escape" => ZoomKey::Close,
            "a" if command => {
                self.custom.whole = true;
                ZoomKey::Handled
            }
            key if self.custom.edit_key(key) => {
                self.problem = false;
                ZoomKey::Handled
            }
            _ => ZoomKey::Unhandled,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opens_on_the_sheets_zoom() {
        assert_eq!(ZoomDialog::new(0.75).choice(), ZoomChoice::Percent(75));
        let odd = ZoomDialog::new(0.85);
        assert_eq!(odd.choice(), ZoomChoice::Custom);
        assert_eq!(odd.custom.text, "85");
    }

    #[test]
    fn typing_in_the_box_picks_custom() {
        let mut dialog = ZoomDialog::new(1.0);
        dialog.custom = TextEdit::new("130%".to_string());
        assert_eq!(dialog.choice(), ZoomChoice::Custom);
        assert_eq!(dialog.zoom(|| 0.0), Some(1.3));
        dialog.choose(ZoomChoice::FitSelection);
        assert_eq!(dialog.zoom(|| 0.42), Some(0.42));
        dialog.choose(ZoomChoice::Percent(200));
        assert_eq!(dialog.zoom(|| 0.0), Some(2.0));
    }

    #[test]
    fn a_custom_zoom_outside_10_to_400_is_refused() {
        let mut dialog = ZoomDialog::new(1.0);
        for typed in ["9", "401", "abc", ""] {
            dialog.custom = TextEdit::new(typed.to_string());
            assert_eq!(dialog.zoom(|| 0.0), None, "{typed}");
        }
        dialog.custom = TextEdit::new("400".to_string());
        assert_eq!(dialog.zoom(|| 0.0), Some(4.0));
    }
}
