//! リボンのユーザー設定 and the クイック アクセス ツール バー in the browser:
//! which tabs the ribbon shows, a group of the user's own commands on ホーム,
//! and the commands in the strip before the tabs — chosen in Word の
//! オプション and kept in the browser between visits. The commands
//! themselves are `jade-office-model`'s (`word::customize`).

use gpui::{div, prelude::*, px, AnyElement, Context, Window};
use jade_web::word::customize::{format_commands, parse_commands, QuickCommand, DEFAULT_TOOLBAR};
use jade_web::word::CharacterFormat;

use super::{settings, ViewsAction};
use crate::ribbon::{Action, Tab};
use crate::view::DocView as DocumentView;
use crate::{i18n, palette as theme};

const RIBBON: &str = "ribbon.txt";

/// Runs a toolbar command.
pub fn run(
    view: &mut DocumentView,
    command: QuickCommand,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) {
    use QuickCommand as Q;
    let action = match command {
        Q::Save => return view.save(cx),
        Q::Undo => Action::Undo,
        Q::Redo => Action::Redo,
        Q::Print => return view.open_print_panel(cx),
        Q::ExportPdf => return view.export_pdf(cx),
        Q::Find => return view.open_find(false, cx),
        Q::AdvancedFind => return view.open_find(true, cx),
        Q::Bold => Action::Toggle(CharacterFormat::Bold),
        Q::Italic => Action::Toggle(CharacterFormat::Italic),
        Q::Underline => Action::Toggle(CharacterFormat::Underline),
        Q::Bullets => Action::List(true),
        Q::Numbering => Action::List(false),
        Q::AlignLeft => Action::Align("left"),
        Q::AlignCenter => Action::Align("center"),
        Q::AlignRight => Action::Align("right"),
        Q::Font => {
            crate::view::word_characters::open_font_dialog(view, cx);
            return cx.notify();
        }
        Q::Paragraph => Action::ParagraphDialog,
        Q::InsertTable => Action::InsertTable(2, 2),
        Q::Picture => Action::InsertPicture,
        Q::Link => Action::InsertLink,
        Q::Comment => Action::Review(crate::word_review::ReviewAction::NewComment),
        Q::TrackChanges => Action::Review(crate::word_review::ReviewAction::ToggleTracking),
        Q::FormattingMarks => Action::View(crate::ribbon::ViewAction::ToggleMarks),
        Q::ZoomIn => Action::View(crate::ribbon::ViewAction::ZoomIn),
        Q::ZoomOut => Action::View(crate::ribbon::ViewAction::ZoomOut),
        Q::AutoCorrect => Action::Views(ViewsAction::OpenAutoCorrect),
        Q::Overtype => {
            super::prefs::set_overtype(!super::prefs::overtype());
            return cx.notify();
        }
    };
    view.ribbon_action(action, window, cx);
}

fn list(key: &str, default: &[QuickCommand]) -> Vec<QuickCommand> {
    match settings::value(RIBBON, key) {
        Some(saved) => parse_commands(&saved),
        None => default.to_vec(),
    }
}

/// The commands the toolbar holds: saved, or Word's 保存, 元に戻す, やり直し.
pub fn toolbar_commands() -> Vec<QuickCommand> {
    list("qat", &DEFAULT_TOOLBAR)
}

/// The commands of the user's own ribbon group.
pub fn group_commands() -> Vec<QuickCommand> {
    list("custom_group", &[])
}

/// Saves the toolbar's or the group's commands.
pub fn set_commands(key: &str, commands: &[QuickCommand]) {
    settings::set_value(RIBBON, key, &format_commands(commands));
}

fn tab_key(tab: Tab) -> String {
    format!("tab.{tab:?}")
}

/// Whether リボンのユーザー設定 shows `tab`.
pub fn tab_shown(tab: Tab) -> bool {
    settings::value(RIBBON, &tab_key(tab)).as_deref() != Some("hidden")
}

pub fn set_tab_shown(tab: Tab, shown: bool) {
    settings::set_value(
        RIBBON,
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
        .tooltip(crate::tooltip::Tooltip::text(i18n::text(
            command.label_key(),
        )))
        .on_click(crate::ribbon::listener(
            Action::Views(ViewsAction::Quick(command)),
            cx,
        ))
        .map(|button| match command.glyph() {
            (Some(icon), _) => {
                button.child(gpui::svg().path(icon).size_3p5().text_color(theme::text()))
            }
            (None, text) => button.child(text),
        })
        .into_any_element()
}

/// The クイック アクセス ツール バー, before the tab strip.
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
