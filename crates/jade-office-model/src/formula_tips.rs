//! Formula argument tips: inside a function's parentheses while a formula
//! is typed, the function's arguments as Excel shows them under the cell —
//! `SUM(number1, [number2], ...)` — with the one the caret is in marked.

/// The arguments of the functions the tip knows, as Excel names them.
const SIGNATURES: &[(&str, &[&str])] = &[
    ("ABS", &["number"]),
    ("AND", &["logical1", "[logical2]", "..."]),
    ("AVERAGE", &["number1", "[number2]", "..."]),
    ("AVERAGEIF", &["range", "criteria", "[average_range]"]),
    (
        "AVERAGEIFS",
        &["average_range", "criteria_range1", "criteria1", "..."],
    ),
    ("CONCAT", &["text1", "[text2]", "..."]),
    ("CONCATENATE", &["text1", "[text2]", "..."]),
    ("COUNT", &["value1", "[value2]", "..."]),
    ("COUNTA", &["value1", "[value2]", "..."]),
    ("COUNTBLANK", &["range"]),
    ("COUNTIF", &["range", "criteria"]),
    ("COUNTIFS", &["criteria_range1", "criteria1", "..."]),
    ("DATE", &["year", "month", "day"]),
    ("DATEDIF", &["start_date", "end_date", "unit"]),
    ("DAY", &["serial_number"]),
    ("EDATE", &["start_date", "months"]),
    ("EOMONTH", &["start_date", "months"]),
    ("FILTER", &["array", "include", "[if_empty]"]),
    ("FIND", &["find_text", "within_text", "[start_num]"]),
    (
        "HLOOKUP",
        &[
            "lookup_value",
            "table_array",
            "row_index_num",
            "[range_lookup]",
        ],
    ),
    (
        "IF",
        &["logical_test", "[value_if_true]", "[value_if_false]"],
    ),
    ("IFERROR", &["value", "value_if_error"]),
    ("IFNA", &["value", "value_if_na"]),
    ("IFS", &["logical_test1", "value_if_true1", "..."]),
    ("INDEX", &["array", "row_num", "[column_num]"]),
    ("INT", &["number"]),
    ("LEFT", &["text", "[num_chars]"]),
    ("LEN", &["text"]),
    ("LOWER", &["text"]),
    ("MATCH", &["lookup_value", "lookup_array", "[match_type]"]),
    ("MAX", &["number1", "[number2]", "..."]),
    (
        "MAXIFS",
        &["max_range", "criteria_range1", "criteria1", "..."],
    ),
    ("MEDIAN", &["number1", "[number2]", "..."]),
    ("MID", &["text", "start_num", "num_chars"]),
    ("MIN", &["number1", "[number2]", "..."]),
    (
        "MINIFS",
        &["min_range", "criteria_range1", "criteria1", "..."],
    ),
    ("MOD", &["number", "divisor"]),
    ("MONTH", &["serial_number"]),
    ("NETWORKDAYS", &["start_date", "end_date", "[holidays]"]),
    ("NOT", &["logical"]),
    ("NOW", &[]),
    ("OR", &["logical1", "[logical2]", "..."]),
    ("PMT", &["rate", "nper", "pv", "[fv]", "[type]"]),
    ("PROPER", &["text"]),
    ("RANK", &["number", "ref", "[order]"]),
    (
        "REPLACE",
        &["old_text", "start_num", "num_chars", "new_text"],
    ),
    ("RIGHT", &["text", "[num_chars]"]),
    ("ROUND", &["number", "num_digits"]),
    ("ROUNDDOWN", &["number", "num_digits"]),
    ("ROUNDUP", &["number", "num_digits"]),
    ("SEARCH", &["find_text", "within_text", "[start_num]"]),
    (
        "SORT",
        &["array", "[sort_index]", "[sort_order]", "[by_col]"],
    ),
    (
        "SUBSTITUTE",
        &["text", "old_text", "new_text", "[instance_num]"],
    ),
    ("SUBTOTAL", &["function_num", "ref1", "..."]),
    ("SUM", &["number1", "[number2]", "..."]),
    ("SUMIF", &["range", "criteria", "[sum_range]"]),
    (
        "SUMIFS",
        &["sum_range", "criteria_range1", "criteria1", "..."],
    ),
    ("SUMPRODUCT", &["array1", "[array2]", "..."]),
    ("TEXT", &["value", "format_text"]),
    ("TEXTJOIN", &["delimiter", "ignore_empty", "text1", "..."]),
    ("TODAY", &[]),
    ("TRIM", &["text"]),
    ("UNIQUE", &["array", "[by_col]", "[exactly_once]"]),
    ("UPPER", &["text"]),
    ("VALUE", &["text"]),
    (
        "VLOOKUP",
        &[
            "lookup_value",
            "table_array",
            "col_index_num",
            "[range_lookup]",
        ],
    ),
    ("WEEKDAY", &["serial_number", "[return_type]"]),
    ("WORKDAY", &["start_date", "days", "[holidays]"]),
    (
        "XLOOKUP",
        &[
            "lookup_value",
            "lookup_array",
            "return_array",
            "[if_not_found]",
            "[match_mode]",
            "[search_mode]",
        ],
    ),
    ("YEAR", &["serial_number"]),
];

