//! Word's spin boxes in the browser, drawn as the native app draws them
//! (`jade-gpui-app/src/word_spin.rs`): an editable field showing its value with the
//! unit — `10.5 pt`, `1 字`, `25.4 mm` — and ▲▼ beside it. The arrows, and
//! ↑ / ↓ while the field has the keyboard, step the value and keep the
//! unit; Enter or Tab (or a click elsewhere) commits what was typed,
//! converted to the field's unit, and Escape drops it. What the field means
//! is `jade-office-model`'s `spin`; what it sets is the closure each dialog
//! hands it.
//!
//! One field at a time has the keyboard: [`SpinFocus`], kept on the view.
//! It belongs to a dialog (its [`SpinOwner`]) and is dropped when that
//! dialog is gone.

use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context, KeyDownEvent, SharedString};
use jade_web::spin::{Metrics, SpinField, SpinKey, SpinSpec};
use jade_web::text::TextEdit;

use super::word_dialogs::Dialog;
use super::{DocView as DocumentView, Document};
use crate::palette as theme;

/// What a field sets, given its new value in the field's unit.
pub type Apply = Rc<dyn Fn(&mut DocumentView, f64)>;

/// What else happens when a field takes the keyboard.
pub type OnFocus = Rc<dyn Fn(&mut DocumentView)>;

/// Which part of the view a field is in, so its keyboard goes when that
/// part closes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinOwner {
    /// フォント, ルビ and the other character dialogs.
    Characters,
    /// 段落.
    Paragraph,
    /// ページ設定, 段組み, ページ番号の書式.
    PageSetup,
    /// 表のプロパティ and the table dialogs.
    Table,
}

/// The field that has the keyboard.
pub struct SpinFocus {
    pub id: SharedString,
    pub owner: SpinOwner,
    pub field: SpinField,
    apply: Apply,
}

/// How the key handler goes on after a field saw a key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinKeyOutcome {
    /// The field took it: nothing else gets it.
    Taken,
    /// A character: the input handler types it into the field.
    Typing,
    /// The field let the keyboard go; the dialog has the key too (Enter is
    /// its OK, Escape its キャンセル).
    PassOn,
}

/// A spin box's description: what it shows and what it sets.
pub struct Spin {
    pub id: SharedString,
    pub owner: SpinOwner,
    pub spec: SpinSpec,
    pub metrics: Metrics,
    /// Its value now, in the field's unit.
    pub value: f64,
    pub enabled: bool,
    pub width: f32,
    pub apply: Apply,
    /// What else happens when the field takes the keyboard: the dialog's
    /// own idea of its focused field follows.
    pub on_focus: Option<OnFocus>,
}

impl Spin {
    pub fn new(
        id: impl Into<SharedString>,
        owner: SpinOwner,
        spec: SpinSpec,
        value: f64,
        apply: impl Fn(&mut DocumentView, f64) + 'static,
    ) -> Self {
        Self {
            id: id.into(),
            owner,
            spec,
            metrics: Metrics::default(),
            value,
            enabled: true,
            width: 72.0,
            apply: Rc::new(apply),
            on_focus: None,
        }
    }

    pub fn on_focus(mut self, on_focus: impl Fn(&mut DocumentView) + 'static) -> Self {
        self.on_focus = Some(Rc::new(on_focus));
        self
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn metrics(mut self, metrics: Metrics) -> Self {
        self.metrics = metrics;
        self
    }
}

impl DocumentView {
    fn spin_owner_open(&self, owner: SpinOwner) -> bool {
        match owner {
            SpinOwner::Characters => self.characters.is_open(),
            SpinOwner::Paragraph => matches!(
                &self.document,
                Document::Pages { dialog: Some(dialog), .. } if matches!(**dialog, Dialog::Paragraph(_))
            ),
            SpinOwner::PageSetup => self.page_dialog.is_some(),
            SpinOwner::Table => self.table_dialog_open(),
        }
    }

    /// The field with the keyboard, dropped first if its dialog has gone.
    fn live_spin(&mut self) -> Option<&mut SpinFocus> {
        let owner = self.spin.as_ref()?.owner;
        if !self.spin_owner_open(owner) {
            self.spin = None;
            return None;
        }
        self.spin.as_mut()
    }

    /// The field typing goes into, while one has the keyboard.
    pub(crate) fn spin_text(&self) -> Option<&TextEdit> {
        let spin = self.spin.as_ref()?;
        self.spin_owner_open(spin.owner).then_some(&spin.field.edit)
    }

    pub(crate) fn spin_text_mut(&mut self) -> Option<&mut TextEdit> {
        Some(&mut self.live_spin()?.field.edit)
    }

    /// What was typed into the field with the keyboard becomes its value,
    /// and the field lets the keyboard go.
    pub(crate) fn commit_spin(&mut self) {
        let Some(mut spin) = self.spin.take() else {
            return;
        };
        if !self.spin_owner_open(spin.owner) {
            return;
        }
        if let Some(value) = spin.field.commit() {
            (spin.apply)(self, value);
        }
    }

