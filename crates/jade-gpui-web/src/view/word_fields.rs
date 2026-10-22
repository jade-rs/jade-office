//! Word's fields in the browser shell — the copy of the native app's
//! `word_fields`, over the same `jade-office-model` `WordFields`: 挿入 ›
//! フィールド (the Field dialog — categories, field names, the code, its
//! options) and フィールドの編集, the prompts ASK and FILLIN show on F9, 文書の
//! プロパティ (summary, statistics, custom properties) with the user
//! information USERNAME and its kin read, and the field keys — ⌘F9 the
//! dialog, F9 update (every story when all is selected), ⌥F9/⇧F9 codes,
//! ⌘F11/⌘⇧F11 lock, ⌘⇧F9 unlink. DATE and TIME take the browser's clock;
//! the user information is kept in the browser's storage.

use std::collections::HashMap;
use std::path::PathBuf;

use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, SharedString, Window};
use jade_web::text::TextEdit;
use jade_web::word::fields::{
    build_code, code_field_name, code_has_switch, field_spec, fields_in, set_operand,
    toggle_switch, CustomProperty, CustomValue, DocumentProperties, FieldCategory, FieldDateTime,
    FieldHost, FieldPrompt, FieldSpec, OperandSource, SwitchArgument, WordFields,
};

use super::word_kit::{button, chip, input, modal, ok_cancel, on};
use super::{DocView as DocumentView, Document};
use crate::i18n;
use crate::palette as theme;

/// The browser's local time and its offset from UTC.
pub(crate) fn browser_now() -> (FieldDateTime, i64) {
    let now = js_sys::Date::new_0();
    (
        FieldDateTime {
            year: now.get_full_year() as i32,
            month: now.get_month() + 1,
            day: now.get_date(),
            hour: now.get_hours(),
            minute: now.get_minutes(),
            second: now.get_seconds(),
            weekday: now.get_day(),
        },
        // getTimezoneOffset is minutes *behind* UTC.
        -(now.get_timezone_offset() as i64) * 60,
    )
}

/// The page's `localStorage`, read and written by key; nothing when the
/// page has none (a private window that refuses it).
pub(crate) mod storage {
    use wasm_bindgen::{JsCast, JsValue};

    fn local() -> Option<js_sys::Object> {
        let global = js_sys::global();
        js_sys::Reflect::get(&global, &JsValue::from_str("localStorage"))
            .ok()
            .and_then(|value| value.dyn_into::<js_sys::Object>().ok())
    }

    fn call(name: &str, arguments: &[JsValue]) -> Option<JsValue> {
        let storage = local()?;
        let method = js_sys::Reflect::get(&storage, &JsValue::from_str(name))
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()?;
        let arguments = arguments.iter().cloned().collect::<js_sys::Array>();
        method.apply(&storage, &arguments).ok()
    }

    pub fn get(key: &str) -> Option<String> {
        call("getItem", &[JsValue::from_str(key)])?.as_string()
    }

    pub fn set(key: &str, value: &str) {
        call(
            "setItem",
            &[JsValue::from_str(key), JsValue::from_str(value)],
        );
    }
}

/// The user information fields read (Word's 環境設定 › ユーザー情報).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct UserInfo {
    pub name: String,
    pub initials: String,
    pub address: String,
}

const USER_KEY: &str = "jade-office.user";

impl UserInfo {
    /// What was saved in this browser.
    pub fn load() -> Self {
        let Some(text) = storage::get(USER_KEY) else {
            return Self::default();
        };
        let mut lines = text.lines().map(|line| line.replace("\\n", "\n"));
        Self {
            name: lines.next().unwrap_or_default(),
            initials: lines.next().unwrap_or_default(),
            address: lines.next().unwrap_or_default(),
        }
    }

