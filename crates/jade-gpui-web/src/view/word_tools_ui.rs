//! What the Word view keeps for its paragraph, list, style and design tools
//! — the open dialog, the rulers, the スタイル pane, a file the page's
//! picker is fetching for them — and the one place their commands
//! (`word_tools::ToolCommand`, sent by the ribbon as `Action::Word`) run:
//! the browser's copy of the native app's `word_tools` state.
//!
//! The tools themselves are in `word_dialog` (the dialog frame),
//! `word_ruler`, `word_paragraph` (tabs, borders, drop caps), `word_lists`,
//! `word_styles` and `word_design`.

use gpui::{Context, SharedString};
use jade_web::word::paragraph_options::TabAlignment;
use jade_web::word::ruler::RulerUnit;
use jade_web::word::FormatState;

use super::word_dialog::WordDialog;
use super::{DocView, Document};
use crate::word_tools::ToolCommand;

/// What a file the page's picker brings is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// テンプレートとアドイン → 添付…: the document is attached to it
    /// (`word_files`).
    Template,
    /// 箇条書き → 図: it becomes the bullet.
    PictureBullet,
    /// 図の形式 → 図の変更: it replaces the selected picture.
    ChangePicture,
}

/// The tools' state, kept with the document view.
pub struct WordTools {
    pub dialog: Option<WordDialog>,
    pub styles_pane: bool,
    pub ruler: bool,
    pub unit: RulerUnit,
    /// What a click on the ruler sets.
    pub tab_kind: TabAlignment,
    /// A marker being dragged along the ruler.
    pub ruler_drag: Option<super::word_ruler::RulerDrag>,
    /// Where the ruler was drawn, for turning a click into a position.
    pub ruler_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// Where the ruler's row was drawn, to line the strip up with the page.
    pub ruler_row: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// Why the last tool command did nothing, shown in the open dialog.
    pub status: Option<SharedString>,
    /// A file the page's picker was asked for.
    pub picking: Option<Pick>,
}

impl Default for WordTools {
    fn default() -> Self {
        WordTools {
            dialog: None,
            styles_pane: false,
            ruler: true,
            unit: super::word_ruler::load_unit(),
            tab_kind: TabAlignment::Left,
            ruler_drag: None,
            ruler_bounds: Default::default(),
            ruler_row: Default::default(),
            status: None,
            picking: None,
        }
    }
}

impl WordTools {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }
}

impl DocView {
    /// The formatting at the caret, as last read for the ribbon.
    pub(crate) fn word_format_state(&self) -> FormatState {
        match &self.document {
            Document::Pages {
                format: Some((_, state)),
                ..
            } => state.clone(),
            Document::Pages { document, .. } => {
                use jade_web::word::WordFormatting;
                document.format_state()
            }
            _ => FormatState::default(),
        }
    }

    /// The page (0-based) the caret is on.
    pub(crate) fn caret_page(&self) -> usize {
        let Document::Pages {
            document, current, ..
        } = &self.document
        else {
            return 0;
        };
        document
            .caret_rect()
            .map(|caret| document.page_and_offset(caret.document_y).0)
            .unwrap_or(*current)
    }

    /// After a tool changed the document: everything repainted, the
    /// gallery's styles read again.
    pub(crate) fn tool_done(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.tools.status = result.as_ref().err().map(|error| error.clone().into());
        if let Document::Pages {
            ribbon, document, ..
        } = &mut self.document
        {
            ribbon.read_styles(document);
        }
        self.refresh_repainted();
        self.word_result(result, cx);
    }

    /// Runs a tool's menu command.
    pub(crate) fn run_tool(&mut self, command: ToolCommand, cx: &mut Context<Self>) {
        if let Document::Pages { ribbon, .. } = &mut self.document {
            ribbon.popup = None;
        }
        match command {
            ToolCommand::OpenTabs => self.open_tabs_dialog(cx),
            ToolCommand::OpenBorders => self.open_borders_dialog(cx),
            ToolCommand::OpenDropCap => self.open_drop_cap_dialog(cx),
            ToolCommand::PictureBullet => self.prompt_for_picture_bullet(cx),
            ToolCommand::OpenListStart => self.open_list_start_dialog(cx),
            ToolCommand::OpenListIndent => self.open_list_indent_dialog(cx),
            ToolCommand::OpenDefineList => self.open_define_list_dialog(None, cx),
            ToolCommand::OpenListStyle => self.open_define_list_dialog(Some(String::new()), cx),
            ToolCommand::OpenNewStyle => self.open_style_dialog(None, cx),
            ToolCommand::OpenModifyStyle(id) => self.open_style_dialog(Some(id), cx),
            ToolCommand::ToggleStylesPane => {
                self.tools.styles_pane = !self.tools.styles_pane;
                cx.notify();
            }
            ToolCommand::AttachTemplate => self.open_templates_dialog(cx),
            ToolCommand::ToggleRuler => {
                self.tools.ruler = !self.tools.ruler;
                cx.notify();
            }
            ToolCommand::RulerUnit(unit) => {
                self.tools.unit = unit;
                super::word_ruler::save_unit(unit);
                cx.notify();
            }
            ToolCommand::NextTabKind => {
                self.tools.tab_kind = self.tools.tab_kind.next();
                cx.notify();
            }
            ToolCommand::InsertQuickPart(index) => self.insert_quick_part(index, cx),
            ToolCommand::OpenSaveQuickPart => self.open_quick_part_dialog(cx),
            ToolCommand::DeleteQuickPart(index) => self.delete_quick_part(index, cx),
            command => {
                let Some(document) = self.word_mut() else {
                    return;
                };
                let result = crate::word_tools::run(document, command);
                self.tool_done(result, cx);
            }
        }
    }

    /// For a paragraph whose style updates itself (自動的に更新する): the
    /// style takes the format just applied directly.
    pub(crate) fn auto_update_style(&mut self) {
        use jade_web::word::styles::WordStyles;
        let Some(document) = self.word_mut() else {
            return;
        };
        if let Ok(true) = document.auto_update_style() {
            if let Document::Pages {
                ribbon, document, ..
            } = &mut self.document
            {
                ribbon.read_styles(document);
            }
            self.refresh_repainted();
        }
    }

    /// Whether a file the page's picker brings is for the tools.
    pub(crate) fn picking_for_tools(&self) -> bool {
        self.tools.picking.is_some()
    }

    /// The file the page's picker brought for a tool.
    pub(crate) fn take_picked(&mut self, name: &str, bytes: &[u8], cx: &mut Context<Self>) {
        match self.tools.picking.take() {
            Some(Pick::Template) => self.attach_template_file(name, bytes, cx),
            Some(Pick::ChangePicture) => self.change_selected_picture(bytes.to_vec(), cx),
            Some(Pick::PictureBullet) => {
                let extension = name.rsplit_once('.').map_or("", |(_, extension)| extension);
                self.set_picture_bullet(bytes, extension, cx)
            }
            None => {}
        }
    }
}
