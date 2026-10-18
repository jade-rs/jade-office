//! Excel's clipboard text: a tab between cells, a line break between rows,
//! and a cell that holds a line break, a tab or a quote wrapped in quotes
//! with its own quotes doubled. Rows may end in CR LF (Windows) or LF.

/// The rows and cells of clipboard text. A trailing line break ends the
/// last row rather than starting an empty one.
pub fn parse(text: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut chars = text.chars().peekable();
    let mut at_field_start = true;
    let mut anything = false;
    while let Some(c) = chars.next() {
        anything = true;
        match c {
            '"' if at_field_start => {
                // Quoted: to the closing quote; `""` is one quote.
                loop {
                    match chars.next() {
                        Some('"') if chars.peek() == Some(&'"') => {
                            chars.next();
                            field.push('"');
                        }
                        Some('"') | None => break,
                        Some(other) => field.push(other),
                    }
                }
                at_field_start = false;
            }
            '\t' => {
                row.push(std::mem::take(&mut field));
                at_field_start = true;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' | '\r' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                at_field_start = true;
                anything = false;
            }
            other => {
                field.push(other);
                at_field_start = false;
            }
        }
    }
    if anything {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// One cell's text as a clipboard field, quoted when Excel would split it.
pub fn field(text: &str) -> String {
    if text.contains(['\t', '\n', '\r', '"']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_quoted_text_round_trips() {
        assert_eq!(parse("a\tb\nc\td\n"), vec![vec!["a", "b"], vec!["c", "d"]]);
        assert_eq!(parse("a\tb"), vec![vec!["a", "b"]]);
        assert_eq!(parse("\"x\r\ny\"\r\n"), vec![vec!["x\r\ny"]]);
        assert_eq!(parse("a\t\n"), vec![vec!["a", ""]]);
        for text in ["say \"hi\"", "a\tb", "two\nlines", "plain"] {
            assert_eq!(parse(&field(text)), vec![vec![text]]);
        }
    }
}
