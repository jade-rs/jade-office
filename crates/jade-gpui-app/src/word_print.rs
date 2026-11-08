//! Word's 印刷 (⌘P): settings beside a preview of the sheets they print.
//!
//! The settings are `jade-office-model`'s (`word::printing`): 部数 and 部単位,
//! the pages (すべて, 現在のページ, 選択した部分, or a range such as
//! `1,3,5-12` or `p1s2-p3s3`), 変更履歴/コメントの印刷, 両面印刷, 1 枚あたりの
//! ページ数, 用紙サイズに合わせる, and the 印刷オプション. The preview shows a
//! sheet at a time as it will print, with ‹ › to step through them and a
//! zoom; 印刷 hands the sheets' PDF to the system's print panel with the
//! copies, collation and two-sided setting already chosen, and PDF writes
//! the same PDF to a file.

use std::sync::Arc;

use gpui::{
    anchored, deferred, div, img, point, prelude::*, px, AnyElement, ClickEvent, Context,
    MouseButton, RenderImage, SharedString, Window,
};
use jade_docx_native_render::{NativeDocument, PdfExportOptions, PdfRenderingMode};
use jade_office_model::text::TextEdit;
use jade_office_model::word::page::Paper;
use jade_office_model::word::printing::{
    self, Duplex, PageLabel, PrintRange, PrintSettings, Sheet, PAGES_PER_SHEET,
};

use crate::word_characters::{button, caption, check};
use crate::{i18n, theme, DocumentView};

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// The 印刷 panel while it is open.
pub struct PrintPanel {
    pub settings: PrintSettings,
    /// The ユーザー指定 range as typed.
    pub range: TextEdit,
    /// Whether typing goes into the range.
    pub typing: bool,
    /// The sheet the preview shows.
    pub sheet: usize,
    /// The preview's zoom: 1.0 fits the sheet to the space.
    pub zoom: f32,
    /// Why the settings print nothing, when they do not.
    pub problem: Option<SharedString>,
    preview: Option<Preview>,
}

/// What the preview shows, made for the settings it was made for.
struct Preview {
    settings: PrintSettings,
    /// The document laid out for printing, when it is not the open one.
    document: Option<NativeDocument>,
    images: Vec<Arc<RenderImage>>,
    /// Each page's size in points.
    sizes: Vec<(f64, f64)>,
    sheets: Vec<Sheet>,
}

impl PrintPanel {
    pub fn new() -> Self {
        Self {
            settings: PrintSettings::default(),
            range: TextEdit::new(String::new()),
            typing: false,
            sheet: 0,
            zoom: 1.0,
            problem: None,
            preview: None,
        }
    }

    pub fn field(&self) -> Option<&TextEdit> {
        self.typing.then_some(&self.range)
    }

    pub fn field_mut(&mut self) -> Option<&mut TextEdit> {
        self.typing.then_some(&mut self.range)
    }

    /// The settings with the typed range in them.
    fn effective(&self) -> PrintSettings {
        let mut settings = self.settings.clone();
        if let PrintRange::Custom(_) = settings.range {
            settings.range = PrintRange::Custom(self.range.text.clone());
        }
        settings
    }
}

fn validate_print_selection(
    document: &NativeDocument,
    settings: &PrintSettings,
) -> Result<(), String> {
    if settings.range == PrintRange::Selection && document.selection().is_none() {
        return Err("Select content before printing the selection.".into());
    }
    Ok(())
}

/// The document `settings` print, laid out: the open one when it prints as
/// it is, otherwise one made for the purpose.
fn print_document(
    source: &NativeDocument,
    settings: &PrintSettings,
) -> Result<Option<NativeDocument>, String> {
    validate_print_selection(source, settings)?;
    let docx = source.export_docx()?;
    let mut changed = printing::has_background(&docx) && !settings.background
        || printing::has_hidden_text(&docx) && settings.hidden_text
        || !settings.markup;
    let mut bytes = printing::print_docx(&docx, settings)?;
    if settings.range == PrintRange::Selection {
        if let Some((start, end)) = source.selection() {
            bytes = printing::selection_docx(&bytes, start, end)?;
            changed = true;
        }
    }
    if !changed {
        return Ok(None);
    }
    let mut document = crate::open_document_bytes(&bytes)?;
    crate::word_characters::start_field_clock(&mut document);
    if !settings.markup {
        document.set_markup_visible(false)?;
    }
    Ok(Some(document))
}

/// Each page's label and size in points.
fn page_facts(document: &NativeDocument) -> (Vec<PageLabel>, Vec<(f64, f64)>) {
    let labels = document
        .page_labels()
        .into_iter()
        .map(|(section, number)| PageLabel {
            section: section + 1,
            number,
        })
        .collect();
    let fallback = document.layout_page_size();
    let sizes = (0..document.page_count())
        .map(|index| {
            let (width, height) = document
                .renderer()
                .page_dimensions(index)
                .unwrap_or(fallback);
            (width * 0.75, height * 0.75)
        })
        .collect();
    (labels, sizes)
}