    /// A key while a field has the keyboard; `None` when none has.
    pub(crate) fn spin_key(
        &mut self,
        event: &KeyDownEvent,
        cx: &mut Context<Self>,
    ) -> Option<SpinKeyOutcome> {
        let spin = self.live_spin()?;
        if spin.field.edit.marked.is_some() {
            return Some(SpinKeyOutcome::Typing);
        }
        let modifiers = &event.keystroke.modifiers;
        if modifiers.platform || modifiers.control {
            return Some(SpinKeyOutcome::Taken);
        }
        let apply = Rc::clone(&spin.apply);
        let outcome = match spin.field.key(event.keystroke.key.as_str()) {
            SpinKey::Stepped(value) => {
                apply(self, value);
                SpinKeyOutcome::Taken
            }
            SpinKey::Committed(value) => {
                self.spin = None;
                if let Some(value) = value {
                    apply(self, value);
                }
                // Enter goes on to be the dialog's OK, Tab to move to its
                // next field.
                SpinKeyOutcome::PassOn
            }
            SpinKey::Reverted => {
                self.spin = None;
                SpinKeyOutcome::PassOn
            }
            SpinKey::Edited => SpinKeyOutcome::Taken,
            SpinKey::Unhandled => SpinKeyOutcome::Typing,
        };
        cx.notify();
        Some(outcome)
    }

    /// `spin` takes the keyboard, on its value, all of it selected; the
    /// field that had it commits first.
    pub(crate) fn focus_spin(&mut self, spin: &Spin) {
        self.commit_spin();
        if let Some(on_focus) = spin.on_focus.as_ref() {
            on_focus(self);
        }
        self.spin = Some(SpinFocus {
            id: spin.id.clone(),
            owner: spin.owner,
            field: SpinField::focus(spin.spec, spin.metrics, spin.value),
            apply: Rc::clone(&spin.apply),
        });
    }

    /// Whether the field `id` has the keyboard.
    pub(crate) fn spin_has_focus(&self, id: &str) -> bool {
        self.spin_text().is_some() && self.spin.as_ref().is_some_and(|focus| focus.id == id)
    }

    /// ▲ or ▼: from what is typed when the field has the keyboard, else
    /// from its value.
    fn step_spin(&mut self, spin: &SpinStep, up: bool) {
        let focused = self.spin.as_mut().filter(|focus| focus.id == spin.id);
        let value = match focused {
            Some(focus) => focus.field.step(up),
            None => {
                self.commit_spin();
                spin.spec.step(spin.value, up)
            }
        };
        (spin.apply)(self, value);
    }
}

/// What an arrow needs of its field.
struct SpinStep {
    id: SharedString,
    spec: SpinSpec,
    value: f64,
    apply: Apply,
}

/// The field's text with its caret, or its selection, when it has the
/// keyboard.
fn edited_text(edit: &TextEdit) -> gpui::Div {
    let (before, after) = edit.split();
    let caret = || div().w(px(1.0)).h(px(12.0)).bg(theme::accent());
    if edit.whole {
        div()
            .flex()
            .items_center()
            .child(div().bg(theme::selected()).child(edit.text.clone()))
            .child(caret())
    } else {
        div()
            .flex()
            .items_center()
            .child(before.to_string())
            .child(caret())
            .child(after.to_string())
    }
}

/// A spin box: the field, and ▲▼ at its right.
pub fn spin_field(view: &DocumentView, spin: Spin, cx: &mut Context<DocumentView>) -> AnyElement {
    let focused = view
        .spin
        .as_ref()
        .filter(|focus| focus.id == spin.id && view.spin_owner_open(focus.owner));
    let has_focus = focused.is_some();
    let text = match focused {
        Some(focus) => edited_text(&focus.field.edit),
        None => div().child(spin.spec.format(spin.value)),
    };
    let spin = Rc::new(spin);
    let Spin {
        ref id,
        spec,
        value,
        enabled,
        width,
        ref apply,
        ..
    } = *spin;
    let arrow = |up: bool| {
        let step = SpinStep {
            id: id.clone(),
            spec,
            value,
            apply: Rc::clone(apply),
        };
        div()
            .id(SharedString::from(format!(
                "{id}-{}",
                if up { "up" } else { "down" }
            )))
            .flex_1()
            .flex()
            .items_center()
            .justify_center()
            .px(px(3.0))
            .text_size(px(7.0))
            .text_color(if enabled {
                theme::text_dim()
            } else {
                theme::text_faint()
            })
            .when(enabled, |arrow| {
                arrow
                    .cursor_pointer()
                    .hover(|arrow| arrow.bg(theme::hovered()))
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        cx.stop_propagation();
                        view.step_spin(&step, up);
                        cx.notify();
                    }))
            })
            .child(if up { "▲" } else { "▼" })
    };
    let up = arrow(true);
    let down = arrow(false);
    let focus = Rc::clone(&spin);
    div()
        .id(SharedString::from(format!("{id}-spin")))
        .flex()
        .flex_none()
        .w(px(width))
        .h(px(20.0))
        .rounded_sm()
        .border_1()
        .border_color(if has_focus {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .text_xs()
        .when(has_focus, |field: gpui::Stateful<gpui::Div>| {
            // A click anywhere else commits what was typed.
            field.on_mouse_down_out(cx.listener(|view, _, _, cx| {
                view.commit_spin();
                cx.notify();
            }))
        })
        .child(
            div()
                .id(SharedString::from(format!("{id}-text")))
                .flex_1()
                .min_w_0()
                .px_1()
                .flex()
                .items_center()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_color(if enabled {
                    theme::text()
                } else {
                    theme::text_faint()
                })
                .when(enabled, |field| {
                    field
                        .cursor_text()
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            cx.stop_propagation();
                            if !view.spin_has_focus(&focus.id) {
                                view.focus_spin(&focus);
                                cx.notify();
                            }
                        }))
                })
                .child(text),
        )
        .child(
            div()
                .h_full()
                .flex()
                .flex_col()
                .border_l_1()
                .border_color(theme::border())
                .child(up)
                .child(div().h(px(1.0)).bg(theme::border()))
                .child(down),
        )
        .into_any_element()
}
