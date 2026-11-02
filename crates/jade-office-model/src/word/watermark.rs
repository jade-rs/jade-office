//! Word's 透かし (デザイン › ページの背景 › 透かし): the gallery of ready-made
//! text watermarks — 社外秘, 複製を禁ず, 至急, 下書き and サンプル, each
//! 対角線上 or 水平 — 透かしの削除, and the ユーザー設定の透かし dialog
//! (なし / テキスト: 言語, テキスト, フォント, サイズ, 色, 半透明にする,
//! レイアウト). Word keeps a watermark as a VML text path in every header;
//! the engine writes and reads it (`set_watermark`), and this is what the
//! ribbon and the dialog do with it, in both shells.

use jade_docx_headless_api::HeadlessCommand;
pub use jade_docx_reader::document::WatermarkStyle;

use super::WordSession;

/// A ready-made watermark of the gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WatermarkPreset {
    pub text: &'static str,
    /// 水平 (`… 2`) rather than 対角線上 (`… 1`).
    pub horizontal: bool,
}

/// The gallery, as Word offers it: each text 対角線上 then 水平.
pub const WATERMARK_PRESETS: [WatermarkPreset; 10] = [
    WatermarkPreset {
        text: "社外秘",
        horizontal: false,
    },
    WatermarkPreset {
        text: "社外秘",
        horizontal: true,
    },
    WatermarkPreset {
        text: "複製を禁ず",
        horizontal: false,
    },
    WatermarkPreset {
        text: "複製を禁ず",
        horizontal: true,
    },
    WatermarkPreset {
        text: "至急",
        horizontal: false,
    },
    WatermarkPreset {
        text: "至急",
        horizontal: true,
    },
    WatermarkPreset {
        text: "下書き",
        horizontal: false,
    },
    WatermarkPreset {
        text: "下書き",
        horizontal: true,
    },
    WatermarkPreset {
        text: "サンプル",
        horizontal: false,
    },
    WatermarkPreset {
        text: "サンプル",
        horizontal: true,
    },
];

impl WatermarkPreset {
    /// The gallery's caption: `社外秘 1` (対角線上) or `社外秘 2` (水平).
    pub fn caption(&self) -> String {
        format!("{} {}", self.text, if self.horizontal { 2 } else { 1 })
    }
}

/// The dialog's 言語, which chooses the texts it lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatermarkLanguage {
    Japanese,
    English,
}

impl WatermarkLanguage {
    pub const ALL: [WatermarkLanguage; 2] =
        [WatermarkLanguage::Japanese, WatermarkLanguage::English];

    pub fn label_key(self) -> &'static str {
        match self {
            WatermarkLanguage::Japanese => "watermark.language.japanese",
            WatermarkLanguage::English => "watermark.language.english",
        }
    }

    /// The テキスト list in this language.
    pub fn texts(self) -> &'static [&'static str] {
        match self {
            WatermarkLanguage::Japanese => &[
                "社外秘",
                "複製を禁ず",
                "至急",
                "下書き",
                "サンプル",
                "回覧",
                "見本",
                "部外秘",
                "取扱注意",
                "コピー",
                "原本",
                "緊急",
            ],
            WatermarkLanguage::English => &[
                "ASAP",
                "CONFIDENTIAL",
                "COPY",
                "DO NOT COPY",
                "DRAFT",
                "ORIGINAL",
                "PERSONAL",
                "SAMPLE",
                "TOP SECRET",
                "URGENT",
            ],
        }
    }

    /// The フォント the dialog starts with in this language.
    pub fn default_font(self) -> &'static str {
        match self {
            WatermarkLanguage::Japanese => "游明朝",
            WatermarkLanguage::English => "Calibri",
        }
    }
}

/// The フォント list.
pub const WATERMARK_FONTS: [&str; 8] = [
    "游明朝",
    "游ゴシック",
    "ＭＳ 明朝",
    "ＭＳ ゴシック",
    "メイリオ",
    "Calibri",
    "Arial",
    "Times New Roman",
];

/// The サイズ list, in points; 自動 is `None` before them.
pub const WATERMARK_SIZES: [u32; 14] = [36, 40, 44, 48, 54, 60, 66, 72, 80, 90, 96, 105, 120, 144];

/// The 色 swatches (`#RRGGBB`) and their names' keys; silver is Word's.
pub const WATERMARK_COLORS: [(&str, &str); 10] = [
    ("#C0C0C0", "watermark.color.silver"),
    ("#808080", "watermark.color.gray"),
    ("#000000", "watermark.color.black"),
    ("#C00000", "watermark.color.dark_red"),
    ("#FF0000", "watermark.color.red"),
    ("#FFC000", "watermark.color.orange"),
    ("#00B050", "watermark.color.green"),
    ("#0070C0", "watermark.color.blue"),
    ("#002060", "watermark.color.navy"),
    ("#7030A0", "watermark.color.purple"),
];

