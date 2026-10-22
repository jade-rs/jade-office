//! The Word settings the browser keeps between visits — オートコレクト, Word
//! のオプション (marks, typing, paste defaults), the リボンのユーザー設定 and
//! the クイック アクセス ツール バー — in the page's `localStorage`, as the
//! same small texts the native app writes to Application Support.
//! Off the page (a native test build) they live in memory.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};

use jade_web::word::autocorrect::{options_from_text, options_to_text, AutoCorrectOptions};
use jade_web::word::customize::{format_keyed, parse_keyed};

const PREFIX: &str = "jade-office.settings.";

thread_local! {
    static AUTOCORRECT: RefCell<Option<AutoCorrectOptions>> = const { RefCell::new(None) };
    static KEYED: RefCell<HashMap<String, BTreeMap<String, String>>> =
        RefCell::new(HashMap::new());
}

/// A settings text, if it was ever saved.
pub fn load(name: &str) -> Option<String> {
    crate::view::word_fields::storage::get(&format!("{PREFIX}{name}"))
}

/// Saves a settings text.
pub fn save(name: &str, text: &str) {
    crate::view::word_fields::storage::set(&format!("{PREFIX}{name}"), text);
}

/// The AutoCorrect settings, read once.
pub fn autocorrect() -> AutoCorrectOptions {
    AUTOCORRECT.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                load("autocorrect.txt")
                    .map(|text| options_from_text(&text))
                    .unwrap_or_default()
            })
            .clone()
    })
}

/// Keeps and saves the AutoCorrect settings.
pub fn set_autocorrect(options: AutoCorrectOptions) {
    save("autocorrect.txt", &options_to_text(&options));
    AUTOCORRECT.with(|cell| *cell.borrow_mut() = Some(options));
}

fn keyed(name: &str) -> BTreeMap<String, String> {
    KEYED.with(|cell| {
        cell.borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| parse_keyed(&load(name).unwrap_or_default()))
            .clone()
    })
}

/// One value of a `key=value` settings text.
pub fn value(name: &str, key: &str) -> Option<String> {
    keyed(name).get(key).cloned()
}

/// Sets one value of a `key=value` settings text and saves it.
pub fn set_value(name: &str, key: &str, value: &str) {
    let mut values = keyed(name);
    values.insert(key.to_string(), value.to_string());
    save(name, &format_keyed(&values));
    KEYED.with(|cell| cell.borrow_mut().insert(name.to_string(), values));
}

/// A yes/no setting, `default` until set.
pub fn flag(name: &str, key: &str, default: bool) -> bool {
    match value(name, key).as_deref() {
        Some("1") => true,
        Some("0") => false,
        _ => default,
    }
}

pub fn set_flag(name: &str, key: &str, on: bool) {
    set_value(name, key, if on { "1" } else { "0" });
}
