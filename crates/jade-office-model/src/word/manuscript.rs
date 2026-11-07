//! Word's 原稿用紙設定 (レイアウト › 原稿用紙設定): manuscript paper. スタイル
//! (なし, マス目付き原稿用紙, 下線付き原稿用紙, 外枠付き原稿用紙), 文字数 ×
//! 行数, 罫線の色, 用紙サイズ and 印刷の向き. Applying it lays every section's
//! text on a `snapToChars` grid — one character to a square — with the
//! margins Word gives manuscript paper, and draws the grid in the headers,
//! as Word does; なし takes the grid away and gives the text its line grid
//! back.

use jade_docx_headless_api::{
    HeadlessCommand, HeadlessDocGrid, HeadlessSectionLayoutPatch, HeadlessSectionScope,
};
pub use jade_docx_reader::document::{ManuscriptGrid, ManuscriptStyle};

use super::page::Paper;
use super::WordSession;

/// The margins Word gives manuscript paper: 72 pt at the top and bottom
/// (1440 twips) and 85 pt at the sides (1700 twips).
pub const MANUSCRIPT_MARGIN_TOP: f32 = 72.0;
pub const MANUSCRIPT_MARGIN_SIDE: f32 = 85.0;

/// The 文字数 × 行数 Word offers for a style.
pub fn manuscript_sizes(style: Option<ManuscriptStyle>) -> &'static [(u32, u32)] {
    match style {
        None => &[],
        Some(ManuscriptStyle::Squares) => &[(20, 20), (20, 10)],
        Some(ManuscriptStyle::Lines | ManuscriptStyle::Frame) => {
            &[(20, 20), (20, 10), (26, 20), (30, 20), (40, 20), (40, 40)]
        }
    }
}

/// The styles in the スタイル list, なし first, with their label keys.
pub const MANUSCRIPT_STYLES: [(Option<ManuscriptStyle>, &str); 4] = [
    (None, "manuscript.style.none"),
    (Some(ManuscriptStyle::Squares), "manuscript.style.squares"),
    (Some(ManuscriptStyle::Lines), "manuscript.style.lines"),
    (Some(ManuscriptStyle::Frame), "manuscript.style.frame"),
];

/// 罫線の色: the colors offered, with their label keys.
pub const MANUSCRIPT_COLORS: [(&str, &str); 6] = [
    ("#FF0000", "watermark.color.red"),
    ("#C00000", "watermark.color.dark_red"),
    ("#00B050", "watermark.color.green"),
    ("#0070C0", "watermark.color.blue"),
    ("#808080", "watermark.color.gray"),
    ("#000000", "watermark.color.black"),
];

/// The papers manuscript paper is printed on.
pub const MANUSCRIPT_PAPERS: [Paper; 5] =
    [Paper::A4, Paper::B5, Paper::A3, Paper::B4, Paper::Letter];

/// The 原稿用紙設定 dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct ManuscriptDialog {
    pub style: Option<ManuscriptStyle>,
    pub chars: u32,
    pub lines: u32,
    pub color: String,
    pub paper: Paper,
    pub landscape: bool,
    /// The section's text runs vertically (縦書き): squares run down the
    /// page and lines from the right.
    pub vertical: bool,
}

impl ManuscriptDialog {
    /// The dialog on a document's grid (or none), paper and direction.
    pub fn open(
        grid: Option<&ManuscriptGrid>,
        page: (f32, f32),
        landscape: bool,
        vertical: bool,
    ) -> Self {
        let (short, long) = if page.0 <= page.1 {
            (page.0, page.1)
        } else {
            (page.1, page.0)
        };
        let paper = Paper::ALL
            .into_iter()
            .find(|paper| {
                let (w, h) = paper.size();
                (w - short).abs() < 3.0 && (h - long).abs() < 3.0
            })
            .unwrap_or(Paper::A4);
        match grid {
            Some(grid) => Self {
                style: Some(grid.style),
                chars: grid.chars,
                lines: grid.lines,
                color: grid.color.clone(),
                paper,
                landscape,
                vertical,
            },
            None => Self {
                style: None,
                chars: 20,
                lines: 20,
                color: MANUSCRIPT_COLORS[0].0.to_string(),
                paper,
                landscape,
                vertical,
            },
        }
    }

    /// Picks a style; the size stays if the style offers it, else the
    /// style's first.
    pub fn set_style(&mut self, style: Option<ManuscriptStyle>) {
        self.style = style;
        let sizes = manuscript_sizes(style);
        if !sizes.contains(&(self.chars, self.lines)) {
            if let Some(&(chars, lines)) = sizes.first() {
                self.chars = chars;
                self.lines = lines;
            }
        }
    }

