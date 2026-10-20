//! Save As (⌘⇧S) in the browser, which has no file dialog: a bar over the
//! sheet with the file's name to edit. Its extension picks the format, as
//! Excel's Save As does — `.csv` the active sheet as CSV, `.xlsx` (or the
//! other workbook extensions) the workbook — and the tab takes the name.

use super::*;

impl DocView {
    /// Opens the bar with the file's name selected.
    pub(super) fn open_save_as(&mut self, cx: &mut Context<Self>) {
        let name = self.name.to_string();
        let Some(sheet) = self.sheet_mut() else {
            return;
        };
        sheet.saving_as = Some(TextEdit::selected(name));
        cx.notify();
    }

    /// Keys while the bar is open: Return saves under the name typed,
    /// Escape leaves; the rest edit the name.
    pub(super) fn save_as_key(
        &mut self,
        key: &str,
        modifiers: gpui::Modifiers,
        cx: &mut Context<Self>,
    ) -> bool {
        let command = modifiers.platform || modifiers.control;
        let Some(sheet) = self.sheet_mut() else {
            return false;
        };
        let Some(text) = sheet.saving_as.as_mut() else {
            return false;
        };
        if text.marked.is_some() {
            return false;
        }
        match key {
            "enter" => {
                let name = text.text.trim().to_string();
                sheet.saving_as = None;
                if !name.is_empty() {
                    self.save_named(name, cx);
                }
                return true;
            }
            "escape" => sheet.saving_as = None,
            "backspace" => text.backspace(),
            "delete" => text.delete(),
            "left" => text.step(false),
            "right" => text.step(true),
            "home" => text.to_end(false),
            "end" => text.to_end(true),
            "a" if command => text.whole = true,
            _ => return command,
        }
        cx.notify();
        true
    }

    /// Saves under `name`: the tab is that file from now on. A `.pdf`
    /// exports the sheet's pages instead, and the tab keeps its name.
    fn save_named(&mut self, name: String, cx: &mut Context<Self>) {
        if jade_web::print::is_pdf(&name) {
            return self.download_pdf(name, cx);
        }
        // A workbook's own extensions, or .csv; anything else becomes a
        // workbook with .xlsx on the end.
        let name = if Kind::of(&name) == Some(Kind::Workbook) {
            name
        } else {
            format!("{name}.xlsx")
        };
        self.name = name.into();
        self.save(cx);
    }

    /// ⌘P: the active sheet's pages as a PDF to print, handed to the page
    /// as a download — the browser prints a PDF better than a canvas.
    pub(super) fn print_sheet(&mut self, cx: &mut Context<Self>) {
        let name = match self.name.rsplit_once('.') {
            Some((stem, _)) => format!("{stem}.pdf"),
            None => format!("{}.pdf", self.name),
        };
        self.download_pdf(name, cx);
    }

    fn download_pdf(&mut self, name: String, cx: &mut Context<Self>) {
        if !self.commit_cell(cx) {
            return;
        }
        let fields = jade_web::print::PrintFields {
            date: now_text(false),
            time: now_text(true),
            file: self.name.to_string(),
        };
        let Some(sheet) = self.sheet_ref() else {
            return;
        };
        self.status = match sheet.book.pdf(&fields) {
            Ok(bytes) => {
                self.host.save(&name, &bytes);
                i18n::format("web.downloaded", &[("name", &name)]).into()
            }
            Err(error) if error == jade_web::print::NOTHING_TO_PRINT => {
                i18n::text("sheet.nothing_to_print").into()
            }
            Err(error) => i18n::format("sheet.print_failed", &[("error", &error)]).into(),
        };
        cx.notify();
    }

    /// The bar, while it is open.
    pub(super) fn save_as_bar(&self) -> Option<gpui::Div> {
        let text = self.sheet_ref()?.saving_as.as_ref()?;
        Some(
            div()
                .absolute()
                .top_2()
                .right_4()
                .w(px(320.0))
                .flex()
                .flex_col()
                .gap_1()
                .px_2()
                .py_1p5()
                .rounded_md()
                .border_1()
                .border_color(palette::accent())
                .bg(palette::panel())
                .shadow_lg()
                .text_xs()
                .text_color(palette::text())
                .occlude()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_color(palette::dim())
                                .child(i18n::text("sheet.save_as")),
                        )
                        .child(div().flex_1().child(caret_line(
                            text,
                            palette::accent().into(),
                            false,
                        ))),
                )
                .child(
                    div()
                        .text_color(palette::dim())
                        .child(i18n::text("sheet.save_as_hint")),
                ),
        )
    }
}
