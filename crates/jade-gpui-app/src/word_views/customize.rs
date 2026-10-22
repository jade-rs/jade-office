//! リボンのユーザー設定 and the クイック アクセス ツール バー: which tabs
//! the ribbon shows, a group of the user's own commands on ホーム, and the
//! commands in the strip above the tabs — all chosen in Word のオプション
//! and kept between runs.

use gpui::{div, prelude::*, px, AnyElement, Context};

use super::settings;
use crate::ribbon::Action;
use crate::tooltip::Tooltip;
use crate::word_dialog::on;
use crate::{i18n, theme, DocumentView, RibbonTab};

pub use jade_office_model::word::customize::QuickCommand;

/// Runs a toolbar command.
pub fn run(view: &mut DocumentView, command: QuickCommand, cx: &mut Context<DocumentView>) {
    use jade_office_model::word::CharacterFormat;
    use QuickCommand as Q;
    let action = match command {
        Q::Save => return view.save(cx),
        Q::Undo => Action::Undo,
        Q::Redo => Action::Redo,
        Q::Print => return view.open_print_panel(cx),
        Q::ExportPdf => return view.export_pdf(cx),
        Q::Find => return view.open_find_bar(cx),
        Q::AdvancedFind => {
            super::find::open(view, false);
            return cx.notify();
        }
        Q::Bold => Action::Toggle(CharacterFormat::Bold),
        Q::Italic => Action::Toggle(CharacterFormat::Italic),
        Q::Underline => Action::Toggle(CharacterFormat::Underline),
        Q::Bullets => Action::List(true),
        Q::Numbering => Action::List(false),
        Q::AlignLeft => Action::Align("left"),
        Q::AlignCenter => Action::Align("center"),
        Q::AlignRight => Action::Align("right"),
        Q::Font => return crate::word_characters::open_font_dialog(view, cx),
        Q::Paragraph => return view.open_paragraph_dialog(cx),
        Q::InsertTable => Action::InsertTable(2, 2),
        Q::Picture => return view.prompt_for_picture(cx),
        Q::Link => return view.open_link_dialog(cx),
        Q::Comment => return view.add_comment(cx),
        Q::TrackChanges => return view.toggle_tracking(cx),
        Q::FormattingMarks => return view.toggle_formatting_marks(cx),
        Q::ZoomIn => {
            use crate::Editor;
            let zoom = view.zoom * 1.1;
            return view.set_zoom(zoom, cx);
        }
        Q::ZoomOut => {
            use crate::Editor;
            let zoom = view.zoom / 1.1;
            return view.set_zoom(zoom, cx);
        }
        Q::AutoCorrect => return super::run(view, super::ViewAction::OpenAutoCorrect, cx),
        Q::Overtype => return super::input::toggle_overtype(view, cx),
    };
    view.run_ribbon(action, cx);
}

/// The commands the toolbar holds: saved, or Word's 保存, 元に戻す, やり直し.
pub fn toolbar_commands() -> Vec<QuickCommand> {
    list(
        "qat",
        &[QuickCommand::Save, QuickCommand::Undo, QuickCommand::Redo],
    )
}

/// The commands of the user's own ribbon group.
pub fn group_commands() -> Vec<QuickCommand> {
    list("custom_group", &[])
}

fn list(key: &str, default: &[QuickCommand]) -> Vec<QuickCommand> {
    match settings::value("ribbon.txt", key) {
        Some(saved) => saved
            .split(',')
            .filter_map(|id| QuickCommand::from_id(id.trim()))
            .collect(),
        None => default.to_vec(),
    }
}

/// Saves the toolbar's or the group's commands.
pub fn set_commands(key: &str, commands: &[QuickCommand]) {
    let text: Vec<&str> = commands.iter().map(|command| command.id()).collect();
    settings::set_value("ribbon.txt", key, &text.join(","));
}

/// A tab's saved key.
pub fn tab_key(tab: RibbonTab) -> String {
    format!("tab.{tab:?}")
}

/// Whether リボンのユーザー設定 shows `tab`.
pub fn tab_shown(tab: RibbonTab) -> bool {
    settings::value("ribbon.txt", &tab_key(tab)).as_deref() != Some("hidden")
}

pub fn set_tab_shown(tab: RibbonTab, shown: bool) {
    settings::set_value(
        "ribbon.txt",
        &tab_key(tab),
        if shown { "shown" } else { "hidden" },
    );
}

fn quick_button(
    command: QuickCommand,
    id: (&'static str, usize),
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(24.0))
        .h(px(22.0))
        .px_1()
        .rounded_md()
        .text_xs()
        .text_color(theme::text())
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(i18n::text(command.label_key())))
        .on_click(on(cx, move |view, cx| run(view, command, cx)))
        .map(|button| match command.glyph() {
            (Some(icon), _) => {
                button.child(gpui::svg().path(icon).size_3p5().text_color(theme::text()))
            }
            (None, text) => button.child(text),
        })
        .into_any_element()
}

/// The クイック アクセス ツール バー, at the left of the tab strip.
pub fn toolbar(cx: &mut Context<DocumentView>) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_0p5()
        .pr_2()
        .mr_1()
        .border_r_1()
        .border_color(theme::border())
        .children(
            toolbar_commands()
                .into_iter()
                .enumerate()
                .map(|(index, command)| quick_button(command, ("qat", index), cx)),
        )
        .into_any_element()
}

/// The user's group on ホーム, when it has commands.
pub fn custom_group(cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let commands = group_commands();
    if commands.is_empty() {
        return None;
    }
    let buttons: Vec<AnyElement> = commands
        .into_iter()
        .enumerate()
        .map(|(index, command)| quick_button(command, ("custom-group", index), cx))
        .collect();
    Some(crate::ribbon::group(
        i18n::text("customize.group"),
        vec![crate::ribbon::row(buttons)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lists_round_trip_through_the_settings() {
        assert_eq!(
            toolbar_commands(),
            [QuickCommand::Save, QuickCommand::Undo, QuickCommand::Redo]
        );
        set_commands("qat", &[QuickCommand::Print, QuickCommand::Bold]);
        assert_eq!(
            toolbar_commands(),
            [QuickCommand::Print, QuickCommand::Bold]
        );
        assert!(tab_shown(RibbonTab::Design));
        set_tab_shown(RibbonTab::Design, false);
        assert!(!tab_shown(RibbonTab::Design));
        assert!(settings::load("ribbon.txt")
            .unwrap()
            .contains("tab.Design=hidden"));
    }
}
