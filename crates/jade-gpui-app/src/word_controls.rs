//! The 開発 tab and content controls on the pages, as Word has them: the
//! controls put in from the ribbon, the box (and title tab, and drop button)
//! drawn round the one the caret is in, design mode's tags, a click that
//! ticks a box or drops a list or a calendar, typing over a placeholder,
//! the プロパティ dialogs, and the legacy form fields with protection for
//! filling them in. What each does is `jade-office-model`'s
//! (`word::controls`); the browser shell has the same
//! (`jade-gpui-web/src/view/word_controls.rs`).

use std::cell::RefCell;

use gpui::{div, prelude::*, px, AnyElement, Context, ElementId, SharedString, Window};
use jade_office_model::text::TextEdit;
use jade_office_model::word::controls::{
    character_styles, kind_key, ContentControlAction, ContentControlKind, ControlChrome,
    ControlClick, ControlPropertiesForm, DatePicker, FormFieldForm, FormFieldKind, WordControls,
    INSERTABLE,
};
use jade_office_model::word::form_view::{FormEvent, FormOutcome};
use jade_office_model::word::WordSession;

use crate::{i18n, theme, word_form, DocumentView};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// A control's dialog.
pub enum ControlsDialog {
    Properties(Box<ControlPropertiesForm>),
    FormField(Box<FormFieldForm>),
}

/// What drops under a control a click landed on.
pub enum ControlPopup {
    List {
        control: usize,
        entries: Vec<String>,
        selected: Option<usize>,
    },
    Date(DatePicker),
    Gallery {
        control: usize,
        blocks: Vec<String>,
    },
    FormList {
        field: usize,
        entries: Vec<String>,
        selected: usize,
    },
}

/// Where a popup hangs: the page and the point under the control, in page
/// coordinates.
#[derive(Clone, Copy)]
pub struct Anchor {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}

/// What was drawn, for the edit and the selection it was read at.
type ChromeCache = (
    u64,
    Option<((usize, usize), (usize, usize))>,
    Vec<ControlChrome>,
);

/// The view's share of the content controls.
#[derive(Default)]
pub struct ControlsUi {
    pub dialog: Option<ControlsDialog>,
    pub popup: Option<(ControlPopup, Anchor)>,
    /// フォーム フィールドの網かけ: whether form fields are shaded.
    pub hide_shading: bool,
    /// Whether the document has any control or form field, for the edit
    /// count it was read at: most have none, and then nothing is drawn.
    any: RefCell<Option<(u64, bool)>>,
    /// What to draw, for the edit count and selection it was read at.
    chrome: RefCell<Option<ChromeCache>>,
}

impl ControlsUi {
    pub fn is_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn field(&self) -> Option<&TextEdit> {
        match self.dialog.as_ref()? {
            ControlsDialog::Properties(form) => Some(form.field()),
            ControlsDialog::FormField(form) => Some(form.field()),
        }
    }

    pub fn field_mut(&mut self) -> Option<&mut TextEdit> {
        match self.dialog.as_mut()? {
            ControlsDialog::Properties(form) => Some(form.field_mut()),
            ControlsDialog::FormField(form) => Some(form.field_mut()),
        }
    }
}

fn hex(color: &str) -> gpui::Rgba {
    gpui::rgb(u32::from_str_radix(color, 16).unwrap_or(0x8EAADB))
}

