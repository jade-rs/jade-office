//! Word's field keys in the browser shell, over the same
//! `jade-office-model` `WordFields` the native app drives: F9 updates the
//! selection's fields (every story when all is selected), ⌘F11 and ⌘⇧F11
//! lock and unlock them, ⌘⇧F9 turns them into text. DATE and TIME take the
//! browser's clock.

use std::collections::HashMap;

use gpui::{Context, KeyDownEvent};
use jade_web::word::fields::{FieldDateTime, FieldHost, WordFields};

use super::{DocView, Document};
use crate::i18n;

/// The browser's local time and its offset from UTC.
fn browser_now() -> (FieldDateTime, i64) {
    let now = js_sys::Date::new_0();
    (
        FieldDateTime {
            year: now.get_full_year() as i32,
            month: now.get_month() + 1,
            day: now.get_date(),
            hour: now.get_hours(),
            minute: now.get_minutes(),
            second: now.get_seconds(),
            weekday: now.get_day(),
        },
        // getTimezoneOffset is minutes *behind* UTC.
        -(now.get_timezone_offset() as i64) * 60,
    )
}

impl DocView {
    /// Runs a field key over a Word document; `false` when the key is none.
    pub(super) fn field_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) -> bool {
        if !matches!(self.document, Document::Pages { .. }) {
            return false;
        }
        let modifiers = event.keystroke.modifiers;
        let command = modifiers.platform || modifiers.control;
        match (event.keystroke.key.as_str(), command, modifiers.shift) {
            ("f9", false, false) => {
                let (now, offset) = browser_now();
                let host = FieldHost {
                    now,
                    utc_offset_seconds: offset,
                    japanese: !i18n::is_english(),
                    ..FieldHost::default()
                };
                self.word_edit(cx, |document| {
                    let scope = document.field_scope();
                    // The browser has no prompt dialog: ASK and FILLIN take
                    // their defaults.
                    document.update_fields(scope, &host, HashMap::new())
                });
            }
            ("f11", true, shift) => self.word_edit(cx, |document| document.lock_fields(!shift)),
            ("f9", true, true) => self.word_edit(cx, |document| document.unlink_fields()),
            _ => return false,
        }
        true
    }
}
