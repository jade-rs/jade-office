//! The document for screen readers. gpui 0.2 builds no accessibility tree
//! of its own, so the pages are given one element of their own: an
//! `NSAccessibilityElement` with the text-area role, a child of gpui's view,
//! whose value is the document's text — a paragraph a line — with the
//! caret and the selection as its selected range, kept up to date as the
//! document is edited and the caret moves, and announced to VoiceOver as
//! it changes. What it holds is `jade-office-model`'s
//! `word::screen_reader`.

use gpui::Window;
use jade_office_model::word::screen_reader::{screen_reader_text, ScreenReaderText};

use crate::DocumentView;
use jade_office_model::word::WordSession;

/// What was last handed to the system, and the element it went to.
#[derive(Default)]
pub struct ScreenReaderUi {
    /// The edit count and selection it was read at.
    key: Option<(u64, Option<jade_office_model::word::SelectionRangeOf>)>,
    pub shown: Option<ScreenReaderText>,
    #[cfg(all(target_os = "macos", not(test)))]
    element: Option<objc2::rc::Retained<objc2_app_kit::NSAccessibilityElement>>,
}

/// After a frame's changes: the text and the selection handed over again
/// if they changed.
pub fn update(view: &mut DocumentView, window: &Window) {
    let key = (view.edits, view.document.selection_range());
    if view.review.screen_reader.key.as_ref() == Some(&key) {
        return;
    }
    view.review.screen_reader.key = Some(key);
    let read = screen_reader_text(&view.document);
    let changed_text = view
        .review
        .screen_reader
        .shown
        .as_ref()
        .is_none_or(|shown| shown.text != read.text);
    #[cfg(all(target_os = "macos", not(test)))]
    mac::expose(
        &mut view.review.screen_reader,
        window,
        crate::shell::file_name(std::path::Path::new(view.path.as_ref())).as_ref(),
        &read,
        changed_text,
    );
    #[cfg(any(not(target_os = "macos"), test))]
    let _ = (window, changed_text);
    view.review.screen_reader.shown = Some(read);
}

#[cfg(all(target_os = "macos", not(test)))]
mod mac {
    use gpui::Window;
    use jade_office_model::word::screen_reader::ScreenReaderText;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{
        NSAccessibility, NSAccessibilityElement, NSAccessibilityPostNotification,
        NSAccessibilitySelectedTextChangedNotification, NSAccessibilityTextAreaRole,
        NSAccessibilityValueChangedNotification, NSView,
    };
    use objc2_foundation::{NSArray, NSRange, NSString};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::ScreenReaderUi;

    pub fn expose(
        ui: &mut ScreenReaderUi,
        window: &Window,
        title: &str,
        read: &ScreenReaderText,
        changed_text: bool,
    ) {
        let Ok(handle) = HasWindowHandle::window_handle(window) else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: the handle is the window's live content view, used on the
        // main thread that draws it.
        let view: &NSView = unsafe { handle.ns_view.cast::<NSView>().as_ref() };
        let element = ui.element.get_or_insert_with(|| {
            let element = NSAccessibilityElement::new();
            element.setAccessibilityRole(Some(unsafe { NSAccessibilityTextAreaRole }));
            element.setAccessibilityElement(true);
            // SAFETY: the parent is the view the element is a child of.
            unsafe {
                element.setAccessibilityParent(Some(view as &AnyObject));
                let children = NSArray::from_retained_slice(&[Retained::into_super(
                    Retained::into_super(element.clone()),
                )]);
                view.setAccessibilityChildren(Some(&children));
            }
            element
        });
        element.setAccessibilityLabel(Some(&NSString::from_str(title)));
        element.setAccessibilityFrameInParentSpace(view.bounds());
        let length = read.text.encode_utf16().count();
        if changed_text {
            // SAFETY: the value is an NSString, as a text area's is.
            unsafe { element.setAccessibilityValue(Some(&NSString::from_str(&read.text))) };
            element.setAccessibilityNumberOfCharacters(length as isize);
        }
        let (start, end) = read.selection;
        element.setAccessibilitySelectedTextRange(NSRange::new(
            start.min(length),
            end.saturating_sub(start).min(length.saturating_sub(start)),
        ));
        // SAFETY: posting is how AppKit tells assistive apps of a change.
        unsafe {
            if changed_text {
                NSAccessibilityPostNotification(element, NSAccessibilityValueChangedNotification);
            }
            NSAccessibilityPostNotification(
                element,
                NSAccessibilitySelectedTextChangedNotification,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    /// Typing and moving the caret hand the text and the caret over again;
    /// a frame with neither does not.
    #[gpui::test]
    fn the_text_follows_edits_and_the_caret(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "read.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("Hello reader");
        cx.run_until_parked();
        view.update(cx, |view, _| {
            let shown = view.review.screen_reader.shown.clone().unwrap();
            assert!(shown.text.starts_with("Hello reader"), "{:?}", shown.text);
            assert_eq!(shown.selection, (12, 12));
            view.caret = view.document.place_caret(0, 5);
        });
        cx.update(|window, cx| {
            view.update(cx, |view, _| super::update(view, window));
        });
        view.update(cx, |view, _| {
            let shown = view.review.screen_reader.shown.as_ref().unwrap();
            assert_eq!(shown.selection, (5, 5));
            assert_eq!(shown.paragraph_text, "Hello reader");
        });
    }
}