impl DocumentView {
    /// The 開発 tab: コントロール, 従来のフォーム and 保護.
    pub(crate) fn developer_groups(&mut self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let design = self.document.design_mode();
        let protected = self.document.protected_for_forms();
        let shading = !self.controls.hide_shading;
        let mut first: Vec<AnyElement> = Vec::new();
        let mut second: Vec<AnyElement> = Vec::new();
        for (index, kind) in INSERTABLE.iter().enumerate() {
            let kind = kind.clone();
            let id: &'static str = [
                "cc-rich",
                "cc-plain",
                "cc-picture",
                "cc-gallery",
                "cc-check",
                "cc-combo",
                "cc-list",
                "cc-date",
                "cc-repeat",
            ][index];
            let label = t(kind_key(&kind));
            let button = self
                .ribbon_button(id, label, label, false, cx, move |view, cx| {
                    view.insert_control(kind.clone(), cx)
                })
                .into_any_element();
            if index < 5 {
                first.push(button);
            } else {
                second.push(button);
            }
        }
        second.extend([
            self.ribbon_button(
                "cc-design",
                t("cc.design_mode"),
                t("cc.design_mode_title"),
                design,
                cx,
                |view, cx| {
                    let on = !view.document.design_mode();
                    view.run_control(ContentControlAction::DesignMode(on), cx)
                },
            )
            .into_any_element(),
            self.ribbon_button(
                "cc-properties",
                t("cc.properties"),
                t("cc.properties_title"),
                false,
                cx,
                |view, cx| view.open_control_properties(cx),
            )
            .into_any_element(),
            self.ribbon_button(
                "cc-group",
                t("cc.group"),
                t("cc.group_title"),
                false,
                cx,
                |view, cx| view.insert_control(ContentControlKind::Group, cx),
            )
            .into_any_element(),
            self.ribbon_button(
                "cc-ungroup",
                t("cc.ungroup"),
                t("cc.ungroup_title"),
                false,
                cx,
                |view, cx| view.ungroup(cx),
            )
            .into_any_element(),
        ]);
        vec![
            Self::ribbon_group(
                t("cc.group.controls"),
                vec![Self::ribbon_row(first), Self::ribbon_row(second)],
            )
            .into_any_element(),
            Self::ribbon_group(
                t("form.group"),
                vec![
                    Self::ribbon_row(vec![
                        self.ribbon_button(
                            "form-text",
                            t("form.text"),
                            t("form.text_title"),
                            false,
                            cx,
                            |view, cx| {
                                view.run_control(
                                    ContentControlAction::InsertFormField {
                                        kind: FormFieldKind::Text,
                                    },
                                    cx,
                                )
                            },
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "form-check",
                            t("form.check"),
                            t("form.check_title"),
                            false,
                            cx,
                            |view, cx| {
                                view.run_control(
                                    ContentControlAction::InsertFormField {
                                        kind: FormFieldKind::CheckBox,
                                    },
                                    cx,
                                )
                            },
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "form-dropdown",
                            t("form.dropdown"),
                            t("form.dropdown_title"),
                            false,
                            cx,
                            |view, cx| {
                                view.run_control(
                                    ContentControlAction::InsertFormField {
                                        kind: FormFieldKind::DropDown,
                                    },
                                    cx,
                                )
                            },
                        )
                        .into_any_element(),
                    ]),
                    Self::ribbon_row(vec![
                        self.ribbon_button(
                            "form-options",
                            t("form.options"),
                            t("form.options_title"),
                            false,
                            cx,
                            |view, cx| view.open_form_field_options(cx),
                        )
                        .into_any_element(),
                        self.ribbon_button(
                            "form-shading",
                            t("form.shading"),
                            t("form.shading_title"),
                            shading,
                            cx,
                            |view, cx| {
                                view.controls.hide_shading ^= true;
                                cx.notify();
                            },
                        )
                        .into_any_element(),
                    ]),
                ],
            )
            .into_any_element(),
            Self::ribbon_group(
                t("form.group.protect"),
                vec![Self::ribbon_row(vec![self
                    .ribbon_button(
                        "form-protect",
                        t("form.protect"),
                        t("form.protect_title"),
                        protected,
                        cx,
                        |view, cx| {
                            let on = !view.document.protected_for_forms();
                            view.run_control(ContentControlAction::ProtectForms(on), cx)
                        },
                    )
                    .into_any_element()])],
            )
            .into_any_element(),
        ]
    }

    /// Hands the view's caret or selection to the document, so an action
    /// works on what the user sees.
    fn sync_control_selection(&mut self) {
        if self.document.selection().is_none() {
            self.document
                .place_caret(self.insert_at.0, self.insert_at.1);
        }
    }

    /// 開発 → a control.
    pub(crate) fn insert_control(&mut self, kind: ContentControlKind, cx: &mut Context<Self>) {
        self.sync_control_selection();
        let japanese = i18n::language() == i18n::Language::Japanese;
        let result = self.document.insert_content_control(kind, japanese);
        self.after_control_edit(result, cx);
    }

    /// Runs an action on the selection, and shows where the caret is after.
    pub(crate) fn run_control(&mut self, action: ContentControlAction, cx: &mut Context<Self>) {
        self.sync_control_selection();
        let result = self.document.run_control_action(action);
        self.after_control_edit(result, cx);
    }

    fn after_control_edit(&mut self, result: Result<(), String>, cx: &mut Context<Self>) {
        match result {
            Ok(()) => {
                self.table_status = None;
                self.dirty = true;
                self.show_document_selection();
                self.rebuild_all_images();
            }
            Err(error) => self.table_status = Some(SharedString::from(error)),
        }
        cx.notify();
    }

    /// The view's caret and selection made the document's.
    pub(crate) fn show_document_selection(&mut self) {
        let range = WordSession::selection_range(&self.document).map(|range| {
            (
                (range.anchor.paragraph, range.anchor.offset),
                (range.focus.paragraph, range.focus.offset),
            )
        });
        match range {
            Some((anchor, focus)) if anchor != focus => {
                self.caret = self.document.place_caret(focus.0, focus.1);
                self.selection_anchor = Some(anchor);
                self.insert_at = focus;
                let page = self.caret_page_1idx();
                if let Err(error) = self.document.select(anchor, focus, page) {
                    eprintln!("select failed: {error}");
                }
            }
            Some((_, focus)) => {
                self.selection_anchor = None;
                self.insert_at = focus;
                self.caret = self.document.place_caret(focus.0, focus.1);
            }
            None => {
                self.caret = self
                    .document
                    .place_caret(self.insert_at.0, self.insert_at.1);
            }
        }
    }

    /// グループ解除: the group the caret is in taken away.
    fn ungroup(&mut self, cx: &mut Context<Self>) {
        self.sync_control_selection();
        let group = self
            .document
            .content_controls()
            .into_iter()
            .filter(|control| {
                let start = (control.start.paragraph, control.start.offset);
                let end = (control.end.paragraph, control.end.offset);
                control.kind == ContentControlKind::Group
                    && start <= self.insert_at
                    && self.insert_at <= end
            })
            .max_by_key(|control| control.depth);
        match group {
            Some(group) => self.run_control(
                ContentControlAction::Remove {
                    control: group.index,
                },
                cx,
            ),
            None => {
                self.table_status = Some(t("cc.no_group").into());
                cx.notify();
            }
        }
    }

    /// プロパティ: the dialog of the control the caret is in.
    pub(crate) fn open_control_properties(&mut self, cx: &mut Context<Self>) {
        self.sync_control_selection();
        let Some(control) = self.document.caret_control() else {
            // A form field's options, when the caret is on one.
            return self.open_form_field_options(cx);
        };
        let styles = character_styles(&self.document);
        self.controls.dialog = Some(ControlsDialog::Properties(Box::new(
            ControlPropertiesForm::new(&control, styles),
        )));
        cx.notify();
    }

    /// オプション: the dialog of the form field at the caret.
    pub(crate) fn open_form_field_options(&mut self, cx: &mut Context<Self>) {
        let Some(field) = self.document.form_field_at(self.insert_at) else {
            self.table_status = Some(t("cc.nothing_at_caret").into());
            return cx.notify();
        };
        self.controls.dialog = Some(ControlsDialog::FormField(Box::new(FormFieldForm::new(
            &field,
        ))));
        cx.notify();
    }

    /// A click or a key in the open control dialog.
    fn control_form_event(&mut self, event: FormEvent, cx: &mut Context<Self>) {
        let Some(dialog) = self.controls.dialog.as_mut() else {
            return;
        };
        let outcome = match dialog {
            ControlsDialog::Properties(form) => form.event(event),
            ControlsDialog::FormField(form) => form.event(event),
        };
        match outcome {
            FormOutcome::Stay => cx.notify(),
            FormOutcome::Close => {
                self.controls.dialog = None;
                cx.notify();
            }
            FormOutcome::Do(action) => {
                self.controls.dialog = None;
                self.run_control(action, cx);
            }
        }
    }

    /// Keys while a control dialog is open.
    pub(crate) fn controls_key(&mut self, key: &str, shift: bool, cx: &mut Context<Self>) -> bool {
        if !self.controls.is_open() {
            return false;
        }
        if let Some(event) = word_form::key_event(key, shift) {
            self.control_form_event(event, cx);
            return true;
        }
        let Some(field) = self.controls.field_mut() else {
            return false;
        };
        if field.marked.is_some() || !field.edit_key(key) {
            return false;
        }
        cx.notify();
        true
    }

    /// The open control dialog, over everything.
    pub(crate) fn controls_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let form = match self.controls.dialog.as_ref()? {
            ControlsDialog::Properties(form) => form.view(t),
            ControlsDialog::FormField(form) => form.view(t),
        };
        Some(word_form::overlay(
            &form,
            window,
            |view, event, cx| view.control_form_event(event, cx),
            cx,
        ))
    }

    /// After a click put the caret at `position`: what the click does to a
    /// control there — a box ticked, a list or a calendar dropped, a
    /// picture asked for, a placeholder selected.
    pub(crate) fn click_control(
        &mut self,
        position: (usize, usize),
        page: usize,
        cx: &mut Context<Self>,
    ) {
        self.controls.popup = None;
        if self.document.content_controls().is_empty() && self.document.form_fields().is_empty() {
            return;
        }
        let now = crate::word_characters::local_now();
        let today = (now.year, now.month, now.day);
        let anchor = self.control_anchor(position, page);
        match self.document.click_control(position, today) {
            ControlClick::None => {}
            ControlClick::Toggled => {
                self.dirty = true;
                self.show_document_selection();
                self.rebuild_all_images();
            }
            ControlClick::SelectedPlaceholder => self.show_document_selection(),
            ControlClick::List {
                control,
                entries,
                selected,
            } => {
                self.controls.popup = Some((
                    ControlPopup::List {
                        control,
                        entries,
                        selected,
                    },
                    anchor,
                ))
            }
            ControlClick::Date(picker) => {
                self.controls.popup = Some((ControlPopup::Date(picker), anchor))
            }
            ControlClick::Gallery { control, blocks } => {
                self.controls.popup = Some((ControlPopup::Gallery { control, blocks }, anchor))
            }
            ControlClick::FormList {
                field,
                entries,
                selected,
            } => {
                self.controls.popup = Some((
                    ControlPopup::FormList {
                        field,
                        entries,
                        selected,
                    },
                    anchor,
                ))
            }
            ControlClick::Picture { control } => self.prompt_for_control_picture(control, cx),
        }
        cx.notify();
    }

    /// Where a popup for the control at `position` hangs: under its last
    /// line.
    fn control_anchor(&self, position: (usize, usize), page: usize) -> Anchor {
        let control = self.document.control_at(position);
        let regions = match &control {
            Some(control) => self.document.text_regions(
                (control.start.paragraph, control.start.offset),
                (control.end.paragraph, control.end.offset),
            ),
            None => self.document.text_regions(position, position),
        };
        match regions.last() {
            Some(region) => {
                let (page, top) = self.document.page_and_offset(region.y + region.height);
                Anchor {
                    page,
                    x: regions.first().map_or(region.x, |first| first.x) as f32,
                    y: top as f32,
                }
            }
            None => Anchor {
                page,
                x: 0.0,
                y: 0.0,
            },
        }
    }

    fn prompt_for_control_picture(&mut self, control: usize, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some(SharedString::from(t("picture.insert"))),
        });
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.first() else {
                return;
            };
            let Ok(bytes) = std::fs::read(path) else {
                return;
            };
            let mime = match path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("jpg" | "jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                _ => "image/png",
            };
            view.update(cx, |view, cx| {
                view.run_control(
                    ContentControlAction::SetPicture {
                        control,
                        bytes,
                        mime: mime.to_string(),
                    },
                    cx,
                )
            })
            .ok();
        })
        .detach();
    }

    /// Typing while a placeholder (or a form text box's five spaces) is
    /// selected: it replaces the placeholder. Returns whether it did.
    pub(crate) fn type_over_control(&mut self, text: &str, cx: &mut Context<Self>) -> bool {
        match self.document.type_over_placeholder(text) {
            None => false,
            Some(result) => {
                self.after_control_edit(result, cx);
                true
            }
        }
    }

    /// What to draw over the pages, read again only after an edit or when
    /// the selection moves.
    fn chrome(&self) -> Vec<ControlChrome> {
        let key = (
            self.edits,
            self.document
                .selection()
                .or(Some((self.insert_at, self.insert_at))),
        );
        if let Some((edits, selection, chrome)) = self.controls.chrome.borrow().as_ref() {
            if (*edits, *selection) == key {
                return chrome.clone();
            }
        }
        let any = match *self.controls.any.borrow() {
            Some((edits, any)) if edits == self.edits => Some(any),
            _ => None,
        };
        let any = any.unwrap_or_else(|| {
            let any = !self.document.content_controls().is_empty()
                || !self.document.form_fields().is_empty();
            *self.controls.any.borrow_mut() = Some((self.edits, any));
            any
        });
        if !any {
            return Vec::new();
        }
        let chrome = self.document.control_chrome();
        *self.controls.chrome.borrow_mut() = Some((key.0, key.1, chrome.clone()));
        chrome
    }

    /// The controls' boxes, tags and shading over `page`.
    pub(crate) fn controls_overlay(&self, page: usize, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let zoom = self.zoom;
        let local = |y: f64| self.document.page_and_offset(y);
        let mut out = Vec::new();
        for chrome in self.chrome() {
            match chrome {
                ControlChrome::Box {
                    control,
                    regions,
                    title,
                    color,
                    drop_button,
                    add_button,
                } => {
                    let color = hex(&color);
                    let mut first_on_page = true;
                    for region in &regions {
                        let (on, top) = local(region.y);
                        if on != page {
                            continue;
                        }
                        let left = region.x as f32 - 2.0;
                        let width = region.width.max(4.0) as f32 + 4.0;
                        out.push(
                            div()
                                .absolute()
                                .left(px(left * zoom))
                                .top(px((top as f32 - 1.0) * zoom))
                                .w(px(width * zoom))
                                .h(px((region.height as f32 + 2.0) * zoom))
                                .border_1()
                                .border_color(color)
                                .into_any_element(),
                        );
                        if first_on_page {
                            first_on_page = false;
                            if let Some(title) = title.clone() {
                                out.push(
                                    div()
                                        .absolute()
                                        .left(px(left * zoom))
                                        .top(px((top as f32 - 15.0) * zoom))
                                        .h(px(14.0 * zoom))
                                        .px(px(4.0 * zoom))
                                        .bg(color)
                                        .text_color(gpui::rgb(0xFFFFFF))
                                        .text_size(px(9.0 * zoom))
                                        .whitespace_nowrap()
                                        .child(title)
                                        .into_any_element(),
                                );
                            }
                        }
                    }
                    let last = regions.last().copied();
                    if let Some(region) = last.filter(|region| local(region.y).0 == page) {
                        let (_, top) = local(region.y);
                        let right = (region.x + region.width) as f32 + 2.0;
                        if drop_button {
                            out.push(
                                div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "cc-drop-{control}"
                                    ))))
                                    .absolute()
                                    .left(px(right * zoom))
                                    .top(px((top as f32 - 1.0) * zoom))
                                    .w(px(14.0 * zoom))
                                    .h(px((region.height as f32 + 2.0) * zoom))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .bg(color)
                                    .text_color(gpui::rgb(0xFFFFFF))
                                    .text_size(px(8.0 * zoom))
                                    .cursor_pointer()
                                    .child("▼")
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            cx.stop_propagation();
                                            let start =
                                                view.document.content_controls().get(control).map(
                                                    |control| {
                                                        (
                                                            control.start.paragraph,
                                                            control.start.offset
                                                                + control
                                                                    .text
                                                                    .chars()
                                                                    .next()
                                                                    .map_or(0, char::len_utf8),
                                                        )
                                                    },
                                                );
                                            if let Some(at) = start {
                                                view.click_control(at, page, cx);
                                            }
                                        }),
                                    )
                                    .into_any_element(),
                            );
                        }
                        if add_button {
                            out.push(
                                div()
                                    .id(ElementId::Name(SharedString::from(format!(
                                        "cc-add-{control}"
                                    ))))
                                    .absolute()
                                    .left(px(right * zoom))
                                    .top(px((top as f32 + region.height as f32 - 12.0) * zoom))
                                    .size(px(14.0 * zoom))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded_sm()
                                    .bg(color)
                                    .text_color(gpui::rgb(0xFFFFFF))
                                    .text_size(px(11.0 * zoom))
                                    .cursor_pointer()
                                    .child("+")
                                    .on_mouse_down(
                                        gpui::MouseButton::Left,
                                        cx.listener(move |view, _, _, cx| {
                                            cx.stop_propagation();
                                            view.run_control(
                                                ContentControlAction::AddRepeatingItem { control },
                                                cx,
                                            )
                                        }),
                                    )
                                    .into_any_element(),
                            );
                        }
                    }
                }
                ControlChrome::Tags {
                    control: _,
                    start,
                    end,
                    title,
                    color,
                } => {
                    let color = hex(&color);
                    let tag = |region: jade_office_model::word::controls::TextRegion,
                               text: String,
                               closing: bool| {
                        let (on, top) = local(region.y);
                        (on == page).then(|| {
                            let x = if closing {
                                region.x + region.width
                            } else {
                                region.x - 6.0
                            } as f32;
                            div()
                                .absolute()
                                .left(px(x * zoom))
                                .top(px((top as f32 - 10.0) * zoom))
                                .h(px(10.0 * zoom))
                                .px(px(2.0 * zoom))
                                .rounded_sm()
                                .bg(color)
                                .text_color(gpui::rgb(0xFFFFFF))
                                .text_size(px(7.0 * zoom))
                                .whitespace_nowrap()
                                .child(text)
                                .into_any_element()
                        })
                    };
                    let opening = if title.is_empty() {
                        "[".to_string()
                    } else {
                        format!("[{title}")
                    };
                    let closing = if title.is_empty() {
                        "]".to_string()
                    } else {
                        format!("{title}]")
                    };
                    out.extend(start.and_then(|region| tag(region, opening, false)));
                    out.extend(end.and_then(|region| tag(region, closing, true)));
                }
                ControlChrome::Shading { regions } => {
                    if self.controls.hide_shading {
                        continue;
                    }
                    for region in regions {
                        let (on, top) = local(region.y);
                        if on != page {
                            continue;
                        }
                        out.push(
                            div()
                                .absolute()
                                .left(px(region.x as f32 * zoom))
                                .top(px(top as f32 * zoom))
                                .w(px(region.width.max(6.0) as f32 * zoom))
                                .h(px(region.height as f32 * zoom))
                                .bg(gpui::rgba(0x80808040))
                                .into_any_element(),
                        );
                    }
                }
            }
        }
        // Deferred, so it is over the pages and the overlays after it, and
        // takes the clicks on it.
        out.extend(
            self.controls_popup(page, cx)
                .map(|popup| gpui::deferred(popup).with_priority(2).into_any_element()),
        );
        out
    }

    /// The list or calendar dropped under a control, on its page.
    fn controls_popup(&self, page: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (popup, anchor) = self.controls.popup.as_ref()?;
        if anchor.page != page {
            return None;
        }
        let zoom = self.zoom;
        let panel = div()
            .id("cc-popup")
            .absolute()
            .left(px(anchor.x * zoom))
            .top(px((anchor.y + 2.0) * zoom))
            .min_w(px(160.0))
            .p_1()
            .flex()
            .flex_col()
            .rounded_md()
            .border_1()
            .border_color(theme::border())
            .bg(theme::panel())
            .shadow_lg()
            .text_xs()
            .text_color(theme::text())
            .occlude()
            .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation());
        let entry_row = |id: String, label: String, on: bool| {
            div()
                .id(ElementId::Name(SharedString::from(id)))
                .px_2()
                .py_0p5()
                .rounded_sm()
                .whitespace_nowrap()
                .cursor_pointer()
                .when(on, |row| row.bg(theme::selected()))
                .hover(|row| row.bg(theme::hovered()))
                .child(label)
        };
        let element = match popup {
            ControlPopup::List {
                control,
                entries,
                selected,
            } => {
                let control = *control;
                panel.children(entries.iter().enumerate().map(|(index, entry)| {
                    entry_row(
                        format!("cc-entry-{index}"),
                        entry.clone(),
                        *selected == Some(index),
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |view, _: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            view.controls.popup = None;
                            view.run_control(
                                ContentControlAction::ChooseEntry {
                                    control,
                                    entry: index,
                                },
                                cx,
                            )
                        }),
                    )
                }))
            }
            ControlPopup::FormList {
                field,
                entries,
                selected,
            } => {
                let field = *field;
                panel.children(entries.iter().enumerate().map(|(index, entry)| {
                    entry_row(
                        format!("form-entry-{index}"),
                        entry.clone(),
                        *selected == index,
                    )
                    .on_mouse_down(
                        gpui::MouseButton::Left,
                        cx.listener(move |view, _: &gpui::MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            view.controls.popup = None;
                            view.run_control(
                                ContentControlAction::ChooseFormEntry {
                                    field,
                                    entry: index,
                                },
                                cx,
                            )
                        }),
                    )
                }))
            }
            ControlPopup::Gallery { control, blocks } => {
                let control = *control;
                if blocks.is_empty() {
                    panel.child(
                        div()
                            .px_2()
                            .text_color(theme::text_dim())
                            .child(t("cc.no_blocks")),
                    )
                } else {
                    panel.children(blocks.iter().enumerate().map(|(index, name)| {
                        let name = name.clone();
                        entry_row(format!("cc-block-{index}"), name.clone(), false).on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |view, _: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                view.controls.popup = None;
                                view.run_control(
                                    ContentControlAction::InsertBuildingBlock {
                                        control,
                                        name: name.clone(),
                                    },
                                    cx,
                                )
                            }),
                        )
                    }))
                }
            }
            ControlPopup::Date(picker) => {
                let japanese = i18n::language() == i18n::Language::Japanese;
                let weekdays: [&str; 7] = if japanese {
                    ["日", "月", "火", "水", "木", "金", "土"]
                } else {
                    ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
                };
                let picker_state = picker.clone();
                let header = div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_1()
                    .child(
                        div()
                            .id("cc-month-back")
                            .px_1()
                            .cursor_pointer()
                            .hover(|button| button.bg(theme::hovered()))
                            .child("◀")
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|view, _: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some((ControlPopup::Date(picker), _)) =
                                        view.controls.popup.as_mut()
                                    {
                                        picker.step(false);
                                    }
                                    cx.notify();
                                }),
                            ),
                    )
                    .child(picker.title(japanese))
                    .child(
                        div()
                            .id("cc-month-next")
                            .px_1()
                            .cursor_pointer()
                            .hover(|button| button.bg(theme::hovered()))
                            .child("▶")
                            .on_mouse_down(
                                gpui::MouseButton::Left,
                                cx.listener(|view, _: &gpui::MouseDownEvent, _, cx| {
                                    cx.stop_propagation();
                                    if let Some((ControlPopup::Date(picker), _)) =
                                        view.controls.popup.as_mut()
                                    {
                                        picker.step(true);
                                    }
                                    cx.notify();
                                }),
                            ),
                    );
                let cell = |content: AnyElement| {
                    div()
                        .w(px(22.0))
                        .h(px(18.0))
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(content)
                };
                let names = div().flex().children(weekdays.iter().map(|name| {
                    cell(
                        div()
                            .text_color(theme::text_dim())
                            .child(*name)
                            .into_any_element(),
                    )
                }));
                let weeks =
                    picker
                        .weeks()
                        .into_iter()
                        .enumerate()
                        .map(|(week_index, week)| {
                            div().flex().children(week.into_iter().enumerate().map(
                                |(column, day)| match day {
                                    Some(day) => {
                                        let date = (picker_state.year, picker_state.month, day);
                                        let selected = picker_state.selected == Some(date);
                                        let today = picker_state.today == date;
                                        let action = picker_state.choose(day);
                                        cell(
                                        div()
                                            .id(ElementId::Name(SharedString::from(format!(
                                                "cc-day-{week_index}-{column}"
                                            ))))
                                            .w_full()
                                            .h_full()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .rounded_sm()
                                            .cursor_pointer()
                                            .when(selected, |day| day.bg(theme::selected()))
                                            .when(today, |day| {
                                                day.border_1().border_color(theme::accent())
                                            })
                                            .hover(|day| day.bg(theme::hovered()))
                                            .child(day.to_string())
                                            .on_mouse_down(gpui::MouseButton::Left, cx.listener(
                                                move |view, _: &gpui::MouseDownEvent, _, cx| {
 cx.stop_propagation();
                                                    view.controls.popup = None;
                                                    view.run_control(action.clone(), cx)
                                                },
                                            ))
                                            .into_any_element(),
                                    )
                                    }
                                    None => cell(div().into_any_element()),
                                },
                            ))
                        });
                let today = picker_state.today;
                let control = picker_state.control;
                panel.child(header).child(names).children(weeks).child(
                    div()
                        .id("cc-today")
                        .mt_1()
                        .px_2()
                        .cursor_pointer()
                        .text_color(theme::accent())
                        .hover(|row| row.bg(theme::hovered()))
                        .child(t("cc.today"))
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(move |view, _: &gpui::MouseDownEvent, _, cx| {
                                cx.stop_propagation();
                                view.controls.popup = None;
                                view.run_control(
                                    ContentControlAction::SetDate {
                                        control,
                                        date: format!(
                                            "{:04}-{:02}-{:02}",
                                            today.0, today.1, today.2
                                        ),
                                    },
                                    cx,
                                )
                            }),
                        ),
                )
            }
        };
        Some(element.into_any_element())
    }
}

