//! 校閲 › 文章校正 › 類義語辞典: the pane beside the pages with the words
//! that mean what the word at the caret means, sense by sense; a click on
//! one looks that up in turn, 挿入 puts it in the word's place. The words
//! come from the system's thesaurus (Dictionary Services, the thesauri the
//! user has in the Dictionary app); what the shared model does with them
//! is `jade-office-model`'s `word::thesaurus`.

use gpui::{div, prelude::*, px, AnyElement, ClickEvent, Context};
use jade_office_model::word::thesaurus::{BuiltinThesaurus, Sense, Thesaurus, WordThesaurus};

use crate::word_characters::button;
use crate::{i18n, theme, DocumentView};

type Target = ((usize, usize), (usize, usize), String);

/// The pane, while it is open.
pub struct ThesaurusPane {
    /// The word looked up.
    pub word: String,
    pub senses: Vec<Sense>,
    /// The word in the document 挿入 replaces.
    pub target: Option<Target>,
}

/// The system's thesaurus, else the built-in list.
fn thesaurus() -> Box<dyn Thesaurus> {
    #[cfg(all(target_os = "macos", not(test)))]
    {
        let system = mac::SystemThesaurus::new();
        if system.available() {
            return Box::new(system);
        }
    }
    Box::new(BuiltinThesaurus)
}

/// 類義語辞典: the pane for the word at the caret, or shut.
pub fn toggle(view: &mut DocumentView, cx: &mut Context<DocumentView>) {
    if view.review.thesaurus.take().is_some() {
        return cx.notify();
    }
    if view.document.selection().is_none() {
        view.caret = view
            .document
            .place_caret(view.insert_at.0, view.insert_at.1);
    }
    let target = view.document.word_for_thesaurus();
    let word = target
        .as_ref()
        .map(|(_, _, word)| word.clone())
        .unwrap_or_default();
    let senses = if word.is_empty() {
        Vec::new()
    } else {
        thesaurus().lookup(&word)
    };
    view.review.thesaurus = Some(ThesaurusPane {
        word,
        senses,
        target,
    });
    cx.notify();
}

/// A synonym clicked: looked up in its turn.
pub fn look_up(view: &mut DocumentView, word: &str, cx: &mut Context<DocumentView>) {
    if let Some(pane) = view.review.thesaurus.as_mut() {
        pane.word = word.to_string();
        pane.senses = thesaurus().lookup(word);
    }
    cx.notify();
}

/// 挿入: `synonym` in the word's place.
pub fn insert(view: &mut DocumentView, synonym: &str, cx: &mut Context<DocumentView>) {
    let Some(target) = view
        .review
        .thesaurus
        .as_ref()
        .and_then(|pane| pane.target.clone())
    else {
        return;
    };
    let result = view.document.replace_with_synonym(&target, synonym);
    if result.is_ok() {
        // The new word is the one 挿入 replaces next.
        if let Some(pane) = view.review.thesaurus.as_mut() {
            let (start, _, word) = &target;
            let replaced = jade_office_model::word::thesaurus::matching_case(word, synonym);
            pane.target = Some((*start, (start.0, start.1 + replaced.len()), replaced));
        }
    }
    view.reviewed(result, cx);
}

