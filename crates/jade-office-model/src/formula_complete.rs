//! Formula AutoComplete: while a formula is typed, the functions, defined
//! names and tables whose names start with the word before the caret, as
//! Excel lists them under the cell; Tab puts the one picked in — a
//! function with its `(`.

use std::sync::OnceLock;

use crate::text::TextEdit;

/// How many names the list shows at most.
pub const MAX_SHOWN: usize = 8;

/// Every function the engine knows, in alphabetical order.
pub fn function_names() -> &'static [String] {
    static NAMES: OnceLock<Vec<String>> = OnceLock::new();
    NAMES.get_or_init(ironcalc_base::supported_function_names)
}

/// The list for a formula being typed: where the word it completes starts
/// (a byte offset), and the names that start with it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub start: usize,
    pub names: Vec<String>,
}

/// Whether `name` is a function's, which goes in with its `(`.
pub fn is_function(name: &str) -> bool {
    function_names().iter().any(|function| function == name)
}

/// The list for `text` with the caret at byte `caret`, when it is a formula
/// and the caret ends a word that could be a function's name — not inside
/// a quoted string, not a sheet's or a reference's tail, and not followed
/// by more of the word.
pub fn completion(text: &str, caret: usize) -> Option<Completion> {
    completion_with(text, caret, &[])
}

/// [`completion`] listing the workbook's defined names and tables (`names`)
/// with the functions, in one alphabetical list, as Excel does.
pub fn completion_with(text: &str, caret: usize, names: &[String]) -> Option<Completion> {
    // `+` and `-` start a formula too, as in Excel.
    if !text.starts_with(['=', '+', '-']) || caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    let word = |c: char| c.is_ascii_alphanumeric() || c == '.' || c == '_';
    if text[caret..].chars().next().is_some_and(word) {
        return None;
    }
    let before = &text[..caret];
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| word(*c))
        .last()
        .map_or(caret, |(at, _)| at);
    let typed = &text[start..caret];
    if !typed
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return None;
    }
    // A sheet name (`Sheet1!`), an absolute reference (`$A`), or text in
    // quotes: no function goes there.
    if matches!(text[..start].chars().last(), Some('!' | '$' | '\'' | ':')) {
        return None;
    }
    if text[..start].matches('"').count() % 2 == 1 {
        return None;
    }
    let typed = typed.to_uppercase();
    let mut listed: Vec<String> = function_names()
        .iter()
        .filter(|name| name.starts_with(&typed))
        .take(MAX_SHOWN)
        .cloned()
        .chain(
            names
                .iter()
                .filter(|name| name.to_uppercase().starts_with(&typed))
                .cloned(),
        )
        .collect();
    listed.sort_by_key(|name| name.to_uppercase());
    listed.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    listed.truncate(MAX_SHOWN);
    (!listed.is_empty()).then_some(Completion {
        start,
        names: listed,
    })
}

/// The name picked put in for the word being typed — a function's with its
/// `(` — the caret after it.
pub fn accept(edit: &mut TextEdit, completion: &Completion, name: &str) {
    let end = edit.caret.min(edit.text.len());
    let text = if is_function(name) {
        format!("{name}(")
    } else {
        name.to_string()
    };
    edit.text.replace_range(completion.start..end, &text);
    edit.caret = completion.start + text.len();
    edit.whole = false;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_word_before_the_caret_lists_the_functions_it_starts() {
        let list = completion("=SU", 3).unwrap();
        assert_eq!(list.start, 1);
        assert!(list.names.iter().any(|name| name == "SUM"));
        assert!(list.names.iter().any(|name| name == "SUMIF"));
        assert!(list.names.len() <= MAX_SHOWN);
        // Lower case works; inside an argument too.
        let list = completion("=1+vlo", 6).unwrap();
        assert_eq!((list.start, list.names[0].as_str()), (3, "VLOOKUP"));
        // Not a formula, mid-word, in quotes, after a sheet or a `$`.
        assert!(completion("SU", 2).is_none());
        assert!(completion("=SUM", 2).is_none());
        assert!(completion("=\"su", 4).is_none());
        assert!(completion("=Sheet1!SU", 10).is_none());
        assert!(completion("=$A", 3).is_none());
        assert!(completion("=12", 3).is_none());
        assert!(completion("=QQQ", 4).is_none());
    }

    #[test]
    fn accepting_puts_the_name_in_with_its_parenthesis() {
        let mut edit = TextEdit::new("=1+vlo".into());
        edit.caret = 6;
        let list = completion(&edit.text, edit.caret).unwrap();
        accept(&mut edit, &list, "VLOOKUP");
        assert_eq!(edit.text, "=1+VLOOKUP(");
        assert_eq!(edit.caret, edit.text.len());
    }

    #[test]
    fn defined_names_and_tables_are_listed_with_the_functions() {
        let names = vec!["SalesTax".to_string(), "Sales_2024".to_string(), "Rates".to_string()];
        let list = completion_with("=sa", 3, &names).unwrap();
        // Alphabetical, the names among the functions.
        assert!(list.names.iter().any(|name| name == "SalesTax"));
        assert!(list.names.iter().any(|name| name == "Sales_2024"));
        assert!(!list.names.iter().any(|name| name == "Rates"));
        let mut sorted = list.names.clone();
        sorted.sort_by_key(|name| name.to_uppercase());
        assert_eq!(sorted, list.names);
        // A name goes in as it is, without a parenthesis.
        let mut edit = TextEdit::new("=SUM(sal".into());
        edit.caret = 8;
        let list = completion_with(&edit.text, edit.caret, &names).unwrap();
        accept(&mut edit, &list, "SalesTax");
        assert_eq!(edit.text, "=SUM(SalesTax");
        assert_eq!(edit.caret, edit.text.len());
        // A formula started with + or - lists them too.
        assert!(completion_with("+Rat", 4, &names).is_some());
        assert!(completion_with("-su", 3, &[]).is_some());
    }
}
