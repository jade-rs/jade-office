//! 枠 (Word's legacy text frame, `w:framePr`): 開発 › 旧形式のツール › 枠
//! puts the selected paragraphs in a frame with a box round it, as Word
//! does, and 枠の書式設定 sets it — 文字列の折り返し (なし / 周囲), サイズ
//! (幅 自動 / 固定, 高さ 自動 / 最小値 / 固定値), 水平方向 and 垂直方向
//! (位置 by alignment or distance, 基準, 文字列との間隔), 文字列と一緒に
//! 移動する — or takes it away (枠の削除). Lengths are millimetres here, as
//! the dialog shows them.

use jade_docx_editor::commands::set_paragraph_frame::frame_to_headless;
use jade_docx_headless_api::{HeadlessBorderEdge, HeadlessCommand, HeadlessParagraphFrame};

use super::WordSession;

const MM_PER_PT: f32 = 25.4 / 72.0;

fn mm(points: f32) -> f32 {
    (points * MM_PER_PT * 10.0).round() / 10.0
}

fn pt(millimetres: f32) -> f32 {
    millimetres / MM_PER_PT
}

/// 高さ's rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameHeight {
    /// 自動.
    Auto,
    /// 最小値.
    AtLeast,
    /// 固定値.
    Exact,
}