impl DocumentView {
    /// ⌘P: the 印刷 panel, with the preview of what prints.
    pub(crate) fn open_print_panel(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.close_story_editing() {
            self.table_status = Some(SharedString::from(error));
        }
        self.printing = Some(PrintPanel::new());
        self.refresh_print_preview();
        cx.notify();
    }

    fn close_print_panel(&mut self, cx: &mut Context<Self>) {
        self.printing = None;
        cx.notify();
    }

    /// Lays the preview out again when the settings it was made for have
    /// changed.
    pub(crate) fn refresh_print_preview(&mut self) {
        let Some(panel) = self.printing.as_ref() else {
            return;
        };
        let settings = panel.effective();
        let stale = panel
            .preview
            .as_ref()
            .is_none_or(|preview| preview.settings != settings);
        if !stale {
            return;
        }
        let result = self.build_preview(&settings);
        let Some(panel) = self.printing.as_mut() else {
            return;
        };
        match result {
            Ok(preview) => {
                panel.sheet = panel.sheet.min(preview.sheets.len().saturating_sub(1));
                panel.problem = None;
                panel.preview = Some(preview);
            }
            Err(problem) => {
                panel.problem = Some(problem.into());
                panel.preview = None;
            }
        }
    }

    fn build_preview(&mut self, settings: &PrintSettings) -> Result<Preview, String> {
        validate_print_selection(&self.document, settings)?;
        // The document laid out again only when what it prints differs; a
        // range or a layout change reuses it.
        let reuse = self.printing.as_mut().and_then(|panel| {
            let preview = panel.preview.take()?;
            let same_document = preview.settings.markup == settings.markup
                && preview.settings.background == settings.background
                && preview.settings.hidden_text == settings.hidden_text
                && preview.settings.update_fields == settings.update_fields
                && (preview.settings.range == PrintRange::Selection)
                    == (settings.range == PrintRange::Selection);
            same_document.then_some(preview)
        });
        let (document, images) = match reuse {
            Some(preview) => (preview.document, preview.images),
            None => {
                let document = if settings.update_fields {
                    // Predict the actual print-time update without changing
                    // the editor's document, selection or undo history.
                    let mut snapshot = crate::open_document_bytes(&self.document.export_docx()?)?;
                    snapshot.set_selection_range(self.document.selection_range());
                    printing::update_fields(&mut snapshot)?;
                    crate::word_characters::start_field_clock(&mut snapshot);
                    print_document(&snapshot, settings)?.or(Some(snapshot))
                } else {
                    print_document(&self.document, settings)?
                };
                let images = match &document {
                    Some(document) => (0..document.page_count())
                        .filter_map(|index| crate::page_image(document, index))
                        .collect(),
                    None => self.images.clone(),
                };
                (document, images)
            }
        };
        let printed: &NativeDocument = document.as_ref().unwrap_or(&self.document);
        let (labels, sizes) = page_facts(printed);
        let pages = printing::pages_to_print(settings, &labels, self.current_page)
            .map_err(|error| range_problem(&error))?;
        let sheets = printing::sheets(settings, &pages, &sizes);
        Ok(Preview {
            settings: settings.clone(),
            document,
            images,
            sizes,
            sheets,
        })
    }

    /// The PDF the settings print: the document's pages, vector, on their
    /// sheets.
    pub(crate) fn print_pdf(&mut self, settings: &PrintSettings) -> Result<Vec<u8>, String> {
        // Reject an empty selection before print-time field updates can edit
        // the document. Preview and output use the same validation.
        validate_print_selection(&self.document, settings)?;
        if settings.update_fields {
            // 印刷前にフィールドを更新する: in the document itself, as Word
            // does — the table of contents, and the date and time now.
            printing::update_fields(&mut self.document)?;
            crate::word_characters::start_field_clock(&mut self.document);
            self.rebuild_all_images();
        }
        let document = print_document(&self.document, settings)?;
        let printed: &NativeDocument = document.as_ref().unwrap_or(&self.document);
        let pdf = printed.export_pdf_with_options(PdfExportOptions {
            rendering: PdfRenderingMode::Vector,
            markup: Some(settings.markup),
            ..Default::default()
        })?;
        let (labels, sizes) = page_facts(printed);
        let pages = printing::pages_to_print(settings, &labels, self.current_page)
            .map_err(|error| range_problem(&error))?;
        let sheets = printing::sheets(settings, &pages, &sizes);
        printing::impose_pdf(&pdf, &sheets)
    }

