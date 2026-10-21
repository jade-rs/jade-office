//! What the Word view keeps for its paragraph, list, style and design tools
//! — the open dialog, the ruler, the スタイル pane — and the one command
//! enum the ribbon's menus send them through, so the ribbon and the view
//! each need only one hook (`Action::Word`, `Popup::Word`).
//!
//! The tools themselves are in `word_dialog` (the dialog kit), `word_ruler`
//! (the rulers), `word_paragraph` (tabs, borders, drop caps, the 段落
//! switches), `word_lists`, `word_styles` and `word_design`.

use gpui::{Context, SharedString};
use jade_office_model::word::design::WordDesign;
use jade_office_model::word::lists::{MultilevelPreset, WordLists, BULLETS, NUMBER_FORMATS};
use jade_office_model::word::paragraph_options::{
    DropCapPosition, ParagraphSwitch, TabAlignment, WordParagraphOptions,
};
use jade_office_model::word::ruler::RulerUnit;
use jade_office_model::word::styles::WordStyles;

use crate::word_dialog::WordDialog;
use crate::DocumentView;

/// The lists the tools drop under their ribbon buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolMenu {
    Bullets,
    Numbering,
    Multilevel,
    /// The 段落 group's settings: the three switches, タブ, 罫線と網かけ.
    ParagraphSettings,
    DropCap,
    Themes,
    Colors,
    Fonts,
    Effects,
    StyleSets,
    Spacing,
    RulerUnits,
    QuickParts,
    /// A style gallery card's menu (変更, 選択個所と一致するように更新).
    StyleCard(usize),
    /// A row of the スタイル pane's menu.
    PaneStyle(usize),
}

/// What a tool's menu entry does.
#[derive(Clone, Debug, PartialEq)]
pub enum ToolCommand {
    Switch(ParagraphSwitch, bool),
    OpenTabs,
    OpenBorders,
    OpenDropCap,
    DropCap(Option<(DropCapPosition, u8)>),
    /// 段落番号 library entry, by index into `NUMBER_FORMATS`.
    NumberFormat(usize),
    /// 箇条書き library entry, by index into `BULLETS`.
    Bullet(usize),
    Multilevel(MultilevelPreset),
    ListLevel(u8),
    RestartList,
    ContinueList,
    OpenListStart,
    OpenListIndent,
    OpenDefineList,
    OpenListStyle,
    /// A list style, by its list's `w:numId`.
    ApplyListStyle(String),
    ApplyStyle(String),
    OpenNewStyle,
    OpenModifyStyle(String),
    UpdateStyle(String),
    ToggleStylesPane,
    Theme(usize),
    Colors(usize),
    Fonts(usize),
    Effects(usize),
    StyleSet(usize),
    Spacing(usize),
    SetAsDefault,
    AttachTemplate,
    ToggleRuler,
    RulerUnit(RulerUnit),
    /// The ruler's tab selector: the next alignment.
    NextTabKind,
    InsertQuickPart(usize),
    OpenSaveQuickPart,
    DeleteQuickPart(usize),
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
    pub ruler_drag: Option<crate::word_ruler::RulerDrag>,
    /// Where the ruler was drawn, for turning a click into a position.
    pub ruler_bounds: std::rc::Rc<std::cell::Cell<Option<gpui::Bounds<gpui::Pixels>>>>,
    /// Why the last tool command did nothing, shown in the status bar.
    pub status: Option<SharedString>,
}

impl Default for WordTools {
    fn default() -> Self {
        WordTools {
            dialog: None,
            styles_pane: false,
            ruler: true,
            unit: crate::word_ruler::load_unit(),
            tab_kind: TabAlignment::Left,
            ruler_drag: None,
            ruler_bounds: Default::default(),
            status: None,
        }
    }
}

impl DocumentView {
    /// The caret as the document's selection, for a tool that acts on it.
    pub(crate) fn tools_caret(&mut self) {
        if self.document.selection().is_none() {
            let (paragraph, offset) = self.insert_at;
            self.document.place_caret(paragraph, offset);
        }
    }