    pub fn save(&self) {
        let text = [&self.name, &self.initials, &self.address]
            .iter()
            .map(|value| value.replace('\n', "\\n"))
            .collect::<Vec<_>>()
            .join("\n");
        storage::set(USER_KEY, &text);
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

/// 文書のプロパティ.
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
    Properties(Box<PropertiesDialog>),
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

/// The view's field state: an open dialog and the user information.
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

impl FieldsUi {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    /// Where typing goes while a dialog is open.
    pub fn text_field(&self) -> Option<&TextEdit> {
        self.dialog.as_ref()?.text()
    }

    pub fn text_field_mut(&mut self) -> Option<&mut TextEdit> {
        self.dialog.as_mut()?.text_mut()
    }
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

impl DocumentView {
    /// What field evaluation needs from the shell.
    pub(crate) fn field_host(&self) -> FieldHost {
        let (now, offset) = browser_now();
        FieldHost {
            now,
            utc_offset_seconds: offset,
            user_name: self.fields.user.name.clone(),
            user_initials: self.fields.user.initials.clone(),
            user_address: self.fields.user.address.clone(),
            file: Some(PathBuf::from(self.name.to_string())),
            japanese: !i18n::is_english(),
            // Merge fields show the recipient 結果のプレビュー shows.
            merge: self
                .mailings
                .as_ref()
                .and_then(|mailings| mailings.merge.previewed()),
        }
    }

    /// After a field command: the pages again, the caret where the command
    /// left the selection.
    fn after_field_command(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.fields.status = result.as_ref().err().map(|error| error.clone().into());
        self.refresh_repainted();
        self.word_result(result, cx);
    }

    /// 挿入 › フィールド (⌘F9).
    pub(crate) fn open_field_dialog(&mut self, preset: Option<&str>, cx: &mut Context<Self>) {
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
        let Some(field) = self.doc().selected_fields().into_iter().next() else {
            self.fields.status = Some(SharedString::from(i18n::text("field.status.none")));
            self.status = i18n::text("field.status.none").into();
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

    /// 文書のプロパティ.
    pub(crate) fn open_properties_dialog(&mut self, cx: &mut Context<Self>) {
        let properties = self.doc().document_properties();
        self.fields.dialog = Some(FieldsDialog::Properties(Box::new(PropertiesDialog::open(
            properties,
            &self.fields.user,
        ))));
        cx.notify();
    }

    /// F9: the selection's fields, or every field when the document is
    /// selected — after asking what ASK and FILLIN want to know.
    pub(crate) fn update_fields(&mut self, cx: &mut Context<Self>) {
        let scope = self.doc().field_scope();
        let all = matches!(scope, jade_web::word::fields::FieldScope::Document);
        let prompts = self.doc().field_prompts(scope);
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
        let result = {
            let document = self.doc_mut();
            // DATE and TIME show the clock of this update.
            document.set_field_datetime(Some(host.now));
            let scope = if all {
                jade_web::word::fields::FieldScope::Document
            } else {
                document.field_scope()
            };
            document.update_fields(scope, &host, answers)
        };
        self.after_field_command(result, cx);
    }

    /// ⌥F9: every field's code, or every result.
    pub(crate) fn toggle_all_field_codes(&mut self, cx: &mut Context<Self>) {
        let mut view = self.doc().field_code_view().clone();
        view.all = !view.all;
        view.toggled.clear();
        let result = self.doc_mut().set_field_code_view(view);
        self.after_view_change(result, cx);
    }

    /// ⇧F9: the selected fields' codes, or their results.
    pub(crate) fn toggle_selected_field_codes(&mut self, cx: &mut Context<Self>) {
        let view = self.doc().toggled_field_codes(self.doc().field_code_view());
        let result = self.doc_mut().set_field_code_view(view);
        self.after_view_change(result, cx);
    }

    fn after_view_change(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        self.refresh_repainted();
        if let Err(error) = result {
            self.status = error.into();
        }
        // Not an edit: nothing unsaved, but the pages show something else.
        self.generation += 1;
        cx.notify();
    }

    /// ⌘F11 / ⌘⇧F11.
    pub(crate) fn lock_fields(&mut self, locked: bool, cx: &mut Context<Self>) {
        let result = WordFields::lock_fields(self.doc_mut(), locked);
        self.after_field_command(result, cx);
    }

    /// ⌘⇧F9.
    pub(crate) fn unlink_fields(&mut self, cx: &mut Context<Self>) {
        let result = WordFields::unlink_fields(self.doc_mut());
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
                    self.doc_mut().edit_field_code(&code, &host)
                } else {
                    self.doc_mut().insert_field_code(&code, &host)
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
                if properties != self.doc().document_properties() {
                    let result = self.doc_mut().set_document_properties(properties);
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

    /// Runs a field key over a Word document; `false` when the key is none.
    pub(super) fn field_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !matches!(self.document, Document::Pages { .. }) {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        let command = modifiers.platform || modifiers.control;
        match (
            event.keystroke.key.as_str(),
            command,
            modifiers.shift,
            modifiers.alt,
        ) {
            ("f9", false, false, false) => self.update_fields(cx),
            ("f9", true, false, false) => self.open_field_dialog(None, cx),
            ("f9", false, false, true) => self.toggle_all_field_codes(cx),
            ("f9", false, true, false) => self.toggle_selected_field_codes(cx),
            ("f11", true, shift, false) => self.lock_fields(!shift, cx),
            ("f9", true, true, false) => self.unlink_fields(cx),
            _ => return false,
        }
        true
    }

    /// The open dialog, over the window.
    pub(crate) fn fields_dialog_element(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let dialog = self.fields.dialog.as_ref()?;
        let buttons = ok_cancel(
            "fields",
            cx,
            |view, cx| {
                view.fields.dialog = None;
                cx.notify();
            },
            |view, cx| view.confirm_fields_dialog(cx),
        );
        let status = self.fields.status.clone();
        Some(match dialog {
            FieldsDialog::Field(dialog) => {
                let title = if dialog.editing {
                    i18n::text("field.dialog.edit_title")
                } else {
                    i18n::text("field.dialog.title")
                };
                let body = self.field_dialog_body(dialog, cx);
                modal("field-dialog", 760.0, title, body, None, buttons, window)
            }
            FieldsDialog::Prompt(dialog) => {
                let prompt = &dialog.prompts[dialog.index];
                let title = if prompt.field == "ASK" {
                    i18n::text("field.prompt.ask")
                } else {
                    i18n::text("field.prompt.fillin")
                };
                let body = div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(div().child(prompt.prompt.clone()))
                    .child(input("field-prompt-input", &dialog.input, true))
                    .child(div().text_color(theme::text_faint()).child(i18n::format(
                        "field.prompt.progress",
                        &[
                            ("index", &(dialog.index + 1)),
                            ("count", &dialog.prompts.len()),
                        ],
                    )))
                    .into_any_element();
                modal("field-prompt", 420.0, title, body, None, buttons, window)
            }
            FieldsDialog::Properties(dialog) => {
                let body = self.properties_dialog_body(dialog, cx);
                modal(
                    "properties-dialog",
                    560.0,
                    i18n::text("field.properties.title_dialog"),
                    body,
                    status,
                    buttons,
                    window,
                )
            }
        })
    }

    fn field_dialog_body(&self, dialog: &FieldDialog, cx: &mut Context<Self>) -> AnyElement {
        let mut categories = vec![list_row(
            "field-category-all",
            i18n::text("field.category.all"),
            dialog.category.is_none(),
        )
        .on_click(on(cx, |view, cx| {
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
                .on_click(on(cx, move |view, cx| {
                    view.edit_field_dialog(cx, |dialog| dialog.category = Some(category))
                }))
                .into_any_element(),
            );
        }
        let names: Vec<AnyElement> = fields_in(dialog.category)
            .into_iter()
            .enumerate()
            .map(|(index, spec)| {
                let on_now = dialog.field.is_some_and(|field| field.name == spec.name);
                list_row(("field-name", index), spec.name, on_now)
                    .on_click(on(cx, move |view, cx| {
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
        let document = self.doc();
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
                    OperandSource::Bookmarks => document.bookmark_names(),
                    OperandSource::Styles => document.paragraph_style_names(),
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
                        document
                            .document_properties()
                            .custom
                            .into_iter()
                            .map(|property| property.name),
                    )
                    .collect(),
                    OperandSource::Variables => document
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
                        .on_click(on(cx, move |view, cx| {
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
                            let on_now = code_has_switch(&code, name, Some(choice));
                            let label = if name == "\\@" {
                                // A picture shows as what it gives today.
                                let (now, _) = browser_now();
                                jade_web::word::fields::format_word_date(choice, &now)
                            } else {
                                choice.to_string()
                            };
                            row = row.child(
                                chip(
                                    ("field-switch", switch_index * 100 + choice_index),
                                    label,
                                    on_now,
                                )
                                .on_click(on(
                                    cx,
                                    move |view, cx| {
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
                        let on_now = code_has_switch(&code, name, None);
                        row = row.child(
                            chip(
                                ("field-switch", switch_index * 100),
                                i18n::text("field.switch.on"),
                                on_now,
                            )
                            .on_click(on(cx, move |view, cx| {
                                view.edit_field_dialog(cx, |dialog| {
                                    dialog.code =
                                        TextEdit::new(toggle_switch(&dialog.code.text, name, None));
                                })
                            })),
                        );
                    }
                    SwitchArgument::Text | SwitchArgument::Number => {
                        let on_now = code_has_switch(&code, name, None);
                        row = row.child(
                            chip(
                                ("field-switch", switch_index * 100),
                                i18n::text("field.switch.add"),
                                on_now,
                            )
                            .on_click(on(cx, move |view, cx| {
                                view.edit_field_dialog(cx, |dialog| {
                                    let placeholder = match switch.argument {
                                        SwitchArgument::Number => "1",
                                        _ => "",
                                    };
                                    let value = (!code_has_switch(&dialog.code.text, name, None))
                                        .then_some(placeholder);
                                    dialog.code = TextEdit::new(toggle_switch(
                                        &dialog.code.text,
                                        name,
                                        value,
                                    ));
                                })
                            })),
                        );
                    }
                }
                options.push(row.into_any_element());
            }
        }
        let preserve = code_has_switch(&code, "\\*", Some("MERGEFORMAT"));
        let column = |id: &'static str, caption: &'static str, rows: Vec<AnyElement>| {
            div()
                .w(px(150.0))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_color(theme::text_dim())
                        .child(i18n::text(caption)),
                )
                .child(
                    div()
                        .id(id)
                        .flex()
                        .flex_col()
                        .overflow_y_scroll()
                        .children(rows),
                )
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .gap_3()
                    .h(px(260.0))
                    .child(column(
                        "field-categories",
                        "field.dialog.categories",
                        categories,
                    ))
                    .child(column("field-names", "field.dialog.names", names))
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
                .on_click(on(cx, |view, cx| {
                    view.edit_field_dialog(cx, |dialog| {
                        dialog.code = TextEdit::new(toggle_switch(
                            &dialog.code.text,
                            "\\*",
                            Some("MERGEFORMAT"),
                        ));
                    })
                })),
            )
            .into_any_element()
    }

    fn properties_dialog_body(
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
            .on_click(on(cx, move |view, cx| {
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
                        .on_click(on(cx, move |view, cx| {
                            view.edit_properties_dialog(cx, |dialog| dialog.focus = index)
                        }))
                        .into_any_element(),
                    )
                })
                .collect(),
            PropertiesTab::Statistics => {
                let properties = &dialog.properties;
                let date = |value: &str| {
                    use jade_web::word::fields::{
                        datetime_from_unix, format_word_date, parse_core_property_datetime,
                        unix_seconds,
                    };
                    parse_core_property_datetime(value)
                        .map(|utc| {
                            let (_, offset) = browser_now();
                            let local = datetime_from_unix(unix_seconds(&utc), offset);
                            format_word_date("yyyy/MM/dd H:mm", &local)
                        })
                        .unwrap_or_else(|| "—".to_string())
                };
                let pages = self.doc().page_count();
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
                    ("field.properties.pages", pages.to_string()),
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
                                .on_click(on(
                                    cx,
                                    move |view, cx| {
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
                        .on_click(on(cx, |view, cx| {
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
                    .on_click(on(cx, move |view, cx| {
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
                        .on_click(on(cx, |view, cx| {
                            view.edit_properties_dialog(cx, |dialog| dialog.focus = 1)
                        }))
                        .into_any_element(),
                ));
                rows.push(
                    div()
                        .flex()
                        .justify_end()
                        .child(
                            button("custom-add", i18n::text("field.properties.add"), false)
                                .on_click(on(cx, |view, cx| {
                                    let mut message = None;
                                    view.edit_properties_dialog(cx, |dialog| {
                                        message = dialog.add_custom().err();
                                    });
                                    view.fields.status =
                                        message.map(|key| SharedString::from(i18n::text(key)));
                                })),
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
                    .on_click(on(cx, move |view, cx| {
                        view.edit_properties_dialog(cx, |dialog| dialog.focus = index)
                    }))
                    .into_any_element(),
                )
            })
            .collect(),
        };
        div()
            .flex()
            .flex_col()
            .gap_3()
            .child(div().flex().gap_1().children(tabs))
            .child(
                div()
                    .min_h(px(260.0))
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(body),
            )
            .into_any_element()
    }
}

/// 挿入's テキスト group: フィールド, 日付と時刻, フィールドの更新, 文書の
/// プロパティ and フィールド コード.
pub fn fields_group(codes: bool, cx: &mut Context<DocumentView>) -> AnyElement {
    let button = |id: &'static str,
                  label: &'static str,
                  tooltip: &'static str,
                  active: bool,
                  run: fn(&mut DocumentView, &mut Context<DocumentView>),
                  cx: &mut Context<DocumentView>| {
        div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .h(px(26.0))
            .px_2()
            .rounded_md()
            .text_xs()
            .text_color(theme::text())
            .whitespace_nowrap()
            .cursor_pointer()
            .when(active, |button| button.bg(theme::selected()))
            .hover(|button| button.bg(theme::hovered()))
            .tooltip(crate::tooltip::Tooltip::text(tooltip))
            .on_click(on(cx, move |view, cx| {
                if let Document::Pages { ribbon, .. } = &mut view.document {
                    ribbon.popup = None;
                }
                run(view, cx)
            }))
            .child(label)
            .into_any_element()
    };
    let t = i18n::text;
    crate::ribbon::group(
        t("field.group"),
        vec![
            crate::ribbon::row(vec![
                button(
                    "insert-field",
                    t("field.insert"),
                    t("field.insert_title"),
                    false,
                    |view, cx| view.open_field_dialog(None, cx),
                    cx,
                ),
                button(
                    "edit-field",
                    t("field.dialog.edit_title"),
                    t("field.dialog.edit_title"),
                    false,
                    |view, cx| view.open_edit_field_dialog(cx),
                    cx,
                ),
                button(
                    "insert-date",
                    t("field.date_time"),
                    t("field.date_time_title"),
                    false,
                    |view, cx| view.open_field_dialog(Some("DATE"), cx),
                    cx,
                ),
            ]),
            crate::ribbon::row(vec![
                button(
                    "update-fields",
                    t("field.update"),
                    t("field.update_title"),
                    false,
                    |view, cx| view.update_fields(cx),
                    cx,
                ),
                button(
                    "field-codes",
                    t("field.codes"),
                    t("field.codes_title"),
                    codes,
                    |view, cx| view.toggle_all_field_codes(cx),
                    cx,
                ),
                button(
                    "document-properties",
                    t("field.properties"),
                    t("field.properties_title"),
                    false,
                    |view, cx| view.open_properties_dialog(cx),
                    cx,
                ),
            ]),
        ],
    )
}