    fn print_now(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.printing.as_ref().map(PrintPanel::effective) else {
            return;
        };
        match self.print_pdf(&settings) {
            Ok(pdf) => {
                self.printing = None;
                cx.notify();
                let job = crate::native_print::PrintJob {
                    copies: settings.copies,
                    collate: settings.collate,
                    duplex: settings.duplex,
                };
                if let Err(error) = crate::native_print::show_pdf_print_dialog_with(&pdf, job) {
                    self.show_message(t("print.title"), &error, cx);
                }
            }
            Err(error) => {
                if let Some(panel) = self.printing.as_mut() {
                    panel.problem = Some(error.into());
                }
                cx.notify();
            }
        }
    }

    fn print_to_pdf_file(&mut self, cx: &mut Context<Self>) {
        let Some(settings) = self.printing.as_ref().map(PrintPanel::effective) else {
            return;
        };
        let source = std::path::PathBuf::from(self.path.to_string());
        let suggested = format!(
            "{}.pdf",
            source
                .file_stem()
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_else(|| "document".to_string())
        );
        let prompt = cx.prompt_for_new_path(&self.save_directory(), Some(&suggested));
        cx.spawn(async move |view, cx| {
            let Ok(Ok(Some(path))) = prompt.await else {
                return;
            };
            view.update(cx, |view, cx| {
                match view
                    .print_pdf(&settings)
                    .and_then(|pdf| std::fs::write(&path, pdf).map_err(|e| e.to_string()))
                {
                    Ok(()) => {
                        view.printing = None;
                        view.saved_to = Some(SharedString::from(crate::shell::file_name(&path)));
                    }
                    Err(error) => {
                        if let Some(panel) = view.printing.as_mut() {
                            panel.problem = Some(error.into());
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn change_print(&mut self, change: impl FnOnce(&mut PrintPanel), cx: &mut Context<Self>) {
        if let Some(panel) = self.printing.as_mut() {
            change(panel);
        }
        self.refresh_print_preview();
        cx.notify();
    }
}

fn range_problem(error: &printing::RangeError) -> String {
    match error {
        printing::RangeError::Syntax(part) => i18n::format("print.range_syntax", &[("part", part)]),
        printing::RangeError::NoSuchPage(part) => {
            i18n::format("print.range_missing", &[("part", part)])
        }
    }
}

/// The keys the panel takes while it is open. Whether it was open.
pub fn handle_key(view: &mut DocumentView, key: &str, cx: &mut Context<DocumentView>) -> bool {
    let Some(panel) = view.printing.as_mut() else {
        return false;
    };
    if panel.typing && panel.range.marked.is_some() {
        return false;
    }
    match key {
        "escape" => {
            if panel.typing {
                panel.typing = false;
                cx.notify();
            } else {
                view.close_print_panel(cx);
            }
        }
        "enter" => {
            if panel.typing {
                panel.typing = false;
                view.refresh_print_preview();
                cx.notify();
            } else {
                view.print_now(cx);
            }
        }
        "left" | "pageup" if !panel.typing => {
            panel.sheet = panel.sheet.saturating_sub(1);
            cx.notify();
        }
        "right" | "pagedown" if !panel.typing => {
            let last = panel
                .preview
                .as_ref()
                .map(|preview| preview.sheets.len().saturating_sub(1))
                .unwrap_or(0);
            panel.sheet = (panel.sheet + 1).min(last);
            cx.notify();
        }
        _ if panel.typing => {
            if panel.range.edit_key(key) {
                cx.notify();
            }
        }
        _ => {}
    }
    true
}

// ---- the panel ---------------------------------------------------------------------

fn choice(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    on: bool,
) -> gpui::Stateful<gpui::Div> {
    let id: SharedString = id.into();
    div()
        .id(id)
        .px_2()
        .py(px(3.0))
        .rounded_sm()
        .border_1()
        .border_color(if on { theme::accent() } else { theme::border() })
        .when(on, |choice| choice.bg(theme::selected()))
        .cursor_pointer()
        .hover(|choice| choice.bg(theme::hovered()))
        .child(label.into())
}

fn settings_column(
    panel: &PrintPanel,
    has_selection: bool,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let settings = &panel.settings;
    let copies = settings.copies;
    let range_kind = match settings.range {
        PrintRange::All => 0,
        PrintRange::Current => 1,
        PrintRange::Selection => 2,
        PrintRange::Custom(_) => 3,
    };
    let ranges = [
        ("print-all", "print.all", PrintRange::All),
        ("print-current", "print.current", PrintRange::Current),
        ("print-selection", "print.selection", PrintRange::Selection),
    ];
    let (before, after) = panel.range.split();
    let range_field = div()
        .id("print-range-field")
        .flex()
        .items_center()
        .w(px(200.0))
        .h(px(24.0))
        .px_2()
        .rounded_sm()
        .border_1()
        .border_color(if panel.typing {
            theme::accent()
        } else {
            theme::border()
        })
        .bg(theme::canvas())
        .cursor_text()
        .child(before.to_string())
        .when(panel.typing, |field| {
            field.child(div().w(px(1.0)).h(px(14.0)).bg(theme::accent()))
        })
        .child(after.to_string())
        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
            view.change_print(
                |panel| {
                    panel.typing = true;
                    panel.settings.range = PrintRange::Custom(panel.range.text.clone());
                },
                cx,
            )
        }));
    let duplexes = [
        ("print-simplex", "print.one_sided", Duplex::OneSided),
        ("print-long", "print.long_edge", Duplex::LongEdge),
        ("print-short", "print.short_edge", Duplex::ShortEdge),
    ];
    /// A checkbox: its id, label key, whether it is on, and its toggle.
    type Toggle = (&'static str, &'static str, bool, fn(&mut PrintSettings));
    let options: [Toggle; 4] = [
        ("print-markup", "print.markup", settings.markup, |s| {
            s.markup = !s.markup
        }),
        (
            "print-fields",
            "print.update_fields",
            settings.update_fields,
            |s| s.update_fields = !s.update_fields,
        ),
        (
            "print-background",
            "print.background",
            settings.background,
            |s| s.background = !s.background,
        ),
        (
            "print-hidden",
            "print.hidden_text",
            settings.hidden_text,
            |s| s.hidden_text = !s.hidden_text,
        ),
    ];
    div()
        .id("print-settings")
        .w(px(300.0))
        .h_full()
        .flex_none()
        .overflow_y_scroll()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .border_r_1()
        .border_color(theme::border())
        .child(div().text_sm().child(t("print.title")))
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(caption(t("print.copies")))
                .child(
                    button("print-copies-less", "−", false).on_click(cx.listener(
                        |view, _: &ClickEvent, _, cx| {
                            view.change_print(
                                |p| p.settings.copies = p.settings.copies.saturating_sub(1).max(1),
                                cx,
                            )
                        },
                    )),
                )
                .child(div().w(px(28.0)).text_center().child(copies.to_string()))
                .child(
                    button("print-copies-more", "+", false).on_click(cx.listener(
                        |view, _: &ClickEvent, _, cx| {
                            view.change_print(
                                |p| p.settings.copies = (p.settings.copies + 1).min(999),
                                cx,
                            )
                        },
                    )),
                )
                .child(
                    check("print-collate", t("print.collate"), settings.collate).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            view.change_print(|p| p.settings.collate = !p.settings.collate, cx)
                        }),
                    ),
                ),
        )
        .child(caption(t("print.pages")))
        .children(
            ranges
                .into_iter()
                .enumerate()
                .map(|(index, (id, key, range))| {
                    let enabled = range != PrintRange::Selection || has_selection;
                    check(id, t(key), range_kind == index)
                        .when(!enabled, |row| row.opacity(0.4))
                        .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                            if enabled {
                                let range = range.clone();
                                view.change_print(
                                    move |panel| {
                                        panel.settings.range = range;
                                        panel.typing = false;
                                    },
                                    cx,
                                )
                            }
                        }))
                }),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    check("print-custom", t("print.custom"), range_kind == 3).on_click(
                        cx.listener(|view, _: &ClickEvent, _, cx| {
                            view.change_print(
                                |panel| {
                                    panel.typing = true;
                                    panel.settings.range =
                                        PrintRange::Custom(panel.range.text.clone());
                                },
                                cx,
                            )
                        }),
                    ),
                )
                .child(range_field),
        )
        .child(
            div()
                .text_color(theme::text_faint())
                .child(t("print.range_hint")),
        )
        .child(caption(t("print.duplex")))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .children(duplexes.into_iter().map(|(id, key, duplex)| {
                    choice(id, t(key), settings.duplex == duplex).on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| {
                            view.change_print(|p| p.settings.duplex = duplex, cx)
                        },
                    ))
                })),
        )
        .child(caption(t("print.per_sheet")))
        .child(
            div()
                .flex()
                .gap_1()
                .children(PAGES_PER_SHEET.into_iter().map(|count| {
                    choice(
                        SharedString::from(format!("print-per-sheet-{count}")),
                        count.to_string(),
                        settings.pages_per_sheet == count,
                    )
                    .on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| {
                            view.change_print(|p| p.settings.pages_per_sheet = count, cx)
                        },
                    ))
                })),
        )
        .child(caption(t("print.scale_to")))
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_1()
                .child(
                    choice(
                        "print-scale-none",
                        t("print.no_scaling"),
                        settings.scale_to.is_none(),
                    )
                    .on_click(cx.listener(|view, _: &ClickEvent, _, cx| {
                        view.change_print(|p| p.settings.scale_to = None, cx)
                    })),
                )
                .children(Paper::ALL.into_iter().map(|paper| {
                    choice(
                        SharedString::from(format!("print-scale-{}", paper.name())),
                        paper.name(),
                        settings.scale_to == Some(paper),
                    )
                    .on_click(cx.listener(
                        move |view, _: &ClickEvent, _, cx| {
                            view.change_print(|p| p.settings.scale_to = Some(paper), cx)
                        },
                    ))
                })),
        )
        .child(caption(t("print.options")))
        .children(options.into_iter().map(|(id, key, on, toggle)| {
            check(id, t(key), on).on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                view.change_print(|p| toggle(&mut p.settings), cx)
            }))
        }))
        .children(
            panel
                .problem
                .clone()
                .map(|problem| div().text_color(gpui::rgb(0xd13438)).child(problem)),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .pt_2()
                .child(button("print-cancel", t("format.cancel"), false).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| view.close_print_panel(cx)),
                ))
                .child(button("print-pdf", t("print.pdf"), false).on_click(
                    cx.listener(|view, _: &ClickEvent, _, cx| view.print_to_pdf_file(cx)),
                ))
                .child(
                    button("print-go", t("print.print"), true)
                        .on_click(cx.listener(|view, _: &ClickEvent, _, cx| view.print_now(cx))),
                ),
        )
        .into_any_element()
}