/// The pane, beside the pages while it is open.
pub fn pane(view: &mut DocumentView, cx: &mut Context<DocumentView>) -> Option<AnyElement> {
    let pane = view.review.thesaurus.as_ref()?;
    let mut rows: Vec<AnyElement> = Vec::new();
    rows.push(
        div()
            .text_sm()
            .text_color(theme::text())
            .child(pane.word.clone())
            .into_any_element(),
    );
    if pane.senses.is_empty() {
        rows.push(
            div()
                .text_color(theme::text_dim())
                .child(i18n::text(if pane.word.is_empty() {
                    "thesaurus.no_word"
                } else {
                    "thesaurus.none"
                }))
                .into_any_element(),
        );
    }
    let mut index = 0usize;
    for sense in &pane.senses {
        rows.push(
            div()
                .mt_1()
                .text_color(theme::text_dim())
                .child(sense.label.clone())
                .into_any_element(),
        );
        for synonym in &sense.synonyms {
            let (looked, inserted) = (synonym.clone(), synonym.clone());
            rows.push(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .pl_2()
                    .child(
                        div()
                            .id(("thesaurus-word", index))
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|word| word.bg(theme::hovered()))
                            .text_color(theme::text())
                            .child(synonym.clone())
                            .on_click(cx.listener(move |view, _: &ClickEvent, _, cx| {
                                look_up(view, &looked, cx)
                            })),
                    )
                    .when(pane.target.is_some(), |row| {
                        row.child(
                            button(
                                ("thesaurus-insert", index),
                                i18n::text("thesaurus.insert"),
                                false,
                            )
                            .on_click(cx.listener(
                                move |view, _: &ClickEvent, _, cx| insert(view, &inserted, cx),
                            )),
                        )
                    })
                    .into_any_element(),
            );
            index += 1;
        }
    }
    Some(
        div()
            .id("thesaurus-pane")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(260.0))
            .h_full()
            .gap_1()
            .p_3()
            .overflow_y_scroll()
            .bg(theme::panel())
            .border_l_1()
            .border_color(theme::border())
            .text_xs()
            .text_color(theme::text())
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_center()
                    .child(div().text_sm().child(i18n::text("thesaurus.title")))
                    .child(
                        div()
                            .id("thesaurus-close")
                            .px_1()
                            .rounded_md()
                            .cursor_pointer()
                            .hover(|close| close.bg(theme::hovered()))
                            .child("×")
                            .on_click(cx.listener(|view, _: &ClickEvent, _, cx| toggle(view, cx))),
                    ),
            )
            .children(rows)
            .into_any_element(),
    )
}

#[cfg(all(target_os = "macos", not(test)))]
mod mac {
    //! Dictionary Services: `DCSCopyTextDefinition` over the active
    //! dictionaries that are thesauri (found by name through
    //! `DCSGetActiveDictionaries` / `DCSDictionaryGetName`, looked up at run
    //! time so a system without them simply has none).

    use std::ffi::{c_char, c_void};

    use jade_office_model::word::thesaurus::{parse_thesaurus_entry, Sense, Thesaurus};
    use objc2::rc::Retained;
    use objc2_foundation::NSString;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CFRange {
        location: isize,
        length: isize,
    }

    #[link(name = "CoreServices", kind = "framework")]
    extern "C" {
        fn DCSCopyTextDefinition(
            dictionary: *const c_void,
            text: *const c_void,
            range: CFRange,
        ) -> *const c_void;
    }

    #[link(name = "CoreFoundation", kind = "framework")]
    extern "C" {
        fn CFArrayGetCount(array: *const c_void) -> isize;
        fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
        fn CFRelease(object: *const c_void);
    }

    extern "C" {
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }

    /// `RTLD_DEFAULT` on macOS.
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

    type GetActive = unsafe extern "C" fn() -> *const c_void;
    type GetName = unsafe extern "C" fn(*const c_void) -> *const c_void;

    pub struct SystemThesaurus {
        dictionaries: Vec<*const c_void>,
        active: *const c_void,
    }

