//! レイアウト ▸ ハイフネーション: なし, 自動 and ハイフネーションのオプション…,
//! the dialog over the document's automatic hyphenation (`jade-office-model`'s
//! `word::hyphenation`).

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::hyphenation::{HyphenationChoice, HyphenationDialog, WordHyphenation};

use crate::ribbon::{dropdown, menu_panel, Popup};
use crate::tooltip::Tooltip;
use crate::word_dialog::{check, labeled, on, DialogError, Inputs, WordDialog};
use crate::{i18n, theme, DocumentView};

/// The dialog's state: the model's dialog and its two text fields (the zone,
/// then the limit).
pub struct HyphenationDialogState {
    pub dialog: HyphenationDialog,
    pub inputs: Inputs,
}

/// レイアウト's ハイフネーション group: the button and its list.
pub(crate) fn layout_group(view: &DocumentView, cx: &mut Context<DocumentView>) -> AnyElement {
    let open = view.ribbon.popup == Some(Popup::Hyphenation);
    let current = view.document.hyphenation_choice();
    let trigger = div()
        .id("hyphenation-button")
        .flex()
        .flex_none()
        .items_center()
        .gap_1()
        .h(px(28.0))
        .px_2()
        .rounded_md()
        .cursor_pointer()
        .text_sm()
        .text_color(theme::text())
        .when(open, |button| button.bg(theme::selected()))
        .hover(|button| button.bg(theme::hovered()))
        .tooltip(Tooltip::text(i18n::text("hyphenation.title")))
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
            view.ribbon.toggle(Popup::Hyphenation);
            cx.notify();
        }))
        .child(i18n::text("hyphenation.button"))
        .child(
            gpui::svg()
                .path("icons/chevron-down.svg")
                .size_3()
                .text_color(theme::text_dim()),
        )
        .into_any_element();
    let menu = open.then(|| {
        menu_panel("hyphenation-menu")
            .w(px(240.0))
            .children(HyphenationChoice::ALL.into_iter().map(|choice| {
                let checked = choice == current;
                div()
                    .id(choice.label_key())
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(px(26.0))
                    .pl_1()
                    .pr_3()
                    .rounded_md()
                    .whitespace_nowrap()
                    .cursor_pointer()
                    .hover(|row| row.bg(theme::hovered()))
                    .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                        view.ribbon.popup = None;
                        view.choose_hyphenation(choice, cx);
                    }))
                    .child(div().size_4().flex_none().when(checked, |cell| {
                        cell.child(
                            gpui::svg()
                                .path("icons/check.svg")
                                .size_4()
                                .text_color(theme::accent()),
                        )
                    }))
                    .child(i18n::text(choice.label_key()))
                    .into_any_element()
            }))
            .into_any_element()
    });
    DocumentView::ribbon_group(
        i18n::text("hyphenation.group"),
        vec![DocumentView::ribbon_row(vec![dropdown(trigger, menu)])],
    )
    .into_any_element()
}

impl DocumentView {
    /// A choice from the ハイフネーション list.
    pub(crate) fn choose_hyphenation(&mut self, choice: HyphenationChoice, cx: &mut Context<Self>) {
        match choice {
            HyphenationChoice::Off => {
                let result = self.document.set_auto_hyphenation(false);
                self.tool_done(result, cx);
            }
            HyphenationChoice::Automatic => {
                let result = self.document.set_auto_hyphenation(true);
                self.tool_done(result, cx);
            }
            HyphenationChoice::Options => self.open_hyphenation_dialog(cx),
        }
    }

    /// ハイフネーションのオプション…
    pub(crate) fn open_hyphenation_dialog(&mut self, cx: &mut Context<Self>) {
        let dialog = self.document.hyphenation_dialog();
        let inputs = Inputs::new(vec![dialog.zone.clone(), dialog.limit.clone()]);
        self.tools.dialog = Some(WordDialog::Hyphenation(HyphenationDialogState {
            dialog,
            inputs,
        }));
        cx.notify();
    }

