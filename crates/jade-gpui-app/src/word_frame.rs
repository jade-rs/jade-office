//! 開発 › 旧形式のツール › 枠 and 枠の書式設定 (`jade-office-model`'s
//! `frame`; the browser shell has the same): 枠 puts the selected paragraphs
//! in a frame with a box round them; the dialog — a bar over the pages —
//! sets 文字列の折り返し, サイズ, 水平方向 and 垂直方向, and has 枠の削除.

use gpui::{div, prelude::*, AnyElement, Context, SharedString};
use jade_office_model::word::frame::{
    FrameForm, FrameHeight, FramePosition, WordFrames, HORIZONTAL_ALIGNS, HORIZONTAL_ANCHORS,
    VERTICAL_ALIGNS, VERTICAL_ANCHORS,
};

use crate::word_dialog::{button, chip, on};
use crate::word_dialogs::{bar, Dialog};
use crate::{i18n, DocumentView};

impl DocumentView {
    /// 枠: the selection in a frame, its dialog opened on it.
    pub(crate) fn insert_frame(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        self.tools_caret();
        let result = self.document.insert_frame();
        self.tool_done(result, cx);
    }

    /// 枠の書式設定 on the caret's frame, or on a new one's defaults.
    pub(crate) fn open_frame_dialog(&mut self, cx: &mut Context<Self>) {
        self.sync_selection();
        self.tools_caret();
        let form = self
            .document
            .caret_frame()
            .map(|frame| FrameForm::from_headless(&frame))
            .unwrap_or_default();
        self.open_dialog(Dialog::Frame(form), cx);
    }

    fn edit_frame(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut FrameForm)) {
        if let Some(Dialog::Frame(form)) = self.dialog.as_mut() {
            edit(form);
            cx.notify();
        }
    }

    /// 枠の削除.
    fn remove_frame(&mut self, cx: &mut Context<Self>) {
        self.dialog = None;
        let result = self.document.remove_frame();
        self.tool_done(result, cx);
    }
}

/// The 旧形式のツール buttons: 枠 and 枠の書式設定.
pub fn buttons(view: &DocumentView, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    vec![
        view.ribbon_button(
            "frame-insert",
            i18n::text("frame.insert"),
            i18n::text("frame.insert_title"),
            false,
            cx,
            |view, cx| view.insert_frame(cx),
        )
        .into_any_element(),
        view.ribbon_button(
            "frame-format",
            i18n::text("frame.format"),
            i18n::text("frame.title"),
            false,
            cx,
            |view, cx| view.open_frame_dialog(cx),
        )
        .into_any_element(),
    ]
}

fn length(value: f32) -> String {
    format!("{value:.1} mm")
}

/// A click that steps a length by `delta` mm.
fn length_step(
    cx: &Context<DocumentView>,
    set: fn(&mut FrameForm, f32),
    get: fn(&FrameForm) -> f32,
    delta: f32,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    on(cx, move |view: &mut DocumentView, cx| {
        view.edit_frame(cx, |form| set(form, FrameForm::step(get(form), delta)))
    })
}

/// A click that moves a 位置 distance by `delta` mm; it may go negative,
/// left of or above its base.
fn offset_step(
    cx: &Context<DocumentView>,
    horizontal: bool,
    delta: f32,
) -> impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static {
    on(cx, move |view: &mut DocumentView, cx| {
        view.edit_frame(cx, |form| {
            let now = if horizontal { form.horizontal } else { form.vertical };
            if let FramePosition::Offset(value) = now {
                let next = FramePosition::Offset(((value + delta) * 10.0).round() / 10.0);
                if horizontal {
                    form.horizontal = next;
                } else {
                    form.vertical = next;
                }
            }
        })
    })
}

/// A length with − and + (1 mm a step).
fn stepper(
    id: &'static str,
    value: f32,
    cx: &mut Context<DocumentView>,
    set: fn(&mut FrameForm, f32),
    get: fn(&FrameForm) -> f32,
) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap_1()
        .child(
            chip(SharedString::from(format!("{id}-less")), "−", false)
                .on_click(length_step(cx, set, get, -1.0)),
        )
        .child(div().text_xs().child(length(value)))
        .child(
            chip(SharedString::from(format!("{id}-more")), "+", false)
                .on_click(length_step(cx, set, get, 1.0)),
        )
        .into_any_element()
}

fn caption(key: &'static str) -> AnyElement {
    div()
        .text_xs()
        .text_color(crate::theme::text_dim())
        .child(i18n::text(key))
        .into_any_element()
}

fn section(children: Vec<AnyElement>) -> AnyElement {
    div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_1()
        .children(children)
        .into_any_element()
}