    impl SystemThesaurus {
        pub fn new() -> Self {
            let mut thesaurus = Self {
                dictionaries: Vec::new(),
                active: std::ptr::null(),
            };
            // SAFETY: the symbols are looked up by name and called with the
            // signatures Dictionary Services has had since macOS 10.5; the
            // array is released on drop, its elements live as long as it.
            unsafe {
                let get_active = dlsym(RTLD_DEFAULT, c"DCSGetActiveDictionaries".as_ptr());
                let get_name = dlsym(RTLD_DEFAULT, c"DCSDictionaryGetName".as_ptr());
                if get_active.is_null() || get_name.is_null() {
                    return thesaurus;
                }
                let get_active: GetActive = std::mem::transmute(get_active);
                let get_name: GetName = std::mem::transmute(get_name);
                let active = get_active();
                if active.is_null() {
                    return thesaurus;
                }
                thesaurus.active = active;
                for index in 0..CFArrayGetCount(active) {
                    let dictionary = CFArrayGetValueAtIndex(active, index);
                    let name = get_name(dictionary);
                    if name.is_null() {
                        continue;
                    }
                    let Some(name) = Retained::retain(name as *mut NSString) else {
                        continue;
                    };
                    let name = name.to_string();
                    if ["Thesaurus", "thesaurus", "類語", "シソーラス"]
                        .iter()
                        .any(|mark| name.contains(mark))
                    {
                        thesaurus.dictionaries.push(dictionary);
                    }
                }
            }
            thesaurus
        }

        pub fn available(&self) -> bool {
            !self.dictionaries.is_empty()
        }
    }

    impl Drop for SystemThesaurus {
        fn drop(&mut self) {
            if !self.active.is_null() {
                // SAFETY: the array came from a Copy/Get-active call that
                // hands it to the caller.
                unsafe { CFRelease(self.active) };
            }
        }
    }

    impl Thesaurus for SystemThesaurus {
        fn lookup(&self, word: &str) -> Vec<Sense> {
            let text = NSString::from_str(word);
            let range = CFRange {
                location: 0,
                length: word.encode_utf16().count() as isize,
            };
            let mut senses = Vec::new();
            for &dictionary in &self.dictionaries {
                // SAFETY: NSString is toll-free bridged with CFString; the
                // result follows the Copy rule and is taken over.
                let entry = unsafe {
                    let found = DCSCopyTextDefinition(
                        dictionary,
                        Retained::as_ptr(&text) as *const c_void,
                        range,
                    );
                    if found.is_null() {
                        continue;
                    }
                    Retained::from_raw(found as *mut NSString)
                };
                if let Some(entry) = entry {
                    senses.extend(parse_thesaurus_entry(&entry.to_string()));
                }
            }
            senses
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::word_review_tools::ReviewAction;

    /// The pane looks up the word at the caret, a click looks up a synonym,
    /// and 挿入 puts one in the word's place with its capital.
    #[gpui::test]
    fn the_thesaurus_replaces_the_word_at_the_caret(cx: &mut gpui::TestAppContext) {
        use gpui::Focusable;
        let bytes = jade_office_model::word::new::blank_document(
            jade_office_model::word::new::Blank::English,
        );
        let (view, cx) = cx
            .add_window_view(|_window, cx| crate::untitled_view(&bytes, "words.docx", cx).unwrap());
        cx.update(|window, cx| window.focus(&view.focus_handle(cx)));
        cx.run_until_parked();
        view.update(cx, |view, _| {
            view.insert_at = (0, 0);
            view.caret = view.document.place_caret(0, 0);
        });
        cx.simulate_input("Begin now");
        view.update(cx, |view, cx| {
            view.insert_at = (0, 2);
            view.caret = view.document.place_caret(0, 2);
            crate::word_review_tools::run(view, ReviewAction::Thesaurus, cx);
            let pane = view.review.thesaurus.as_ref().unwrap();
            assert_eq!(pane.word, "Begin");
            assert!(pane.senses[0].synonyms.contains(&"start".to_string()));
            insert(view, "start", cx);
            assert!(view
                .document
                .paragraph_text(0)
                .unwrap()
                .starts_with("Start now"));
            look_up(view, "start", cx);
            assert_eq!(
                view.review.thesaurus.as_ref().unwrap().senses[0].label,
                "begin"
            );
            toggle(view, cx);
            assert!(view.review.thesaurus.is_none());
        });
    }
}
