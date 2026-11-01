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
        "ACOS",
        "Returns the arccosine of a number, in radians.",
        "数値のアークコサインをラジアンで返します。",
    ),
    (
        "ADDRESS",
        "Returns a cell reference as text.",
        "セル参照を文字列で返します。",
    ),
    (
        "AND",
        "Checks whether all arguments are TRUE.",
        "すべての引数が TRUE かどうかを調べます。",
    ),
    (
        "ASC",
        "Converts full-width letters to half-width.",
        "全角の英数カナ文字を半角に変換します。",
    ),
    (
        "ASIN",
        "Returns the arcsine of a number, in radians.",
        "数値のアークサインをラジアンで返します。",
    ),
    (
        "ATAN",
        "Returns the arctangent of a number, in radians.",
        "数値のアークタンジェントをラジアンで返します。",
    ),
    (
        "ATAN2",
        "Returns the arctangent of x and y coordinates.",
        "x 座標と y 座標のアークタンジェントを返します。",
    ),
    (
        "AVERAGE",
        "Returns the average (arithmetic mean) of its arguments.",
        "引数の平均値 (算術平均) を返します。",
    ),
    (
        "AVERAGEA",
        "Averages its arguments, counting text and FALSE as 0.",
        "引数の平均値を返します (文字列と FALSE は 0 として計算)。",
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
        "BIN2DEC",
        "Converts a binary number to decimal.",
        "2 進数を 10 進数に変換します。",
    ),
    (
        "BYCOL",
        "Applies a LAMBDA to each column.",
        "各列に LAMBDA を適用します。",
    ),
    (
        "BYROW",
        "Applies a LAMBDA to each row.",
        "各行に LAMBDA を適用します。",
    ),
    (
        "CEILING",
        "Rounds a number up to the nearest multiple of significance.",
        "数値を基準値の倍数に切り上げます。",
    ),
    (
        "CHAR",
        "Returns the character for a code number.",
        "数値で指定した文字を返します。",
    ),
    (
        "CHOOSE",
        "Picks a value from a list by its index.",
        "インデックスに従って、引数リストから値を選びます。",
    ),
    (
        "CHOOSECOLS",
        "Returns the chosen columns of an array.",
        "配列から指定した列を返します。",
    ),
    (
        "CHOOSEROWS",
        "Returns the chosen rows of an array.",
        "配列から指定した行を返します。",
    ),
    (
        "CLEAN",
        "Removes non-printable characters from text.",
        "文字列から印刷できない文字を削除します。",
    ),
    (
        "CODE",
        "Returns the code number of the first character.",
        "文字列の先頭文字の数値コードを返します。",
    ),
    (
        "COLUMN",
        "Returns the column number of a reference.",
        "セル参照の列番号を返します。",
    ),
    (
        "COLUMNS",
        "Returns the number of columns in a reference.",
        "範囲の列数を返します。",
    ),
    (
        "COMBIN",
        "Returns the number of combinations.",
        "組み合わせの数を返します。",
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
        "CONVERT",
        "Converts a number from one unit to another.",
        "数値の単位を変換します。",
    ),
    (
        "CORREL",
        "Returns the correlation coefficient of two data sets.",
        "2 つのデータの相関係数を返します。",
    ),
    (
        "COS",
        "Returns the cosine of an angle.",
        "角度のコサインを返します。",
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
        "DATEVALUE",
        "Converts a date written as text into a serial number.",
        "日付を表す文字列をシリアル値に変換します。",
    ),
    (
        "DAVERAGE",
        "Averages a database column's values that meet criteria.",
        "データベースの条件に一致するレコードの値の平均を返します。",
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
        "DAYS360",
        "Returns the days between two dates on a 360-day year.",
        "1 年を 360 日として 2 つの日付の間の日数を返します。",
    ),
    (
        "DB",
        "Returns fixed-declining-balance depreciation.",
        "定率法による減価償却費を返します。",
    ),
    (
        "DCOUNT",
        "Counts a database column's numbers that meet criteria.",
        "データベースの条件に一致するレコードの数値の個数を返します。",
    ),
    (
        "DDB",
        "Returns double-declining-balance depreciation.",
        "倍率法による減価償却費を返します。",
    ),
    (
        "DEC2BIN",
        "Converts a decimal number to binary.",
        "10 進数を 2 進数に変換します。",
    ),
    (
        "DEC2HEX",
        "Converts a decimal number to hexadecimal.",
        "10 進数を 16 進数に変換します。",
    ),
    (
        "DEGREES",
        "Converts radians to degrees.",
        "ラジアンを度に変換します。",
    ),
    (
        "DOLLAR",
        "Formats a number as currency text.",
        "数値を通貨形式の文字列に変換します。",
    ),
    (
        "DROP",
        "Removes rows or columns from the start or end of an array.",
        "配列の先頭または末尾から指定した行や列を除きます。",
    ),
    (
        "DSUM",
        "Adds the values in a database column that meet criteria.",
        "データベースの条件に一致するレコードの値を合計します。",
    ),
    (
        "EDATE",
        "Returns the date a number of months before or after a date.",
        "指定した月数だけ前または後の日付を返します。",
    ),
    (
        "EFFECT",
        "Returns the effective annual interest rate.",
        "実効年利率を返します。",
    ),
    (
        "EOMONTH",
        "Returns the last day of the month a number of months away.",
        "指定した月数だけ離れた月の末日を返します。",
    ),
    (
        "EVEN",
        "Rounds a number up to the nearest even integer.",
        "数値を最も近い偶数に切り上げます。",
    ),
    (
        "EXACT",
        "Checks whether two text strings are exactly the same.",
        "2 つの文字列が完全に同じかどうかを調べます。",
    ),
    (
        "EXP",
        "Returns e raised to the power of a number.",
        "e を底とする数値のべき乗を返します。",
    ),
    (
        "FACT",
        "Returns the factorial of a number.",
        "数値の階乗を返します。",
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
        "FIXED",
        "Formats a number as text with a fixed number of decimals.",
        "数値を指定した小数点以下桁数の文字列に変換します。",
    ),
    (
        "FLOOR",
        "Rounds a number down to the nearest multiple of significance.",
        "数値を基準値の倍数に切り捨てます。",
    ),
    (
        "FORECAST",
        "Predicts a value along a linear trend.",
        "線形トレンドに沿って値を予測します。",
    ),
    (
        "FORECAST.LINEAR",
        "Predicts a value along a linear trend.",
        "線形トレンドに沿って値を予測します。",
    ),
    (
        "FORMULATEXT",
        "Returns a cell's formula as text.",
        "セルの数式を文字列で返します。",
    ),
    (
        "FV",
        "Returns the future value of an investment.",
        "投資の将来価値を返します。",
    ),
    (
        "GCD",
        "Returns the greatest common divisor.",
        "最大公約数を返します。",
    ),
    (
        "GEOMEAN",
        "Returns the geometric mean.",
        "相乗平均を返します。",
    ),
    (
        "GETPIVOTDATA",
        "Returns data stored in a pivot table.",
        "ピボットテーブルのデータを返します。",
    ),
    (
        "HARMEAN",
        "Returns the harmonic mean.",
        "調和平均を返します。",
    ),
    (
        "HEX2DEC",
        "Converts a hexadecimal number to decimal.",
        "16 進数を 10 進数に変換します。",
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
        "HSTACK",
        "Stacks arrays horizontally.",
        "配列を横に連結します。",
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
        "INTERCEPT",
        "Returns where the regression line crosses the y-axis.",
        "回帰直線の切片を返します。",
    ),
    (
        "IPMT",
        "Returns the interest part of a loan payment.",
        "ローン返済額のうち金利部分を返します。",
    ),
    (
        "IRR",
        "Returns the internal rate of return.",
        "内部利益率を返します。",
    ),
    (
        "ISBLANK",
        "Checks whether a value is empty.",
        "セルが空白かどうかを調べます。",
    ),
    (
        "ISERR",
        "Checks whether a value is an error other than #N/A.",
        "値が #N/A 以外のエラーかどうかを調べます。",
    ),
    (
        "ISERROR",
        "Checks whether a value is any error.",
        "値がエラーかどうかを調べます。",
    ),
    (
        "ISEVEN",
        "Checks whether a number is even.",
        "数値が偶数かどうかを調べます。",
    ),
    (
        "ISFORMULA",
        "Checks whether a cell holds a formula.",
        "セルに数式が含まれるかどうかを調べます。",
    ),
    (
        "ISLOGICAL",
        "Checks whether a value is TRUE or FALSE.",
        "値が論理値かどうかを調べます。",
    ),
    (
        "ISNA",
        "Checks whether a value is #N/A.",
        "値が #N/A かどうかを調べます。",
    ),
    (
        "ISNUMBER",
        "Checks whether a value is a number.",
        "値が数値かどうかを調べます。",
    ),
    (
        "ISODD",
        "Checks whether a number is odd.",
        "数値が奇数かどうかを調べます。",
    ),
    (
        "ISOWEEKNUM",
        "Returns the ISO week number of a date.",
        "日付の ISO 週番号を返します。",
    ),
    (
        "ISTEXT",
        "Checks whether a value is text.",
        "値が文字列かどうかを調べます。",
    ),
    (
        "JIS",
        "Converts half-width letters to full-width.",
        "半角の英数カナ文字を全角に変換します。",
    ),
    (
        "LAMBDA",
        "Creates a function from parameters and a calculation.",
        "パラメーターと計算式から関数を作成します。",
    ),
    (
        "LARGE",
        "Returns the k-th largest value in a data set.",
        "データの中で k 番目に大きい値を返します。",
    ),
    (
        "LCM",
        "Returns the least common multiple.",
        "最小公倍数を返します。",
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
        "LN",
        "Returns the natural logarithm of a number.",
        "数値の自然対数を返します。",
    ),
    (
        "LOG",
        "Returns the logarithm of a number to a base.",
        "指定した底での数値の対数を返します。",
    ),
    (
        "LOG10",
        "Returns the base-10 logarithm of a number.",
        "数値の常用対数を返します。",
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
        "MAP",
        "Applies a LAMBDA to each value of arrays.",
        "配列の各値に LAMBDA を適用した結果を返します。",
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
        "MMULT",
        "Returns the matrix product of two arrays.",
        "2 つの配列の行列積を返します。",
    ),
    (
        "MOD",
        "Returns the remainder after division.",
        "割り算の余りを返します。",
    ),
    (
        "MODE",
        "Returns the most frequent value.",
        "最も頻繁に出現する値を返します。",
    ),
    (
        "MONTH",
        "Returns the month, 1 to 12.",
        "日付の月 (1～12) を返します。",
    ),
    (
        "MROUND",
        "Rounds a number to the nearest multiple.",
        "数値を指定した値の倍数に丸めます。",
    ),
    (
        "NA",
        "Returns the #N/A error.",
        "エラー値 #N/A を返します。",
    ),
    (
        "NETWORKDAYS",
        "Returns the number of working days between two dates.",
        "2 つの日付の間の稼働日数を返します。",
    ),
    (
        "NETWORKDAYS.INTL",
        "Returns working days between two dates with chosen weekends.",
        "週末を指定して 2 つの日付の間の稼働日数を返します。",
    ),
    (
        "NORM.DIST",
        "Returns the normal distribution.",
        "正規分布の値を返します。",
    ),
    (
        "NORM.INV",
        "Returns the inverse of the normal distribution.",
        "正規分布の逆関数の値を返します。",
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
        "NPER",
        "Returns the number of payment periods.",
        "支払回数を返します。",
    ),
    (
        "NPV",
        "Returns the net present value of an investment.",
        "投資の正味現在価値を返します。",
    ),
    (
        "NUMBERVALUE",
        "Converts text to a number, whatever the locale.",
        "ロケールに依存せずに文字列を数値に変換します。",
    ),
    (
        "ODD",
        "Rounds a number up to the nearest odd integer.",
        "数値を最も近い奇数に切り上げます。",
    ),
    (
        "OFFSET",
        "Returns a reference offset from a given reference.",
        "基準から指定した行数と列数だけ離れた参照を返します。",
    ),
    (
        "OR",
        "Checks whether any argument is TRUE.",
        "いずれかの引数が TRUE かどうかを調べます。",
    ),
    (
        "PERCENTILE",
        "Returns the k-th percentile of values.",
        "値の k 番目の百分位数を返します。",
    ),
    (
        "PERCENTILE.INC",
        "Returns the k-th percentile, k from 0 to 1 inclusive.",
        "値の k 番目の百分位数を返します (k は 0 以上 1 以下)。",
    ),
    (
        "PERMUT",
        "Returns the number of permutations.",
        "順列の数を返します。",
    ),
    (
        "PHONETIC",
        "Returns the furigana of a cell's text.",
        "文字列からふりがなを取り出します。",
    ),
    (
        "PI",
        "Returns the value of pi.",
        "円周率 π の値を返します。",
    ),
    (
        "PMT",
        "Returns the periodic payment for a loan.",
        "ローンの定期支払額を返します。",
    ),
    (
        "POWER",
        "Returns a number raised to a power.",
        "数値のべき乗を返します。",
    ),
    (
        "PPMT",
        "Returns the principal part of a loan payment.",
        "ローン返済額のうち元金部分を返します。",
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
        "QUARTILE",
        "Returns a quartile of a data set.",
        "データの四分位数を返します。",
    ),
    (
        "QUARTILE.INC",
        "Returns a quartile, 0 to 4 inclusive.",
        "データの四分位数を返します (0 以上 4 以下)。",
    ),
    (
        "QUOTIENT",
        "Returns the integer part of a division.",
        "割り算の商の整数部を返します。",
    ),
    (
        "RADIANS",
        "Converts degrees to radians.",
        "度をラジアンに変換します。",
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
        "RANK.AVG",
        "Returns the rank of a number, ties sharing the average rank.",
        "数値の順位を返します (同じ値には平均の順位)。",
    ),
    (
        "RANK.EQ",
        "Returns the rank of a number, ties sharing the top rank.",
        "数値の順位を返します (同じ値には最上位の順位)。",
    ),
    (
        "RATE",
        "Returns the interest rate per period.",
        "期間あたりの利率を返します。",
    ),
    (
        "REDUCE",
        "Reduces an array to one value with a LAMBDA.",
        "LAMBDA で配列を 1 つの値に集約します。",
    ),
    (
        "REPLACE",
        "Replaces part of a text string by position.",
        "文字列の指定した位置の文字を置き換えます。",
    ),
    (
        "REPT",
        "Repeats text a number of times.",
        "文字列を指定した回数だけ繰り返します。",
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
        "ROW",
        "Returns the row number of a reference.",
        "セル参照の行番号を返します。",
    ),
    (
        "ROWS",
        "Returns the number of rows in a reference.",
        "範囲の行数を返します。",
    ),
    (
        "RSQ",
        "Returns the square of the correlation coefficient.",
        "相関係数の 2 乗を返します。",
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
        "SIGN",
        "Returns the sign of a number: 1, 0 or -1.",
        "数値の符号 (1、0、-1) を返します。",
    ),
    (
        "SIN",
        "Returns the sine of an angle.",
        "角度のサインを返します。",
    ),
    (
        "SLN",
        "Returns straight-line depreciation for one period.",
        "定額法による 1 期間の減価償却費を返します。",
    ),
    (
        "SLOPE",
        "Returns the slope of the linear regression line.",
        "回帰直線の傾きを返します。",
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
        "SORTBY",
        "Sorts a range by the values in other ranges.",
        "別の範囲の値に基づいて範囲を並べ替えます。",
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
        "STDEV.P",
        "Returns the standard deviation of a whole population.",
        "母集団全体の標準偏差を返します。",
    ),
    (
        "STDEV.S",
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
        "SUMSQ",
        "Returns the sum of the squares of the arguments.",
        "引数の 2 乗の和を返します。",
    ),
    (
        "SWITCH",
        "Compares a value with a list and returns the matching result.",
        "値を一覧と比較し、一致した値に対応する結果を返します。",
    ),
    (
        "T",
        "Returns text if the value is text, else an empty string.",
        "値が文字列ならその文字列を、そうでなければ空の文字列を返します。",
    ),
    (
        "TAKE",
        "Returns rows or columns from the start or end of an array.",
        "配列の先頭または末尾から指定した行や列を返します。",
    ),
    (
        "TAN",
        "Returns the tangent of an angle.",
        "角度のタンジェントを返します。",
    ),
    (
        "TEXT",
        "Formats a number as text with a number format.",
        "数値を表示形式に従った文字列に変換します。",
    ),
    (
        "TEXTAFTER",
        "Returns the text after a delimiter.",
        "区切り文字より後の文字列を返します。",
    ),
    (
        "TEXTBEFORE",
        "Returns the text before a delimiter.",
        "区切り文字より前の文字列を返します。",
    ),
    (
        "TEXTJOIN",
        "Joins text with a delimiter between the items.",
        "区切り文字を挟んで文字列を結合します。",
    ),
    (
        "TEXTSPLIT",
        "Splits text into rows or columns at delimiters.",
        "区切り文字で文字列を行や列に分割します。",
    ),
    (
        "TIME",
        "Returns the serial number of a time from hour, minute and second.",
        "時、分、秒から時刻のシリアル値を返します。",
    ),
    (
        "TIMEVALUE",
        "Converts a time written as text into a serial number.",
        "時刻を表す文字列をシリアル値に変換します。",
    ),
    (
        "TOCOL",
        "Returns an array as one column.",
        "配列を 1 列にして返します。",
    ),
    ("TODAY", "Returns today's date.", "現在の日付を返します。"),
    (
        "TOROW",
        "Returns an array as one row.",
        "配列を 1 行にして返します。",
    ),
    (
        "TRANSPOSE",
        "Turns rows into columns and columns into rows.",
        "配列の行と列を入れ替えます。",
    ),
    (
        "TREND",
        "Returns values along a linear trend.",
        "線形トレンドに沿った値を返します。",
    ),
    (
        "TRIM",
        "Removes extra spaces from text.",
        "文字列から余分なスペースを削除します。",
    ),
    (
        "TRUNC",
        "Truncates a number to an integer or a number of digits.",
        "数値の小数部を切り捨てて整数または指定した桁数にします。",
    ),
    (
        "TYPE",
        "Returns a number telling the type of a value.",
        "値の種類を表す数値を返します。",
    ),
    (
        "UNICHAR",
        "Returns the Unicode character for a number.",
        "数値に対応する Unicode 文字を返します。",
    ),
    (
        "UNICODE",
        "Returns the Unicode number of the first character.",
        "先頭文字の Unicode 番号を返します。",
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
        "VAR",
        "Estimates variance from a sample.",
        "標本に基づいて分散を推定します。",
    ),
    (
        "VAR.P",
        "Returns the variance of a whole population.",
        "母集団全体の分散を返します。",
    ),
    (
        "VAR.S",
        "Estimates variance from a sample.",
        "標本に基づいて分散を推定します。",
    ),
    (
        "VLOOKUP",
        "Looks up a value in the first column and returns a value beside it.",
        "範囲の先頭列で値を検索し、その行の値を返します。",
    ),
    (
        "VSTACK",
        "Stacks arrays vertically.",
        "配列を縦に連結します。",
    ),
    (
        "WEEKDAY",
        "Returns the day of the week as a number.",
        "日付の曜日を数値で返します。",
    ),
    (
        "WEEKNUM",
        "Returns the week number of a date in the year.",
        "日付がその年の何週目かを返します。",
    ),
    (
        "WORKDAY",
        "Returns the date a number of working days away.",
        "指定した稼働日数だけ離れた日付を返します。",
    ),
    (
        "WORKDAY.INTL",
        "Returns the date a number of working days away, with chosen weekends.",
        "週末を指定して稼働日数だけ離れた日付を返します。",
    ),
    (
        "XIRR",
        "Returns the internal rate of return of irregular cash flows.",
        "不定期なキャッシュフローの内部利益率を返します。",
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
        "XNPV",
        "Returns the net present value of irregular cash flows.",
        "不定期なキャッシュフローの正味現在価値を返します。",
    ),
    (
        "XOR",
        "Checks whether an odd number of arguments are TRUE.",
        "TRUE の引数が奇数個かどうかを調べます。",
    ),
    (
        "YEAR",
        "Returns the year of a date.",
        "日付の年を返します。",
    ),
    (
        "YEARFRAC",
        "Returns the fraction of a year between two dates.",
        "2 つの日付の間の期間が 1 年に占める割合を返します。",
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