/// One axis: 位置 (an alignment, or a distance), 基準 and 文字列との間隔.
#[allow(clippy::too_many_arguments)]
fn axis(
    prefix: &'static str,
    title: &'static str,
    position: FramePosition,
    aligns: &'static [(&'static str, &'static str)],
    anchor: &'static str,
    anchors: &'static [(&'static str, &'static str)],
    space: f32,
    horizontal: bool,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let t = i18n::text;
    let set_position = move |form: &mut FrameForm, position: FramePosition| {
        if horizontal {
            form.horizontal = position;
        } else {
            form.vertical = position;
        }
    };
    let mut children = vec![caption(title)];
    for (index, (token, label)) in aligns.iter().enumerate() {
        let token: &'static str = token;
        children.push(
            chip(
                SharedString::from(format!("{prefix}-align-{index}")),
                t(label),
                position == FramePosition::Align(token),
            )
            .on_click(on(cx, move |view, cx| {
                view.edit_frame(cx, |form| set_position(form, FramePosition::Align(token)))
            }))
            .into_any_element(),
        );
    }
    let offset = match position {
        FramePosition::Offset(offset) => Some(offset),
        FramePosition::Align(_) => None,
    };
    children.push(
        chip(
            SharedString::from(format!("{prefix}-offset")),
            t("frame.offset"),
            offset.is_some(),
        )
        .on_click(on(cx, move |view, cx| {
            view.edit_frame(cx, |form| set_position(form, FramePosition::Offset(0.0)))
        }))
        .into_any_element(),
    );
    if let Some(offset) = offset {
        children.push(
            chip(SharedString::from(format!("{prefix}-offset-less")), "−", false)
                .on_click(offset_step(cx, horizontal, -1.0))
                .into_any_element(),
        );
        children.push(div().text_xs().child(length(offset)).into_any_element());
        children.push(
            chip(SharedString::from(format!("{prefix}-offset-more")), "+", false)
                .on_click(offset_step(cx, horizontal, 1.0))
                .into_any_element(),
        );
    }
    children.push(caption("frame.anchor"));
    for (index, (token, label)) in anchors.iter().enumerate() {
        let token: &'static str = token;
        children.push(
            chip(
                SharedString::from(format!("{prefix}-anchor-{index}")),
                t(label),
                anchor == token,
            )
            .on_click(on(cx, move |view, cx| {
                view.edit_frame(cx, |form| {
                    if horizontal {
                        form.h_anchor = token;
                    } else {
                        form.v_anchor = token;
                    }
                })
            }))
            .into_any_element(),
        );
    }
    children.push(caption("frame.distance"));
    children.push(if horizontal {
        stepper(
            "frame-h-space",
            space,
            cx,
            |form, value| form.h_space = value,
            |form| form.h_space,
        )
    } else {
        stepper(
            "frame-v-space",
            space,
            cx,
            |form, value| form.v_space = value,
            |form| form.v_space,
        )
    });
    section(children)
}

/// The bar's contents.
pub fn bar_body(form: &FrameForm, cx: &mut Context<DocumentView>) -> Vec<AnyElement> {
    let t = i18n::text;
    let mut wrap = vec![caption("frame.wrap")];
    for (index, (around, key)) in [(false, "frame.wrap_none"), (true, "frame.wrap_around")]
        .into_iter()
        .enumerate()
    {
        wrap.push(
            chip(("frame-wrap", index), t(key), form.wrap_around == around)
                .on_click(on(cx, move |view, cx| {
                    view.edit_frame(cx, |form| form.wrap_around = around)
                }))
                .into_any_element(),
        );
    }
    let mut size = vec![caption("frame.width")];
    size.push(
        chip("frame-width-auto", t("frame.auto"), form.width.is_none())
            .on_click(on(cx, |view, cx| view.edit_frame(cx, |form| form.width = None)))
            .into_any_element(),
    );
    size.push(
        chip("frame-width-fixed", t("frame.fixed"), form.width.is_some())
            .on_click(on(cx, |view, cx| {
                view.edit_frame(cx, |form| form.width = Some(form.width.unwrap_or(50.0)))
            }))
            .into_any_element(),
    );
    if let Some(width) = form.width {
        size.push(stepper(
            "frame-width",
            width,
            cx,
            |form, value| form.width = Some(value.max(1.0)),
            |form| form.width.unwrap_or(0.0),
        ));
    }
    size.push(caption("frame.height"));
    for (index, (rule, key)) in FrameHeight::ALL.into_iter().enumerate() {
        size.push(
            chip(("frame-height", index), t(key), form.height_rule == rule)
                .on_click(on(cx, move |view, cx| {
                    view.edit_frame(cx, |form| {
                        form.height_rule = rule;
                        if rule != FrameHeight::Auto && form.height <= 0.0 {
                            form.height = 10.0;
                        }
                    })
                }))
                .into_any_element(),
        );
    }
    if form.height_rule != FrameHeight::Auto {
        size.push(stepper(
            "frame-height-value",
            form.height,
            cx,
            |form, value| form.height = value,
            |form| form.height,
        ));
    }
    let moves = form.moves_with_text();
    vec![
        section(wrap),
        section(size),
        axis(
            "frame-h",
            "frame.horizontal",
            form.horizontal,
            &HORIZONTAL_ALIGNS,
            form.h_anchor,
            &HORIZONTAL_ANCHORS,
            form.h_space,
            true,
            cx,
        ),
        axis(
            "frame-v",
            "frame.vertical",
            form.vertical,
            &VERTICAL_ALIGNS,
            form.v_anchor,
            &VERTICAL_ANCHORS,
            form.v_space,
            false,
            cx,
        ),
        chip("frame-moves", t("frame.move_with_text"), moves)
            .on_click(on(cx, |view, cx| {
                view.edit_frame(cx, FrameForm::toggle_moves_with_text)
            }))
            .into_any_element(),
        button("frame-remove", t("frame.remove"), false)
            .on_click(on(cx, |view, cx| view.remove_frame(cx)))
            .into_any_element(),
        button("frame-ok", t("format.ok"), true)
            .on_click(on(cx, |view, cx| {
                view.submit_dialog(cx);
            }))
            .into_any_element(),
        button("frame-cancel", t("format.cancel"), false)
            .on_click(on(cx, |view, cx| {
                view.dialog = None;
                cx.notify();
            }))
            .into_any_element(),
    ]
}

/// The dialog's bar over the pages.
pub fn dialog_bar(form: &FrameForm, cx: &mut Context<DocumentView>) -> AnyElement {
    let body = bar_body(form, cx);
    bar(i18n::text("frame.title"), i18n::text("frame.keys"), body)
}