#[cfg(test)]
mod tests {
    use super::{ControlPopup, ControlsDialog};
    use jade_office_model::word::controls::{ContentControlKind, WordControls};
    use jade_office_model::word::form_view::{FormEvent, FormId};

    fn open_contract(
        cx: &mut gpui::TestAppContext,
    ) -> (
        gpui::Entity<crate::DocumentView>,
        &mut gpui::VisualTestContext,
    ) {
        use gpui::Focusable;
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

    /// 開発 → リッチ テキスト puts a control in at the caret with its
    /// placeholder selected; what is typed replaces the placeholder, the box
    /// is drawn round it, and プロパティ gives it a title.
    #[gpui::test]
    fn a_control_is_put_in_typed_over_and_given_a_title(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert!(view.ribbon_tabs().contains(&crate::RibbonTab::Developer));
            view.insert_control(ContentControlKind::RichText, cx);
            let control = view.document.caret_control().expect("the caret is in it");
            assert!(control.showing_placeholder);
            assert!(
                view.selection_anchor.is_some(),
                "the placeholder is selected"
            );
        });
        cx.simulate_input("株式会社ジェイド");
        view.update(cx, |view, cx| {
            let control = view.document.content_controls().remove(0);
            assert!(!control.showing_placeholder);
            assert_eq!(control.text, "株式会社ジェイド");
            assert!(view.chrome().iter().any(|chrome| matches!(
                chrome,
                jade_office_model::word::controls::ControlChrome::Box { .. }
            )));
            view.open_control_properties(cx);
            assert!(matches!(
                view.controls.dialog,
                Some(ControlsDialog::Properties(_))
            ));
        });
        cx.simulate_input("会社名");
        view.read_with(cx, |view, _| {
            assert_eq!(
                view.controls.field().map(|field| field.text.as_str()),
                Some("会社名")
            );
        });
        cx.simulate_keystrokes("enter");
        view.read_with(cx, |view, _| {
            assert!(view.controls.dialog.is_none());
            let control = view.document.content_controls().remove(0);
            assert_eq!(control.title.as_deref(), Some("会社名"));
        });
    }

    /// A click on a check box ticks it; one on a drop-down list drops its
    /// entries, and picking one puts it in.
    #[gpui::test]
    fn clicks_tick_a_box_and_pick_from_a_list(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            view.insert_control(ContentControlKind::CheckBox, cx);
            let check = view.document.content_controls().remove(0);
            let at = (check.start.paragraph, check.start.offset + 1);
            view.click_control(at, 0, cx);
            assert!(view.document.content_controls()[0].checked);

            let after = view.document.content_controls()[0].end;
            view.document.place_caret(after.paragraph, after.offset);
            view.insert_at = (after.paragraph, after.offset);
            view.insert_control(ContentControlKind::ComboBox, cx);
            let list = view.document.content_controls().remove(1);
            let mut form =
                jade_office_model::word::controls::ControlPropertiesForm::new(&list, Vec::new());
            form.event(FormEvent::Click(FormId::new("entry-display")));
            form.field_mut().insert("東京");
            form.event(FormEvent::Click(FormId::new("entry-add")));
            view.controls.dialog = Some(ControlsDialog::Properties(Box::new(form)));
            view.control_form_event(FormEvent::Enter, cx);
            let list = view.document.content_controls().remove(1);
            view.click_control((list.start.paragraph, list.start.offset + 1), 0, cx);
            let Some((
                ControlPopup::List {
                    control, entries, ..
                },
                _,
            )) = view.controls.popup.as_ref()
            else {
                panic!("the list is dropped");
            };
            assert_eq!(entries.last().map(String::as_str), Some("東京"));
            let (control, entry) = (*control, entries.len() - 1);
            view.controls.popup = None;
            view.run_control(
                jade_office_model::word::controls::ContentControlAction::ChooseEntry {
                    control,
                    entry,
                },
                cx,
            );
            assert_eq!(view.document.content_controls()[1].text, "東京");
        });
    }
}
