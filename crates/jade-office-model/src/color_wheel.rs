//! その他の色…: the colour wheel Word for Mac's 色の設定 opens on — the hue
//! by the angle around the circle (red at the right, going round
//! counter-clockwise through yellow, green, cyan, blue and magenta), the
//! saturation by the distance from the centre (white, or grey, at the
//! centre), and the brightness on a slider beside it — with the new colour
//! shown against the current one and its hex code to type.
//!
//! Points are relative to the wheel's centre, `y` growing downwards as on
//! the screen. The state keeps hue, saturation and brightness rather than
//! the RGB they make, so dragging the brightness to black and back keeps the
//! hue.

/// A colour as hue (degrees, `0..360`), saturation and brightness (`0..=1`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hsv {
    pub hue: f64,
    pub saturation: f64,
    pub value: f64,
}

/// `RRGGBB`, or `#RGB` short for it, with or without the `#`.
pub fn parse_hex(text: &str) -> Option<(u8, u8, u8)> {
    let hex = text.trim().trim_start_matches('#');
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let full = match hex.len() {
        6 => hex.to_string(),
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        _ => return None,
    };
    let value = u32::from_str_radix(&full, 16).ok()?;
    Some(((value >> 16) as u8, (value >> 8) as u8, value as u8))
}

/// `RRGGBB`, upper case.
pub fn hex((r, g, b): (u8, u8, u8)) -> String {
    format!("{r:02X}{g:02X}{b:02X}")
}

