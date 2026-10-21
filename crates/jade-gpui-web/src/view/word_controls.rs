//! The browser shell's copy of the native app's `word_controls`: the 開発
//! tab, the box drawn round the content control the caret is in, design
//! mode's tags, a click that ticks a box or drops a list or a calendar, the
//! プロパティ dialogs, and the legacy form fields with protection for filling
//! them in — over `jade-office-model`'s `word::controls`.

use std::cell::RefCell;

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, ElementId, SharedString, Window};
use jade_web::text::TextEdit;
use jade_web::word::controls::{
    character_styles, kind_key, ContentControlAction, ContentControlKind, ControlChrome,
    ControlClick, ControlPropertiesForm, DatePicker, FormFieldForm, FormFieldKind, WordControls,
    INSERTABLE,
};
use jade_web::word::form_view::{FormEvent, FormOutcome};
use jade_web::word::WordSession;

use super::word_form;
use super::{DocView, Document};
use crate::i18n;
use crate::palette as theme;
use crate::ribbon::{group, row, Action};
use crate::word_page::text_button;

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// What the 開発 tab's buttons do.
#[derive(Clone, Debug, PartialEq)]
pub enum ControlsAction {
    Insert(ContentControlKind),
    DesignMode,
    Properties,
    Group,
    Ungroup,
    InsertForm(FormFieldKind),
    FormOptions,
    Shading,
    Protect,
}

/// What the 開発 tab shows as on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeveloperState {
    pub design: bool,
    pub protected: bool,
    pub shading: bool,
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

/// Where a popup hangs: the page and the point under the control.
#[derive(Clone, Copy)]
pub struct Anchor {
    pub page: usize,
    pub x: f32,
    pub y: f32,
}

/// What was drawn, for the edit and the selection it was read at.
type ChromeCache = (u64, Option<((usize, usize), (usize, usize))>, Vec<ControlChrome>);