fn preview_area(
    panel: &PrintPanel,
    width: f32,
    height: f32,
    cx: &mut Context<DocumentView>,
) -> AnyElement {
    let Some(preview) = panel.preview.as_ref() else {
        return div().flex_1().into_any_element();
    };
    let count = preview.sheets.len();
    let index = panel.sheet.min(count.saturating_sub(1));
    let sheet_element = preview.sheets.get(index).map(|sheet| {
        // Fitted to the space, then the zoom.
        let fit = ((width - 80.0) / sheet.width as f32).min((height - 120.0) / sheet.height as f32);
        let scale = (fit * panel.zoom).max(0.05);
        div()
            .relative()
            .flex_none()
            .w(px(sheet.width as f32 * scale))
            .h(px(sheet.height as f32 * scale))
            .bg(gpui::white())
            .shadow_lg()
            .children(sheet.placements.iter().filter_map(|placement| {
                let image = preview.images.get(placement.page)?.clone();
                let (page_width, page_height) = preview
                    .sizes
                    .get(placement.page)
                    .map(|(w, h)| (*w as f32, *h as f32))?;
                Some(
                    div()
                        .absolute()
                        .left(px(placement.x as f32 * scale))
                        .top(px(placement.y as f32 * scale))
                        .w(px(page_width * placement.scale as f32 * scale))
                        .h(px(page_height * placement.scale as f32 * scale))
                        .border_1()
                        .border_color(gpui::rgba(0x00000010))
                        .child(img(image).size_full()),
                )
            }))
    });
    div()
        .id("print-preview")
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .bg(theme::canvas())
        .child(
            div()
                .id("print-preview-scroll")
                .flex_1()
                .w_full()
                .overflow_scroll()
                .flex()
                .items_center()
                .justify_center()
                .p_6()
                .children(sheet_element),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .py_2()
                .child(button("print-previous", "‹", false).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.change_print(|p| p.sheet = p.sheet.saturating_sub(1), cx)
                    },
                )))
                .child(i18n::format(
                    "print.sheet_of",
                    &[("n", &(index + 1)), ("count", &count.max(1))],
                ))
                .child(button("print-next", "›", false).on_click(cx.listener(
                    move |view, _: &ClickEvent, _, cx| {
                        view.change_print(
                            |p| p.sheet = (p.sheet + 1).min(count.saturating_sub(1)),
                            cx,
                        )
                    },
                )))
                .child(div().w(px(24.0)))
                .child(button("print-zoom-out", "−", false).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.change_print(|p| p.zoom = (p.zoom / 1.25).max(0.25), cx)
                    },
                )))
                .child(format!("{}%", (panel.zoom * 100.0).round()))
                .child(button("print-zoom-in", "+", false).on_click(cx.listener(
                    |view, _: &ClickEvent, _, cx| {
                        view.change_print(|p| p.zoom = (p.zoom * 1.25).min(4.0), cx)
                    },
                )))
                .child(
                    button("print-zoom-fit", t("print.fit"), false).on_click(cx.listener(
                        |view, _: &ClickEvent, _, cx| view.change_print(|p| p.zoom = 1.0, cx),
                    )),
                ),
        )
        .into_any_element()
}

