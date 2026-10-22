//! The document for screen readers in the browser. The pages are pixels on
//! a canvas, so the page also carries an offscreen, read-only ARIA text box
//! with the document's text, a paragraph a line, kept up to date as it is
//! edited, and a polite live region that reads out the paragraph the caret
//! moves into — what `jade-office-model`'s `word::screen_reader` gives.

use jade_web::word::screen_reader::screen_reader_text;

use super::*;

/// What was last handed over: the document, its edit count and caret,
/// and the caret's paragraph.
type Handed = (SharedString, u64, Option<(usize, usize)>, usize);

thread_local! {
    static LAST: std::cell::RefCell<Option<Handed>> = const { std::cell::RefCell::new(None) };
}

impl DocView {
    /// After a frame's changes: the text and the caret's paragraph handed
    /// to the page again if they changed.
    pub(super) fn update_screen_reader(&self) {
        let Document::Pages { document, .. } = &self.document else {
            return;
        };
        let caret = document.caret();
        let key = (self.name.clone(), self.generation, caret);
        let unchanged = LAST.with(|last| {
            last.borrow()
                .as_ref()
                .is_some_and(|(name, generation, at, _)| {
                    (name, generation, at) == (&key.0, &key.1, &key.2)
                })
        });
        if unchanged {
            return;
        }
        let read = screen_reader_text(&**document);
        let moved = LAST.with(|last| {
            let mut last = last.borrow_mut();
            let moved = last.as_ref().is_none_or(|(name, _, _, paragraph)| {
                name != &key.0 || *paragraph != read.paragraph
            });
            let text_changed = last
                .as_ref()
                .is_none_or(|(name, generation, _, _)| name != &key.0 || *generation != key.1);
            *last = Some((key.0.clone(), key.1, key.2, read.paragraph));
            if text_changed {
                dom::set_text(&self.name, &read.text);
            }
            moved
        });
        if moved {
            dom::announce(&read.paragraph_text);
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod dom {
    use wasm_bindgen::{JsCast, JsValue};

    fn call(target: &JsValue, method: &str, args: &[JsValue]) -> Option<JsValue> {
        let function = js_sys::Reflect::get(target, &method.into())
            .ok()?
            .dyn_into::<js_sys::Function>()
            .ok()?;
        let args: js_sys::Array = args.iter().collect();
        function.apply(target, &args).ok()
    }

    /// The element with `id`, made offscreen with `attributes` if it is not
    /// there yet.
    fn element(id: &str, attributes: &[(&str, &str)]) -> Option<JsValue> {
        let document = js_sys::Reflect::get(&js_sys::global(), &"document".into()).ok()?;
        if let Some(found) =
            call(&document, "getElementById", &[id.into()]).filter(|found| !found.is_null())
        {
            return Some(found);
        }
        let created = call(&document, "createElement", &["div".into()])?;
        call(&created, "setAttribute", &["id".into(), id.into()]);
        for (name, value) in attributes {
            call(&created, "setAttribute", &[(*name).into(), (*value).into()]);
        }
        call(
            &created,
            "setAttribute",
            &[
                "style".into(),
                "position:absolute;left:-10000px;top:0;width:1px;height:1px;overflow:hidden;white-space:pre-wrap".into(),
            ],
        );
        let body = js_sys::Reflect::get(&document, &"body".into()).ok()?;
        call(&body, "appendChild", std::slice::from_ref(&created));
        Some(created)
    }

    pub fn set_text(name: &str, text: &str) {
        let Some(mirror) = element(
            "jade-screen-reader",
            &[
                ("role", "textbox"),
                ("aria-multiline", "true"),
                ("aria-readonly", "true"),
                ("tabindex", "-1"),
            ],
        ) else {
            return;
        };
        call(&mirror, "setAttribute", &["aria-label".into(), name.into()]);
        js_sys::Reflect::set(&mirror, &"textContent".into(), &text.into()).ok();
    }

    pub fn announce(text: &str) {
        let Some(live) = element(
            "jade-screen-reader-live",
            &[("aria-live", "polite"), ("aria-atomic", "true")],
        ) else {
            return;
        };
        js_sys::Reflect::set(&live, &"textContent".into(), &text.into()).ok();
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod dom {
    pub fn set_text(_name: &str, _text: &str) {}
    pub fn announce(_text: &str) {}
}
