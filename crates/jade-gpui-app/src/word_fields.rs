//! Word's fields in the native shell, over `jade-office-model`'s
//! `WordFields`: 挿入 › フィールド (the Field dialog — categories, field
//! names, the code, its options), the prompts ASK and FILLIN show on F9,
//! ファイル › プロパティ (summary, statistics, custom properties) with the
//! user information USERNAME and its kin read, and the field keys —
//! F9 update, ⌥F9/⇧F9 codes, ⌘F11/⌘⇧F11 lock, ⌘⇧F9 unlink.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, MouseButton, SharedString};
use jade_office_model::text::TextEdit;
use jade_office_model::word::fields::{
    build_code, code_field_name, code_has_switch, field_spec, fields_in, set_operand,
    toggle_switch, CustomProperty, CustomValue, DocumentProperties, FieldCategory, FieldHost,
    FieldPrompt, FieldSpec, OperandSource, SwitchArgument, WordFields,
};

use crate::{i18n, theme, DocumentView};

/// The user information fields read (Word's 環境設定 › ユーザー情報).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UserInfo {
    pub name: String,
    pub initials: String,
    pub address: String,
}

impl UserInfo {
    fn store() -> Option<PathBuf> {
        if cfg!(test) {
            return None;
        }
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library/Application Support/jade-office/user.txt"))
    }

    /// What was saved, else the account's full name.
    pub fn load() -> Self {
        if let Some(text) = Self::store().and_then(|store| std::fs::read_to_string(store).ok()) {
            let mut lines = text.lines().map(|line| line.replace("\\n", "\n"));
            return Self {
                name: lines.next().unwrap_or_default(),
                initials: lines.next().unwrap_or_default(),
                address: lines.next().unwrap_or_default(),
            };
        }
        Self {
            name: account_name().unwrap_or_default(),
            ..Self::default()
        }
    }

    pub fn save(&self) {
        let Some(store) = Self::store() else { return };
        if let Some(directory) = store.parent() {
            std::fs::create_dir_all(directory).ok();
        }
        let text = [&self.name, &self.initials, &self.address]
            .iter()
            .map(|value| value.replace('\n', "\\n"))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(store, text).ok();
    }
}

/// The login account's full name.
fn account_name() -> Option<String> {
    // SAFETY: getpwuid returns a pointer into static storage or null; the
    // name is copied out before anything else can call it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_gecos.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*entry).pw_gecos)
            .to_string_lossy()
            .split(',')
            .next()
            .unwrap_or_default()
            .trim()
            .to_string();
        (!name.is_empty()).then_some(name)
    }
}

/// The local time now and the zone's offset from UTC.
pub(crate) fn local_now() -> (jade_docx_editor::field_datetime::FieldDateTime, i64) {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as libc::time_t)
        .unwrap_or(0);
    // SAFETY: localtime_r writes only into the `tm` it is given, which is
    // plain data that all zeroes is a valid value of.
    let local = unsafe {
        let mut local: libc::tm = std::mem::zeroed();
        libc::localtime_r(&seconds, &mut local);
        local
    };
    (
        jade_docx_editor::field_datetime::FieldDateTime {
            year: local.tm_year + 1900,
            month: (local.tm_mon + 1) as u32,
            day: local.tm_mday as u32,
            hour: local.tm_hour as u32,
            minute: local.tm_min as u32,
            second: local.tm_sec as u32,
            weekday: local.tm_wday as u32,
        },
        local.tm_gmtoff,
    )
}

/// Gives a newly opened document the clock, repainting it when it has
/// DATE or TIME fields to show it.
pub(crate) fn set_clock_on_open(document: &mut jade_docx_native_render::NativeDocument) {
    use jade_docx_editor::document::RunFieldKind;
    document.set_field_datetime(Some(local_now().0));
    let word = document.word_document();
    let has_clock = (0..word.paragraph_count()).any(|index| {
        word.paragraph(index).is_some_and(|paragraph| {
            paragraph.runs.iter().any(|run| {
                matches!(
                    run.field.as_ref().map(|field| field.kind.base_kind()),
                    Some(RunFieldKind::Date { .. } | RunFieldKind::Time { .. })
                )
            })
        })
    });
    if has_clock {
        document.repaint_all().ok();
    }
}

/// The Field dialog.
pub(crate) struct FieldDialog {
    /// フィールドの編集 rather than 挿入.
    pub editing: bool,
    /// The category list's choice; `None` is (すべて).
    pub category: Option<FieldCategory>,
    pub field: Option<&'static FieldSpec>,
    pub code: TextEdit,
}