pub fn hsv_to_rgb(hsv: Hsv) -> (u8, u8, u8) {
    let hue = hsv.hue.rem_euclid(360.0) / 60.0;
    let s = hsv.saturation.clamp(0.0, 1.0);
    let v = hsv.value.clamp(0.0, 1.0);
    let chroma = v * s;
    let x = chroma * (1.0 - ((hue % 2.0) - 1.0).abs());
    let (r, g, b) = match hue as u32 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = v - chroma;
    let byte = |c: f64| ((c + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (byte(r), byte(g), byte(b))
}

pub fn rgb_to_hsv((r, g, b): (u8, u8, u8)) -> Hsv {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let hue = if delta == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };
    Hsv {
        hue,
        saturation: if max == 0.0 { 0.0 } else { delta / max },
        value: max,
    }
}

/// The hue and saturation at `(dx, dy)` from the centre of a wheel of
/// `radius`; a point outside the circle is taken at its edge.
pub fn hue_saturation_at(dx: f64, dy: f64, radius: f64) -> (f64, f64) {
    if radius <= 0.0 {
        return (0.0, 0.0);
    }
    let distance = (dx * dx + dy * dy).sqrt();
    let saturation = (distance / radius).min(1.0);
    let hue = if distance == 0.0 {
        0.0
    } else {
        (-dy).atan2(dx).to_degrees().rem_euclid(360.0)
    };
    (hue, saturation)
}

/// Where a hue and saturation sit on a wheel of `radius`, from its centre:
/// where the marker is drawn.
pub fn point_of(hue: f64, saturation: f64, radius: f64) -> (f64, f64) {
    let angle = hue.to_radians();
    let distance = saturation.clamp(0.0, 1.0) * radius;
    (distance * angle.cos(), -distance * angle.sin())
}

/// The brightness at `y` down a slider `height` tall: full at the top,
/// black at the bottom, clamped to its ends.
pub fn brightness_at(y: f64, height: f64) -> f64 {
    if height <= 0.0 {
        return 1.0;
    }
    (1.0 - y / height).clamp(0.0, 1.0)
}

/// One square of the wheel as it is drawn: its top-left corner from the
/// wheel's top-left, and its colour.
#[derive(Clone, Debug, PartialEq)]
pub struct WheelCell {
    pub x: f64,
    pub y: f64,
    pub rgb: (u8, u8, u8),
}

/// The wheel of `radius` cut into `cell`-sized squares, the ones whose
/// centre is inside the circle, each the colour at its centre at
/// `brightness` — what the dialog paints.
pub fn wheel_cells(radius: f64, cell: f64, brightness: f64) -> Vec<WheelCell> {
    if radius <= 0.0 || cell <= 0.0 {
        return Vec::new();
    }
    let count = (2.0 * radius / cell).ceil() as usize;
    let mut cells = Vec::with_capacity(count * count);
    for row in 0..count {
        for column in 0..count {
            let (x, y) = (column as f64 * cell, row as f64 * cell);
            let (dx, dy) = (x + cell / 2.0 - radius, y + cell / 2.0 - radius);
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let (hue, saturation) = hue_saturation_at(dx, dy, radius);
            cells.push(WheelCell {
                x,
                y,
                rgb: hsv_to_rgb(Hsv {
                    hue,
                    saturation,
                    value: brightness,
                }),
            });
        }
    }
    cells
}

/// The picker's state: the colour being made, and the one it opened on.
#[derive(Clone, Debug, PartialEq)]
pub struct ColorWheel {
    pub hsv: Hsv,
    /// The colour the dialog opened on, `RRGGBB`, shown beside the new one.
    pub current: String,
}

impl ColorWheel {
    /// Opened on `current` (`RRGGBB`), black when it is not a colour.
    pub fn new(current: Option<&str>) -> Self {
        let rgb = current.and_then(parse_hex).unwrap_or((0, 0, 0));
        Self {
            hsv: rgb_to_hsv(rgb),
            current: hex(rgb),
        }
    }

    /// The new colour, `RRGGBB`.
    pub fn hex(&self) -> String {
        hex(hsv_to_rgb(self.hsv))
    }

    /// A click or drag on the wheel at `(dx, dy)` from its centre. A wheel
    /// at black would show nothing to pick: the brightness comes up to full.
    pub fn pick_point(&mut self, dx: f64, dy: f64, radius: f64) {
        let (hue, saturation) = hue_saturation_at(dx, dy, radius);
        self.hsv.hue = hue;
        self.hsv.saturation = saturation;
        if self.hsv.value == 0.0 {
            self.hsv.value = 1.0;
        }
    }

    /// A click or drag on the brightness slider.
    pub fn set_brightness(&mut self, value: f64) {
        self.hsv.value = value.clamp(0.0, 1.0);
    }

    /// A colour typed or picked from a swatch: the wheel goes to it. A grey
    /// keeps the hue the wheel had.
    pub fn set_hex(&mut self, text: &str) -> bool {
        let Some(rgb) = parse_hex(text) else {
            return false;
        };
        let hsv = rgb_to_hsv(rgb);
        self.hsv = Hsv {
            hue: if hsv.saturation == 0.0 {
                self.hsv.hue
            } else {
                hsv.hue
            },
            ..hsv
        };
        true
    }

    /// Where the marker sits on a wheel of `radius`, from its centre.
    pub fn marker(&self, radius: f64) -> (f64, f64) {
        point_of(self.hsv.hue, self.hsv.saturation, radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const R: f64 = 80.0;

    fn at(dx: f64, dy: f64, brightness: f64) -> String {
        let (hue, saturation) = hue_saturation_at(dx, dy, R);
        hex(hsv_to_rgb(Hsv {
            hue,
            saturation,
            value: brightness,
        }))
    }

    #[test]
    fn red_is_at_angle_zero_on_the_edge() {
        assert_eq!(at(R, 0.0, 1.0), "FF0000");
    }

    #[test]
    fn the_hues_go_round_counter_clockwise() {
        // Up the screen is a quarter turn: between yellow and green.
        assert_eq!(at(0.0, -R, 1.0), "80FF00");
        // Left: cyan. Down: between blue and magenta.
        assert_eq!(at(-R, 0.0, 1.0), "00FFFF");
        assert_eq!(at(0.0, R, 1.0), "8000FF");
        let (green, _) = hue_saturation_at(-R / 2.0, -R * 3f64.sqrt() / 2.0, R);
        assert!((green - 120.0).abs() < 1e-9);
    }

    #[test]
    fn the_centre_is_white_at_full_brightness_and_black_at_none() {
        assert_eq!(at(0.0, 0.0, 1.0), "FFFFFF");
        assert_eq!(at(0.0, 0.0, 0.0), "000000");
        assert_eq!(at(R, 0.0, 0.0), "000000");
        assert_eq!(at(0.0, 0.0, 0.5), "808080");
    }

    #[test]
    fn halfway_out_is_half_saturated() {
        assert_eq!(at(R / 2.0, 0.0, 1.0), "FF8080");
    }

    #[test]
    fn a_point_outside_the_wheel_is_taken_at_its_edge() {
        let (hue, saturation) = hue_saturation_at(3.0 * R, 0.0, R);
        assert_eq!((hue, saturation), (0.0, 1.0));
        let (hue, saturation) = hue_saturation_at(-5.0 * R, -5.0 * R, R);
        assert!((hue - 135.0).abs() < 1e-9);
        assert_eq!(saturation, 1.0);
        let mut wheel = ColorWheel::new(None);
        wheel.pick_point(0.0, 10.0 * R, R);
        let (x, y) = wheel.marker(R);
        assert!(x.abs() < 1e-9 && (y - R).abs() < 1e-9);
    }

    #[test]
    fn hex_round_trips_through_the_wheel() {
        for code in [
            "FF0000", "00FF00", "0000FF", "FFFF00", "00FFFF", "FF00FF", "123456", "C00000",
            "2F5496", "7030A0", "FFFFFF", "000000", "808080", "A5A5A5", "FFC000",
        ] {
            let wheel = ColorWheel::new(Some(code));
            assert_eq!(wheel.hex(), code);
            // The marker's point picks the same hue and saturation back.
            let (dx, dy) = wheel.marker(R);
            let mut again = ColorWheel::new(None);
            again.pick_point(dx, dy, R);
            again.set_brightness(wheel.hsv.value);
            assert_eq!(again.hex(), code, "{code}");
        }
    }

    #[test]
    fn the_brightness_slider_runs_from_full_at_the_top_to_black() {
        assert_eq!(brightness_at(0.0, 100.0), 1.0);
        assert_eq!(brightness_at(100.0, 100.0), 0.0);
        assert_eq!(brightness_at(25.0, 100.0), 0.75);
        assert_eq!(brightness_at(-10.0, 100.0), 1.0);
        assert_eq!(brightness_at(150.0, 100.0), 0.0);
    }

    #[test]
    fn dimming_to_black_and_back_keeps_the_hue() {
        let mut wheel = ColorWheel::new(Some("2F5496"));
        let hue = wheel.hsv.hue;
        wheel.set_brightness(0.0);
        assert_eq!(wheel.hex(), "000000");
        wheel.set_brightness(1.0);
        assert!((wheel.hsv.hue - hue).abs() < 1e-9);
        assert_ne!(wheel.hex(), "000000");
    }

    #[test]
    fn picking_on_a_black_wheel_brings_the_brightness_up() {
        let mut wheel = ColorWheel::new(Some("000000"));
        wheel.pick_point(R, 0.0, R);
        assert_eq!(wheel.hex(), "FF0000");
    }

    #[test]
    fn typed_codes_move_the_wheel_and_bad_ones_do_not() {
        let mut wheel = ColorWheel::new(Some("FF0000"));
        assert!(wheel.set_hex("#0f0"));
        assert_eq!(wheel.hex(), "00FF00");
        assert!(!wheel.set_hex("12345"));
        assert!(!wheel.set_hex("GG0000"));
        assert_eq!(wheel.hex(), "00FF00");
        // A grey keeps the hue the wheel had.
        assert!(wheel.set_hex("808080"));
        assert!((wheel.hsv.hue - 120.0).abs() < 1e-9);
        assert_eq!(wheel.current, "FF0000");
    }

    #[test]
    fn the_painted_cells_fill_the_circle_only() {
        let cells = wheel_cells(R, 4.0, 1.0);
        let area = std::f64::consts::PI * R * R / 16.0;
        assert!((cells.len() as f64 - area).abs() / area < 0.05);
        assert!(cells
            .iter()
            .all(|cell| cell.x >= 0.0 && cell.y >= 0.0 && cell.x < 2.0 * R && cell.y < 2.0 * R));
        // The cell at the right edge's middle is red.
        let right = cells
            .iter()
            .find(|cell| cell.x == 2.0 * R - 4.0 && cell.y == R - 4.0)
            .unwrap();
        assert_eq!(right.rgb.0, 255);
        assert!(right.rgb.1 < 20 && right.rgb.2 < 20);
        assert!(wheel_cells(0.0, 4.0, 1.0).is_empty());
    }
}
