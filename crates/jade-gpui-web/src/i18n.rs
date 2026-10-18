//! The shell's words: the native app's tables (`jade-gpui-app/i18n`),
//! compiled in here too, so both builds say the same thing. The language is
//! the page's (`navigator.language`).

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;

const JAPANESE: &str = include_str!("../../jade-gpui-app/i18n/ja.tsv");
const ENGLISH: &str = include_str!("../../jade-gpui-app/i18n/en.tsv");

thread_local! {
    static ENGLISH_LIVE: Cell<bool> = const { Cell::new(false) };
}

fn parse(table: &'static str) -> HashMap<&'static str, &'static str> {
    table
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

fn table(english: bool) -> &'static HashMap<&'static str, &'static str> {
    static JA: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    static EN: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    if english {
        EN.get_or_init(|| parse(ENGLISH))
    } else {
        JA.get_or_init(|| parse(JAPANESE))
    }
}

/// `language` is a BCP 47 tag; English for `en…`, Japanese otherwise.
pub fn init(language: &str) {
    ENGLISH_LIVE.with(|cell| cell.set(language.to_lowercase().starts_with("en")));
}

pub fn text(key: &'static str) -> &'static str {
    table(ENGLISH_LIVE.with(Cell::get))
        .get(key)
        .or_else(|| table(false).get(key))
        .copied()
        .unwrap_or(key)
}

pub fn format(key: &'static str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = text(key).to_string();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}