    pub(crate) fn hyphenation_dialog_body(
        &self,
        state: &HyphenationDialogState,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let toggle = |id: &'static str, key: &'static str, value: bool, auto: bool| {
            check(id, i18n::text(key), value).on_click(on(cx, move |view, cx| {
                view.edit_dialog(cx, |dialog| {
                    if let WordDialog::Hyphenation(state) = dialog {
                        if auto {
                            state.dialog.auto = !state.dialog.auto;
                        } else {
                            state.dialog.hyphenate_caps = !state.dialog.hyphenate_caps;
                        }
                    }
                })
            }))
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(toggle(
                "hyphenation-auto",
                "hyphenation.auto",
                state.dialog.auto,
                true,
            ))
            .child(toggle(
                "hyphenation-caps",
                "hyphenation.caps",
                state.dialog.hyphenate_caps,
                false,
            ))
            .child(
                labeled(i18n::text("hyphenation.zone"))
                    .child(self.dialog_field_element(0, 70.0, cx))
                    .child(i18n::text("setup.unit.mm")),
            )
            .child(
                labeled(i18n::text("hyphenation.limit"))
                    .child(self.dialog_field_element(1, 70.0, cx))
                    .child(
                        div()
                            .text_color(theme::text_faint())
                            .child(i18n::text("hyphenation.no_limit")),
                    ),
            )
            .into_any_element()
    }

    pub(crate) fn apply_hyphenation_dialog(
        &mut self,
        mut state: HyphenationDialogState,
    ) -> Result<(), DialogError> {
        state.dialog.zone = state.inputs.text(0).to_string();
        state.dialog.limit = state.inputs.text(1).to_string();
        match self.document.apply_hyphenation_dialog(&state.dialog) {
            Ok(()) => Ok(()),
            Err(key) if key.starts_with("hyphenation.") => {
                let message = match key.as_str() {
                    "hyphenation.bad_zone" => i18n::text("hyphenation.bad_zone"),
                    _ => i18n::text("hyphenation.bad_limit"),
                };
                Err(DialogError::Stay(
                    Box::new(WordDialog::Hyphenation(state)),
                    message.to_string(),
                ))
            }
            Err(error) => Err(DialogError::Failed(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ribbon::Popup;
    use crate::word_dialog::WordDialog;
    use jade_office_model::word::hyphenation::{HyphenationChoice, WordHyphenation};

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

    /// レイアウト ▸ ハイフネーション ▸ 自動 turns hyphenation on as one undo
    /// step; the options dialog sets the zone, the limit and capitals, and a
    /// zone it cannot take keeps it open.
    #[gpui::test]
    fn hyphenation_is_chosen_from_the_layout_tab(cx: &mut gpui::TestAppContext) {
        use crate::Undo;
        let (view, cx) = open_contract(cx);
        view.update(cx, |view, cx| {
            assert_eq!(view.document.hyphenation_choice(), HyphenationChoice::Off);
            view.ribbon.toggle(Popup::Hyphenation);
            view.choose_hyphenation(HyphenationChoice::Automatic, cx);
            assert_eq!(
                view.document.hyphenation_choice(),
                HyphenationChoice::Automatic
            );
        });
        cx.dispatch_action(Undo);
        view.update(cx, |view, cx| {
            assert_eq!(view.document.hyphenation_choice(), HyphenationChoice::Off);

            view.choose_hyphenation(HyphenationChoice::Options, cx);
            let Some(WordDialog::Hyphenation(state)) = view.tools.dialog.as_mut() else {
                panic!("the ハイフネーション dialog");
            };
            assert_eq!(state.inputs.text(0), "6.35");
            state.dialog.auto = true;
            state.dialog.hyphenate_caps = false;
            state.inputs.set(0, "oops".into());
            view.dialog_ok(cx);
            assert!(
                matches!(view.tools.dialog, Some(WordDialog::Hyphenation(_))),
                "a zone it cannot take keeps the dialog open"
            );
            let Some(WordDialog::Hyphenation(state)) = view.tools.dialog.as_mut() else {
                unreachable!()
            };
            state.inputs.set(0, "10".into());
            state.inputs.set(1, "2".into());
            view.dialog_ok(cx);
            assert!(view.tools.dialog.is_none());
            let settings = view.document.hyphenation_settings();
            assert!(settings.auto && settings.do_not_hyphenate_caps);
            assert_eq!(settings.consecutive_limit, Some(2));
            assert_eq!(settings.zone_twips, Some(567));
        });
    }
}