/// The tip for a formula being typed: the function and its arguments, and
/// which of them the caret is in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArgumentTip {
    pub name: &'static str,
    pub arguments: &'static [&'static str],
    /// The argument the caret is in, counted from 0; past a repeating
    /// `...`, the last named one.
    pub current: usize,
}

impl ArgumentTip {
    /// The tip's pieces, in order, each with whether it is the current
    /// argument: `SUM(`, `number1`, `, `, `[number2]`, … `)`.
    pub fn pieces(&self) -> Vec<(String, bool)> {
        let mut pieces = vec![(format!("{}(", self.name), false)];
        for (index, argument) in self.arguments.iter().enumerate() {
            if index > 0 {
                pieces.push((", ".to_string(), false));
            }
            pieces.push((argument.to_string(), index == self.current));
        }
        pieces.push((")".to_string(), false));
        pieces
    }
}

/// The tip for `text` with the caret at byte `caret`: the innermost call
/// the caret is inside of — not in a quoted string — when its function is
/// one the tip knows.
pub fn argument_tip(text: &str, caret: usize) -> Option<ArgumentTip> {
    if !text.starts_with('=') || caret > text.len() || !text.is_char_boundary(caret) {
        return None;
    }
    // Each open call before the caret: where its name ends, and the commas
    // at its own depth so far.
    let mut calls: Vec<(usize, usize)> = Vec::new();
    let mut quoted = false;
    for (at, c) in text[..caret].char_indices() {
        match c {
            '"' => quoted = !quoted,
            _ if quoted => {}
            '(' => calls.push((at, 0)),
            ')' => {
                calls.pop();
            }
            ',' => {
                if let Some(call) = calls.last_mut() {
                    call.1 += 1;
                }
            }
            _ => {}
        }
    }
    if quoted {
        return None;
    }
    let &(open, commas) = calls.last()?;
    let before = &text[..open];
    let start = before
        .char_indices()
        .rev()
        .take_while(|(_, c)| c.is_ascii_alphanumeric() || *c == '.' || *c == '_')
        .last()
        .map(|(at, _)| at)?;
    let name = before[start..].to_ascii_uppercase();
    let &(name, arguments) = SIGNATURES.iter().find(|(known, _)| *known == name)?;
    let repeating = arguments.last() == Some(&"...");
    let current = if repeating && commas + 1 >= arguments.len() {
        // Past the named ones, a repeating list stays on its last name.
        arguments.len().saturating_sub(2)
    } else {
        commas.min(arguments.len().saturating_sub(1))
    };
    Some(ArgumentTip {
        name,
        arguments,
        current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tip_follows_the_innermost_call_and_its_argument() {
        let tip = argument_tip("=VLOOKUP(A1,B:C,", 16).unwrap();
        assert_eq!((tip.name, tip.current), ("VLOOKUP", 2));
        // Inside a call nested in another, the inner one.
        let tip = argument_tip("=IF(sum(A1,", 11).unwrap();
        assert_eq!((tip.name, tip.current), ("SUM", 1));
        // Closed again, the outer one, on its own commas.
        let tip = argument_tip("=IF(SUM(A1,A2),", 15).unwrap();
        assert_eq!((tip.name, tip.current), ("IF", 1));
        // A comma inside quotes is text.
        let tip = argument_tip("=TEXT(A1,\"#,##0", 15);
        assert!(tip.is_none());
        let tip = argument_tip("=TEXTJOIN(\",\",", 14).unwrap();
        assert_eq!(tip.current, 1);
        // Past the named arguments of a repeating list.
        let tip = argument_tip("=SUM(1,2,3,4", 12).unwrap();
        assert_eq!(tip.current, 1);
        // Not a formula, outside any call, or a function it does not know.
        assert!(argument_tip("SUM(", 4).is_none());
        assert!(argument_tip("=A1+", 4).is_none());
        assert!(argument_tip("=NOSUCH(", 8).is_none());
        let pieces = argument_tip("=ROUND(A1,", 10).unwrap().pieces();
        assert_eq!(
            pieces,
            [
                ("ROUND(".to_string(), false),
                ("number".to_string(), false),
                (", ".to_string(), false),
                ("num_digits".to_string(), true),
                (")".to_string(), false),
            ]
        );
    }

    #[test]
    fn every_function_the_tip_knows_is_one_the_engine_has() {
        let known = crate::formula_complete::function_names();
        for (name, _) in SIGNATURES {
            assert!(known.iter().any(|known| known == name), "{name}");
        }
    }
}