    /// After a tool changed the document: everything repainted, the caret
    /// where it was.
    pub(crate) fn tool_done(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.dirty = true;
                self.tools.status = None;
                self.ribbon.read_styles(&self.document);
                if self.document.selection().is_none() {
                    self.caret = self
                        .document
                        .place_caret(self.insert_at.0, self.insert_at.1);
                }
                self.rebuild_all_images();
            }
            Err(error) => self.tools.status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// Runs a tool's menu command.
    pub(crate) fn run_tool(&mut self, command: ToolCommand, cx: &mut Context<Self>) {
        self.ribbon.popup = None;
        self.tools_caret();
        let document = &mut self.document;
        let result =
            match command {
                ToolCommand::Switch(switch, on) => document.set_paragraph_switch(switch, on),
                ToolCommand::OpenTabs => return self.open_tabs_dialog(cx),
                ToolCommand::OpenBorders => return self.open_borders_dialog(cx),
                ToolCommand::OpenDropCap => return self.open_drop_cap_dialog(cx),
                ToolCommand::DropCap(drop_cap) => document.set_drop_cap(drop_cap),
                ToolCommand::NumberFormat(index) => {
                    let format = NUMBER_FORMATS[index];
                    document.apply_list_format(format.format, format.text, None)
                }
                ToolCommand::Bullet(index) => {
                    let (glyph, font) = BULLETS[index];
                    document.apply_list_format("bullet", glyph, font)
                }
                ToolCommand::Multilevel(preset) => document.apply_multilevel_preset(preset),
                ToolCommand::ListLevel(level) => document.set_list_level(level),
                ToolCommand::RestartList => document.set_list_start(1),
                ToolCommand::ContinueList => document.continue_list(),
                ToolCommand::OpenListStart => return self.open_list_start_dialog(cx),
                ToolCommand::OpenListIndent => return self.open_list_indent_dialog(cx),
                ToolCommand::OpenDefineList => return self.open_define_list_dialog(None, cx),
                ToolCommand::OpenListStyle => {
                    return self.open_define_list_dialog(Some(String::new()), cx)
                }
                ToolCommand::ApplyListStyle(num_id) => document.apply_list_style(&num_id),
                ToolCommand::ApplyStyle(id) => document.apply_style(&id),
                ToolCommand::OpenNewStyle => return self.open_style_dialog(None, cx),
                ToolCommand::OpenModifyStyle(id) => return self.open_style_dialog(Some(id), cx),
                ToolCommand::UpdateStyle(id) => document.update_style_to_match(&id),
                ToolCommand::ToggleStylesPane => {
                    self.tools.styles_pane = !self.tools.styles_pane;
                    return cx.notify();
                }
                ToolCommand::Theme(index) => {
                    document.apply_theme(&jade_office_model::word::design::THEMES[index])
                }
                ToolCommand::Colors(index) => document
                    .apply_color_scheme(&jade_office_model::word::design::COLOR_SCHEMES[index]),
                ToolCommand::Fonts(index) => document
                    .apply_font_scheme(&jade_office_model::word::design::FONT_SCHEMES[index]),
                ToolCommand::Effects(index) => document
                    .apply_effect_scheme(&jade_office_model::word::design::EFFECT_SCHEMES[index]),
                ToolCommand::StyleSet(index) => {
                    let sets = jade_office_model::word::design::style_sets();
                    document.apply_style_set(&sets[index])
                }
                ToolCommand::Spacing(index) => {
                    let (_, _, before, after, line) =
                        jade_office_model::word::design::PARAGRAPH_SPACINGS[index];
                    document.apply_paragraph_spacing(before, after, line)
                }
                ToolCommand::SetAsDefault => document.set_as_default(),
                ToolCommand::AttachTemplate => return self.prompt_for_template(cx),
                ToolCommand::ToggleRuler => {
                    self.tools.ruler = !self.tools.ruler;
                    return cx.notify();
                }
                ToolCommand::RulerUnit(unit) => {
                    self.tools.unit = unit;
                    crate::word_ruler::save_unit(unit);
                    return cx.notify();
                }
                ToolCommand::NextTabKind => {
                    self.tools.tab_kind = self.tools.tab_kind.next();
                    return cx.notify();
                }
                ToolCommand::InsertQuickPart(index) => return self.insert_quick_part(index, cx),
                ToolCommand::OpenSaveQuickPart => return self.open_quick_part_dialog(cx),
                ToolCommand::DeleteQuickPart(index) => return self.delete_quick_part(index, cx),
            };
        self.tool_done(result, cx);
    }