/// The view's share of the content controls.
#[derive(Default)]
pub struct ControlsUi {
    pub dialog: Option<ControlsDialog>,
    pub popup: Option<(ControlPopup, Anchor)>,
    pub hide_shading: bool,
    /// The picture control a picked picture goes into.
    pub picture_for: Option<usize>,
    any: RefCell<Option<(u64, bool)>>,
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

/// The 開発 tab: コントロール, 従来のフォーム and 保護.
pub fn developer(state: &DeveloperState, cx: &mut Context<DocView>) -> Vec<AnyElement> {
    const IDS: [&str; 9] = [
        "cc-rich",
        "cc-plain",
        "cc-picture",
        "cc-gallery",
        "cc-check",
        "cc-combo",
        "cc-list",
        "cc-date",
        "cc-repeat",
    ];
    let controls = |action| Action::Controls(action);
    let mut first = Vec::new();
    let mut second = Vec::new();
    for (index, kind) in INSERTABLE.iter().enumerate() {
        let label = t(kind_key(kind));
        let button = text_button(
            IDS[index],
            label,
            label,
            false,
            controls(ControlsAction::Insert(kind.clone())),
            cx,
        );
        if index < 5 {
            first.push(button);
        } else {
            second.push(button);
        }
    }
    second.extend([
        text_button(
            "cc-design",
            t("cc.design_mode"),
            t("cc.design_mode_title"),
            state.design,
            controls(ControlsAction::DesignMode),
            cx,
        ),
        text_button(
            "cc-properties",
            t("cc.properties"),
            t("cc.properties_title"),
            false,
            controls(ControlsAction::Properties),
            cx,
        ),
        text_button(
            "cc-group",
            t("cc.group"),
            t("cc.group_title"),
            false,
            controls(ControlsAction::Group),
            cx,
        ),
        text_button(
            "cc-ungroup",
            t("cc.ungroup"),
            t("cc.ungroup_title"),
            false,
            controls(ControlsAction::Ungroup),
            cx,
        ),
    ]);
    vec![
        group(t("cc.group.controls"), vec![row(first), row(second)]),
        group(
            t("form.group"),
            vec![
                row(vec![
                    text_button(
                        "form-text",
                        t("form.text"),
                        t("form.text_title"),
                        false,
                        controls(ControlsAction::InsertForm(FormFieldKind::Text)),
                        cx,
                    ),
                    text_button(
                        "form-check",
                        t("form.check"),
                        t("form.check_title"),
                        false,
                        controls(ControlsAction::InsertForm(FormFieldKind::CheckBox)),
                        cx,
                    ),
                    text_button(
                        "form-dropdown",
                        t("form.dropdown"),
                        t("form.dropdown_title"),
                        false,
                        controls(ControlsAction::InsertForm(FormFieldKind::DropDown)),
                        cx,
                    ),
                ]),
                row(vec![
                    text_button(
                        "form-options",
                        t("form.options"),
                        t("form.options_title"),
                        false,
                        controls(ControlsAction::FormOptions),
                        cx,
                    ),
                    text_button(
                        "form-shading",
                        t("form.shading"),
                        t("form.shading_title"),
                        state.shading,
                        controls(ControlsAction::Shading),
                        cx,
                    ),
                ]),
            ],
        ),
        group(
            t("form.group.protect"),
            vec![row(vec![text_button(
                "form-protect",
                t("form.protect"),
                t("form.protect_title"),
                state.protected,
                controls(ControlsAction::Protect),
                cx,
            )])],
        ),
    ]
}

/// What the tab shows as on, read from the document.
pub fn state(view: &DocView) -> DeveloperState {
    let Document::Pages { document, .. } = &view.document else {
        return DeveloperState::default();
    };
    DeveloperState {
        design: document.design_mode(),
        protected: document.protected_for_forms(),
        shading: !view.controls.hide_shading,
    }
}

/// A 開発 button.
pub fn run(view: &mut DocView, action: ControlsAction, cx: &mut Context<DocView>) {
    match action {
        ControlsAction::Insert(kind) => {
            let japanese = !i18n::is_english();
            view.word_edit(cx, move |document| {
                document.insert_content_control(kind, japanese)
            });
        }
        ControlsAction::DesignMode => view.word_edit(cx, |document| {
            let on = !document.design_mode();
            document.run_control_action(ContentControlAction::DesignMode(on))
        }),
        ControlsAction::Properties => open_properties(view, cx),
        ControlsAction::Group => view.word_edit(cx, |document| {
            document.insert_content_control(ContentControlKind::Group, true)
        }),
        ControlsAction::Ungroup => view.word_edit(cx, |document| {
            let caret = document.caret().unwrap_or((0, 0));
            let group = document
                .content_controls()
                .into_iter()
                .filter(|control| {
                    control.kind == ContentControlKind::Group
                        && (control.start.paragraph, control.start.offset) <= caret
                        && caret <= (control.end.paragraph, control.end.offset)
                })
                .max_by_key(|control| control.depth)
                .ok_or_else(|| t("cc.no_group").to_string())?;
            document.run_control_action(ContentControlAction::Remove {
                control: group.index,
            })
        }),
        ControlsAction::InsertForm(kind) => view.word_edit(cx, move |document| {
            document.run_control_action(ContentControlAction::InsertFormField { kind })
        }),
        ControlsAction::FormOptions => open_form_options(view, cx),
        ControlsAction::Shading => {
            view.controls.hide_shading ^= true;
            cx.notify();
        }
        ControlsAction::Protect => view.word_edit(cx, |document| {
            let on = !document.protected_for_forms();
            document.run_control_action(ContentControlAction::ProtectForms(on))
        }),
    }
}

fn open_properties(view: &mut DocView, cx: &mut Context<DocView>) {
    let Document::Pages { document, .. } = &view.document else {
        return;
    };
    let Some(control) = document.caret_control() else {
        return open_form_options(view, cx);
    };
    let styles = character_styles(&**document);
    view.controls.dialog = Some(ControlsDialog::Properties(Box::new(
        ControlPropertiesForm::new(&control, styles),
    )));
    cx.notify();
}

fn open_form_options(view: &mut DocView, cx: &mut Context<DocView>) {
    let Document::Pages { document, .. } = &view.document else {
        return;
    };
    let field = document
        .caret()
        .and_then(|caret| document.form_field_at(caret));
    match field {
        Some(field) => {
            view.controls.dialog = Some(ControlsDialog::FormField(Box::new(FormFieldForm::new(
                &field,
            ))))
        }
        None => view.status = t("cc.nothing_at_caret").into(),
    }
    cx.notify();
}

fn run_action(view: &mut DocView, action: ContentControlAction, cx: &mut Context<DocView>) {
    view.word_edit(cx, move |document| document.run_control_action(action));
}

fn form_event(view: &mut DocView, event: FormEvent, cx: &mut Context<DocView>) {
    let Some(dialog) = view.controls.dialog.as_mut() else {
        return;
    };
    let outcome = match dialog {
        ControlsDialog::Properties(form) => form.event(event),
        ControlsDialog::FormField(form) => form.event(event),
    };
    match outcome {
        FormOutcome::Stay => cx.notify(),
        FormOutcome::Close => {
            view.controls.dialog = None;
            cx.notify();
        }
        FormOutcome::Do(action) => {
            view.controls.dialog = None;
            run_action(view, action, cx);
        }
    }
}

/// Keys while a control dialog is open. Whether the key was taken.
pub fn handle_key(view: &mut DocView, key: &str, shift: bool, cx: &mut Context<DocView>) -> bool {
    if let Some(event) = word_form::key_event(key, shift) {
        form_event(view, event, cx);
        return true;
    }
    let Some(field) = view.controls.field_mut() else {
        return false;
    };
    if field.marked.is_some() || !field.edit_key(key) {
        return false;
    }
    cx.notify();
    true
}

/// The open control dialog, over everything.
pub fn dialog(
    view: &DocView,
    window: &mut Window,
    cx: &mut Context<DocView>,
) -> Option<AnyElement> {
    let form = match view.controls.dialog.as_ref()? {
        ControlsDialog::Properties(form) => form.view(t),
        ControlsDialog::FormField(form) => form.view(t),
    };
    Some(word_form::overlay(&form, window, form_event, cx))
}

/// After a press put the caret where it landed: what it does to a control
/// there.
pub fn click(view: &mut DocView, page: usize, cx: &mut Context<DocView>) {
    view.controls.popup = None;
    let Document::Pages { document, .. } = &mut view.document else {
        return;
    };
    let Some(position) = document.caret() else {
        return;
    };
    if document.content_controls().is_empty() && document.form_fields().is_empty() {
        return;
    }
    let today = today();
    let anchor = anchor_for(document, position, page);
    let clicked = document.click_control(position, today);
    match clicked {
        ControlClick::None => {}
        ControlClick::Toggled => {
            view.refresh_repainted();
            view.word_result(Ok(()), cx);
            return;
        }
        ControlClick::SelectedPlaceholder => {
            view.refresh_repainted();
        }
        ControlClick::List {
            control,
            entries,
            selected,
        } => {
            view.controls.popup = Some((
                ControlPopup::List {
                    control,
                    entries,
                    selected,
                },
                anchor,
            ))
        }
        ControlClick::Date(picker) => {
            view.controls.popup = Some((ControlPopup::Date(picker), anchor))
        }
        ControlClick::Gallery { control, blocks } => {
            view.controls.popup = Some((ControlPopup::Gallery { control, blocks }, anchor))
        }
        ControlClick::FormList {
            field,
            entries,
            selected,
        } => {
            view.controls.popup = Some((
                ControlPopup::FormList {
                    field,
                    entries,
                    selected,
                },
                anchor,
            ))
        }
        ControlClick::Picture { control } => {
            view.controls.picture_for = Some(control);
            view.host.pick();
        }
    }
    cx.notify();
}

/// A picture the page's picker brought for a picture control. Whether one
/// was waiting for it.
pub fn take_picture(view: &mut DocView, bytes: &[u8], cx: &mut Context<DocView>) -> bool {
    let Some(control) = view.controls.picture_for.take() else {
        return false;
    };
    let mime = match jade_web::word::picture::picture_type(bytes) {
        Some(mime) => mime.to_string(),
        None => return false,
    };
    run_action(
        view,
        ContentControlAction::SetPicture {
            control,
            bytes: bytes.to_vec(),
            mime,
        },
        cx,
    );
    true
}

/// Today, from the browser's clock.
fn today() -> (i32, u32, u32) {
    #[cfg(target_arch = "wasm32")]
    {
        let now = js_sys::Date::new_0();
        (
            now.get_full_year() as i32,
            now.get_month() + 1,
            now.get_date(),
        )
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        (2026, 1, 1)
    }
}

fn anchor_for(
    document: &jade_web::editing::EditableDocument,
    position: (usize, usize),
    page: usize,
) -> Anchor {
    let control = document.control_at(position);
    let regions = match &control {
        Some(control) => document.text_regions(
            (control.start.paragraph, control.start.offset),
            (control.end.paragraph, control.end.offset),
        ),
        None => document.text_regions(position, position),
    };
    match regions.last() {
        Some(region) => {
            let (page, top) = document.page_and_offset(region.y + region.height);
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

fn chrome(view: &DocView) -> Vec<ControlChrome> {
    let Document::Pages { document, .. } = &view.document else {
        return Vec::new();
    };
    let generation = view.generation;
    let any = match *view.controls.any.borrow() {
        Some((seen, any)) if seen == generation => Some(any),
        _ => None,
    };
    let any = any.unwrap_or_else(|| {
        let any = !document.content_controls().is_empty() || !document.form_fields().is_empty();
        *view.controls.any.borrow_mut() = Some((generation, any));
        any
    });
    if !any {
        return Vec::new();
    }
    let selection = document
        .selection()
        .or_else(|| document.caret().map(|caret| (caret, caret)));
    if let Some((seen, seen_selection, chrome)) = view.controls.chrome.borrow().as_ref() {
        if *seen == generation && *seen_selection == selection {
            return chrome.clone();
        }
    }
    let chrome = document.control_chrome();
    *view.controls.chrome.borrow_mut() = Some((generation, selection, chrome.clone()));
    chrome
}

/// The controls' boxes, tags and shading over `page`, and a dropped list.
pub fn overlay(
    view: &DocView,
    page: usize,
    zoom: f32,
    cx: &mut Context<DocView>,
) -> Vec<AnyElement> {
    let Document::Pages { document, .. } = &view.document else {
        return Vec::new();
    };
    let local = |y: f64| document.page_and_offset(y);
    let mut out = Vec::new();
    for item in chrome(view) {
        match item {
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
                if let Some(region) = regions
                    .last()
                    .copied()
                    .filter(|region| local(region.y).0 == page)
                {
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
                                        if let Document::Pages { document, .. } = &mut view.document
                                        {
                                            let start = document
                                                .content_controls()
                                                .get(control)
                                                .map(|control| {
                                                    (
                                                        control.start.paragraph,
                                                        control.start.offset + 1,
                                                    )
                                                });
                                            if let Some(at) = start {
                                                document.place_caret(at.0, at.1);
                                            }
                                        }
                                        click(view, page, cx);
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
                                        run_action(
                                            view,
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
                start,
                end,
                title,
                color,
                ..
            } => {
                let color = hex(&color);
                let tag =
                    |region: jade_web::word::controls::TextRegion, text: String, closing: bool| {
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
                if view.controls.hide_shading {
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
    out.extend(popup(view, page, zoom, cx));
    out
}

fn popup(view: &DocView, page: usize, zoom: f32, cx: &mut Context<DocView>) -> Option<AnyElement> {
    let (popup, anchor) = view.controls.popup.as_ref()?;
    if anchor.page != page {
        return None;
    }
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
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.controls.popup = None;
                    run_action(
                        view,
                        ContentControlAction::ChooseEntry {
                            control,
                            entry: index,
                        },
                        cx,
                    )
                }))
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
                .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                    view.controls.popup = None;
                    run_action(
                        view,
                        ContentControlAction::ChooseFormEntry {
                            field,
                            entry: index,
                        },
                        cx,
                    )
                }))
            }))
        }
        ControlPopup::Gallery { control, blocks } => {
            let control = *control;
            if blocks.is_empty() {
                panel.child(
                    div()
                        .px_2()
                        .text_color(theme::dim())
                        .child(t("cc.no_blocks")),
                )
            } else {
                panel.children(blocks.iter().enumerate().map(|(index, name)| {
                    let name = name.clone();
                    entry_row(format!("cc-block-{index}"), name.clone(), false).on_click(
                        cx.listener(move |view, _: &ClickEvent, _, cx| {
                            view.controls.popup = None;
                            run_action(
                                view,
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
            let japanese = !i18n::is_english();
            let weekdays: [&str; 7] = if japanese {
                ["日", "月", "火", "水", "木", "金", "土"]
            } else {
                ["Su", "Mo", "Tu", "We", "Th", "Fr", "Sa"]
            };
            let state = picker.clone();
            let step = |forwards: bool| {
                move |view: &mut DocView,
                      _: &ClickEvent,
                      _: &mut Window,
                      cx: &mut Context<DocView>| {
                    if let Some((ControlPopup::Date(picker), _)) = view.controls.popup.as_mut() {
                        picker.step(forwards);
                    }
                    cx.notify();
                }
            };
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
                        .on_click(cx.listener(step(false))),
                )
                .child(picker.title(japanese))
                .child(
                    div()
                        .id("cc-month-next")
                        .px_1()
                        .cursor_pointer()
                        .hover(|button| button.bg(theme::hovered()))
                        .child("▶")
                        .on_click(cx.listener(step(true))),
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
                        .text_color(theme::dim())
                        .child(*name)
                        .into_any_element(),
                )
            }));
            let weeks = picker
                .weeks()
                .into_iter()
                .enumerate()
                .map(|(week_index, week)| {
                    div().flex().children(week.into_iter().enumerate().map(
                        |(column, day)| match day {
                            Some(day) => {
                                let date = (state.year, state.month, day);
                                let selected = state.selected == Some(date);
                                let today = state.today == date;
                                let action = state.choose(day);
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
                                        .on_click(cx.listener(
                                            move |view, _: &ClickEvent, _, cx| {
                                                view.controls.popup = None;
                                                run_action(view, action.clone(), cx)
                                            },
                                        ))
                                        .into_any_element(),
                                )
                            }
                            None => cell(div().into_any_element()),
                        },
                    ))
                });
            let today = state.today;
            let control = state.control;
            panel.child(header).child(names).children(weeks).child(
                div()
                    .id("cc-today")
                    .mt_1()
                    .px_2()
                    .cursor_pointer()
                    .text_color(theme::accent())
                    .hover(|row| row.bg(theme::hovered()))
                    .child(t("cc.today"))
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        view.controls.popup = None;
                        run_action(
                            view,
                            ContentControlAction::SetDate {
                                control,
                                date: format!("{:04}-{:02}-{:02}", today.0, today.1, today.2),
                            },
                            cx,
                        )
                    })),
            )
        }
    };
    Some(element.into_any_element())
}