/// The prompts of ASK and FILLIN, one after another, before an update.
pub(crate) struct PromptDialog {
    pub prompts: Vec<FieldPrompt>,
    pub index: usize,
    pub answers: HashMap<String, String>,
    pub input: TextEdit,
    pub all: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PropertiesTab {
    Summary,
    Statistics,
    Custom,
    User,
}

/// The summary fields, by i18n key, in the dialog's order.
const SUMMARY: [&str; 9] = [
    "field.properties.title",
    "field.properties.subject",
    "field.properties.author",
    "field.properties.manager",
    "field.properties.company",
    "field.properties.category",
    "field.properties.keywords",
    "field.properties.comments",
    "field.properties.hyperlink_base",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CustomKind {
    Text,
    Number,
    Date,
    YesNo,
}

/// ファイル › プロパティ.
pub(crate) struct PropertiesDialog {
    pub tab: PropertiesTab,
    pub properties: DocumentProperties,
    pub summary: Vec<TextEdit>,
    pub custom_name: TextEdit,
    pub custom_value: TextEdit,
    pub custom_kind: CustomKind,
    pub user: Vec<TextEdit>,
    /// Which input on the tab takes typing.
    pub focus: usize,
}

impl PropertiesDialog {
    fn open(properties: DocumentProperties, user: &UserInfo) -> Self {
        let summary = [
            &properties.title,
            &properties.subject,
            &properties.author,
            &properties.manager,
            &properties.company,
            &properties.category,
            &properties.keywords,
            &properties.comments,
            &properties.hyperlink_base,
        ]
        .iter()
        .map(|value| TextEdit::new((*value).clone()))
        .collect();
        Self {
            tab: PropertiesTab::Summary,
            properties,
            summary,
            custom_name: TextEdit::new(String::new()),
            custom_value: TextEdit::new(String::new()),
            custom_kind: CustomKind::Text,
            user: vec![
                TextEdit::new(user.name.clone()),
                TextEdit::new(user.initials.clone()),
                TextEdit::new(user.address.clone()),
            ],
            focus: 0,
        }
    }

    fn inputs(&self) -> usize {
        match self.tab {
            PropertiesTab::Summary => self.summary.len(),
            PropertiesTab::Custom => 2,
            PropertiesTab::User => self.user.len(),
            PropertiesTab::Statistics => 0,
        }
    }

    fn focused(&mut self) -> Option<&mut TextEdit> {
        match self.tab {
            PropertiesTab::Summary => self.summary.get_mut(self.focus),
            PropertiesTab::Custom => Some(if self.focus == 0 {
                &mut self.custom_name
            } else {
                &mut self.custom_value
            }),
            PropertiesTab::User => self.user.get_mut(self.focus),
            PropertiesTab::Statistics => None,
        }
    }

    fn focused_ref(&self) -> Option<&TextEdit> {
        match self.tab {
            PropertiesTab::Summary => self.summary.get(self.focus),
            PropertiesTab::Custom => Some(if self.focus == 0 {
                &self.custom_name
            } else {
                &self.custom_value
            }),
            PropertiesTab::User => self.user.get(self.focus),
            PropertiesTab::Statistics => None,
        }
    }

    /// The properties as the inputs have them.
    fn result(&self) -> DocumentProperties {
        let mut properties = self.properties.clone();
        let values: Vec<String> = self.summary.iter().map(|edit| edit.text.clone()).collect();
        properties.title = values[0].clone();
        properties.subject = values[1].clone();
        properties.author = values[2].clone();
        properties.manager = values[3].clone();
        properties.company = values[4].clone();
        properties.category = values[5].clone();
        properties.keywords = values[6].clone();
        properties.comments = values[7].clone();
        properties.hyperlink_base = values[8].clone();
        properties
    }

    fn user_info(&self) -> UserInfo {
        UserInfo {
            name: self.user[0].text.clone(),
            initials: self.user[1].text.clone(),
            address: self.user[2].text.clone(),
        }
    }

    /// 追加: the custom property typed, replacing one of the same name.
    fn add_custom(&mut self) -> Result<(), &'static str> {
        let name = self.custom_name.text.trim().to_string();
        if name.is_empty() {
            return Err("field.properties.name_missing");
        }
        let text = self.custom_value.text.trim().to_string();
        let value = match self.custom_kind {
            CustomKind::Text => CustomValue::Text(text),
            CustomKind::Number => CustomValue::Number(
                text.replace(',', "")
                    .parse()
                    .map_err(|_| "field.properties.not_a_number")?,
            ),
            CustomKind::Date => {
                let parts: Vec<u32> = text
                    .split(['/', '-', '.'])
                    .filter_map(|part| part.trim().parse().ok())
                    .collect();
                match parts.as_slice() {
                    [year, month, day] if (1..=12).contains(month) && (1..=31).contains(day) => {
                        CustomValue::Date(format!("{year:04}-{month:02}-{day:02}T00:00:00Z"))
                    }
                    _ => return Err("field.properties.not_a_date"),
                }
            }
            CustomKind::YesNo => CustomValue::YesNo(matches!(
                text.to_lowercase().as_str(),
                "" | "yes" | "y" | "true" | "はい" | "1"
            )),
        };
        self.properties
            .custom
            .retain(|property| !property.name.eq_ignore_ascii_case(&name));
        self.properties.custom.push(CustomProperty { name, value });
        self.custom_name = TextEdit::new(String::new());
        self.custom_value = TextEdit::new(String::new());
        self.focus = 0;
        Ok(())
    }
}

pub(crate) enum FieldsDialog {
    Field(FieldDialog),
    Prompt(PromptDialog),
    Properties(PropertiesDialog),
}

impl FieldsDialog {
    pub(crate) fn text(&self) -> Option<&TextEdit> {
        match self {
            FieldsDialog::Field(dialog) => Some(&dialog.code),
            FieldsDialog::Prompt(dialog) => Some(&dialog.input),
            FieldsDialog::Properties(dialog) => dialog.focused_ref(),
        }
    }

    pub(crate) fn text_mut(&mut self) -> Option<&mut TextEdit> {
        match self {
            FieldsDialog::Field(dialog) => Some(&mut dialog.code),
            FieldsDialog::Prompt(dialog) => Some(&mut dialog.input),
            FieldsDialog::Properties(dialog) => dialog.focused(),
        }
    }
}

/// The shell's field state: an open dialog and the user information.
pub(crate) struct FieldsUi {
    pub dialog: Option<FieldsDialog>,
    pub user: UserInfo,
    /// Why the last field command did nothing, if it did nothing.
    pub status: Option<SharedString>,
}

impl Default for FieldsUi {
    fn default() -> Self {
        Self {
            dialog: None,
            user: UserInfo::load(),
            status: None,
        }
    }
}

fn japanese() -> bool {
    i18n::language() == i18n::Language::Japanese
}

/// A choice: highlighted when chosen.
fn chip(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |chip| chip.bg(theme::selected()))
        .cursor_pointer()
        .hover(|chip| chip.bg(theme::hovered()))
        .child(label.into())
}

