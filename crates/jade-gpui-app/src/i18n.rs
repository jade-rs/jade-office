//! The shell's words, in Japanese and English.
//!
//! Each language is one tab-separated table compiled in (`i18n/<lang>.tsv`):
//! a key, a tab, the text. `{name}` in a text is a placeholder that
//! [`format`] fills. A key missing from the live language falls back to
//! Japanese, and a key missing from both shows as itself — visibly wrong,
//! rather than an empty label.
//!
//! The language is picked once at start-up: `JADE_LANG` (`ja` / `en`) wins,
//! then the system's preferred language.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Japanese,
    English,
}

const JAPANESE: &str = include_str!("../i18n/ja.tsv");
const ENGLISH: &str = include_str!("../i18n/en.tsv");

static LANGUAGE: AtomicU8 = AtomicU8::new(0);

fn parse(table: &'static str) -> HashMap<&'static str, &'static str> {
    table
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.split_once('\t'))
        .collect()
}

fn table(language: Language) -> &'static HashMap<&'static str, &'static str> {
    static JA: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    static EN: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    match language {
        Language::Japanese => JA.get_or_init(|| parse(JAPANESE)),
        Language::English => EN.get_or_init(|| parse(ENGLISH)),
    }
}

pub fn language() -> Language {
    match LANGUAGE.load(Ordering::Relaxed) {
        1 => Language::English,
        _ => Language::Japanese,
    }
}

pub fn set_language(language: Language) {
    LANGUAGE.store(
        match language {
            Language::Japanese => 0,
            Language::English => 1,
        },
        Ordering::Relaxed,
    );
}

/// Picks the language from `JADE_LANG`, else from the system.
pub fn init() {
    let wanted = std::env::var("JADE_LANG")
        .ok()
        .or_else(system_language)
        .unwrap_or_default();
    set_language(if wanted.to_lowercase().starts_with("en") {
        Language::English
    } else {
        Language::Japanese
    });
}

#[cfg(target_os = "macos")]
fn system_language() -> Option<String> {
    let languages = objc2_foundation::NSLocale::preferredLanguages();
    languages.firstObject().map(|language| language.to_string())
}

#[cfg(not(target_os = "macos"))]
fn system_language() -> Option<String> {
    std::env::var("LC_ALL")
        .ok()
        .or_else(|| std::env::var("LANG").ok())
}

/// The text for `key` in the live language.
pub fn text(key: &'static str) -> &'static str {
    table(language())
        .get(key)
        .or_else(|| table(Language::Japanese).get(key))
        .copied()
        .unwrap_or(key)
}

/// [`text`] with each `{name}` replaced by its value.
pub fn format(key: &'static str, values: &[(&str, &dyn std::fmt::Display)]) -> String {
    let mut out = text(key).to_string();
    for (name, value) in values {
        out = out.replace(&format!("{{{name}}}"), &value.to_string());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    fn keys(table: &'static str) -> Vec<&'static str> {
        table
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(|line| line.split_once('\t').map(|(key, _)| key).unwrap_or(line))
            .collect()
    }

    /// A key in one table and not the other is a label that silently
    /// changes language, or shows its key.
    #[test]
    fn both_languages_have_the_same_keys_once_each() {
        for table in [JAPANESE, ENGLISH] {
            let keys = keys(table);
            let unique: HashSet<_> = keys.iter().collect();
            assert_eq!(keys.len(), unique.len(), "a key appears twice");
            for line in table
                .lines()
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
            {
                assert!(line.contains('\t'), "no tab in {line:?}");
            }
        }
        let ja: HashSet<_> = keys(JAPANESE).into_iter().collect();
        let en: HashSet<_> = keys(ENGLISH).into_iter().collect();
        assert_eq!(
            ja.symmetric_difference(&en).collect::<Vec<_>>(),
            Vec::<&&str>::new()
        );
    }

    /// Placeholders must match too, or one language drops a number.
    #[test]
    fn placeholders_agree_between_languages() {
        fn placeholders(text: &str) -> Vec<String> {
            let mut found: Vec<String> = text
                .split('{')
                .skip(1)
                .filter_map(|part| part.split_once('}').map(|(name, _)| name.to_string()))
                .collect();
            found.sort();
            found
        }
        let ja = parse(JAPANESE);
        let en = parse(ENGLISH);
        for (key, text) in &ja {
            assert_eq!(placeholders(text), placeholders(en[key]), "{key}");
        }
    }

    #[test]
    fn format_fills_placeholders() {
        let out = format("deck.position", &[("current", &2), ("count", &5)]);
        assert!(out.contains('2') && out.contains('5'), "{out}");
        assert!(!out.contains('{'));
    }

    #[test]
    fn a_missing_key_shows_itself() {
        assert_eq!(text("no.such.key"), "no.such.key");
    }
}
