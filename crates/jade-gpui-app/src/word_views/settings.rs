//! The Word settings the app keeps between runs — オートコレクト, the
//! Word オプション (paste defaults, typing), the リボンのユーザー設定 and
//! the クイック アクセス ツール バー — as small text files beside the
//! theme's in Application Support.
//!
//! Tests read and write an in-memory store instead, so they never touch the
//! user's choices.

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use jade_office_model::word::autocorrect::{
    options_from_text, options_to_text, AutoCorrectOptions,
};

thread_local! {
    /// What tests save, by file name.
    static MEMORY: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static AUTOCORRECT: RefCell<Option<AutoCorrectOptions>> = const { RefCell::new(None) };
    static KEYED: RefCell<HashMap<String, HashMap<String, String>>> = RefCell::new(HashMap::new());
}

fn file(name: &str) -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join("Library/Application Support/jade-office")
            .join(name),
    )
}

/// A settings file's text, if it was ever saved.
pub fn load(name: &str) -> Option<String> {
    if cfg!(test) {
        return MEMORY.with(|memory| memory.borrow().get(name).cloned());
    }
    std::fs::read_to_string(file(name)?).ok()
}

/// Saves a settings file.
pub fn save(name: &str, text: &str) {
    if cfg!(test) {
        MEMORY.with(|memory| {
            memory
                .borrow_mut()
                .insert(name.to_string(), text.to_string())
        });
        return;
    }
    let Some(path) = file(name) else {
        return;
    };
    if let Some(directory) = path.parent() {
        std::fs::create_dir_all(directory).ok();
    }
    std::fs::write(path, text).ok();
}

/// The AutoCorrect settings, read once.
pub fn autocorrect() -> AutoCorrectOptions {
    AUTOCORRECT.with(|cell| {
        cell.borrow_mut()
            .get_or_insert_with(|| {
                load("autocorrect.txt")
                    .map(|text| options_from_text(&text))
                    .unwrap_or_else(|| {
                        // Tests type text and expect it as typed: they turn
                        // AutoCorrect on when it is what they test.
                        if cfg!(test) {
                            all_off()
                        } else {
                            AutoCorrectOptions::default()
                        }
                    })
            })
            .clone()
    })
}

/// Every AutoCorrect setting off.
fn all_off() -> AutoCorrectOptions {
    use jade_office_model::word::autocorrect::{flag_mut, AUTOCORRECT_KEYS, AUTOFORMAT_KEYS};
    let mut options = AutoCorrectOptions::default();
    for key in AUTOCORRECT_KEYS.iter().chain(AUTOFORMAT_KEYS.iter()) {
        if let Some(flag) = flag_mut(&mut options, key) {
            *flag = false;
        }
    }
    options
}

/// Keeps and saves the AutoCorrect settings.
pub fn set_autocorrect(options: AutoCorrectOptions) {
    save("autocorrect.txt", &options_to_text(&options));
    AUTOCORRECT.with(|cell| *cell.borrow_mut() = Some(options));
}

/// A `key=value` settings file, read once.
fn keyed(name: &str) -> HashMap<String, String> {
    KEYED.with(|cell| {
        cell.borrow_mut()
            .entry(name.to_string())
            .or_insert_with(|| {
                load(name)
                    .unwrap_or_default()
                    .lines()
                    .filter_map(|line| line.split_once('='))
                    .map(|(key, value)| (key.to_string(), value.to_string()))
                    .collect()
            })
            .clone()
    })
}

/// One value of a `key=value` settings file.
pub fn value(name: &str, key: &str) -> Option<String> {
    keyed(name).get(key).cloned()
}

/// Sets one value of a `key=value` settings file and saves it.
pub fn set_value(name: &str, key: &str, value: &str) {
    let mut values = keyed(name);
    values.insert(key.to_string(), value.to_string());
    let mut keys: Vec<_> = values.keys().cloned().collect();
    keys.sort();
    let text: String = keys
        .iter()
        .map(|key| format!("{key}={}\n", values[key]))
        .collect();
    save(name, &text);
    KEYED.with(|cell| cell.borrow_mut().insert(name.to_string(), values));
}