/// The ユーザー設定の透かし dialog.
#[derive(Clone, Debug, PartialEq)]
pub struct WatermarkDialog {
    /// テキスト (`true`) or なし.
    pub text_on: bool,
    pub language: WatermarkLanguage,
    pub text: String,
    pub font: String,
    /// サイズ in points; `None` is 自動.
    pub size: Option<f32>,
    /// 色 as `#RRGGBB`.
    pub color: String,
    pub semitransparent: bool,
    /// レイアウト 水平; otherwise 対角線上.
    pub horizontal: bool,
}

impl WatermarkDialog {
    /// The dialog on the document's watermark, or on Word's defaults (なし,
    /// 社外秘 ready to choose) when it has none.
    pub fn open(watermark: Option<(&str, &WatermarkStyle)>) -> Self {
        match watermark {
            Some((text, style)) => {
                let language = if text.is_ascii() {
                    WatermarkLanguage::English
                } else {
                    WatermarkLanguage::Japanese
                };
                WatermarkDialog {
                    text_on: true,
                    language,
                    text: text.to_string(),
                    font: style
                        .font
                        .clone()
                        .unwrap_or_else(|| language.default_font().to_string()),
                    size: style.size_pt,
                    color: style.color.to_ascii_uppercase(),
                    semitransparent: style.semitransparent,
                    horizontal: style.horizontal,
                }
            }
            None => WatermarkDialog {
                text_on: false,
                language: WatermarkLanguage::Japanese,
                text: WatermarkLanguage::Japanese.texts()[0].to_string(),
                font: WatermarkLanguage::Japanese.default_font().to_string(),
                size: None,
                color: WATERMARK_COLORS[0].0.to_string(),
                semitransparent: true,
                horizontal: false,
            },
        }
    }

    /// Chooses 言語: the text becomes that language's first unless it is
    /// already one of its texts, and the font follows when it was the other
    /// language's default.
    pub fn set_language(&mut self, language: WatermarkLanguage) {
        if self.language == language {
            return;
        }
        if !language.texts().contains(&self.text.as_str()) {
            self.text = language.texts()[0].to_string();
        }
        if self.font == self.language.default_font() {
            self.font = language.default_font().to_string();
        }
        self.language = language;
    }

    /// サイズ as the field shows it: 自動 or the number.
    pub fn size_label(&self) -> Option<String> {
        self.size.map(|size| {
            let text = format!("{size:.1}");
            text.trim_end_matches('0').trim_end_matches('.').to_string()
        })
    }

    /// サイズ typed: blank or 自動 / auto is 自動; otherwise a size in points.
    pub fn set_size_text(&mut self, text: &str) -> Result<(), &'static str> {
        let text = text.trim();
        if text.is_empty() || text == "自動" || text.eq_ignore_ascii_case("auto") {
            self.size = None;
            return Ok(());
        }
        let size = text
            .trim_end_matches("pt")
            .trim()
            .parse::<f32>()
            .ok()
            .filter(|size| size.is_finite() && (1.0..=1638.0).contains(size))
            .ok_or("watermark.bad_size")?;
        self.size = Some(size);
        Ok(())
    }

    /// What OK applies, or why it cannot.
    pub fn command(&self) -> Result<HeadlessCommand, &'static str> {
        if !self.text_on {
            return Ok(remove_command());
        }
        let text = self.text.trim();
        if text.is_empty() {
            return Err("watermark.no_text");
        }
        if self.font.trim().is_empty() {
            return Err("watermark.no_font");
        }
        Ok(HeadlessCommand::SetWatermark {
            clear: false,
            text: Some(text.to_string()),
            font: Some(self.font.trim().to_string()),
            size: self.size,
            color: Some(self.color.clone()),
            semitransparent: Some(self.semitransparent),
            layout: Some(
                if self.horizontal {
                    "horizontal"
                } else {
                    "diagonal"
                }
                .to_string(),
            ),
        })
    }
}

fn remove_command() -> HeadlessCommand {
    HeadlessCommand::SetWatermark {
        clear: true,
        text: None,
        font: None,
        size: None,
        color: None,
        semitransparent: None,
        layout: None,
    }
}

/// The document's watermark, for any Word session.
pub trait WordWatermark: WordSession {
    /// The watermark's text and look, when the document has one.
    fn watermark(&self) -> Option<(String, WatermarkStyle)> {
        let document = self.word_document().document.as_deref()?;
        let text = document.watermark_text.clone()?;
        Some((text, document.watermark_style.clone().unwrap_or_default()))
    }