    /// Tab and ⇧Tab at the start of a list item (or over several items)
    /// move the items a level down or up, as in Word. Returns whether it
    /// did.
    pub(crate) fn tab_in_list(&mut self, backwards: bool, cx: &mut Context<Self>) -> bool {
        let at_start = match self.document.selection() {
            Some((anchor, focus)) => anchor.0 != focus.0 || anchor.1.min(focus.1) == 0,
            None => self.insert_at.1 == 0,
        };
        if !at_start {
            return false;
        }
        self.tools_caret();
        let Some(state) = self.document.list_state() else {
            return false;
        };
        // On a list's first item Word moves the whole list rather than the
        // item; here Tab stays a tab there.
        let paragraph = self.insert_at.0;
        let first_item = !(0..paragraph).any(|index| {
            self.document
                .word_document()
                .paragraph(index)
                .and_then(|paragraph| paragraph.properties.as_ref())
                .and_then(|props| props.numbering.as_ref())
                .and_then(|numbering| numbering.id.as_deref())
                == Some(state.num_id.as_str())
        });
        if first_item && self.document.selection().is_none() {
            return false;
        }
        let result = self
            .document
            .change_list_level(if backwards { -1 } else { 1 });
        self.tool_done(result, cx);
        true
    }

    /// For a paragraph whose style updates itself: the style takes the
    /// format just applied directly (自動的に更新する).
    pub(crate) fn auto_update_style(&mut self) {
        self.tools_caret();
        if let Ok(true) = self.document.auto_update_style() {
            self.ribbon.read_styles(&self.document);
            self.rebuild_all_images();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ToolCommand;
    use crate::ribbon::Action;
    use crate::word_dialog::WordDialog;
    use gpui::{point, px, Modifiers, MouseButton};
    use jade_office_model::word::building_blocks::WordBuildingBlocks;
    use jade_office_model::word::design::WordDesign;
    use jade_office_model::word::lists::{WordLists, NUMBER_FORMATS};
    use jade_office_model::word::new::{blank_document, Blank};
    use jade_office_model::word::paragraph_options::{TabAlignment, WordParagraphOptions};
    use jade_office_model::word::ruler::{snap, RulerUnit};
    use jade_office_model::word::styles::WordStyles;
    use jade_office_model::word::WordFormatting;

    /// A blank Japanese document in a window, with these lines typed.
    fn open_blank<'a>(
        cx: &'a mut gpui::TestAppContext,
        lines: &[&str],
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &'a mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
        let bytes = blank_document(Blank::Japanese);
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "test.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.document.place_caret(0, 0);
        });
        for (index, line) in lines.iter().enumerate() {
            if index > 0 {
                cx.simulate_keystrokes("enter");
            }
            cx.simulate_input(line);
        }
        cx.run_until_parked();
        (view, cx)
    }

    fn caret_to(view: &mut crate::DocumentView, paragraph: usize, offset: usize) {
        view.insert_at = (paragraph, offset);
        view.document.place_caret(paragraph, offset);
        view.selection_anchor = None;
    }

    /// A click on the ruler sets a tab stop where it lands; dragged off the
    /// ruler it goes; the タブとリーダー dialog sets one with its alignment.
    #[gpui::test]
    fn tab_stops_from_the_ruler_and_the_dialog(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["名前"]);
        cx.simulate_keystrokes("tab");
        cx.simulate_input("値");
        cx.run_until_parked();
        let (bounds, left, zoom, unit) = view.update(cx, |view, cx| {
            view.run_tool(ToolCommand::RulerUnit(RulerUnit::Millimeters), cx);
            let left = jade_office_model::word::page::WordPage::page_setup(&view.document)
                .margins
                .left;
            (
                view.tools.ruler_bounds.get(),
                left,
                view.zoom,
                view.tools.unit,
            )
        });
        assert_eq!(unit, RulerUnit::Millimeters);
        let bounds = bounds.expect("the ruler was drawn");
        let y = bounds.origin.y + bounds.size.height / 2.0;
        let at = |points: f32| bounds.origin.x + px(points * zoom);
        cx.simulate_click(point(at(left + 50.0), y), Modifiers::default());
        let expected = snap(RulerUnit::Millimeters, 10.5, 50.0);
        let tabs = view.read_with(cx, |view, _| view.document.paragraph_options().tabs);
        let status = view.read_with(cx, |view, _| {
            (
                view.tools.status.clone(),
                view.tools.ruler_bounds.get(),
                view.insert_at,
            )
        });
        assert_eq!(tabs.len(), 1, "{tabs:?} {status:?} {bounds:?}");
        assert!((tabs[0].position - expected).abs() < 0.1, "{tabs:?}");
        assert_eq!(tabs[0].alignment, TabAlignment::Left);

        // Dragged off the ruler, it is cleared.
        cx.simulate_mouse_down(
            point(at(left + expected), y),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_move(
            point(at(left + expected), y + px(80.0)),
            MouseButton::Left,
            Modifiers::default(),
        );
        cx.simulate_mouse_up(
            point(at(left + expected), y + px(80.0)),
            MouseButton::Left,
            Modifiers::default(),
        );
        view.read_with(cx, |view, _| {
            assert!(view.document.paragraph_options().tabs.is_empty())
        });

        // The selector's next kind, set from the dialog by typing.
        view.update(cx, |view, cx| {
            caret_to(view, 0, 0);
            view.run_tool(ToolCommand::OpenTabs, cx);
            assert!(matches!(view.tools.dialog, Some(WordDialog::Tabs(_))));
            view.edit_dialog(cx, |dialog| {
                if let WordDialog::Tabs(dialog) = dialog {
                    dialog.alignment = TabAlignment::Right;
                }
            });
        });
        cx.simulate_input("30 mm");
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.tools.dialog.is_none());
            let tabs = view.document.paragraph_options().tabs;
            assert_eq!(tabs.len(), 1);
            assert!(
                (tabs[0].position - 30.0 * 72.0 / 25.4).abs() < 0.1,
                "{tabs:?}"
            );
            assert_eq!(tabs[0].alignment, TabAlignment::Right);
        });
    }

    /// The borders dialog's box, the 段落 switches and 均等割り付け land on
    /// the paragraph; Undo takes each back.
    #[gpui::test]
    fn borders_switches_and_distributed_alignment(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["罫線のある段落"]);
        view.update(cx, |view, cx| {
            caret_to(view, 0, 0);
            view.run_tool(ToolCommand::OpenBorders, cx);
            view.edit_dialog(cx, |dialog| {
                if let WordDialog::Borders(dialog) = dialog {
                    dialog.style = "double";
                    dialog.borders = jade_office_model::word::paragraph_options::BorderBox::boxed(
                        jade_office_model::word::paragraph_options::BorderLine::new(
                            "double", 1.5, None,
                        ),
                    );
                }
            });
            view.dialog_ok(cx);
            let state = view.document.paragraph_options();
            assert_eq!(state.borders.left.as_ref().unwrap().style, "double");
            view.run_tool(
                ToolCommand::Switch(
                    jade_office_model::word::paragraph_options::ParagraphSwitch::ContextualSpacing,
                    true,
                ),
                cx,
            );
            assert!(view.document.paragraph_options().contextual_spacing);
            view.run_ribbon(Action::Align("distribute"), cx);
            assert_eq!(view.format_state().alignment.as_deref(), Some("distribute"));
            view.run_tool(ToolCommand::OpenDropCap, cx);
            view.edit_dialog(cx, |dialog| {
                if let WordDialog::DropCap(dialog) = dialog {
                    dialog.position =
                        Some(jade_office_model::word::paragraph_options::DropCapPosition::Dropped);
                }
            });
            view.dialog_ok(cx);
            assert!(view.document.paragraph_options().drop_cap.is_some());
        });
        cx.dispatch_action(crate::Undo);
        view.read_with(cx, |view, _| {
            assert!(view.document.paragraph_options().drop_cap.is_none())
        });
    }

    /// A Japanese number format from the library, then Tab at the second
    /// item's start takes it a level down.
    #[gpui::test]
    fn lists_from_the_library_and_tab_changes_the_level(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["一つ目", "二つ目"]);
        let kanji = NUMBER_FORMATS
            .iter()
            .position(|format| format.format == "japaneseCounting")
            .unwrap();
        view.update(cx, |view, cx| {
            view.document.select((0, 0), (1, 3), 1).unwrap();
            view.run_tool(ToolCommand::NumberFormat(kanji), cx);
            view.document.place_caret(1, 0);
            caret_to(view, 1, 0);
            let state = view.document.list_state().unwrap();
            assert_eq!(state.definition.levels[0].format, "japaneseCounting");
            assert_eq!(state.level, 0);
        });
        cx.simulate_keystrokes("tab");
        view.update(cx, |view, _| {
            view.document.place_caret(1, 0);
            assert_eq!(view.document.list_state().unwrap().level, 1);
        });
        cx.simulate_keystrokes("shift-tab");
        view.update(cx, |view, cx| {
            view.document.place_caret(1, 0);
            assert_eq!(view.document.list_state().unwrap().level, 0);
            view.run_tool(ToolCommand::OpenListStart, cx);
        });
        cx.simulate_input("7");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, _| {
            view.document.place_caret(1, 0);
            assert_eq!(view.document.list_state().unwrap().start, 7);
        });
    }

    /// スタイルの作成 from the dialog: the new style is applied and in the
    /// gallery; スタイルの変更 changes its size on the page.
    #[gpui::test]
    fn styles_are_created_and_modified_from_the_dialog(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["スタイルを作る段落"]);
        view.update(cx, |view, cx| {
            caret_to(view, 0, 0);
            view.run_tool(ToolCommand::OpenNewStyle, cx);
        });
        cx.simulate_input("強調段落");
        cx.simulate_keystrokes("enter");
        let id = view.update(cx, |view, _| {
            assert!(view.tools.dialog.is_none(), "{:?}", view.tools.status);
            let style = view
                .document
                .style_catalog()
                .into_iter()
                .find(|style| style.name == "強調段落")
                .expect("the style was made");
            assert_eq!(
                view.format_state().style_id.as_deref(),
                Some(style.id.as_str())
            );
            assert!(view
                .ribbon
                .gallery_names()
                .contains(&"強調段落".to_string()));
            style.id
        });
        view.update(cx, |view, cx| {
            view.run_tool(ToolCommand::OpenModifyStyle(id.clone()), cx);
            view.edit_dialog(cx, |dialog| {
                if let WordDialog::Style(dialog) = dialog {
                    dialog.inputs.focus = Some(1);
                }
            });
        });
        cx.simulate_input("18");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, _| {
            caret_to(view, 0, 1);
            assert_eq!(view.format_state().font_size, Some(18.0));
        });
    }

    /// テーマ, 配色 and a style set from the デザイン tab; 既定に設定.
    #[gpui::test]
    fn the_design_tab_sets_the_theme_and_style_sets(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["デザイン"]);
        view.update(cx, |view, cx| {
            view.run_tool(ToolCommand::Theme(2), cx);
            assert_eq!(
                view.document.design_state().theme.as_deref(),
                Some("Office 2007 - 2010")
            );
            view.run_tool(ToolCommand::Colors(5), cx);
            assert_eq!(
                view.document.design_state().colors.as_deref(),
                Some("Green")
            );
            view.run_tool(ToolCommand::StyleSet(1), cx);
            let heading = view
                .document
                .style_catalog()
                .into_iter()
                .find(|style| style.name == "heading 1")
                .unwrap();
            assert_eq!(heading.format.size, Some(18.0));
            caret_to(view, 0, 0);
            view.run_tool(ToolCommand::SetAsDefault, cx);
            assert!(view.tools.status.is_none(), "{:?}", view.tools.status);
        });
    }

    /// The selection saved as a クイック パーツ entry goes back in at the
    /// caret.
    #[gpui::test]
    fn quick_parts_are_saved_and_inserted(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["株式会社ジェイド", "本文"]);
        view.update(cx, |view, cx| {
            view.document
                .select((0, 0), (0, "株式会社ジェイド".len()), 1)
                .unwrap();
            view.run_tool(ToolCommand::OpenSaveQuickPart, cx);
        });
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, cx| {
            assert_eq!(
                view.document.building_blocks().len(),
                1,
                "{:?}",
                view.tools.status
            );
            caret_to(view, 1, 0);
            view.run_tool(ToolCommand::InsertQuickPart(0), cx);
            assert!(view
                .document
                .paragraph_text(1)
                .unwrap()
                .starts_with("株式会社ジェイド"));
        });
    }

    /// The gallery list runs Word's style tools for the caret's style.
    #[gpui::test]
    fn the_style_pane_applies_a_style(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_blank(cx, &["見出しにする"]);
        view.update(cx, |view, cx| {
            caret_to(view, 0, 0);
            view.run_tool(ToolCommand::ToggleStylesPane, cx);
            assert!(view.tools.styles_pane);
            WordFormatting::set_paragraph_style(&mut view.document, Some("Heading1")).unwrap();
            let id = view.format_state().style_id.unwrap();
            view.run_tool(ToolCommand::ApplyStyle(id.clone()), cx);
            assert_eq!(view.format_state().style_id, Some(id));
        });
        cx.run_until_parked();
    }

    /// Every dialog, menu and pane of the tools draws, on each ribbon tab.
    #[gpui::test]
    fn every_dialog_and_menu_draws(cx: &mut gpui::TestAppContext) {
        use super::ToolMenu;
        use crate::ribbon::Popup;
        let (view, cx) = open_blank(cx, &["一つ目", "二つ目"]);
        view.update(cx, |view, cx| {
            view.document.select((0, 0), (1, 3), 1).unwrap();
            view.run_tool(
                ToolCommand::Multilevel(jade_office_model::word::lists::MultilevelPreset::Articles),
                cx,
            );
            caret_to(view, 1, 0);
            view.tools.styles_pane = true;
        });
        let dialogs = [
            ToolCommand::OpenTabs,
            ToolCommand::OpenBorders,
            ToolCommand::OpenDropCap,
            ToolCommand::OpenListStart,
            ToolCommand::OpenListIndent,
            ToolCommand::OpenDefineList,
            ToolCommand::OpenListStyle,
            ToolCommand::OpenNewStyle,
            ToolCommand::OpenSaveQuickPart,
        ];
        for command in dialogs {
            view.update(cx, |view, cx| {
                caret_to(view, 1, 0);
                view.run_tool(command.clone(), cx);
                assert!(
                    view.tools.dialog.is_some(),
                    "{command:?}: {:?}",
                    view.tools.status
                );
            });
            cx.run_until_parked();
            cx.simulate_keystrokes("escape");
            view.read_with(cx, |view, _| assert!(view.tools.dialog.is_none()));
        }
        let menus = [
            (crate::RibbonTab::Home, ToolMenu::Bullets),
            (crate::RibbonTab::Home, ToolMenu::Numbering),
            (crate::RibbonTab::Home, ToolMenu::Multilevel),
            (crate::RibbonTab::Home, ToolMenu::ParagraphSettings),
            (crate::RibbonTab::Home, ToolMenu::StyleCard(0)),
            (crate::RibbonTab::Home, ToolMenu::PaneStyle(0)),
            (crate::RibbonTab::Insert, ToolMenu::QuickParts),
            (crate::RibbonTab::Insert, ToolMenu::DropCap),
            (crate::RibbonTab::Design, ToolMenu::Themes),
            (crate::RibbonTab::Design, ToolMenu::Colors),
            (crate::RibbonTab::Design, ToolMenu::Fonts),
            (crate::RibbonTab::Design, ToolMenu::Effects),
            (crate::RibbonTab::Design, ToolMenu::StyleSets),
            (crate::RibbonTab::Design, ToolMenu::Spacing),
            (crate::RibbonTab::View, ToolMenu::RulerUnits),
        ];
        for (tab, menu) in menus {
            view.update(cx, |view, cx| {
                view.ribbon_tab = tab;
                view.ribbon.popup = Some(Popup::Word(menu));
                cx.notify();
            });
            cx.run_until_parked();
        }
    }
}