    /// The page size the dialog prints on, oriented.
    pub fn page_size(&self) -> (f32, f32) {
        let (w, h) = self.paper.size();
        if self.landscape {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// 字送り and 行送り: the body box shared out among the squares, rounded
    /// down — the line pitch to the twips Word keeps it in — so that the
    /// last square of a line and the last line still fit the body box, as
    /// 文字数 and 行数 promise.
    pub fn pitches(&self) -> (f32, f32) {
        let (w, h) = self.page_size();
        let body_w = w - 2.0 * MANUSCRIPT_MARGIN_SIDE;
        let body_h = h - 2.0 * MANUSCRIPT_MARGIN_TOP;
        let (chars, lines) = (self.chars.max(1) as f32, self.lines.max(1) as f32);
        let char_pitch = |value: f32| (value * 100.0).floor() / 100.0;
        let line_pitch = |value: f32| (value * 20.0).floor() / 20.0;
        if self.vertical {
            (char_pitch(body_h / chars), line_pitch(body_w / lines))
        } else {
            (char_pitch(body_w / chars), line_pitch(body_h / lines))
        }
    }

    /// The commands OK runs, as one undo step: the page, margins and grid
    /// of every section, then the drawing in the headers.
    pub fn commands(&self) -> Vec<HeadlessCommand> {
        let (width, height) = self.page_size();
        let Some(style) = self.style else {
            return vec![
                HeadlessCommand::SetManuscriptGrid {
                    style: None,
                    chars: self.chars,
                    lines: self.lines,
                    color: self.color.clone(),
                },
                HeadlessCommand::SetSectionLayout {
                    scope: HeadlessSectionScope::All,
                    patch: Box::new(HeadlessSectionLayoutPatch {
                        doc_grid: Some(HeadlessDocGrid {
                            grid_type: "lines".to_string(),
                            line_pitch: Some(18.0),
                            char_pitch: None,
                        }),
                        ..Default::default()
                    }),
                },
            ];
        };
        let (char_pitch, line_pitch) = self.pitches();
        vec![
            HeadlessCommand::SetSectionLayout {
                scope: HeadlessSectionScope::All,
                patch: Box::new(HeadlessSectionLayoutPatch {
                    width: Some(width),
                    height: Some(height),
                    landscape: Some(self.landscape),
                    margin_top: Some(MANUSCRIPT_MARGIN_TOP),
                    margin_bottom: Some(MANUSCRIPT_MARGIN_TOP),
                    margin_left: Some(MANUSCRIPT_MARGIN_SIDE),
                    margin_right: Some(MANUSCRIPT_MARGIN_SIDE),
                    doc_grid: Some(HeadlessDocGrid {
                        grid_type: "snapToChars".to_string(),
                        line_pitch: Some(line_pitch),
                        char_pitch: Some(char_pitch),
                    }),
                    ..Default::default()
                }),
            },
            HeadlessCommand::SetManuscriptGrid {
                style: Some(
                    match style {
                        ManuscriptStyle::Squares => "squares",
                        ManuscriptStyle::Lines => "lines",
                        ManuscriptStyle::Frame => "frame",
                    }
                    .to_string(),
                ),
                chars: self.chars,
                lines: self.lines,
                color: self.color.clone(),
            },
        ]
    }
}

/// 原稿用紙設定 on a Word session.
pub trait WordManuscript: WordSession {
    /// The document's manuscript grid, if it has one.
    fn manuscript_grid(&self) -> Option<ManuscriptGrid> {
        self.word_document()
            .document
            .as_deref()
            .and_then(|document| document.manuscript_grid.clone())
    }

    /// Opens 原稿用紙設定 on the document.
    fn manuscript_dialog(&self) -> ManuscriptDialog {
        let layout = self.word_document().section_layout(None);
        ManuscriptDialog::open(
            self.manuscript_grid().as_ref(),
            (layout.width, layout.height),
            layout.landscape,
            layout.text_direction.starts_with("tb"),
        )
    }

    /// OK, as one undo step. なし on a document without a grid changes
    /// nothing.
    fn apply_manuscript_dialog(&mut self, dialog: &ManuscriptDialog) -> Result<(), String> {
        if dialog.style.is_none() && self.manuscript_grid().is_none() {
            return Ok(());
        }
        self.apply_commands(dialog.commands(), true)
    }
}

impl<T: WordSession + ?Sized> WordManuscript for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_twenty_by_twenty_shares_the_body_out() {
        let mut dialog = ManuscriptDialog::open(None, (595.3, 841.9), false, false);
        assert_eq!(dialog.style, None);
        dialog.set_style(Some(ManuscriptStyle::Squares));
        assert_eq!((dialog.chars, dialog.lines), (20, 20));
        let (char_pitch, line_pitch) = dialog.pitches();
        assert!((char_pitch - 21.26).abs() < 0.001, "{char_pitch}");
        assert!((line_pitch - 34.85).abs() < 0.001, "{line_pitch}");
        // Twenty of each fit the 425.3pt × 697.9pt body box.
        assert!(20.0 * char_pitch <= 425.3 && 20.0 * line_pitch <= 697.9);
        let commands = dialog.commands();
        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[1],
            HeadlessCommand::SetManuscriptGrid { style: Some(style), chars: 20, lines: 20, .. }
                if style == "squares"
        ));
    }

    #[test]
    fn a_style_without_the_size_takes_its_first() {
        let mut dialog = ManuscriptDialog::open(None, (595.3, 841.9), false, true);
        dialog.set_style(Some(ManuscriptStyle::Lines));
        dialog.chars = 40;
        dialog.lines = 40;
        dialog.set_style(Some(ManuscriptStyle::Squares));
        assert_eq!((dialog.chars, dialog.lines), (20, 20));
        // Vertical: squares down the page.
        let (char_pitch, line_pitch) = dialog.pitches();
        assert!((char_pitch - 34.89).abs() < 0.001 && (line_pitch - 21.25).abs() < 0.001);
    }
}
