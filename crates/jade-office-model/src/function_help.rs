//! What a function does, in a line, as Insert Function says it under the
//! function picked — in English or Japanese.

/// Each function's line: its name, then English and Japanese.
const HELP: &[(&str, &str, &str)] = &[
    (
        "ABS",
        "Returns the absolute value of a number.",
        "数値の絶対値を返します。",
    ),
    (
        "AND",
        "Checks whether all arguments are TRUE.",
        "すべての引数が TRUE かどうかを調べます。",
    ),
    (
        "AVERAGE",
        "Returns the average (arithmetic mean) of its arguments.",
        "引数の平均値 (算術平均) を返します。",
    ),
    (
        "AVERAGEIF",
        "Averages the cells that meet a condition.",
        "条件に一致するセルの平均値を返します。",
    ),
    (
        "AVERAGEIFS",
        "Averages the cells that meet several conditions.",
        "複数の条件に一致するセルの平均値を返します。",
    ),
    (
        "CEILING",
        "Rounds a number up to the nearest multiple of significance.",
        "数値を基準値の倍数に切り上げます。",
    ),
    (
        "CHOOSE",
        "Picks a value from a list by its index.",
        "インデックスに従って、引数リストから値を選びます。",
    ),
    (
        "CONCAT",
        "Joins a list or range of text strings.",
        "複数の文字列やセル範囲の文字列を結合します。",
    ),
    (
        "CONCATENATE",
        "Joins several text strings into one.",
        "複数の文字列を結合して 1 つの文字列にします。",
    ),
    (
        "COUNT",
        "Counts the cells that contain numbers.",
        "数値が含まれるセルの個数を返します。",
    ),
    (
        "COUNTA",
        "Counts the cells that are not empty.",
        "空白でないセルの個数を返します。",
    ),
    (
        "COUNTBLANK",
        "Counts the empty cells in a range.",
        "範囲内の空白セルの個数を返します。",
    ),
    (
        "COUNTIF",
        "Counts the cells that meet a condition.",
        "条件に一致するセルの個数を返します。",
    ),
    (
        "COUNTIFS",
        "Counts the cells that meet several conditions.",
        "複数の条件に一致するセルの個数を返します。",
    ),
    (
        "DATE",
        "Returns the serial number of a date from year, month and day.",
        "年、月、日から日付のシリアル値を返します。",
    ),
    (
        "DATEDIF",
        "Returns the years, months or days between two dates.",
        "2 つの日付の間の年数、月数、日数を返します。",
    ),
    (
        "DAY",
        "Returns the day of the month, 1 to 31.",
        "日付の日 (1～31) を返します。",
    ),
    (
        "DAYS",
        "Returns the number of days between two dates.",
        "2 つの日付の間の日数を返します。",
    ),
    (
        "EDATE",
        "Returns the date a number of months before or after a date.",
        "指定した月数だけ前または後の日付を返します。",
    ),
    (
        "EOMONTH",
        "Returns the last day of the month a number of months away.",
        "指定した月数だけ離れた月の末日を返します。",
    ),
    (
        "EXACT",
        "Checks whether two text strings are exactly the same.",
        "2 つの文字列が完全に同じかどうかを調べます。",
    ),
    (
        "FILTER",
        "Filters a range by the conditions you give.",
        "条件に基づいて範囲をフィルターします。",
    ),
    (
        "FIND",
        "Finds one text string within another (case-sensitive).",
        "文字列の中から別の文字列を検索します (大文字と小文字を区別)。",
    ),
    (
        "FLOOR",
        "Rounds a number down to the nearest multiple of significance.",
        "数値を基準値の倍数に切り捨てます。",
    ),
    (
        "FV",
        "Returns the future value of an investment.",
        "投資の将来価値を返します。",
    ),
    (
        "HLOOKUP",
        "Looks up a value in the top row and returns a value below it.",
        "範囲の先頭行で値を検索し、その列の値を返します。",
    ),
    (
        "HOUR",
        "Returns the hour, 0 to 23.",
        "時刻の時 (0～23) を返します。",
    ),
    (
        "HYPERLINK",
        "Creates a link to a place in the workbook or a web page.",
        "ブック内の場所や Web ページへのリンクを作成します。",
    ),
    (
        "IF",
        "Returns one value if a condition is TRUE and another if FALSE.",
        "条件が TRUE のときと FALSE のときで異なる値を返します。",
    ),
    (
        "IFERROR",
        "Returns a value of your choice if a formula gives an error.",
        "数式がエラーのとき指定した値を返し、それ以外は数式の結果を返します。",
    ),
    (
        "IFNA",
        "Returns a value of your choice if a formula gives #N/A.",
        "数式が #N/A のとき指定した値を返します。",
    ),
    (
        "IFS",
        "Returns the value for the first condition that is TRUE.",
        "最初に TRUE になる条件に対応する値を返します。",
    ),
    (
        "INDEX",
        "Returns the value at a row and column of a range.",
        "範囲内の指定した行と列の位置にある値を返します。",
    ),
    (
        "INDIRECT",
        "Returns the reference a text string names.",
        "文字列で指定されたセル参照を返します。",
    ),
    (
        "INT",
        "Rounds a number down to the nearest integer.",
        "数値を超えない最大の整数を返します。",
    ),
    (
        "ISBLANK",
        "Checks whether a value is empty.",
        "セルが空白かどうかを調べます。",
    ),
    (
        "ISERROR",
        "Checks whether a value is any error.",
        "値がエラーかどうかを調べます。",
    ),
    (
        "ISNUMBER",
        "Checks whether a value is a number.",
        "値が数値かどうかを調べます。",
    ),
    (
        "ISTEXT",
        "Checks whether a value is text.",
        "値が文字列かどうかを調べます。",
    ),
    (
        "LARGE",
        "Returns the k-th largest value in a data set.",
        "データの中で k 番目に大きい値を返します。",
    ),
    (
        "LEFT",
        "Returns characters from the start of a text string.",
        "文字列の先頭から指定した数の文字を返します。",
    ),
    (
        "LEN",
        "Returns the number of characters in a text string.",
        "文字列の文字数を返します。",
    ),
    (
        "LET",
        "Names the results of calculations to use in a formula.",
        "計算結果に名前を付けて、数式の中で使えるようにします。",
    ),
    (
        "LOOKUP",
        "Looks up a value in one row or column.",
        "1 行または 1 列の範囲で値を検索します。",
    ),
    (
        "LOWER",
        "Converts text to lowercase.",
        "文字列を小文字に変換します。",
    ),
    (
        "MATCH",
        "Returns the position of a value in a range.",
        "範囲内で値が何番目にあるかを返します。",
    ),
    ("MAX", "Returns the largest value.", "最大値を返します。"),
    (
        "MAXIFS",
        "Returns the largest value among the cells that meet conditions.",
        "条件に一致するセルの中の最大値を返します。",
    ),
    (
        "MEDIAN",
        "Returns the median of the given numbers.",
        "数値の中央値を返します。",
    ),
    (
        "MID",
        "Returns characters from the middle of a text string.",
        "文字列の指定した位置から指定した数の文字を返します。",
    ),
    ("MIN", "Returns the smallest value.", "最小値を返します。"),
    (
        "MINIFS",
        "Returns the smallest value among the cells that meet conditions.",
        "条件に一致するセルの中の最小値を返します。",
    ),
    (
        "MINUTE",
        "Returns the minute, 0 to 59.",
        "時刻の分 (0～59) を返します。",
    ),
    (
        "MOD",
        "Returns the remainder after division.",
        "割り算の余りを返します。",
    ),
    (
        "MONTH",
        "Returns the month, 1 to 12.",
        "日付の月 (1～12) を返します。",
    ),
    (
        "NETWORKDAYS",
        "Returns the number of working days between two dates.",
        "2 つの日付の間の稼働日数を返します。",
    ),
    (
        "NOT",
        "Reverses TRUE and FALSE.",
        "TRUE と FALSE を反転します。",
    ),
    (
        "NOW",
        "Returns the current date and time.",
        "現在の日付と時刻を返します。",
    ),
    (
        "NPV",
        "Returns the net present value of an investment.",
        "投資の正味現在価値を返します。",
    ),
    (
        "OR",
        "Checks whether any argument is TRUE.",
        "いずれかの引数が TRUE かどうかを調べます。",
    ),
    (
        "PMT",
        "Returns the periodic payment for a loan.",
        "ローンの定期支払額を返します。",
    ),
    (
        "PRODUCT",
        "Multiplies all the numbers given.",
        "引数の積を返します。",
    ),
    (
        "PROPER",
        "Capitalises the first letter of each word.",
        "各単語の先頭文字を大文字にします。",
    ),
    (
        "PV",
        "Returns the present value of an investment.",
        "投資の現在価値を返します。",
    ),
    (
        "RAND",
        "Returns a random number from 0 up to 1.",
        "0 以上 1 未満の乱数を返します。",
    ),
    (
        "RANDBETWEEN",
        "Returns a random integer between two numbers.",
        "指定した範囲の整数の乱数を返します。",
    ),
    (
        "RANK",
        "Returns the rank of a number in a list.",
        "数値の順位を返します。",
    ),
    (
        "REPLACE",
        "Replaces part of a text string by position.",
        "文字列の指定した位置の文字を置き換えます。",
    ),
    (
        "RIGHT",
        "Returns characters from the end of a text string.",
        "文字列の末尾から指定した数の文字を返します。",
    ),
    (
        "ROUND",
        "Rounds a number to a number of digits.",
        "数値を指定した桁数に四捨五入します。",
    ),
    (
        "ROUNDDOWN",
        "Rounds a number down, toward zero.",
        "数値を指定した桁数に切り捨てます。",
    ),
    (
        "ROUNDUP",
        "Rounds a number up, away from zero.",
        "数値を指定した桁数に切り上げます。",
    ),
    (
        "SEARCH",
        "Finds one text string within another (not case-sensitive).",
        "文字列の中から別の文字列を検索します (大文字と小文字を区別しない)。",
    ),
    (
        "SECOND",
        "Returns the second, 0 to 59.",
        "時刻の秒 (0～59) を返します。",
    ),
    (
        "SEQUENCE",
        "Returns a list of sequential numbers.",
        "連続した数値の配列を返します。",
    ),
    (
        "SIN",
        "Returns the sine of an angle.",
        "角度のサインを返します。",
    ),
    (
        "SMALL",
        "Returns the k-th smallest value in a data set.",
        "データの中で k 番目に小さい値を返します。",
    ),
    (
        "SORT",
        "Sorts the contents of a range.",
        "範囲の内容を並べ替えます。",
    ),
    (
        "SQRT",
        "Returns the square root of a number.",
        "数値の平方根を返します。",
    ),
    (
        "STDEV",
        "Estimates standard deviation from a sample.",
        "標本に基づいて標準偏差を推定します。",
    ),
    (
        "SUBSTITUTE",
        "Replaces old text with new text in a text string.",
        "文字列中の指定した文字列を別の文字列に置き換えます。",
    ),
    (
        "SUBTOTAL",
        "Returns a subtotal, leaving out other subtotals.",
        "リストの集計値を返します (他の SUBTOTAL は除きます)。",
    ),
    (
        "SUM",
        "Adds all the numbers in a range of cells.",
        "セル範囲に含まれる数値をすべて合計します。",
    ),
    (
        "SUMIF",
        "Adds the cells that meet a condition.",
        "条件に一致するセルの値を合計します。",
    ),
    (
        "SUMIFS",
        "Adds the cells that meet several conditions.",
        "複数の条件に一致するセルの値を合計します。",
    ),
    (
        "SUMPRODUCT",
        "Returns the sum of the products of matching items.",
        "配列の対応する要素の積を合計します。",
    ),
    (
        "SWITCH",
        "Compares a value with a list and returns the matching result.",
        "値を一覧と比較し、一致した値に対応する結果を返します。",
    ),
    (
        "TEXT",
        "Formats a number as text with a number format.",
        "数値を表示形式に従った文字列に変換します。",
    ),
    (
        "TEXTJOIN",
        "Joins text with a delimiter between the items.",
        "区切り文字を挟んで文字列を結合します。",
    ),
    (
        "TIME",
        "Returns the serial number of a time from hour, minute and second.",
        "時、分、秒から時刻のシリアル値を返します。",
    ),
    ("TODAY", "Returns today's date.", "現在の日付を返します。"),
    (
        "TRIM",
        "Removes extra spaces from text.",
        "文字列から余分なスペースを削除します。",
    ),
    (
        "UNIQUE",
        "Returns the unique values in a range.",
        "範囲内の一意の値を返します。",
    ),
    (
        "UPPER",
        "Converts text to uppercase.",
        "文字列を大文字に変換します。",
    ),
    (
        "VALUE",
        "Converts text that looks like a number into a number.",
        "数値を表す文字列を数値に変換します。",
    ),
    (
        "VLOOKUP",
        "Looks up a value in the first column and returns a value beside it.",
        "範囲の先頭列で値を検索し、その行の値を返します。",
    ),
    (
        "WEEKDAY",
        "Returns the day of the week as a number.",
        "日付の曜日を数値で返します。",
    ),
    (
        "WORKDAY",
        "Returns the date a number of working days away.",
        "指定した稼働日数だけ離れた日付を返します。",
    ),
    (
        "XLOOKUP",
        "Looks up a value and returns the matching item from another range.",
        "範囲で値を検索し、別の範囲の対応する項目を返します。",
    ),
    (
        "XMATCH",
        "Returns the position of a value in a range or array.",
        "範囲や配列の中で値が何番目にあるかを返します。",
    ),
    (
        "YEAR",
        "Returns the year of a date.",
        "日付の年を返します。",
    ),
];

/// What `name` does, in a line, in Japanese when `japanese`; `None` for a
/// function without one.
pub fn function_help(name: &str, japanese: bool) -> Option<&'static str> {
    HELP.iter()
        .find(|(known, _, _)| known.eq_ignore_ascii_case(name))
        .map(|(_, english, japanese_text)| if japanese { *japanese_text } else { *english })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_comes_in_either_language_for_functions_the_engine_has() {
        assert_eq!(
            function_help("sum", false),
            Some("Adds all the numbers in a range of cells.")
        );
        assert_eq!(
            function_help("SUM", true),
            Some("セル範囲に含まれる数値をすべて合計します。")
        );
        assert_eq!(function_help("NOSUCH", false), None);
        let known = crate::formula_complete::function_names();
        let missing: Vec<_> = HELP
            .iter()
            .map(|(name, _, _)| *name)
            .filter(|name| !known.iter().any(|known| known == name))
            .collect();
        assert!(missing.is_empty(), "MISSING {}", missing.join(" "));
    }
}