    /// The gallery entry the document's watermark is, if any.
    fn watermark_preset(&self) -> Option<usize> {
        let (text, style) = self.watermark()?;
        WATERMARK_PRESETS
            .iter()
            .position(|preset| preset.text == text && preset.horizontal == style.horizontal)
    }

    /// A gallery entry, as one undo step: Word's look (silver,
    /// semitransparent, 自動) in its layout.
    fn insert_watermark_preset(&mut self, index: usize) -> Result<(), String> {
        let preset = WATERMARK_PRESETS
            .get(index)
            .ok_or_else(|| format!("no watermark preset {index}"))?;
        let mut dialog = WatermarkDialog::open(None);
        dialog.text_on = true;
        dialog.text = preset.text.to_string();
        dialog.horizontal = preset.horizontal;
        self.apply_watermark_dialog(&dialog)
    }

    /// 透かしの削除.
    fn remove_watermark(&mut self) -> Result<(), String> {
        self.apply_command(remove_command(), true)
    }

    /// Opens ユーザー設定の透かし on the document's watermark.
    fn watermark_dialog(&self) -> WatermarkDialog {
        let watermark = self.watermark();
        WatermarkDialog::open(
            watermark
                .as_ref()
                .map(|(text, style)| (text.as_str(), style)),
        )
    }

    /// The dialog's OK, as one undo step. A value it cannot take comes back
    /// as its message key, and nothing changes.
    fn apply_watermark_dialog(&mut self, dialog: &WatermarkDialog) -> Result<(), String> {
        let command = dialog.command().map_err(str::to_string)?;
        self.apply_command(command, true)
    }
}

impl<T: WordSession + ?Sized> WordWatermark for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dialog_starts_on_words_defaults() {
        let dialog = WatermarkDialog::open(None);
        assert!(!dialog.text_on);
        assert_eq!(dialog.text, "社外秘");
        assert_eq!(dialog.color, "#C0C0C0");
        assert!(dialog.semitransparent && !dialog.horizontal && dialog.size.is_none());
        assert!(matches!(
            dialog.command(),
            Ok(HeadlessCommand::SetWatermark { clear: true, .. })
        ));
    }

    #[test]
    fn the_dialog_reads_a_watermark_back() {
        let style = WatermarkStyle {
            font: Some("Arial".into()),
            size_pt: Some(72.0),
            color: "#FF0000".into(),
            semitransparent: false,
            horizontal: true,
            width_pt: Some(200.0),
            height_pt: Some(72.0),
        };
        let dialog = WatermarkDialog::open(Some(("DRAFT", &style)));
        assert!(dialog.text_on);
        assert_eq!(dialog.language, WatermarkLanguage::English);
        assert_eq!(dialog.font, "Arial");
        assert_eq!(dialog.size_label().as_deref(), Some("72"));
        assert!(!dialog.semitransparent && dialog.horizontal);
        let Ok(HeadlessCommand::SetWatermark {
            text,
            size,
            layout,
            semitransparent,
            ..
        }) = dialog.command()
        else {
            panic!("a set command");
        };
        assert_eq!(text.as_deref(), Some("DRAFT"));
        assert_eq!(size, Some(72.0));
        assert_eq!(layout.as_deref(), Some("horizontal"));
        assert_eq!(semitransparent, Some(false));
    }

    #[test]
    fn values_it_cannot_take_are_refused() {
        let mut dialog = WatermarkDialog::open(None);
        dialog.text_on = true;
        dialog.text = "  ".into();
        assert_eq!(dialog.command().err(), Some("watermark.no_text"));
        assert_eq!(dialog.set_size_text("huge"), Err("watermark.bad_size"));
        assert_eq!(dialog.set_size_text("0"), Err("watermark.bad_size"));
        dialog.set_size_text("80").unwrap();
        assert_eq!(dialog.size, Some(80.0));
        dialog.set_size_text("自動").unwrap();
        assert_eq!(dialog.size, None);
    }

    #[test]
    fn the_language_changes_the_texts_and_the_font() {
        let mut dialog = WatermarkDialog::open(None);
        dialog.set_language(WatermarkLanguage::English);
        assert_eq!(dialog.text, "ASAP");
        assert_eq!(dialog.font, "Calibri");
        dialog.font = "Arial".into();
        dialog.set_language(WatermarkLanguage::Japanese);
        assert_eq!(dialog.text, "社外秘");
        assert_eq!(dialog.font, "Arial", "a chosen font stays");
    }
}
