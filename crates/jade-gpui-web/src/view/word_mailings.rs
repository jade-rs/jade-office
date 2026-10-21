//! The browser shell's copy of the native app's `word_mailings`: the
//! 差し込み文書 tab over `jade-office-model`'s `word::mailings`. The browser
//! has no paths: a list is picked through the page (and handed back by the
//! workspace), a list typed here is offered as a download, the merged
//! document opens in a new tab, and a merged PDF — what printing is here —
//! is offered as a download.

use std::rc::Rc;

use gpui::{div, prelude::*, px, AnyElement, Context, Window};
use jade_tiny_skia_painter::FontBook;
use jade_web::document::ShapingMeasurer;
use jade_web::word::form_view::{FormEvent, ToolGroup};
use jade_web::word::mailings::{MailMerge, MailOutcome};

use super::word_form;
use super::{DocView, Document};
use crate::i18n;

fn t(key: &'static str) -> &'static str {
    i18n::text(key)
}

/// What a merged document is painted with to make a PDF of it.
pub struct MergeFonts {
    pub scale: f64,
    pub fonts: Rc<FontBook>,
    pub measurer: Rc<ShapingMeasurer>,
}

/// The view's share of the mail merge.
pub struct Mailings {
    pub merge: MailMerge,
    /// Whether the page's picker was opened for a list.
    pub wants_source: bool,
    pub fonts: Option<MergeFonts>,
}

impl Mailings {
    pub fn new(document: &jade_web::editing::EditableDocument, fonts: Option<MergeFonts>) -> Self {
        Self {
            merge: MailMerge::read(document, !i18n::is_english()),
            wants_source: false,
            fonts,
        }
    }
}

/// The tab's groups, read for the tab showing them.
pub fn groups(view: &DocView) -> Option<Vec<ToolGroup>> {
    let Document::Pages { document, .. } = &view.document else {
        return None;
    };
    let mailings = view.mailings.as_ref()?;
    Some(mailings.merge.ribbon(&**document, t))
}

/// The tab, drawn.
pub fn tab(groups: &[ToolGroup], cx: &mut Context<DocView>) -> Vec<AnyElement> {
    word_form::tool_groups(groups, ribbon_event, cx)
}

fn ribbon_event(view: &mut DocView, event: FormEvent, cx: &mut Context<DocView>) {
    let (Document::Pages { document, .. }, Some(mailings)) =
        (&mut view.document, view.mailings.as_mut())
    else {
        return;
    };
    let outcome = mailings.merge.ribbon_event(&mut **document, event);
    outcome_of(view, outcome, cx);
}

fn dialog_event(view: &mut DocView, event: FormEvent, cx: &mut Context<DocView>) {
    let (Document::Pages { document, .. }, Some(mailings)) =
        (&mut view.document, view.mailings.as_mut())
    else {
        return;
    };
    let outcome = mailings.merge.dialog_event(&mut **document, event);
    outcome_of(view, outcome, cx);
}

fn outcome_of(view: &mut DocView, outcome: Result<MailOutcome, String>, cx: &mut Context<DocView>) {
    match outcome {
        Err(error) | Ok(MailOutcome::Problem(error)) => view.status = error.into(),
        Ok(MailOutcome::None) => {}
        Ok(MailOutcome::Edited) => {
            view.refresh_repainted();
            view.word_result(Ok(()), cx);
            return;
        }
        Ok(MailOutcome::PickDataSource) => {
            if let Some(mailings) = view.mailings.as_mut() {
                mailings.wants_source = true;
            }
            view.host.pick();
        }
        Ok(MailOutcome::SaveDataSource { name, bytes }) => {
            view.host.save(&name, &bytes);
            let (Document::Pages { document, .. }, Some(mailings)) =
                (&mut view.document, view.mailings.as_mut())
            else {
                return;
            };
            let outcome = mailings
                .merge
                .saved_data_source(&mut **document, &name, None);
            return outcome_of(view, outcome, cx);
        }
        Ok(MailOutcome::OpenDocument { name, bytes }) => crate::open_file(name, bytes),
        Ok(MailOutcome::SavePdf { name, bytes }) => save_pdf(view, &name, &bytes),
        Ok(MailOutcome::Print { bytes }) => {
            let name = if i18n::is_english() {
                "Merged Document.pdf"
            } else {
                "差し込み文書.pdf"
            };
            save_pdf(view, name, &bytes)
        }
    }
    cx.notify();
}

/// The merged document painted into a PDF and offered as a download.
fn save_pdf(view: &mut DocView, name: &str, bytes: &[u8]) {
    let Some(fonts) = view
        .mailings
        .as_ref()
        .and_then(|mailings| mailings.fonts.as_ref())
    else {
        return;
    };
    let pdf = jade_web::editing::EditableDocument::open(
        bytes,
        fonts.scale,
        Rc::clone(&fonts.fonts),
        Rc::clone(&fonts.measurer),
    )
    .and_then(|mut merged| merged.export_pdf());
    match pdf {
        Ok(pdf) => view.host.save(name, &pdf),
        Err(error) => view.status = error.into(),
    }
}

/// Whether the page's picker was opened for a list, so the file it brings
/// is the list's.
pub fn wants_source(view: &DocView) -> bool {
    view.mailings
        .as_ref()
        .is_some_and(|mailings| mailings.wants_source)
}

/// The list the page's picker brought.
pub fn take_source(view: &mut DocView, name: &str, bytes: &[u8], cx: &mut Context<DocView>) {
    let (Document::Pages { document, .. }, Some(mailings)) =
        (&mut view.document, view.mailings.as_mut())
    else {
        return;
    };
    mailings.wants_source = false;
    let outcome =
        mailings
            .merge
            .load_data_source(&mut **document, name, Some(name.to_string()), bytes);
    outcome_of(view, outcome, cx);
}

/// Whether a mail-merge dialog is open.
pub fn is_open(view: &DocView) -> bool {
    view.mailings
        .as_ref()
        .is_some_and(|mailings| mailings.merge.is_open())
}

/// Keys while a dialog is open. Whether the key was taken.
pub fn handle_key(view: &mut DocView, key: &str, shift: bool, cx: &mut Context<DocView>) -> bool {
    if let Some(event) = word_form::key_event(key, shift) {
        dialog_event(view, event, cx);
        return true;
    }
    let Some(field) = view
        .mailings
        .as_mut()
        .and_then(|mailings| mailings.merge.field_mut())
    else {
        return false;
    };
    if field.marked.is_some() || !field.edit_key(key) {
        return false;
    }
    cx.notify();
    true
}

/// The open dialog, over everything.
pub fn dialog(
    view: &DocView,
    window: &mut Window,
    cx: &mut Context<DocView>,
) -> Option<AnyElement> {
    let form = view.mailings.as_ref()?.merge.dialog_view(t)?;
    Some(word_form::overlay(&form, window, dialog_event, cx))
}

/// 差し込みフィールドの強調表示 over `page`.
pub fn overlay(view: &DocView, page: usize, zoom: f32) -> Vec<AnyElement> {
    let (Document::Pages { document, .. }, Some(mailings)) =
        (&view.document, view.mailings.as_ref())
    else {
        return Vec::new();
    };
    if !mailings.merge.highlight {
        return Vec::new();
    }
    mailings
        .merge
        .highlight_regions(&**document)
        .into_iter()
        .filter_map(|region| {
            let (on, top) = document.page_and_offset(region.y);
            (on == page).then(|| {
                div()
                    .absolute()
                    .left(px(region.x as f32 * zoom))
                    .top(px(top as f32 * zoom))
                    .w(px(region.width.max(2.0) as f32 * zoom))
                    .h(px(region.height as f32 * zoom))
                    .bg(gpui::rgba(0x8080803a))
                    .into_any_element()
            })
        })
        .collect()
}
