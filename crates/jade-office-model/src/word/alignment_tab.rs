//! ヘッダーとフッター › 位置合わせタブの挿入: an absolute position tab
//! (`w:ptab`) at the caret. The dialog's 配置 (左揃え / 中央揃え / 右揃え),
//! 基準 (余白 / インデント) and リーダー, as Word for Mac offers them; the tab
//! lines the text after it up there whatever the tab stops say, as a
//! header's centred title and right-hand page number are.

use jade_docx_headless_api::HeadlessCommand;

use super::WordSession;

/// 配置.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAlign {
    Left,
    Center,
    Right,
}

impl TabAlign {
    pub const ALL: [TabAlign; 3] = [TabAlign::Left, TabAlign::Center, TabAlign::Right];

    fn token(self) -> &'static str {
        match self {
            TabAlign::Left => "left",
            TabAlign::Center => "center",
            TabAlign::Right => "right",
        }
    }

    /// Its label's i18n key.
    pub fn label(self) -> &'static str {
        match self {
            TabAlign::Left => "word.ptab.left",
            TabAlign::Center => "word.ptab.center",
            TabAlign::Right => "word.ptab.right",
        }
    }
}

/// リーダー: the `w:leader` values with their samples (なし's an i18n key).
pub const LEADERS: [(Option<&str>, &str); 5] = [
    (None, "word.ptab.leader_none"),
    (Some("dot"), "......."),
    (Some("hyphen"), "-------"),
    (Some("underscore"), "_______"),
    (Some("middleDot"), "·······"),
];

/// The dialog while it is open.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AlignmentTabForm {
    pub alignment: TabAlign,
    /// 基準: インデント rather than 余白.
    pub relative_to_indent: bool,
    pub leader: Option<&'static str>,
}

impl Default for AlignmentTabForm {
    /// What Word's dialog opens on: 左揃え, 余白, なし.
    fn default() -> Self {
        Self {
            alignment: TabAlign::Left,
            relative_to_indent: false,
            leader: None,
        }
    }
}

impl AlignmentTabForm {
    /// The command OK runs at `(paragraph, offset)`.
    pub fn command(&self, paragraph: usize, offset: usize) -> HeadlessCommand {
        HeadlessCommand::InsertPositionalTab {
            paragraph,
            offset,
            alignment: self.alignment.token().to_string(),
            relative_to: Some(
                if self.relative_to_indent {
                    "indent"
                } else {
                    "margin"
                }
                .to_string(),
            ),
            leader: self.leader.map(str::to_string),
        }
    }

    /// OK: the tab at the caret — at the start of the selection, which it
    /// replaces as typing would — as one undo step; the caret after it.
    pub fn submit<S: WordSession + ?Sized>(&self, session: &mut S) -> Result<(), String> {
        let range = session.selection_range().ok_or("no caret")?;
        let (start, end) = range.normalized();
        let mut commands = Vec::new();
        if start != end {
            commands.push(HeadlessCommand::ReplaceRange {
                start_paragraph: start.paragraph,
                start_offset: start.offset,
                end_paragraph: end.paragraph,
                end_offset: end.offset,
                text: String::new(),
            });
        }
        commands.push(self.command(start.paragraph, start.offset));
        session.apply_commands(commands, false)
    }
}