impl FrameHeight {
    pub const ALL: [(FrameHeight, &'static str); 3] = [
        (FrameHeight::Auto, "frame.auto"),
        (FrameHeight::AtLeast, "frame.at_least"),
        (FrameHeight::Exact, "frame.exact"),
    ];
}

/// 位置: one of the alignments, or a distance from the base.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FramePosition {
    Align(&'static str),
    Offset(f32),
}

/// 水平方向's 位置 list.
pub const HORIZONTAL_ALIGNS: [(&str, &str); 5] = [
    ("left", "frame.left"),
    ("center", "frame.center"),
    ("right", "frame.right"),
    ("inside", "frame.inside"),
    ("outside", "frame.outside"),
];

/// 垂直方向's 位置 list.
pub const VERTICAL_ALIGNS: [(&str, &str); 5] = [
    ("top", "frame.top"),
    ("center", "frame.center"),
    ("bottom", "frame.bottom"),
    ("inside", "frame.inside"),
    ("outside", "frame.outside"),
];

/// 水平方向's 基準: 余白, ページ, 段.
pub const HORIZONTAL_ANCHORS: [(&str, &str); 3] = [
    ("margin", "frame.margin"),
    ("page", "frame.page"),
    ("text", "frame.column"),
];

/// 垂直方向's 基準: 余白, ページ, 段落.
pub const VERTICAL_ANCHORS: [(&str, &str); 3] = [
    ("margin", "frame.margin"),
    ("page", "frame.page"),
    ("text", "frame.paragraph"),
];

/// What 枠の書式設定 shows and sets.
#[derive(Clone, Debug, PartialEq)]
pub struct FrameForm {
    /// 文字列の折り返し: 周囲 (`true`) or なし.
    pub wrap_around: bool,
    /// 幅: `None` is 自動, else 固定 in mm.
    pub width: Option<f32>,
    pub height_rule: FrameHeight,
    /// 高さ, in mm (read unless 自動).
    pub height: f32,
    pub horizontal: FramePosition,
    pub h_anchor: &'static str,
    /// 文字列との間隔 left and right, mm.
    pub h_space: f32,
    pub vertical: FramePosition,
    pub v_anchor: &'static str,
    /// 文字列との間隔 above and below, mm.
    pub v_space: f32,
}

impl Default for FrameForm {
    /// The frame 枠 inserts: around the text, as wide as it needs, where the
    /// text is.
    fn default() -> Self {
        Self {
            wrap_around: true,
            width: None,
            height_rule: FrameHeight::Auto,
            height: 0.0,
            horizontal: FramePosition::Offset(0.0),
            h_anchor: "text",
            h_space: 0.0,
            vertical: FramePosition::Offset(0.0),
            v_anchor: "text",
            v_space: 0.0,
        }
    }
}

fn token(list: &[(&'static str, &'static str)], value: Option<&str>, default: &'static str) -> &'static str {
    value
        .and_then(|value| list.iter().find(|(token, _)| *token == value))
        .map_or(default, |(token, _)| token)
}

impl FrameForm {
    /// The form on an existing frame.
    pub fn from_headless(frame: &HeadlessParagraphFrame) -> Self {
        let position = |align: Option<&str>, offset: Option<f32>, list: &[(&'static str, &'static str)]| {
            match align.and_then(|align| list.iter().find(|(token, _)| *token == align)) {
                Some((token, _)) => FramePosition::Align(token),
                None => FramePosition::Offset(mm(offset.unwrap_or(0.0))),
            }
        };
        Self {
            wrap_around: frame.wrap.as_deref() != Some("none"),
            width: frame.width.map(mm),
            height_rule: match frame.height_rule.as_deref() {
                Some("atLeast") => FrameHeight::AtLeast,
                Some("exact") => FrameHeight::Exact,
                _ => FrameHeight::Auto,
            },
            height: frame.height.map_or(0.0, mm),
            horizontal: position(frame.x_align.as_deref(), frame.x, &HORIZONTAL_ALIGNS),
            h_anchor: token(&HORIZONTAL_ANCHORS, frame.h_anchor.as_deref(), "text"),
            h_space: frame.h_space.map_or(0.0, mm),
            vertical: position(frame.y_align.as_deref(), frame.y, &VERTICAL_ALIGNS),
            v_anchor: token(&VERTICAL_ANCHORS, frame.v_anchor.as_deref(), "text"),
            v_space: frame.v_space.map_or(0.0, mm),
        }
    }

    /// The engine's terms.
    pub fn to_headless(&self) -> HeadlessParagraphFrame {
        let (x, x_align) = match self.horizontal {
            FramePosition::Align(align) => (None, Some(align.to_string())),
            FramePosition::Offset(offset) => (Some(pt(offset)), None),
        };
        let (y, y_align) = match self.vertical {
            FramePosition::Align(align) => (None, Some(align.to_string())),
            FramePosition::Offset(offset) => (Some(pt(offset)), None),
        };
        let space = |value: f32| (value > 0.0).then(|| pt(value));
        HeadlessParagraphFrame {
            width: self.width.map(pt),
            height: (self.height_rule != FrameHeight::Auto).then(|| pt(self.height.max(0.0))),
            height_rule: match self.height_rule {
                FrameHeight::Auto => None,
                FrameHeight::AtLeast => Some("atLeast".to_string()),
                FrameHeight::Exact => Some("exact".to_string()),
            },
            x,
            x_align,
            h_anchor: Some(self.h_anchor.to_string()),
            y,
            y_align,
            v_anchor: Some(self.v_anchor.to_string()),
            wrap: Some(if self.wrap_around { "around" } else { "none" }.to_string()),
            h_space: space(self.h_space),
            v_space: space(self.v_space),
        }
    }

    /// 文字列と一緒に移動する: the vertical base is the paragraph.
    pub fn moves_with_text(&self) -> bool {
        self.v_anchor == "text"
    }

    pub fn toggle_moves_with_text(&mut self) {
        self.v_anchor = if self.moves_with_text() { "margin" } else { "text" };
    }

    /// A length stepped by `delta` mm, never below 0.
    pub fn step(value: f32, delta: f32) -> f32 {
        ((value + delta) * 10.0).round().max(0.0) / 10.0
    }
}

/// 枠 over every session.
pub trait WordFrames: WordSession {
    /// The caret paragraph's frame, in the engine's terms.
    fn caret_frame(&self) -> Option<HeadlessParagraphFrame> {
        let paragraph = self.selection_range()?.focus.paragraph;
        self.word_document()
            .paragraph(paragraph)?
            .properties
            .as_ref()?
            .frame
            .as_ref()
            .map(frame_to_headless)
    }

    /// The paragraphs the caret's frame holds: its neighbours with the same
    /// frame. Without a frame, the selected paragraphs.
    fn frame_paragraphs(&self) -> Option<(usize, usize)> {
        let range = self.selection_range()?;
        let (start, end) = range.normalized();
        let document = self.word_document();
        let frame_of = |index: usize| {
            document
                .paragraph(index)
                .and_then(|paragraph| paragraph.properties.as_ref())
                .and_then(|props| props.frame)
        };
        let Some(frame) = frame_of(start.paragraph) else {
            return Some((start.paragraph, end.paragraph));
        };
        let mut first = start.paragraph;
        while first > 0
            && document.is_top_level_body_paragraph_surface(first - 1)
            && frame_of(first - 1) == Some(frame)
        {
            first -= 1;
        }
        let mut last = end.paragraph.max(start.paragraph);
        while last + 1 < document.paragraph_count()
            && document.is_top_level_body_paragraph_surface(last + 1)
            && frame_of(last + 1) == Some(frame)
        {
            last += 1;
        }
        Some((first, last))
    }

    /// 枠: the selected paragraphs in a frame with a box round them, as one
    /// undo step.
    fn insert_frame(&mut self) -> Result<(), String> {
        let (start, end) = self.frame_paragraphs().ok_or("no caret")?;
        let edge = || {
            Some(HeadlessBorderEdge {
                style: Some("single".to_string()),
                width: Some(0.5),
                color: Some("#000000".to_string()),
            })
        };
        self.apply_commands(
            vec![
                HeadlessCommand::SetParagraphFrame {
                    start_paragraph: start,
                    end_paragraph: end,
                    frame: Some(FrameForm::default().to_headless()),
                },
                HeadlessCommand::SetParagraphBorders {
                    start_paragraph: start,
                    end_paragraph: end,
                    top: edge(),
                    bottom: edge(),
                    left: edge(),
                    right: edge(),
                },
            ],
            true,
        )
    }

    /// 枠の書式設定's OK.
    fn apply_frame(&mut self, form: &FrameForm) -> Result<(), String> {
        let (start, end) = self.frame_paragraphs().ok_or("no caret")?;
        self.apply_command(
            HeadlessCommand::SetParagraphFrame {
                start_paragraph: start,
                end_paragraph: end,
                frame: Some(form.to_headless()),
            },
            true,
        )
    }

    /// 枠の削除: the paragraphs back in the text; their borders stay, as
    /// Word leaves them.
    fn remove_frame(&mut self) -> Result<(), String> {
        let (start, end) = self.frame_paragraphs().ok_or("no caret")?;
        self.apply_command(
            HeadlessCommand::SetParagraphFrame {
                start_paragraph: start,
                end_paragraph: end,
                frame: None,
            },
            true,
        )
    }
}

impl<T: WordSession + ?Sized> WordFrames for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_form_reads_back_what_it_writes() {
        let form = FrameForm {
            wrap_around: false,
            width: Some(50.8),
            height_rule: FrameHeight::Exact,
            height: 25.4,
            horizontal: FramePosition::Align("center"),
            h_anchor: "margin",
            h_space: 3.2,
            vertical: FramePosition::Offset(12.7),
            v_anchor: "page",
            v_space: 0.0,
        };
        let spec = form.to_headless();
        assert_eq!(spec.wrap.as_deref(), Some("none"));
        assert_eq!(spec.x_align.as_deref(), Some("center"));
        assert!((spec.width.unwrap() - 144.0).abs() < 0.01);
        assert_eq!(FrameForm::from_headless(&spec), form);
        assert!(FrameForm::default().moves_with_text());
        assert_eq!(FrameForm::step(0.5, -1.0), 0.0);
    }
}
