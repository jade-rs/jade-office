//! 印刷 in the browser: the native app's settings
//! (`jade_office_model::word::printing`), a preview of how the pages sit
//! on each sheet, and a PDF of the sheets handed to the browser to print —
//! whose own print dialog then takes the printer, copies and two-sided
//! printing.

use gpui::{div, prelude::*, px, AnyElement, Context, KeyDownEvent, MouseButton, SharedString};
use jade_web::word::page::Paper;
use jade_web::word::printing::{
    self, PageLabel, PrintRange, PrintSettings, Sheet, PAGES_PER_SHEET,
};

use super::{DocView, Document};
use crate::{i18n, palette};

/// An 印刷オプション: its label, whether it is on, and what turns it over.
type Toggle = (&'static str, bool, fn(&mut PrintSettings));

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The panel while it is open.
pub(crate) struct PrintPanel {
    pub settings: PrintSettings,
    /// The ユーザー指定 range as typed.
    pub range: String,
    pub sheet: usize,
    pub problem: Option<SharedString>,
}

impl PrintPanel {
    fn effective(&self) -> PrintSettings {
        let mut settings = self.settings.clone();
        if let PrintRange::Custom(_) = settings.range {
            settings.range = PrintRange::Custom(self.range.clone());
        }
        settings
    }
}

impl DocView {
    pub(crate) fn open_print_panel(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.document, Document::Pages { .. }) {
            return;
        }
        self.print_panel = Some(Box::new(PrintPanel {
            settings: PrintSettings::default(),
            range: String::new(),
            sheet: 0,
            problem: None,
        }));
        cx.notify();
    }

    fn change_print(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut PrintPanel)) {
        if let Some(panel) = self.print_panel.as_mut() {
            change(panel);
            panel.problem = None;
        }
        cx.notify();
    }

    /// The sheets the settings make of the open document's pages.
    fn print_sheets(&self, settings: &PrintSettings) -> Result<Vec<Sheet>, String> {
        let Document::Pages {
            document, current, ..
        } = &self.document
        else {
            return Ok(Vec::new());
        };
        let labels: Vec<PageLabel> = document
            .page_labels()
            .into_iter()
            .map(|(section, number)| PageLabel {
                section: section + 1,
                number,
            })
            .collect();
        let (width, height) = document.page_size();
        let sizes = vec![(width * 0.75, height * 0.75); labels.len()];
        let pages = printing::pages_to_print(settings, &labels, *current)
            .map_err(|error| error.to_string())?;
        Ok(printing::sheets(settings, &pages, &sizes))
    }

    fn print_now(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.print_panel.as_ref().map(|panel| panel.effective()) else {
            return;
        };
        let Document::Pages {
            document, current, ..
        } = &mut self.document
        else {
            return;
        };
        let current = *current;
        if settings.update_fields {
            // 印刷前にフィールドを更新する: in the document, as Word does.
            printing::update_fields(document.as_mut());
        }
        match document.print_pdf(&settings, current) {
            Ok(pdf) => {
                self.print_panel = None;
                let name = format!(
                    "{}.pdf",
                    self.name
                        .rsplit_once('.')
                        .map_or(self.name.as_ref(), |(stem, _)| stem)
                );
                self.host.print(&name, &pdf);
            }
            Err(error) => {
                if let Some(panel) = self.print_panel.as_mut() {
                    panel.problem = Some(error.into());
                }
            }
        }
        self.refresh_repainted();
        cx.notify();
    }

    /// The keys the panel takes: Escape closes it, Enter prints, and what
    /// a range is typed with goes into the range.
    pub(crate) fn print_panel_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        let Some(panel) = self.print_panel.as_mut() else {
            return false;
        };
        let key = event.keystroke.key.as_str();
        match key {
            "escape" => self.print_panel = None,
            "enter" => {
                self.print_now(cx);
                return true;
            }
            "backspace" => {
                panel.range.pop();
                panel.settings.range = PrintRange::Custom(panel.range.clone());
            }
            _ => {
                let typed = event.keystroke.key_char.as_deref().unwrap_or(key);
                if typed.chars().all(|ch| {
                    ch.is_ascii_digit() || matches!(ch, ',' | '-' | 'p' | 's' | 'P' | 'S' | ' ')
                }) && !typed.is_empty()
                    && typed.chars().count() == 1
                {
                    panel.range.push_str(typed);
                    panel.settings.range = PrintRange::Custom(panel.range.clone());
                }
            }
        }
        cx.notify();
        true
    }

    pub(crate) fn print_panel_element(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let panel = self.print_panel.as_ref()?;
        let settings = panel.effective();
        let sheets = self.print_sheets(&settings);
        let chip = |id: SharedString, label: SharedString, on: bool| {
            div()
                .id(id)
                .px_2()
                .py_0p5()
                .rounded_sm()
                .border_1()
                .border_color(if on {
                    palette::accent()
                } else {
                    palette::border()
                })
                .when(on, |chip| chip.bg(palette::selected()))
                .cursor_pointer()
                .child(label)
        };
        let range_kind = match settings.range {
            PrintRange::All => 0,
            PrintRange::Current => 1,
            PrintRange::Selection => 2,
            PrintRange::Custom(_) => 3,
        };
        let ranges = [
            ("print.all", PrintRange::All),
            ("print.current", PrintRange::Current),
            ("print.selection", PrintRange::Selection),
        ];
        let toggles: [Toggle; 4] = [
            ("print.markup", settings.markup, |s| s.markup = !s.markup),
            ("print.update_fields", settings.update_fields, |s| {
                s.update_fields = !s.update_fields
            }),
            ("print.background", settings.background, |s| {
                s.background = !s.background
            }),
            ("print.hidden_text", settings.hidden_text, |s| {
                s.hidden_text = !s.hidden_text
            }),
        ];
        let preview = match &sheets {
            Ok(sheets) => {
                let index = panel.sheet.min(sheets.len().saturating_sub(1));
                let count = sheets.len();
                let sheet = sheets.get(index).map(|sheet| {
                    let scale = (360.0 / sheet.width as f32).min(420.0 / sheet.height as f32);
                    div()
                        .relative()
                        .w(px(sheet.width as f32 * scale))
                        .h(px(sheet.height as f32 * scale))
                        .bg(gpui::white())
                        .shadow_md()
                        .children(sheet.placements.iter().map(|placement| {
                            let (width, height) = match &self.document {
                                Document::Pages { document, .. } => {
                                    let (w, h) = document.page_size();
                                    (w * 0.75, h * 0.75)
                                }
                                _ => (612.0, 792.0),
                            };
                            div()
                                .absolute()
                                .left(px(placement.x as f32 * scale))
                                .top(px(placement.y as f32 * scale))
                                .w(px((width * placement.scale) as f32 * scale))
                                .h(px((height * placement.scale) as f32 * scale))
                                .border_1()
                                .border_color(gpui::rgb(0x9ca3af))
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_color(gpui::rgb(0x6b7280))
                                .child((placement.page + 1).to_string())
                        }))
                });
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_2()
                    .children(sheet)
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .items_center()
                            .child(chip("print-previous".into(), "‹".into(), false).on_click(
                                cx.listener(|view, _, _, cx| {
                                    view.change_print(cx, |p| p.sheet = p.sheet.saturating_sub(1))
                                }),
                            ))
                            .child(i18n::format(
                                "print.sheet_of",
                                &[("n", &(index + 1)), ("count", &count.max(1))],
                            ))
                            .child(chip("print-next".into(), "›".into(), false).on_click(
                                cx.listener(move |view, _, _, cx| {
                                    view.change_print(cx, |p| {
                                        p.sheet = (p.sheet + 1).min(count.saturating_sub(1))
                                    })
                                }),
                            )),
                    )
                    .into_any_element()
            }
            Err(problem) => div()
                .text_color(gpui::rgb(0xD13438))
                .child(problem.clone())
                .into_any_element(),
        };
        let settings_column = div()
            .flex()
            .flex_col()
            .gap_2()
            .w(px(300.0))
            .child(div().text_color(palette::dim()).child(t("print.pages")))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .children(ranges.into_iter().enumerate().map(|(index, (key, range))| {
                        chip(
                            format!("print-range-{index}").into(),
                            t(key).into(),
                            range_kind == index,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            let range = range.clone();
                            view.change_print(cx, move |p| p.settings.range = range)
                        }))
                    }))
                    .child(
                        chip(
                            "print-range-custom".into(),
                            if panel.range.is_empty() {
                                t("print.custom").into()
                            } else {
                                SharedString::from(panel.range.clone())
                            },
                            range_kind == 3,
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.change_print(cx, |p| {
                                p.settings.range = PrintRange::Custom(p.range.clone())
                            })
                        })),
                    ),
            )
            .child(
                div()
                    .text_color(palette::faint())
                    .child(t("print.range_hint")),
            )
            .child(div().text_color(palette::dim()).child(t("print.per_sheet")))
            .child(
                div()
                    .flex()
                    .gap_1()
                    .children(PAGES_PER_SHEET.into_iter().map(|count| {
                        chip(
                            format!("print-per-sheet-{count}").into(),
                            count.to_string().into(),
                            settings.pages_per_sheet == count,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.change_print(cx, |p| p.settings.pages_per_sheet = count)
                        }))
                    })),
            )
            .child(div().text_color(palette::dim()).child(t("print.scale_to")))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(
                        chip(
                            "print-scale-none".into(),
                            t("print.no_scaling").into(),
                            settings.scale_to.is_none(),
                        )
                        .on_click(cx.listener(|view, _, _, cx| {
                            view.change_print(cx, |p| p.settings.scale_to = None)
                        })),
                    )
                    .children(Paper::ALL.into_iter().map(|paper| {
                        chip(
                            format!("print-scale-{}", paper.name()).into(),
                            paper.name().into(),
                            settings.scale_to == Some(paper),
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.change_print(cx, |p| p.settings.scale_to = Some(paper))
                        }))
                    })),
            )
            .child(div().text_color(palette::dim()).child(t("print.options")))
            .children(
                toggles
                    .into_iter()
                    .enumerate()
                    .map(|(index, (key, on, toggle))| {
                        chip(
                            format!("print-option-{index}").into(),
                            SharedString::from(format!(
                                "{} {}",
                                if on { "☑" } else { "☐" },
                                t(key)
                            )),
                            on,
                        )
                        .on_click(cx.listener(move |view, _, _, cx| {
                            view.change_print(cx, |p| toggle(&mut p.settings))
                        }))
                    }),
            )
            .children(
                panel
                    .problem
                    .clone()
                    .map(|problem| div().text_color(gpui::rgb(0xD13438)).child(problem)),
            )
            .child(
                div()
                    .flex()
                    .gap_2()
                    .justify_end()
                    .child(
                        chip("print-cancel".into(), t("format.cancel").into(), false).on_click(
                            cx.listener(|view, _, _, cx| {
                                view.print_panel = None;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        chip("print-go".into(), t("print.print").into(), true)
                            .on_click(cx.listener(|view, _, _, cx| view.print_now(cx))),
                    ),
            );
        Some(
            div()
                .id("print-backdrop")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000055))
                .occlude()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .id("print-panel")
                        .p_4()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .rounded_md()
                        .border_1()
                        .border_color(palette::border())
                        .bg(palette::panel())
                        .shadow_lg()
                        .text_xs()
                        .text_color(palette::text())
                        .child(div().text_sm().child(t("print.title")))
                        .child(div().flex().gap_4().child(settings_column).child(preview)),
                )
                .into_any_element(),
        )
    }
}