/// A list row: highlighted when chosen.
fn list_row(
    id: impl Into<gpui::ElementId>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_2()
        .py_0p5()
        .rounded_sm()
        .cursor_pointer()
        .when(on, |row| row.bg(theme::selected()))
        .hover(|row| row.bg(theme::hovered()))
        .child(label.into())
}

/// A text input: the text with a caret when focused.
fn input(
    id: impl Into<gpui::ElementId>,
    edit: &TextEdit,
    focused: bool,
) -> gpui::Stateful<gpui::Div> {
    let (before, after) = edit.split();
    div()
        .id(id)
        .flex()
        .items_center()
        .min_h(px(22.0))
        .px_2()
        .rounded_sm()
        .border_1()
        .border_color(if focused {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .text_color(theme::text())
        .cursor_text()
        .child(before.replace('\n', " ⏎ "))
        .when(focused, |input| {
            input.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
        })
        .child(after.replace('\n', " ⏎ "))
}

fn button(id: &'static str, key: &'static str, primary: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .px_3()
        .py_1()
        .rounded_md()
        .border_1()
        .border_color(if primary {
            theme::accent()
        } else {
            theme::border()
        })
        .when(primary, |button| button.bg(theme::selected()))
        .cursor_pointer()
        .hover(|button| button.bg(theme::hovered()))
        .child(i18n::text(key))
}

fn panel(id: &'static str, width: f32) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .w(px(width))
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .rounded_md()
        .border_1()
        .border_color(theme::border())
        .bg(theme::panel())
        .shadow_lg()
        .text_xs()
        .text_color(theme::text())
}

fn backdrop(content: impl IntoElement) -> AnyElement {
    div()
        .id("fields-dialog-backdrop")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(gpui::rgba(0x00000055))
        .occlude()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(content)
        .into_any_element()
}

impl DocumentView {
    /// What field evaluation needs from the shell.
    pub(crate) fn field_host(&self) -> FieldHost {
        let (now, offset) = local_now();
        FieldHost {
            now,
            utc_offset_seconds: offset,
            user_name: self.fields.user.name.clone(),
            user_initials: self.fields.user.initials.clone(),
            user_address: self.fields.user.address.clone(),
            file: (!self.untitled).then(|| PathBuf::from(self.path.to_string())),
            japanese: japanese(),
        }
    }

    /// After a field command: the pages again, the caret where the command
    /// left the selection.
    fn after_field_command(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.dirty = true;
                self.fields.status = None;
                if let Some(range) = self.document.selection_range() {
                    let focus = range.focus;
                    self.insert_at = (focus.paragraph, focus.offset);
                    if range.is_collapsed() {
                        self.selection_anchor = None;
                        self.caret = self.document.place_caret(focus.paragraph, focus.offset);
                    }
                }
                self.rebuild_after_edit();
            }
            Err(error) => self.fields.status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// The selection as the document has it, from the shell's caret when
    /// there is no selection.
    fn sync_field_selection(&mut self) {
        if self.document.selection().is_none() {
            let (paragraph, offset) = self.insert_at;
            self.document.place_caret(paragraph, offset);
        }
    }

    /// 挿入 › フィールド (⌘F9).
    pub(crate) fn open_field_dialog(&mut self, preset: Option<&str>, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let field = preset.and_then(field_spec);
        self.fields.dialog = Some(FieldsDialog::Field(FieldDialog {
            editing: false,
            category: field.map(|spec| spec.category),
            field,
            code: TextEdit::new(field.map(|spec| spec.name.to_string()).unwrap_or_default()),
        }));
        cx.notify();
    }

    /// フィールドの編集: the dialog on the selected field's code.
    pub(crate) fn open_edit_field_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let Some(field) = self.document.selected_fields().into_iter().next() else {
            self.fields.status = Some(SharedString::from(i18n::text("field.status.none")));
            return cx.notify();
        };
        let spec = field_spec(&field.name);
        self.fields.dialog = Some(FieldsDialog::Field(FieldDialog {
            editing: true,
            category: spec.map(|spec| spec.category),
            field: spec,
            code: TextEdit::new(field.code),
        }));
        cx.notify();
    }

    /// ファイル › プロパティ.
    pub(crate) fn open_properties_dialog(&mut self, cx: &mut Context<Self>) {
        let properties = self.document.document_properties();
        self.fields.dialog = Some(FieldsDialog::Properties(PropertiesDialog::open(
            properties,
            &self.fields.user,
        )));
        cx.notify();
    }

    /// F9: the selection's fields, or every field when the document is
    /// selected — after asking what ASK and FILLIN want to know.
    pub(crate) fn update_fields(&mut self, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let scope = self.document.field_scope();
        let all = matches!(scope, jade_docx_editor::fields::FieldScope::Document);
        let prompts = self.document.field_prompts(scope);
        if prompts.is_empty() {
            return self.run_field_update(all, HashMap::new(), cx);
        }
        let input = TextEdit::selected(prompts[0].default.clone());
        self.fields.dialog = Some(FieldsDialog::Prompt(PromptDialog {
            prompts,
            index: 0,
            answers: HashMap::new(),
            input,
            all,
        }));
        cx.notify();
    }

    fn run_field_update(
        &mut self,
        all: bool,
        answers: HashMap<String, String>,
        cx: &mut Context<Self>,
    ) {
        let host = self.field_host();
        // DATE and TIME show the clock of this update.
        self.document.set_field_datetime(Some(host.now));
        let scope = if all {
            jade_docx_editor::fields::FieldScope::Document
        } else {
            self.document.field_scope()
        };
        let result = self.document.update_fields(scope, &host, answers);
        self.after_field_command(result, cx);
    }

    /// ⌥F9: every field's code, or every result.
    pub(crate) fn toggle_all_field_codes(&mut self, cx: &mut Context<Self>) {
        let mut view = self.document.field_code_view().clone();
        view.all = !view.all;
        view.toggled.clear();
        let result = self.document.set_field_code_view(view);
        self.after_view_change(result, cx);
    }

    /// ⇧F9: the selected fields' codes, or their results.
    pub(crate) fn toggle_selected_field_codes(&mut self, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let view = self
            .document
            .toggled_field_codes(self.document.field_code_view());
        let result = self.document.set_field_code_view(view);
        self.after_view_change(result, cx);
    }

    fn after_view_change(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
                self.rebuild_all_images();
            }
            Err(error) => self.fields.status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// ⌘F11 / ⌘⇧F11.
    pub(crate) fn lock_fields(&mut self, locked: bool, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let result = WordFields::lock_fields(&mut self.document, locked);
        self.after_field_command(result, cx);
    }

    /// ⌘⇧F9.
    pub(crate) fn unlink_fields(&mut self, cx: &mut Context<Self>) {
        self.sync_field_selection();
        let result = WordFields::unlink_fields(&mut self.document);
        self.after_field_command(result, cx);
    }

    fn edit_fields_dialog(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut FieldsDialog)) {
        if let Some(dialog) = self.fields.dialog.as_mut() {
            edit(dialog);
            cx.notify();
        }
    }

    fn edit_field_dialog(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut FieldDialog)) {
        self.edit_fields_dialog(cx, |dialog| {
            if let FieldsDialog::Field(dialog) = dialog {
                edit(dialog);
            }
        });
    }

    fn edit_properties_dialog(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut PropertiesDialog),
    ) {
        self.edit_fields_dialog(cx, |dialog| {
            if let FieldsDialog::Properties(dialog) = dialog {
                edit(dialog);
            }
        });
    }

    /// OK.
    pub(crate) fn confirm_fields_dialog(&mut self, cx: &mut Context<Self>) {
        let Some(dialog) = self.fields.dialog.take() else {
            return;
        };
        match dialog {
            FieldsDialog::Field(dialog) => {
                let code = dialog.code.text.trim().to_string();
                if code.is_empty() {
                    self.fields.dialog = Some(FieldsDialog::Field(dialog));
                    return cx.notify();
                }
                let host = self.field_host();
                let result = if dialog.editing {
                    self.document.edit_field_code(&code, &host)
                } else {
                    self.document.insert_field_code(&code, &host)
                };
                self.after_field_command(result, cx);
            }
            FieldsDialog::Prompt(mut dialog) => {
                let key = dialog.prompts[dialog.index].key.clone();
                dialog.answers.insert(key, dialog.input.text.clone());
                dialog.index += 1;
                if dialog.index < dialog.prompts.len() {
                    dialog.input = TextEdit::selected(dialog.prompts[dialog.index].default.clone());
                    self.fields.dialog = Some(FieldsDialog::Prompt(dialog));
                    return cx.notify();
                }
                self.run_field_update(dialog.all, dialog.answers, cx);
            }
            FieldsDialog::Properties(dialog) => {
                let user = dialog.user_info();
                if user != self.fields.user {
                    user.save();
                    self.fields.user = user;
                }
                let properties = dialog.result();
                if properties != self.document.document_properties() {
                    let result = self.document.set_document_properties(properties);
                    self.after_field_command(result, cx);
                } else {
                    cx.notify();
                }
            }
        }
    }

    /// Keys while a dialog is open: Escape cancels, Enter is OK, Tab moves
    /// between inputs, the rest edit the focused input.
    pub(crate) fn handle_fields_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(dialog) = self.fields.dialog.as_mut() else {
            return false;
        };
        if dialog.text().is_some_and(|text| text.marked.is_some()) {
            return false;
        }
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.fields.dialog = None,
            "enter" | "return" => {
                // A comment takes line breaks; everything else confirms.
                if let FieldsDialog::Properties(properties) = dialog {
                    if properties.tab == PropertiesTab::Custom {
                        if let Err(message) = properties.add_custom() {
                            self.fields.status = Some(SharedString::from(i18n::text(message)));
                        }
                        cx.notify();
                        return true;
                    }
                }
                self.confirm_fields_dialog(cx);
                return true;
            }
            "tab" => {
                if let FieldsDialog::Properties(properties) = dialog {
                    let count = properties.inputs().max(1);
                    properties.focus = if event.keystroke.modifiers.shift {
                        (properties.focus + count - 1) % count
                    } else {
                        (properties.focus + 1) % count
                    };
                }
            }
            _ => {
                let Some(text) = dialog.text_mut() else {
                    return true;
                };
                match key {
                    "backspace" => text.backspace(),
                    "delete" => text.delete(),
                    "left" => text.step(false),
                    "right" => text.step(true),
                    "home" | "up" => text.to_end(false),
                    "end" | "down" => text.to_end(true),
                    // Typing reaches the input through the input handler.
                    _ => return false,
                }
            }
        }
        cx.notify();
        true
    }

    /// The open dialog, over the pages.
    pub(crate) fn fields_dialog_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let dialog = self.fields.dialog.as_ref()?;
        Some(match dialog {
            FieldsDialog::Field(dialog) => self.field_dialog_element(dialog, cx),
            FieldsDialog::Prompt(dialog) => self.prompt_dialog_element(dialog, cx),
            FieldsDialog::Properties(dialog) => self.properties_dialog_element(dialog, cx),
        })
    }

    fn dialog_buttons(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .justify_end()
            .gap_2()
            .child(
                button("fields-cancel", "field.cancel", false).on_click(cx.listener(
                    |view, _, _, cx| {
                        view.fields.dialog = None;
                        cx.notify();
                    },
                )),
            )
            .child(
                button("fields-ok", "field.ok", true)
                    .on_click(cx.listener(|view, _, _, cx| view.confirm_fields_dialog(cx))),
            )
    }

    fn field_dialog_element(&self, dialog: &FieldDialog, cx: &mut Context<Self>) -> AnyElement {
        let mut categories =
            vec![list_row(
                "field-category-all",
                i18n::text("field.category.all"),
                dialog.category.is_none(),
            )
            .on_click(cx.listener(|view, _, _, cx| {
                view.edit_field_dialog(cx, |dialog| dialog.category = None)
            }))
            .into_any_element()];
        for (index, category) in FieldCategory::ALL.into_iter().enumerate() {
            categories.push(
                list_row(
                    ("field-category", index),
                    i18n::text(category.key()),
                    dialog.category == Some(category),
                )
                .on_click(cx.listener(move |view, _, _, cx| {
                    view.edit_field_dialog(cx, |dialog| dialog.category = Some(category))
                }))
                .into_any_element(),
            );
        }
        let names: Vec<AnyElement> = fields_in(dialog.category)
            .into_iter()
            .enumerate()
            .map(|(index, spec)| {
                let on = dialog.field.is_some_and(|field| field.name == spec.name);
                list_row(("field-name", index), spec.name, on)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_field_dialog(cx, |dialog| {
                            let preserve =
                                code_has_switch(&dialog.code.text, "\\*", Some("MERGEFORMAT"));
                            dialog.field = Some(spec);
                            dialog.code = TextEdit::new(build_code(spec.name, &[], &[], preserve));
                        })
                    }))
                    .into_any_element()
            })
            .collect();

        let code = dialog.code.text.clone();
        let field = dialog
            .field
            .or_else(|| code_field_name(&code).and_then(|name| field_spec(&name)));
        let mut options: Vec<AnyElement> = Vec::new();
        if let Some(spec) = field {
            options.push(
                div()
                    .text_color(theme::text_dim())
                    .child(i18n::text(spec.key))
                    .into_any_element(),
            );
            for (operand_index, operand) in spec.operands.iter().enumerate() {
                let values: Vec<String> = match operand.source {
                    OperandSource::Bookmarks => self.document.bookmark_names(),
                    OperandSource::Styles => self.document.paragraph_style_names(),
                    OperandSource::Properties => [
                        "Title",
                        "Subject",
                        "Author",
                        "Keywords",
                        "Comments",
                        "Company",
                        "Manager",
                        "Category",
                        "Pages",
                        "Words",
                        "Characters",
                        "CreateTime",
                        "LastSaveTime",
                    ]
                    .iter()
                    .map(|name| name.to_string())
                    .chain(
                        self.document
                            .document_properties()
                            .custom
                            .into_iter()
                            .map(|property| property.name),
                    )
                    .collect(),
                    OperandSource::Variables => self
                        .document
                        .document_variables()
                        .into_iter()
                        .map(|(name, _)| name)
                        .collect(),
                    OperandSource::Choice(choices) => {
                        choices.iter().map(|choice| choice.to_string()).collect()
                    }
                    OperandSource::Free => Vec::new(),
                };
                let mut row = div().flex().flex_wrap().items_center().gap_1().child(
                    div()
                        .w(px(120.0))
                        .text_color(theme::text_dim())
                        .child(i18n::text(operand.key)),
                );
                let quoted = operand.quoted;
                for (value_index, value) in values.into_iter().take(24).enumerate() {
                    let label = value.clone();
                    row = row.child(
                        chip(
                            ("field-operand", operand_index * 100 + value_index),
                            label,
                            false,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            let value = value.clone();
                            view.edit_field_dialog(cx, |dialog| {
                                dialog.code = TextEdit::new(set_operand(
                                    &dialog.code.text,
                                    operand_index,
                                    &value,
                                    quoted,
                                ));
                            })
                        })),
                    );
                }
                options.push(row.into_any_element());
            }
            for (switch_index, switch) in spec.switches.iter().enumerate() {
                let name = switch.switch;
                let mut row = div().flex().flex_wrap().items_center().gap_1().child(
                    div()
                        .w(px(120.0))
                        .text_color(theme::text_dim())
                        .child(format!("{name} {}", i18n::text(switch.key))),
                );
                match switch.argument {
                    SwitchArgument::Choice(choices) => {
                        for (choice_index, choice) in choices.iter().enumerate() {
                            let on = code_has_switch(&code, name, Some(choice));
                            let label = if name == "\\@" {
                                // A picture shows as what it gives today.
                                let (now, _) = local_now();
                                format!(
                                    "{}",
                                    jade_docx_editor::field_datetime::format_word_date(
                                        choice, &now
                                    )
                                )
                            } else {
                                choice.to_string()
                            };
                            row = row.child(
                                chip(
                                    ("field-switch", switch_index * 100 + choice_index),
                                    label,
                                    on,
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        view.edit_field_dialog(cx, |dialog| {
                                            dialog.code = TextEdit::new(toggle_switch(
                                                &dialog.code.text,
                                                name,
                                                Some(choice),
                                            ));
                                        })
                                    },
                                )),
                            );
                        }
                    }
                    SwitchArgument::None => {
                        let on = code_has_switch(&code, name, None);
                        row = row.child(
                            chip(
                                ("field-switch", switch_index * 100),
                                i18n::text("field.switch.on"),
                                on,
                            )
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.edit_field_dialog(cx, |dialog| {
                                        dialog.code = TextEdit::new(toggle_switch(
                                            &dialog.code.text,
                                            name,
                                            None,
                                        ));
                                    })
                                },
                            )),
                        );
                    }
                    SwitchArgument::Text | SwitchArgument::Number => {
                        let on = code_has_switch(&code, name, None);
                        row = row.child(
                            chip(
                                ("field-switch", switch_index * 100),
                                i18n::text("field.switch.add"),
                                on,
                            )
                            .on_click(cx.listener(
                                move |view, _, _, cx| {
                                    view.edit_field_dialog(cx, |dialog| {
                                        let placeholder = match switch.argument {
                                            SwitchArgument::Number => "1",
                                            _ => "",
                                        };
                                        let value =
                                            (!code_has_switch(&dialog.code.text, name, None))
                                                .then_some(placeholder);
                                        dialog.code = TextEdit::new(toggle_switch(
                                            &dialog.code.text,
                                            name,
                                            value,
                                        ));
                                    })
                                },
                            )),
                        );
                    }
                }
                options.push(row.into_any_element());
            }
        }
        let preserve = code_has_switch(&code, "\\*", Some("MERGEFORMAT"));
        let title = if dialog.editing {
            "field.dialog.edit_title"
        } else {
            "field.dialog.title"
        };
        backdrop(
            panel("field-dialog", 760.0)
                .child(div().text_sm().child(i18n::text(title)))
                .child(
                    div()
                        .flex()
                        .gap_3()
                        .h(px(260.0))
                        .child(
                            div()
                                .w(px(150.0))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(theme::text_dim())
                                        .child(i18n::text("field.dialog.categories")),
                                )
                                .child(
                                    div()
                                        .id("field-categories")
                                        .flex()
                                        .flex_col()
                                        .overflow_y_scroll()
                                        .children(categories),
                                ),
                        )
                        .child(
                            div()
                                .w(px(150.0))
                                .flex()
                                .flex_col()
                                .child(
                                    div()
                                        .text_color(theme::text_dim())
                                        .child(i18n::text("field.dialog.names")),
                                )
                                .child(
                                    div()
                                        .id("field-names")
                                        .flex()
                                        .flex_col()
                                        .overflow_y_scroll()
                                        .children(names),
                                ),
                        )
                        .child(
                            div()
                                .id("field-options")
                                .flex_1()
                                .flex()
                                .flex_col()
                                .gap_1()
                                .overflow_y_scroll()
                                .child(
                                    div()
                                        .text_color(theme::text_dim())
                                        .child(i18n::text("field.dialog.options")),
                                )
                                .children(options),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_color(theme::text_dim())
                                .child(i18n::text("field.dialog.code")),
                        )
                        .child(input("field-code", &dialog.code, true)),
                )
                .child(
                    chip(
                        "field-preserve",
                        i18n::text("field.dialog.preserve"),
                        preserve,
                    )
                    .on_click(cx.listener(|view, _, _, cx| {
                        view.edit_field_dialog(cx, |dialog| {
                            dialog.code = TextEdit::new(toggle_switch(
                                &dialog.code.text,
                                "\\*",
                                Some("MERGEFORMAT"),
                            ));
                        })
                    })),
                )
                .child(self.dialog_buttons(cx)),
        )
    }

    fn prompt_dialog_element(&self, dialog: &PromptDialog, cx: &mut Context<Self>) -> AnyElement {
        let prompt = &dialog.prompts[dialog.index];
        let title = if prompt.field == "ASK" {
            "field.prompt.ask"
        } else {
            "field.prompt.fillin"
        };
        backdrop(
            panel("field-prompt", 420.0)
                .child(div().text_sm().child(i18n::text(title)))
                .child(div().child(prompt.prompt.clone()))
                .child(input("field-prompt-input", &dialog.input, true))
                .child(div().text_color(theme::text_faint()).child(i18n::format(
                    "field.prompt.progress",
                    &[
                        ("index", &(dialog.index + 1)),
                        ("count", &dialog.prompts.len()),
                    ],
                )))
                .child(self.dialog_buttons(cx)),
        )
    }

    fn properties_dialog_element(
        &self,
        dialog: &PropertiesDialog,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let tabs: Vec<AnyElement> = [
            (PropertiesTab::Summary, "field.properties.summary"),
            (PropertiesTab::Statistics, "field.properties.statistics"),
            (PropertiesTab::Custom, "field.properties.custom"),
            (PropertiesTab::User, "field.properties.user"),
        ]
        .into_iter()
        .enumerate()
        .map(|(index, (tab, key))| {
            chip(
                ("properties-tab", index),
                i18n::text(key),
                dialog.tab == tab,
            )
            .on_click(cx.listener(move |view, _, _, cx| {
                view.edit_properties_dialog(cx, |dialog| {
                    dialog.tab = tab;
                    dialog.focus = 0;
                })
            }))
            .into_any_element()
        })
        .collect();
        let labelled = |label: &'static str, content: AnyElement| {
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .w(px(130.0))
                        .text_color(theme::text_dim())
                        .child(i18n::text(label)),
                )
                .child(div().flex_1().child(content))
                .into_any_element()
        };
        let body: Vec<AnyElement> = match dialog.tab {
            PropertiesTab::Summary => SUMMARY
                .iter()
                .enumerate()
                .map(|(index, key)| {
                    labelled(
                        key,
                        input(
                            ("properties-summary", index),
                            &dialog.summary[index],
                            dialog.focus == index,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.edit_properties_dialog(cx, |dialog| dialog.focus = index)
                        }))
                        .into_any_element(),
                    )
                })
                .collect(),
            PropertiesTab::Statistics => {
                let properties = &dialog.properties;
                let date = |value: &str| {
                    jade_docx_editor::field_datetime::parse_core_property_datetime(value)
                        .map(|utc| {
                            let (_, offset) = local_now();
                            let local = jade_docx_editor::field_datetime::datetime_from_unix(
                                jade_docx_editor::field_datetime::unix_seconds(&utc),
                                offset,
                            );
                            jade_docx_editor::field_datetime::format_word_date(
                                "yyyy/MM/dd H:mm",
                                &local,
                            )
                        })
                        .unwrap_or_else(|| "—".to_string())
                };
                let words = self.document.page_count();
                [
                    ("field.properties.created", date(&properties.created)),
                    ("field.properties.modified", date(&properties.modified)),
                    ("field.properties.printed", date(&properties.last_printed)),
                    (
                        "field.properties.last_saved_by",
                        properties.last_saved_by.clone(),
                    ),
                    ("field.properties.revision", properties.revision.clone()),
                    ("field.properties.total_time", properties.total_time.clone()),
                    ("field.properties.pages", words.to_string()),
                    ("field.properties.words", properties.words.clone()),
                    ("field.properties.characters", properties.characters.clone()),
                ]
                .into_iter()
                .map(|(key, value)| labelled(key, div().child(value).into_any_element()))
                .collect()
            }
            PropertiesTab::Custom => {
                let mut rows: Vec<AnyElement> = dialog
                    .properties
                    .custom
                    .iter()
                    .enumerate()
                    .map(|(index, property)| {
                        let name = property.name.clone();
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(150.0)).child(property.name.clone()))
                            .child(div().flex_1().child(property.value.display()))
                            .child(
                                chip(
                                    ("custom-remove", index),
                                    i18n::text("field.properties.remove"),
                                    false,
                                )
                                .on_click(cx.listener(
                                    move |view, _, _, cx| {
                                        let name = name.clone();
                                        view.edit_properties_dialog(cx, |dialog| {
                                            dialog.properties.custom.retain(|p| p.name != name)
                                        })
                                    },
                                )),
                            )
                            .into_any_element()
                    })
                    .collect();
                rows.push(labelled(
                    "field.properties.custom_name",
                    input("custom-name", &dialog.custom_name, dialog.focus == 0)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_properties_dialog(cx, |dialog| dialog.focus = 0)
                        }))
                        .into_any_element(),
                ));
                let kinds: Vec<AnyElement> = [
                    (CustomKind::Text, "field.properties.kind_text"),
                    (CustomKind::Number, "field.properties.kind_number"),
                    (CustomKind::Date, "field.properties.kind_date"),
                    (CustomKind::YesNo, "field.properties.kind_yes_no"),
                ]
                .into_iter()
                .enumerate()
                .map(|(index, (kind, key))| {
                    chip(
                        ("custom-kind", index),
                        i18n::text(key),
                        dialog.custom_kind == kind,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_properties_dialog(cx, |dialog| dialog.custom_kind = kind)
                    }))
                    .into_any_element()
                })
                .collect();
                rows.push(labelled(
                    "field.properties.custom_kind",
                    div().flex().gap_1().children(kinds).into_any_element(),
                ));
                rows.push(labelled(
                    "field.properties.custom_value",
                    input("custom-value", &dialog.custom_value, dialog.focus == 1)
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.edit_properties_dialog(cx, |dialog| dialog.focus = 1)
                        }))
                        .into_any_element(),
                ));
                rows.push(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            button("custom-add", "field.properties.add", false).on_click(
                                cx.listener(|view, _, _, cx| {
                                    let mut message = None;
                                    view.edit_properties_dialog(cx, |dialog| {
                                        message = dialog.add_custom().err();
                                    });
                                    view.fields.status =
                                        message.map(|key| SharedString::from(i18n::text(key)));
                                }),
                            ),
                        )
                        .into_any_element(),
                );
                rows
            }
            PropertiesTab::User => [
                "field.properties.user_name",
                "field.properties.user_initials",
                "field.properties.user_address",
            ]
            .iter()
            .enumerate()
            .map(|(index, key)| {
                labelled(
                    key,
                    input(
                        ("properties-user", index),
                        &dialog.user[index],
                        dialog.focus == index,
                    )
                    .on_click(cx.listener(move |view, _, _, cx| {
                        view.edit_properties_dialog(cx, |dialog| dialog.focus = index)
                    }))
                    .into_any_element(),
                )
            })
            .collect(),
        };
        backdrop(
            panel("properties-dialog", 560.0)
                .child(
                    div()
                        .text_sm()
                        .child(i18n::text("field.properties.title_dialog")),
                )
                .child(div().flex().gap_1().children(tabs))
                .child(
                    div()
                        .min_h(px(260.0))
                        .flex()
                        .flex_col()
                        .gap_2()
                        .children(body),
                )
                .children(
                    self.fields
                        .status
                        .clone()
                        .map(|status| div().text_color(theme::text_faint()).child(status)),
                )
                .child(self.dialog_buttons(cx)),
        )
    }

    /// 挿入's テキスト group: フィールド, 日付と時刻, 文書のプロパティ.
    pub(crate) fn fields_group(&mut self, cx: &mut Context<Self>) -> AnyElement {
        Self::ribbon_group(
            i18n::text("field.group"),
            vec![Self::ribbon_row(vec![
                self.ribbon_button(
                    "insert-field",
                    i18n::text("field.insert"),
                    i18n::text("field.insert_title"),
                    false,
                    cx,
                    |view, cx| view.open_field_dialog(None, cx),
                )
                .into_any_element(),
                self.ribbon_button(
                    "insert-date",
                    i18n::text("field.date_time"),
                    i18n::text("field.date_time_title"),
                    false,
                    cx,
                    |view, cx| view.open_field_dialog(Some("DATE"), cx),
                )
                .into_any_element(),
                self.ribbon_button(
                    "update-fields",
                    i18n::text("field.update"),
                    i18n::text("field.update_title"),
                    false,
                    cx,
                    |view, cx| view.update_fields(cx),
                )
                .into_any_element(),
                self.ribbon_button(
                    "document-properties",
                    i18n::text("field.properties"),
                    i18n::text("field.properties_title"),
                    false,
                    cx,
                    |view, cx| view.open_properties_dialog(cx),
                )
                .into_any_element(),
            ])],
        )
        .into_any_element()
    }

    /// 表示's フィールド コード toggle.
    pub(crate) fn field_codes_button(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let on = self.document.field_code_view().all;
        self.ribbon_button(
            "field-codes",
            i18n::text("field.codes"),
            i18n::text("field.codes_title"),
            on,
            cx,
            |view, cx| view.toggle_all_field_codes(cx),
        )
        .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DocumentProperties as PropertiesAction, InsertField, LockFields, ToggleFieldCodes, Undo,
        UnlinkFields, UpdateFields,
    };
    use gpui::Focusable;
    use jade_office_model::word::fields::{SelectionRange, TextPosition};

    fn open(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &mut gpui::VisualTestContext) {
        let path = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../jade-core/contract.docx"
        ));
        let (view, cx) =
            cx.add_window_view(|_window, cx| crate::open_document_view(&path, cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        (view, cx)
    }

    /// The fields in paragraph `paragraph`, selecting it.
    fn fields_in_paragraph(
        view: &mut DocumentView,
        paragraph: usize,
    ) -> Vec<jade_office_model::word::fields::FieldInfo> {
        let length = view
            .document
            .paragraph_text(paragraph)
            .map_or(0, |text| text.len());
        view.document.set_selection_range(Some(SelectionRange::new(
            TextPosition::new(paragraph, 0),
            TextPosition::new(paragraph, length),
        )));
        view.document.selected_fields()
    }

    /// ⌘F9 opens the Field dialog; the typed code goes in with Enter, F9
    /// updates it, ⌥F9 shows codes, ⌘F11 locks, ⌘⇧F9 unlinks, and undo
    /// brings each back.
    #[gpui::test]
    fn the_field_dialog_and_the_field_keys(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        let paragraph = view.update(cx, |view, _| {
            let paragraph = view.insert_at.0;
            view.document.place_caret(paragraph, 0);
            paragraph
        });
        cx.dispatch_action(InsertField);
        view.read_with(cx, |view, _| {
            assert!(matches!(view.fields.dialog, Some(FieldsDialog::Field(_))));
        });
        cx.simulate_input("QUOTE \"jade\" \\* Upper");
        cx.simulate_keystrokes("enter");
        view.update(cx, |view, _| {
            assert!(view.fields.dialog.is_none());
            let fields = fields_in_paragraph(view, paragraph);
            assert_eq!(fields.len(), 1, "{fields:?}");
            assert_eq!(fields[0].code, "QUOTE \"jade\" \\* Upper");
            assert_eq!(fields[0].result, "JADE");
        });

        cx.dispatch_action(UpdateFields);
        view.update(cx, |view, _| {
            assert_eq!(fields_in_paragraph(view, paragraph)[0].result, "JADE");
        });

        cx.dispatch_action(ToggleFieldCodes);
        view.read_with(cx, |view, _| assert!(view.document.field_code_view().all));
        cx.dispatch_action(ToggleFieldCodes);
        view.read_with(cx, |view, _| assert!(!view.document.field_code_view().all));

        view.update(cx, |view, _| {
            fields_in_paragraph(view, paragraph);
        });
        cx.dispatch_action(LockFields);
        view.update(cx, |view, _| {
            assert!(fields_in_paragraph(view, paragraph)[0].locked);
        });
        cx.dispatch_action(UnlinkFields);
        view.update(cx, |view, _| {
            assert!(fields_in_paragraph(view, paragraph).is_empty());
            assert!(view
                .document
                .paragraph_text(paragraph)
                .unwrap()
                .starts_with("JADE"));
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, _| {
            assert_eq!(fields_in_paragraph(view, paragraph).len(), 1);
        });
    }

    /// ファイル › プロパティ: the title typed lands in the document, where a
    /// TITLE field reads it.
    #[gpui::test]
    fn the_properties_dialog_feeds_the_title_field(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        cx.dispatch_action(PropertiesAction);
        view.update(cx, |view, _| {
            let Some(FieldsDialog::Properties(dialog)) = view.fields.dialog.as_mut() else {
                panic!("the Properties dialog is open");
            };
            dialog.summary[0] = TextEdit::new(String::new());
            dialog.focus = 0;
        });
        cx.simulate_input("業務委託契約");
        cx.simulate_keystrokes("enter");
        let paragraph = view.update(cx, |view, cx| {
            assert!(view.fields.dialog.is_none());
            assert_eq!(view.document.document_properties().title, "業務委託契約");
            let paragraph = view.insert_at.0;
            view.document.place_caret(paragraph, 0);
            let host = view.field_host();
            view.document.insert_field_code("TITLE", &host).unwrap();
            cx.notify();
            paragraph
        });
        view.update(cx, |view, _| {
            assert_eq!(
                fields_in_paragraph(view, paragraph)[0].result,
                "業務委託契約"
            );
        });
    }
}
