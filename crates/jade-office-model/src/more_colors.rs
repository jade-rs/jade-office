//! The colour galleries' More Colors…: any colour by its `RRGGBB` code
//! (or `#RGB`), for the fill or the font, or picked as Excel's Custom tab
//! picks one — from a spectrum of hues and saturations, then lighter or
//! darker on a strip of its shades.

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

    /// Takes `rgb` (`RRGGBB`) as the colour, as a click on the spectrum or
    /// the strip of shades does.
    pub fn pick(&mut self, rgb: &str) {
        self.code = TextEdit::selected(rgb.to_string());
    }

    /// The spectrum, as Excel's Custom tab lays it out: hues across, red to
    /// red, and from full saturation at the top to grey at the bottom, all
    /// at middle lightness — [`SPECTRUM_ROWS`] rows of [`SPECTRUM_HUES`]
    /// `RRGGBB` colours.
    pub fn spectrum() -> Vec<Vec<String>> {
        (0..SPECTRUM_ROWS)
            .map(|row| {
                let saturation = 1.0 - row as f64 / (SPECTRUM_ROWS - 1) as f64;
                (0..SPECTRUM_HUES)
                    .map(|column| {
                        let hue = column as f64 / SPECTRUM_HUES as f64;
                        hex(hsl_to_rgb(hue, saturation, 0.5))
                    })
                    .collect()
            })
            .collect()
    }

    /// The strip beside the spectrum: the colour typed (red when none is),
    /// its hue and saturation kept, from white at the top through the colour
    /// itself to black — [`SHADES`] `RRGGBB` colours.
    pub fn shades(&self) -> Vec<String> {
        let rgb = self
            .color()
            .and_then(|code| u32::from_str_radix(&code, 16).ok())
            .map_or([255, 0, 0], |value| {
                [(value >> 16) as u8, (value >> 8) as u8, value as u8]
            });
        let (hue, saturation, _) = rgb_to_hsl(rgb);
        (0..SHADES)
            .map(|step| {
                let lightness = 1.0 - step as f64 / (SHADES - 1) as f64;
                hex(hsl_to_rgb(hue, saturation, lightness))
            })
            .collect()
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

/// The spectrum's columns: a hue every 15°.
pub const SPECTRUM_HUES: usize = 24;
/// The spectrum's rows: saturation from full to none.
pub const SPECTRUM_ROWS: usize = 8;
/// The strip's shades, white to black.
pub const SHADES: usize = 13;

fn hex([r, g, b]: [u8; 3]) -> String {
    format!("{r:02X}{g:02X}{b:02X}")
}

/// Hue, saturation and lightness, each 0–1.
fn rgb_to_hsl([r, g, b]: [u8; 3]) -> (f64, f64, f64) {
    let [r, g, b] = [r, g, b].map(|channel| f64::from(channel) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    if delta == 0.0 {
        return (0.0, 0.0, lightness);
    }
    let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
    let hue = if max == r {
        ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        (b - r) / delta + 2.0
    } else {
        (r - g) / delta + 4.0
    } / 6.0;
    (hue, saturation, lightness)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> [u8; 3] {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let sector = hue.rem_euclid(1.0) * 6.0;
    let x = chroma * (1.0 - (sector.rem_euclid(2.0) - 1.0).abs());
    let (r, g, b) = match sector as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;
    [r, g, b].map(|channel| ((channel + m).clamp(0.0, 1.0) * 255.0).round() as u8)
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

    #[test]
    fn the_spectrum_and_shades_pick_colours() {
        let spectrum = MoreColorsDialog::spectrum();
        assert_eq!(spectrum.len(), SPECTRUM_ROWS);
        assert!(spectrum.iter().all(|row| row.len() == SPECTRUM_HUES));
        assert_eq!(spectrum[0][0], "FF0000");
        assert_eq!(spectrum[0][8], "00FF00");
        assert_eq!(spectrum[0][16], "0000FF");
        assert_eq!(spectrum[SPECTRUM_ROWS - 1][5], "808080");
        let mut dialog = MoreColorsDialog::new(true, None);
        dialog.pick("0000FF");
        assert_eq!(dialog.color().as_deref(), Some("0000FF"));
        let shades = dialog.shades();
        assert_eq!(shades.len(), SHADES);
        assert_eq!(shades[0], "FFFFFF");
        assert_eq!(shades[SHADES / 2], "0000FF");
        assert_eq!(shades[SHADES - 1], "000000");
        dialog.pick(&shades[3]);
        // A lighter blue keeps its hue: its shades are blue's.
        assert_eq!(dialog.shades(), shades);
    }
}