/// The panel over the window, while it is open.
pub fn overlay(
    view: &mut DocumentView,
    window: &mut Window,
    cx: &mut Context<DocumentView>,
) -> Option<AnyElement> {
    let panel = view.printing.as_ref()?;
    let viewport = window.viewport_size();
    let has_selection = view.document.selection().is_some();
    let settings = settings_column(panel, has_selection, cx);
    let preview_width = f32::from(viewport.width) - 300.0;
    let preview = preview_area(panel, preview_width, f32::from(viewport.height), cx);
    Some(
        deferred(
            anchored().position(point(px(0.0), px(0.0))).child(
                div()
                    .id("print-panel")
                    .w(viewport.width)
                    .h(viewport.height)
                    .flex()
                    .bg(theme::panel())
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .font_family(crate::UI_FONT_FAMILY)
                    .text_xs()
                    .text_color(theme::text())
                    .child(settings)
                    .child(preview),
            ),
        )
        .with_priority(5)
        .into_any_element(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use jade_office_model::word::WordSession;

    fn open(
        cx: &mut gpui::TestAppContext,
    ) -> (gpui::Entity<DocumentView>, &mut gpui::VisualTestContext) {
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

    fn pdf_pages(pdf: &[u8]) -> Vec<(f32, f32)> {
        let document = lopdf::Document::load_mem(pdf).unwrap();
        document
            .get_pages()
            .values()
            .map(|id| {
                let page = document.get_dictionary(*id).unwrap();
                let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
                (media[2].as_float().unwrap(), media[3].as_float().unwrap())
            })
            .collect()
    }

    /// ⌘P opens the panel on a preview of every page; two to a sheet halves
    /// the sheets, a range picks pages, and a range that names no page says
    /// so rather than printing nothing.
    #[gpui::test]
    fn the_preview_follows_the_settings(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        cx.dispatch_action(crate::Print);
        let pages = view.read_with(cx, |view, _| {
            let panel = view.printing.as_ref().expect("the panel is open");
            let preview = panel.preview.as_ref().expect("with a preview");
            assert_eq!(preview.sheets.len(), view.document.page_count());
            view.document.page_count()
        });
        assert!(pages >= 2);
        view.update(cx, |view, cx| {
            view.change_print(|p| p.settings.pages_per_sheet = 2, cx)
        });
        view.read_with(cx, |view, _| {
            let preview = view.printing.as_ref().unwrap().preview.as_ref().unwrap();
            assert_eq!(preview.sheets.len(), pages.div_ceil(2));
            assert_eq!(preview.sheets[0].placements.len(), 2);
        });
        view.update(cx, |view, cx| {
            view.change_print(
                |p| {
                    p.settings.pages_per_sheet = 1;
                    p.settings.range = PrintRange::Custom(String::new());
                    p.range = TextEdit::new("2".into());
                },
                cx,
            )
        });
        view.read_with(cx, |view, _| {
            let preview = view.printing.as_ref().unwrap().preview.as_ref().unwrap();
            assert_eq!(preview.sheets.len(), 1);
            assert_eq!(preview.sheets[0].placements[0].page, 1);
        });
        view.update(cx, |view, cx| {
            view.change_print(|p| p.range = TextEdit::new("p9s9".into()), cx)
        });
        view.read_with(cx, |view, _| {
            let panel = view.printing.as_ref().unwrap();
            assert!(panel.preview.is_none());
            assert!(panel.problem.is_some());
        });
        // Escape closes it.
        cx.simulate_keystrokes("escape");
        view.read_with(cx, |view, _| assert!(view.printing.is_none()));
    }

    #[gpui::test]
    fn print_field_updates_cover_all_tables_and_roll_back_on_error(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        let toc = r#"<w:r><w:fldChar w:fldCharType="begin"/></w:r><w:r><w:instrText> TOC \o "1-3" \u </w:instrText></w:r><w:r><w:fldChar w:fldCharType="separate"/></w:r><w:r><w:t>Old cached contents</w:t></w:r><w:r><w:fldChar w:fldCharType="end"/></w:r>"#;
        for (first, fails) in [
            (format!("<w:p>{toc}</w:p>"), false),
            (format!("<w:p><w:bookmarkStart w:id=\"1\" w:name=\"Around\"/>{toc}<w:bookmarkEnd w:id=\"1\"/></w:p>"), false),
            (format!("<w:p><w:commentRangeStart w:id=\"9\"/>{toc}<w:commentRangeEnd w:id=\"9\"/><w:r><w:commentReference w:id=\"9\"/></w:r></w:p>"), false),
            (format!("<w:p><w:permStart w:id=\"9\" w:edGrp=\"everyone\"/>{toc}<w:permEnd w:id=\"9\"/></w:p>"), false),
            (format!("<w:p><w:moveFromRangeStart w:id=\"9\" w:name=\"MovedText\"/>{toc}<w:moveFromRangeEnd w:id=\"9\"/></w:p>"), true),
            (format!("<w:p>{}</w:p>", toc.replacen("w:fldCharType=\"begin\"", "w:fldCharType=\"begin\" w:fldLock=\"true\"", 1)), false),
        ] {
            let blank = jade_office_model::word::new::blank_document(jade_office_model::word::new::Blank::English);
            let mut package = jade_opc::OpcPackage::open(&blank).unwrap();
            let xml = package.part_str("word/document.xml").unwrap().unwrap().replace(
                "<w:body><w:p/>",
                &format!(r#"<w:body>{first}<w:p><w:r><w:t>Before </w:t></w:r>{toc}<w:r><w:t> after</w:t></w:r></w:p><w:p><w:pPr><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>New heading for both tables</w:t></w:r></w:p>"#),
            );
            package.replace("word/document.xml", xml.into_bytes());
            let bytes = package.write().unwrap();
            view.update(cx, |view, _| {
                *view.document = crate::open_document_bytes(&bytes).unwrap();
                let before = view.document.export_docx().unwrap();
                let result = printing::update_fields(&mut view.document);
                if fails {
                    assert!(result.unwrap_err().contains("table of contents 1"));
                    let settings = PrintSettings {
                        update_fields: true,
                        ..Default::default()
                    };
                    assert!(view.build_preview(&settings).err().unwrap().contains("table of contents 1"));
                    assert!(view.print_pdf(&settings).unwrap_err().contains("table of contents 1"));
                    assert_eq!(view.document.export_docx().unwrap(), before);
                    assert!(!view.document.can_undo());
                } else {
                    result.unwrap();
                    let after = view.document.export_docx().unwrap();
                    let package = jade_opc::OpcPackage::open(&after).unwrap();
                    let xml = package.part_str("word/document.xml").unwrap().unwrap();
                    let locked = first.contains("fldLock");
                    assert_eq!(xml.matches("New heading for both tables").count(), if locked { 2 } else { 3 });
                    assert_eq!(xml.matches("Old cached contents").count(), usize::from(locked));
                    assert!(xml.contains("Before "));
                    assert!(xml.contains(" after"));
                    if first.contains("commentRangeStart") {
                        for marker in ["commentRangeStart", "commentRangeEnd", "commentReference"] {
                            assert_eq!(xml.matches(&format!("<w:{marker} w:id=\"9\"")).count(), 1);
                        }
                    }
                    if first.contains("permStart") {
                        let ranges = view.document.word_document().editable_ranges();
                        assert_eq!(ranges.len(), 1);
                        assert_eq!(ranges[0].id, "9");
                        assert_eq!(ranges[0].editors, "everyone");
                        assert_eq!(ranges[0].start, (0, 0));
                        let text_len: usize = view.document.word_document().paragraph(0).unwrap().runs.iter().map(|run| run.text.len()).sum();
                        assert_eq!(ranges[0].end, (0, text_len));
                    }
                    if first.contains("bookmarkStart") {
                        let bookmarks = &view.document.word_document().document.as_ref().unwrap().bookmarks;
                        let ids: std::collections::HashSet<_> = bookmarks.iter().map(|b| b.id.as_str()).collect();
                        assert_eq!(ids.len(), bookmarks.len(), "preserved and generated bookmark IDs must be unique");
                        assert!(bookmarks.iter().any(|b| b.id == "1" && b.name == "Around"));
                    }
                    view.document.undo().unwrap();
                    assert_eq!(view.document.export_docx().unwrap(), before);
                    assert!(!view.document.can_undo(), "all updates form one undo step");
                }
            });
        }
    }

    #[gpui::test]
    fn preview_predicts_field_updates_without_editing(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        let blank = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let mut document = crate::open_document_bytes(&blank).unwrap();
        document.insert_table_of_contents().unwrap();
        let fixture = document.export_docx().unwrap();
        let mut package = jade_opc::OpcPackage::open(&fixture).unwrap();
        let mut xml = package
            .part_str("word/document.xml")
            .unwrap()
            .unwrap()
            .to_owned();
        let headings: String = (0..60).map(|index| format!(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/><w:outlineLvl w:val="0"/></w:pPr><w:r><w:t>Additional preview heading {index}: a long title for the updated table of contents</w:t></w:r></w:p>"#
        )).collect();
        let at = xml
            .rfind("<w:sectPr")
            .unwrap_or_else(|| xml.rfind("</w:body>").unwrap());
        xml.insert_str(at, &headings);
        package.replace("word/document.xml", xml.into_bytes());
        let bytes = package.write().unwrap();
        view.update(cx, |view, _| {
            *view.document = crate::open_document_bytes(&bytes).unwrap();
            view.rebuild_all_images();
            let before = view.document.export_docx().unwrap();
            let undo = view.document.can_undo();
            let selection = view.document.selection_range();
            view.printing = Some(PrintPanel::new());
            view.refresh_print_preview();
            let stale_pages = view
                .printing
                .as_ref()
                .unwrap()
                .preview
                .as_ref()
                .unwrap()
                .sizes
                .len();
            view.printing.as_mut().unwrap().settings.update_fields = true;
            view.refresh_print_preview();
            let preview = view.printing.as_ref().unwrap().preview.as_ref().unwrap();
            let updated_pages = preview.sizes.len();
            assert!(
                updated_pages > stale_pages,
                "the expanded TOC must change pagination: stale={stale_pages}, updated={updated_pages}"
            );
            let predicted = preview.document.as_ref().unwrap().export_docx().unwrap();
            assert_ne!(predicted, before);
            assert_eq!(view.document.export_docx().unwrap(), before);
            assert_eq!(view.document.selection_range(), selection);
            assert_eq!(view.document.can_undo(), undo);
            // Toggling back must retire the updated snapshot, even when every
            // other setting is unchanged.
            view.printing.as_mut().unwrap().settings.update_fields = false;
            view.refresh_print_preview();
            assert_eq!(
                view.printing
                    .as_ref()
                    .unwrap()
                    .preview
                    .as_ref()
                    .unwrap()
                    .sizes
                    .len(),
                stale_pages
            );
            let pdf = view
                .print_pdf(&PrintSettings {
                    update_fields: true,
                    ..PrintSettings::default()
                })
                .unwrap();
            assert_eq!(pdf_pages(&pdf).len(), updated_pages);
            assert_eq!(view.document.export_docx().unwrap(), predicted);
        });
    }

    #[gpui::test]
    fn selection_printing_requires_selected_content(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        view.update(cx, |view, _| {
            let before = view.document.export_docx().unwrap();
            let undo = view.document.can_undo();
            for selection in [
                None,
                Some(jade_selection::SelectionRange::new(
                    jade_selection::TextPosition::new(0, 3),
                    jade_selection::TextPosition::new(0, 3),
                )),
            ] {
                view.document.set_selection_range(selection);
                for update_fields in [false, true] {
                    let settings = PrintSettings {
                        range: PrintRange::Selection,
                        update_fields,
                        ..PrintSettings::default()
                    };
                    assert_eq!(
                        view.print_pdf(&settings).unwrap_err(),
                        "Select content before printing the selection."
                    );
                    assert!(view.build_preview(&settings).is_err());
                    assert_eq!(view.document.export_docx().unwrap(), before);
                    assert_eq!(view.document.can_undo(), undo);
                    assert_eq!(view.document.selection_range(), selection);
                }
            }
            // Switching back to the whole document remains usable.
            assert!(!pdf_pages(&view.print_pdf(&PrintSettings::default()).unwrap()).is_empty());
        });
    }

    /// What prints: the pages chosen, laid out on their sheets, scaled to
    /// the paper asked for; only the selection when that is asked for.
    #[gpui::test]
    fn the_pdf_prints_the_sheets_the_settings_make(cx: &mut gpui::TestAppContext) {
        let (view, cx) = open(cx);
        let (all, two_up, a3, selection) = view.update(cx, |view, _| {
            let all = view.print_pdf(&PrintSettings::default()).unwrap();
            let two_up = view
                .print_pdf(&PrintSettings {
                    range: PrintRange::Custom("1-2".into()),
                    pages_per_sheet: 2,
                    ..PrintSettings::default()
                })
                .unwrap();
            let a3 = view
                .print_pdf(&PrintSettings {
                    range: PrintRange::Current,
                    scale_to: Some(Paper::A3),
                    ..PrintSettings::default()
                })
                .unwrap();
            view.document
                .set_selection_range(Some(jade_selection::SelectionRange::new(
                    jade_selection::TextPosition::new(0, 0),
                    jade_selection::TextPosition::new(0, 6),
                )));
            let selection = view
                .print_pdf(&PrintSettings {
                    range: PrintRange::Selection,
                    ..PrintSettings::default()
                })
                .unwrap();
            (all, two_up, a3, selection)
        });
        let pages = pdf_pages(&all);
        assert!(pages.len() >= 2);
        let sheets = pdf_pages(&two_up);
        assert_eq!(sheets.len(), 1);
        assert!(
            sheets[0].0 > sheets[0].1,
            "two pages side by side on a turned sheet"
        );
        let a3_sheet = pdf_pages(&a3);
        assert_eq!(a3_sheet.len(), 1);
        assert!((a3_sheet[0].0 - 841.9).abs() < 1.0 && (a3_sheet[0].1 - 1190.55).abs() < 1.0);
        assert_eq!(pdf_pages(&selection).len(), 1);
    }
}
