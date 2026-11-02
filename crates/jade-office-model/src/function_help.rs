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
        "ACCRINT",
        "Returns the accrued interest of a security paying periodic interest.",
        "定期的に利息が支払われる証券の未収利息を返します。",
    ),
    (
        "ACCRINTM",
        "Returns the accrued interest of a security paying at maturity.",
        "満期日に利息が支払われる証券の未収利息を返します。",
    ),
    (
        "ACOS",
        "Returns the arccosine of a number, in radians.",
        "数値のアークコサインをラジアンで返します。",
    ),
    (
        "ACOSH",
        "Returns the inverse hyperbolic cosine.",
        "双曲線逆余弦を返します。",
    ),
    (
        "ACOT",
        "Returns the arccotangent of a number.",
        "数値のアークコタンジェントを返します。",
    ),
    (
        "ACOTH",
        "Returns the inverse hyperbolic cotangent.",
        "双曲線逆余接を返します。",
    ),
    (
        "ADDRESS",
        "Returns a cell reference as text.",
        "セル参照を文字列で返します。",
    ),
    (
        "AGGREGATE",
        "Returns an aggregate, optionally ignoring hidden rows and errors.",
        "非表示の行やエラーを無視するかどうかを指定して集計値を返します。",
    ),
    (
        "AMORDEGRC",
        "Returns French accounting depreciation with a coefficient.",
        "フランスの会計方式による減価償却費を係数付きで返します。",
    ),
    (
        "AMORLINC",
        "Returns French accounting depreciation for a period.",
        "フランスの会計方式による期間の減価償却費を返します。",
    ),
    (
        "ANCHORARRAY",
        "Returns the whole spilled range of a dynamic array.",
        "動的配列のスピル範囲全体を返します。",
    ),
    (
        "AND",
        "Checks whether all arguments are TRUE.",
        "すべての引数が TRUE かどうかを調べます。",
    ),
    (
        "ARABIC",
        "Converts a Roman numeral to a number.",
        "ローマ数字を数値に変換します。",
    ),
    (
        "AREAS",
        "Returns the number of areas in a reference.",
        "参照に含まれる領域の数を返します。",
    ),
    (
        "ARRAYTOTEXT",
        "Returns an array's values as text.",
        "配列の値を文字列で返します。",
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
        "ASINH",
        "Returns the inverse hyperbolic sine.",
        "双曲線逆正弦を返します。",
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
        "ATANH",
        "Returns the inverse hyperbolic tangent.",
        "双曲線逆正接を返します。",
    ),
    (
        "AVEDEV",
        "Returns the average absolute deviation from the mean.",
        "平均値からの絶対偏差の平均を返します。",
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
        "BAHTTEXT",
        "Converts a number to Thai text.",
        "数値をタイ語の文字列に変換します。",
    ),
    (
        "BASE",
        "Converts a number to text in another base.",
        "数値を指定した基数の文字列に変換します。",
    ),
    (
        "BESSELI",
        "Returns the modified Bessel function In(x).",
        "修正ベッセル関数 In(x) を返します。",
    ),
    (
        "BESSELJ",
        "Returns the Bessel function Jn(x).",
        "ベッセル関数 Jn(x) を返します。",
    ),
    (
        "BESSELK",
        "Returns the modified Bessel function Kn(x).",
        "修正ベッセル関数 Kn(x) を返します。",
    ),
    (
        "BESSELY",
        "Returns the Bessel function Yn(x).",
        "ベッセル関数 Yn(x) を返します。",
    ),
    (
        "BETA.DIST",
        "Returns the beta distribution.",
        "ベータ分布の値を返します。",
    ),
    (
        "BETA.INV",
        "Returns the inverse of the beta distribution.",
        "ベータ分布の逆関数の値を返します。",
    ),
    (
        "BETADIST",
        "Returns the cumulative beta distribution.",
        "累積ベータ分布の値を返します。",
    ),
    (
        "BETAINV",
        "Returns the inverse of the cumulative beta distribution.",
        "累積ベータ分布の逆関数の値を返します。",
    ),
    (
        "BIN2DEC",
        "Converts a binary number to decimal.",
        "2 進数を 10 進数に変換します。",
    ),
    (
        "BIN2HEX",
        "Converts a binary number to hexadecimal.",
        "2 進数を 16 進数に変換します。",
    ),
    (
        "BIN2OCT",
        "Converts a binary number to octal.",
        "2 進数を 8 進数に変換します。",
    ),
    (
        "BINOM.DIST",
        "Returns the binomial distribution probability.",
        "二項分布の確率を返します。",
    ),
    (
        "BINOM.DIST.RANGE",
        "Returns the probability of a range of successes.",
        "試行結果の成功数が範囲内に収まる確率を返します。",
    ),
    (
        "BINOM.INV",
        "Returns the smallest value whose cumulative binomial reaches a criterion.",
        "累積二項確率が基準値以上になる最小の値を返します。",
    ),
    (
        "BINOMDIST",
        "Returns the binomial distribution probability.",
        "二項分布の確率を返します。",
    ),
    (
        "BITAND",
        "Returns the bitwise AND of two numbers.",
        "2 つの数値のビット単位の AND を返します。",
    ),
    (
        "BITLSHIFT",
        "Shifts a number's bits left.",
        "数値をビット単位で左にシフトします。",
    ),
    (
        "BITOR",
        "Returns the bitwise OR of two numbers.",
        "2 つの数値のビット単位の OR を返します。",
    ),
    (
        "BITRSHIFT",
        "Shifts a number's bits right.",
        "数値をビット単位で右にシフトします。",
    ),
    (
        "BITXOR",
        "Returns the bitwise XOR of two numbers.",
        "2 つの数値のビット単位の XOR を返します。",
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
        "CEILING.MATH",
        "Rounds a number up to an integer or multiple.",
        "数値を整数または基準値の倍数に切り上げます。",
    ),
    (
        "CEILING.PRECISE",
        "Rounds a number up to the nearest multiple, whatever its sign.",
        "数値を符号にかかわらず基準値の倍数に切り上げます。",
    ),
    (
        "CELL",
        "Returns information about a cell's format, place or contents.",
        "セルの書式、位置、内容についての情報を返します。",
    ),
    (
        "CHAR",
        "Returns the character for a code number.",
        "数値で指定した文字を返します。",
    ),
    (
        "CHIDIST",
        "Returns the right-tailed chi-squared probability.",
        "カイ二乗分布の右側確率を返します。",
    ),
    (
        "CHIINV",
        "Returns the inverse of the right-tailed chi-squared distribution.",
        "カイ二乗分布の右側確率の逆関数の値を返します。",
    ),
    (
        "CHISQ.DIST",
        "Returns the chi-squared distribution.",
        "カイ二乗分布の値を返します。",
    ),
    (
        "CHISQ.DIST.RT",
        "Returns the right-tailed chi-squared probability.",
        "カイ二乗分布の右側確率を返します。",
    ),
    (
        "CHISQ.INV",
        "Returns the inverse of the left-tailed chi-squared distribution.",
        "カイ二乗分布の左側確率の逆関数の値を返します。",
    ),
    (
        "CHISQ.INV.RT",
        "Returns the inverse of the right-tailed chi-squared distribution.",
        "カイ二乗分布の右側確率の逆関数の値を返します。",
    ),
    (
        "CHISQ.TEST",
        "Returns the chi-squared test of independence.",
        "カイ二乗検定の確率を返します。",
    ),
    (
        "CHITEST",
        "Returns the chi-squared test of independence.",
        "カイ二乗検定の確率を返します。",
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
        "COMBINA",
        "Returns the number of combinations with repetitions.",
        "重複組み合わせの数を返します。",
    ),
    (
        "COMPLEX",
        "Builds a complex number from real and imaginary parts.",
        "実部と虚部から複素数を作成します。",
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
        "CONFIDENCE",
        "Returns the confidence interval for a mean (normal).",
        "正規分布を使って母集団の平均の信頼区間を返します。",
    ),
    (
        "CONFIDENCE.NORM",
        "Returns the confidence interval for a mean.",
        "母集団の平均の信頼区間を返します。",
    ),
    (
        "CONFIDENCE.T",
        "Returns the confidence interval for a mean (Student's t).",
        "t 分布を使って母集団の平均の信頼区間を返します。",
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
        "COSH",
        "Returns the hyperbolic cosine.",
        "双曲線余弦を返します。",
    ),
    (
        "COT",
        "Returns the cotangent of an angle.",
        "角度のコタンジェントを返します。",
    ),
    (
        "COTH",
        "Returns the hyperbolic cotangent.",
        "双曲線余接を返します。",
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
        "COUPDAYBS",
        "Returns the days from the coupon period's start to settlement.",
        "利払期間の開始日から受渡日までの日数を返します。",
    ),
    (
        "COUPDAYS",
        "Returns the days in the coupon period holding settlement.",
        "受渡日を含む利払期間の日数を返します。",
    ),
    (
        "COUPDAYSNC",
        "Returns the days from settlement to the next coupon date.",
        "受渡日から次の利払日までの日数を返します。",
    ),
    (
        "COUPNCD",
        "Returns the next coupon date after settlement.",
        "受渡日後の次の利払日を返します。",
    ),
    (
        "COUPNUM",
        "Returns the number of coupons between settlement and maturity.",
        "受渡日と満期日の間の利払回数を返します。",
    ),
    (
        "COUPPCD",
        "Returns the coupon date before settlement.",
        "受渡日前の直近の利払日を返します。",
    ),
    (
        "COVAR",
        "Returns the population covariance.",
        "母集団の共分散を返します。",
    ),
    (
        "COVARIANCE.P",
        "Returns the population covariance.",
        "母集団の共分散を返します。",
    ),
    (
        "COVARIANCE.S",
        "Returns the sample covariance.",
        "標本の共分散を返します。",
    ),
    (
        "CRITBINOM",
        "Returns the smallest value whose cumulative binomial reaches a criterion.",
        "累積二項確率が基準値以上になる最小の値を返します。",
    ),
    (
        "CSC",
        "Returns the cosecant of an angle.",
        "角度のコセカントを返します。",
    ),
    (
        "CSCH",
        "Returns the hyperbolic cosecant.",
        "双曲線余割を返します。",
    ),
    (
        "CUMIPMT",
        "Returns the interest paid between two periods.",
        "指定した期間に支払う利息の累計を返します。",
    ),
    (
        "CUMPRINC",
        "Returns the principal paid between two periods.",
        "指定した期間に支払う元金の累計を返します。",
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
        "DATESTRING",
        "Returns a date as Japanese era text.",
        "日付を和暦の文字列で返します。",
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
        "DBCS",
        "Converts half-width characters to full-width.",
        "半角文字を全角文字に変換します。",
    ),
    (
        "DCOUNT",
        "Counts a database column's numbers that meet criteria.",
        "データベースの条件に一致するレコードの数値の個数を返します。",
    ),
    (
        "DCOUNTA",
        "Counts a database column's non-empty cells that meet criteria.",
        "データベースの条件に一致するレコードの空白でないセルの個数を返します。",
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
        "DEC2OCT",
        "Converts a decimal number to octal.",
        "10 進数を 8 進数に変換します。",
    ),
    (
        "DECIMAL",
        "Converts text in a base to a decimal number.",
        "指定した基数の文字列を 10 進数に変換します。",
    ),
    (
        "DEGREES",
        "Converts radians to degrees.",
        "ラジアンを度に変換します。",
    ),
    (
        "DELTA",
        "Checks whether two numbers are equal (1 or 0).",
        "2 つの数値が等しいかどうかを調べます (1 または 0)。",
    ),
    (
        "DEVSQ",
        "Returns the sum of squared deviations from the mean.",
        "平均値からの偏差の 2 乗和を返します。",
    ),
    (
        "DGET",
        "Returns the one database value that meets criteria.",
        "データベースの条件に一致する 1 つの値を返します。",
    ),
    (
        "DISC",
        "Returns the discount rate of a security.",
        "証券の割引率を返します。",
    ),
    (
        "DMAX",
        "Returns the largest database value that meets criteria.",
        "データベースの条件に一致するレコードの最大値を返します。",
    ),
    (
        "DMIN",
        "Returns the smallest database value that meets criteria.",
        "データベースの条件に一致するレコードの最小値を返します。",
    ),
    (
        "DOLLAR",
        "Formats a number as currency text.",
        "数値を通貨形式の文字列に変換します。",
    ),
    (
        "DOLLARDE",
        "Converts a fractional price to a decimal one.",
        "分数で表された価格を小数に変換します。",
    ),
    (
        "DOLLARFR",
        "Converts a decimal price to a fractional one.",
        "小数で表された価格を分数に変換します。",
    ),
    (
        "DPRODUCT",
        "Multiplies the database values that meet criteria.",
        "データベースの条件に一致するレコードの値の積を返します。",
    ),
    (
        "DROP",
        "Removes rows or columns from the start or end of an array.",
        "配列の先頭または末尾から指定した行や列を除きます。",
    ),
    (
        "DSTDEV",
        "Estimates standard deviation from database values that meet criteria.",
        "データベースの条件に一致するレコードから標準偏差を推定します。",
    ),
    (
        "DSTDEVP",
        "Returns the standard deviation of database values that meet criteria.",
        "データベースの条件に一致するレコードの標準偏差を返します。",
    ),
    (
        "DSUM",
        "Adds the values in a database column that meet criteria.",
        "データベースの条件に一致するレコードの値を合計します。",
    ),
    (
        "DURATION",
        "Returns the Macaulay duration of a security.",
        "証券のマコーレー デュレーションを返します。",
    ),
    (
        "DVAR",
        "Estimates variance from database values that meet criteria.",
        "データベースの条件に一致するレコードから分散を推定します。",
    ),
    (
        "DVARP",
        "Returns the variance of database values that meet criteria.",
        "データベースの条件に一致するレコードの分散を返します。",
    ),
    (
        "ECMA.CEILING",
        "Rounds a number up to a multiple, as ECMA-376 defines.",
        "ECMA-376 の定義に従って数値を倍数に切り上げます。",
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
        "ENCODEURL",
        "Returns text encoded for a URL.",
        "文字列を URL 形式にエンコードします。",
    ),
    (
        "EOMONTH",
        "Returns the last day of the month a number of months away.",
        "指定した月数だけ離れた月の末日を返します。",
    ),
    (
        "ERF",
        "Returns the error function.",
        "誤差関数の値を返します。",
    ),
    (
        "ERF.PRECISE",
        "Returns the error function.",
        "誤差関数の値を返します。",
    ),
    (
        "ERFC",
        "Returns the complementary error function.",
        "相補誤差関数の値を返します。",
    ),
    (
        "ERFC.PRECISE",
        "Returns the complementary error function.",
        "相補誤差関数の値を返します。",
    ),
    (
        "ERROR.TYPE",
        "Returns a number for an error value.",
        "エラー値に対応する数値を返します。",
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
        "EXPAND",
        "Expands an array to more rows and columns.",
        "配列を指定した行数と列数に拡張します。",
    ),
    (
        "EXPON.DIST",
        "Returns the exponential distribution.",
        "指数分布の値を返します。",
    ),
    (
        "EXPONDIST",
        "Returns the exponential distribution.",
        "指数分布の値を返します。",
    ),
    (
        "F.DIST",
        "Returns the F distribution.",
        "F 分布の値を返します。",
    ),
    (
        "F.DIST.RT",
        "Returns the right-tailed F distribution.",
        "F 分布の右側確率を返します。",
    ),
    (
        "F.INV",
        "Returns the inverse of the F distribution.",
        "F 分布の逆関数の値を返します。",
    ),
    (
        "F.INV.RT",
        "Returns the inverse of the right-tailed F distribution.",
        "F 分布の右側確率の逆関数の値を返します。",
    ),
    (
        "F.TEST",
        "Returns the result of an F-test.",
        "F 検定の結果を返します。",
    ),
    (
        "FACT",
        "Returns the factorial of a number.",
        "数値の階乗を返します。",
    ),
    (
        "FACTDOUBLE",
        "Returns the double factorial of a number.",
        "数値の二重階乗を返します。",
    ),
    (
        "FALSE",
        "Returns the logical value FALSE.",
        "論理値 FALSE を返します。",
    ),
    (
        "FDIST",
        "Returns the right-tailed F distribution.",
        "F 分布の右側確率を返します。",
    ),
    (
        "FILTER",
        "Filters a range by the conditions you give.",
        "条件に基づいて範囲をフィルターします。",
    ),
    (
        "FILTERXML",
        "Returns the data an XPath selects from XML.",
        "XPath を使って XML から特定のデータを取り出します。",
    ),
    (
        "FIND",
        "Finds one text string within another (case-sensitive).",
        "文字列の中から別の文字列を検索します (大文字と小文字を区別)。",
    ),
    (
        "FINDB",
        "Finds one text within another, counting bytes.",
        "文字列の中から別の文字列を検索し、バイト単位の位置を返します。",
    ),
    (
        "FINV",
        "Returns the inverse of the right-tailed F distribution.",
        "F 分布の右側確率の逆関数の値を返します。",
    ),
    (
        "FISHER",
        "Returns the Fisher transformation.",
        "フィッシャー変換の値を返します。",
    ),
    (
        "FISHERINV",
        "Returns the inverse of the Fisher transformation.",
        "フィッシャー変換の逆関数の値を返します。",
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
        "FLOOR.MATH",
        "Rounds a number down to an integer or multiple.",
        "数値を整数または基準値の倍数に切り捨てます。",
    ),
    (
        "FLOOR.PRECISE",
        "Rounds a number down to the nearest multiple, whatever its sign.",
        "数値を符号にかかわらず基準値の倍数に切り捨てます。",
    ),
    (
        "FORECAST",
        "Predicts a value along a linear trend.",
        "線形トレンドに沿って値を予測します。",
    ),
    (
        "FORECAST.ETS",
        "Forecasts a value by exponential smoothing.",
        "指数平滑法で将来の値を予測します。",
    ),
    (
        "FORECAST.ETS.CONFINT",
        "Returns the confidence interval of an exponential-smoothing forecast.",
        "指数平滑法による予測値の信頼区間を返します。",
    ),
    (
        "FORECAST.ETS.SEASONALITY",
        "Returns the seasonal pattern length exponential smoothing finds.",
        "指数平滑法で検出した季節パターンの長さを返します。",
    ),
    (
        "FORECAST.ETS.STAT",
        "Returns a statistic of an exponential-smoothing forecast.",
        "指数平滑法による予測の統計値を返します。",
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
        "FREQUENCY",
        "Counts how many values fall in each range.",
        "値が各区間に含まれる個数を返します。",
    ),
    (
        "FTEST",
        "Returns the result of an F-test.",
        "F 検定の結果を返します。",
    ),
    (
        "FV",
        "Returns the future value of an investment.",
        "投資の将来価値を返します。",
    ),
    (
        "FVSCHEDULE",
        "Returns the future value after a series of interest rates.",
        "一連の利率を適用した元金の将来価値を返します。",
    ),
    (
        "GAMMA",
        "Returns the gamma function value.",
        "ガンマ関数の値を返します。",
    ),
    (
        "GAMMA.DIST",
        "Returns the gamma distribution.",
        "ガンマ分布の値を返します。",
    ),
    (
        "GAMMA.INV",
        "Returns the inverse of the gamma distribution.",
        "ガンマ分布の逆関数の値を返します。",
    ),
    (
        "GAMMADIST",
        "Returns the gamma distribution.",
        "ガンマ分布の値を返します。",
    ),
    (
        "GAMMAINV",
        "Returns the inverse of the gamma distribution.",
        "ガンマ分布の逆関数の値を返します。",
    ),
    (
        "GAMMALN",
        "Returns the natural logarithm of the gamma function.",
        "ガンマ関数の自然対数を返します。",
    ),
    (
        "GAMMALN.PRECISE",
        "Returns the natural logarithm of the gamma function.",
        "ガンマ関数の自然対数を返します。",
    ),
    (
        "GAUSS",
        "Returns 0.5 less than the standard normal cumulative distribution.",
        "標準正規分布の累積値から 0.5 を引いた値を返します。",
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
        "GESTEP",
        "Checks whether a number is at least a threshold (1 or 0).",
        "数値がしきい値以上かどうかを調べます (1 または 0)。",
    ),
    (
        "GETPIVOTDATA",
        "Returns data stored in a pivot table.",
        "ピボットテーブルのデータを返します。",
    ),
    (
        "GROUPBY",
        "Groups values by row fields and aggregates each group.",
        "行フィールドで値をグループ化し、グループごとに集計します。",
    ),
    (
        "GROWTH",
        "Returns values along an exponential trend.",
        "指数トレンドに沿った値を返します。",
    ),
    (
        "HARMEAN",
        "Returns the harmonic mean.",
        "調和平均を返します。",
    ),
    (
        "HEX2BIN",
        "Converts a hexadecimal number to binary.",
        "16 進数を 2 進数に変換します。",
    ),
    (
        "HEX2DEC",
        "Converts a hexadecimal number to decimal.",
        "16 進数を 10 進数に変換します。",
    ),
    (
        "HEX2OCT",
        "Converts a hexadecimal number to octal.",
        "16 進数を 8 進数に変換します。",
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
        "HYPGEOM.DIST",
        "Returns the hypergeometric distribution.",
        "超幾何分布の値を返します。",
    ),
    (
        "HYPGEOMDIST",
        "Returns the hypergeometric distribution.",
        "超幾何分布の値を返します。",
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
        "IMABS",
        "Returns the absolute value of a complex number.",
        "複素数の絶対値を返します。",
    ),
    (
        "IMAGINARY",
        "Returns the imaginary part of a complex number.",
        "複素数の虚部を返します。",
    ),
    (
        "IMARGUMENT",
        "Returns the argument of a complex number, in radians.",
        "複素数の偏角をラジアンで返します。",
    ),
    (
        "IMCONJUGATE",
        "Returns the complex conjugate.",
        "複素共役を返します。",
    ),
    (
        "IMCOS",
        "Returns the cosine of a complex number.",
        "複素数のコサインを返します。",
    ),
    (
        "IMCOSH",
        "Returns the hyperbolic cosine of a complex number.",
        "複素数の双曲線余弦を返します。",
    ),
    (
        "IMCOT",
        "Returns the cotangent of a complex number.",
        "複素数のコタンジェントを返します。",
    ),
    (
        "IMCSC",
        "Returns the cosecant of a complex number.",
        "複素数のコセカントを返します。",
    ),
    (
        "IMCSCH",
        "Returns the hyperbolic cosecant of a complex number.",
        "複素数の双曲線余割を返します。",
    ),
    (
        "IMDIV",
        "Returns the quotient of two complex numbers.",
        "2 つの複素数の商を返します。",
    ),
    (
        "IMEXP",
        "Returns the exponential of a complex number.",
        "複素数の指数関数の値を返します。",
    ),
    (
        "IMLN",
        "Returns the natural logarithm of a complex number.",
        "複素数の自然対数を返します。",
    ),
    (
        "IMLOG10",
        "Returns the base-10 logarithm of a complex number.",
        "複素数の常用対数を返します。",
    ),
    (
        "IMLOG2",
        "Returns the base-2 logarithm of a complex number.",
        "複素数の 2 を底とする対数を返します。",
    ),
    (
        "IMPOWER",
        "Returns a complex number raised to a power.",
        "複素数のべき乗を返します。",
    ),
    (
        "IMPRODUCT",
        "Returns the product of complex numbers.",
        "複素数の積を返します。",
    ),
    (
        "IMREAL",
        "Returns the real part of a complex number.",
        "複素数の実部を返します。",
    ),
    (
        "IMSEC",
        "Returns the secant of a complex number.",
        "複素数のセカントを返します。",
    ),
    (
        "IMSECH",
        "Returns the hyperbolic secant of a complex number.",
        "複素数の双曲線正割を返します。",
    ),
    (
        "IMSIN",
        "Returns the sine of a complex number.",
        "複素数のサインを返します。",
    ),
    (
        "IMSINH",
        "Returns the hyperbolic sine of a complex number.",
        "複素数の双曲線正弦を返します。",
    ),
    (
        "IMSQRT",
        "Returns the square root of a complex number.",
        "複素数の平方根を返します。",
    ),
    (
        "IMSUB",
        "Returns the difference of two complex numbers.",
        "2 つの複素数の差を返します。",
    ),
    (
        "IMSUM",
        "Returns the sum of complex numbers.",
        "複素数の和を返します。",
    ),
    (
        "IMTAN",
        "Returns the tangent of a complex number.",
        "複素数のタンジェントを返します。",
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
        "INFO",
        "Returns information about the working environment.",
        "現在の操作環境についての情報を返します。",
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
        "INTRATE",
        "Returns the interest rate of a fully invested security.",
        "全額投資された証券の利率を返します。",
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
        "ISNONTEXT",
        "Checks whether a value is not text.",
        "値が文字列以外かどうかを調べます。",
    ),
    (
        "ISNUMBER",
        "Checks whether a value is a number.",
        "値が数値かどうかを調べます。",
    ),
    (
        "ISO.CEILING",
        "Rounds a number up to the nearest multiple, whatever its sign.",
        "数値を符号にかかわらず基準値の倍数に切り上げます。",
    ),
    (
        "ISODD",
        "Checks whether a number is odd.",
        "数値が奇数かどうかを調べます。",
    ),
    (
        "ISOMITTED",
        "Checks whether a LAMBDA argument was left out.",
        "LAMBDA の引数が省略されたかどうかを調べます。",
    ),
    (
        "ISOWEEKNUM",
        "Returns the ISO week number of a date.",
        "日付の ISO 週番号を返します。",
    ),
    (
        "ISPMT",
        "Returns the interest paid in a period of an even-principal loan.",
        "元金均等返済のローンで指定期間に支払う利息を返します。",
    ),
    (
        "ISREF",
        "Checks whether a value is a reference.",
        "値がセル参照かどうかを調べます。",
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
        "KURT",
        "Returns the kurtosis of a data set.",
        "データの尖度を返します。",
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
        "LEFTB",
        "Returns bytes from the start of a text string.",
        "文字列の先頭から指定したバイト数の文字を返します。",
    ),
    (
        "LEN",
        "Returns the number of characters in a text string.",
        "文字列の文字数を返します。",
    ),
    (
        "LENB",
        "Returns the number of bytes in a text string.",
        "文字列のバイト数を返します。",
    ),
    (
        "LET",
        "Names the results of calculations to use in a formula.",
        "計算結果に名前を付けて、数式の中で使えるようにします。",
    ),
    (
        "LINEST",
        "Returns the statistics of a linear trend.",
        "回帰直線の係数などの統計値を返します。",
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
        "LOGEST",
        "Returns the statistics of an exponential trend.",
        "指数回帰曲線の係数などの統計値を返します。",
    ),
    (
        "LOGINV",
        "Returns the inverse of the lognormal distribution.",
        "対数正規分布の逆関数の値を返します。",
    ),
    (
        "LOGNORM.DIST",
        "Returns the lognormal distribution.",
        "対数正規分布の値を返します。",
    ),
    (
        "LOGNORM.INV",
        "Returns the inverse of the lognormal distribution.",
        "対数正規分布の逆関数の値を返します。",
    ),
    (
        "LOGNORMDIST",
        "Returns the cumulative lognormal distribution.",
        "累積対数正規分布の値を返します。",
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
        "MAKEARRAY",
        "Builds an array by applying a LAMBDA to each position.",
        "各位置に LAMBDA を適用して配列を作成します。",
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
        "MAXA",
        "Returns the largest value, counting text and logical values.",
        "文字列や論理値も含めて最大値を返します。",
    ),
    (
        "MAXIFS",
        "Returns the largest value among the cells that meet conditions.",
        "条件に一致するセルの中の最大値を返します。",
    ),
    (
        "MDETERM",
        "Returns the determinant of a matrix.",
        "行列の行列式を返します。",
    ),
    (
        "MDURATION",
        "Returns the modified duration of a security.",
        "証券の修正デュレーションを返します。",
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
    (
        "MIDB",
        "Returns bytes from the middle of a text string.",
        "文字列の指定した位置から指定したバイト数の文字を返します。",
    ),
    ("MIN", "Returns the smallest value.", "最小値を返します。"),
    (
        "MINA",
        "Returns the smallest value, counting text and logical values.",
        "文字列や論理値も含めて最小値を返します。",
    ),
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
        "MINVERSE",
        "Returns the inverse of a matrix.",
        "行列の逆行列を返します。",
    ),
    (
        "MIRR",
        "Returns the modified internal rate of return.",
        "修正内部利益率を返します。",
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
        "MODE.MULT",
        "Returns every most frequent value.",
        "最も頻繁に出現する値をすべて返します。",
    ),
    (
        "MODE.SNGL",
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
        "MULTINOMIAL",
        "Returns the multinomial of the numbers.",
        "数値の多項係数を返します。",
    ),
    (
        "MUNIT",
        "Returns the unit matrix of a size.",
        "指定した次元の単位行列を返します。",
    ),
    (
        "N",
        "Returns a value converted to a number.",
        "値を数値に変換して返します。",
    ),
    (
        "NA",
        "Returns the #N/A error.",
        "エラー値 #N/A を返します。",
    ),
    (
        "NEGBINOM.DIST",
        "Returns the negative binomial distribution.",
        "負の二項分布の値を返します。",
    ),
    (
        "NEGBINOMDIST",
        "Returns the negative binomial distribution.",
        "負の二項分布の値を返します。",
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
        "NOMINAL",
        "Returns the nominal annual interest rate.",
        "名目年利率を返します。",
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
        "NORM.S.DIST",
        "Returns the standard normal distribution.",
        "標準正規分布の値を返します。",
    ),
    (
        "NORM.S.INV",
        "Returns the inverse of the standard normal distribution.",
        "標準正規分布の逆関数の値を返します。",
    ),
    (
        "NORMDIST",
        "Returns the normal distribution.",
        "正規分布の値を返します。",
    ),
    (
        "NORMINV",
        "Returns the inverse of the normal distribution.",
        "正規分布の逆関数の値を返します。",
    ),
    (
        "NORMSDIST",
        "Returns the standard normal cumulative distribution.",
        "標準正規分布の累積値を返します。",
    ),
    (
        "NORMSINV",
        "Returns the inverse of the standard normal distribution.",
        "標準正規分布の逆関数の値を返します。",
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
        "NUMBERSTRING",
        "Returns a number as Japanese numerals.",
        "数値を漢数字の文字列で返します。",
    ),
    (
        "NUMBERVALUE",
        "Converts text to a number, whatever the locale.",
        "ロケールに依存せずに文字列を数値に変換します。",
    ),
    (
        "OCT2BIN",
        "Converts an octal number to binary.",
        "8 進数を 2 進数に変換します。",
    ),
    (
        "OCT2DEC",
        "Converts an octal number to decimal.",
        "8 進数を 10 進数に変換します。",
    ),
    (
        "OCT2HEX",
        "Converts an octal number to hexadecimal.",
        "8 進数を 16 進数に変換します。",
    ),
    (
        "ODD",
        "Rounds a number up to the nearest odd integer.",
        "数値を最も近い奇数に切り上げます。",
    ),
    (
        "ODDFPRICE",
        "Returns the price of a security with an odd first period.",
        "最初の利払期間が半端な証券の価格を返します。",
    ),
    (
        "ODDFYIELD",
        "Returns the yield of a security with an odd first period.",
        "最初の利払期間が半端な証券の利回りを返します。",
    ),
    (
        "ODDLPRICE",
        "Returns the price of a security with an odd last period.",
        "最後の利払期間が半端な証券の価格を返します。",
    ),
    (
        "ODDLYIELD",
        "Returns the yield of a security with an odd last period.",
        "最後の利払期間が半端な証券の利回りを返します。",
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
        "PDURATION",
        "Returns the periods an investment needs to reach a value.",
        "投資が指定した価値に達するまでの期間を返します。",
    ),
    (
        "PEARSON",
        "Returns the Pearson correlation coefficient.",
        "ピアソンの積率相関係数を返します。",
    ),
    (
        "PERCENTILE",
        "Returns the k-th percentile of values.",
        "値の k 番目の百分位数を返します。",
    ),
    (
        "PERCENTILE.EXC",
        "Returns the k-th percentile, k between 0 and 1 exclusive.",
        "値の k 番目の百分位数を返します (k は 0 より大きく 1 より小さい)。",
    ),
    (
        "PERCENTILE.INC",
        "Returns the k-th percentile, k from 0 to 1 inclusive.",
        "値の k 番目の百分位数を返します (k は 0 以上 1 以下)。",
    ),
    (
        "PERCENTOF",
        "Returns the share a subset is of all the data.",
        "部分のデータが全体に占める割合を返します。",
    ),
    (
        "PERCENTRANK",
        "Returns the percentage rank of a value in a data set.",
        "データの中での値の百分率順位を返します。",
    ),
    (
        "PERCENTRANK.EXC",
        "Returns the percentage rank, 0 and 1 excluded.",
        "値の百分率順位を返します (0 と 1 を含まない)。",
    ),
    (
        "PERCENTRANK.INC",
        "Returns the percentage rank, 0 and 1 included.",
        "値の百分率順位を返します (0 と 1 を含む)。",
    ),
    (
        "PERMUT",
        "Returns the number of permutations.",
        "順列の数を返します。",
    ),
    (
        "PERMUTATIONA",
        "Returns the number of permutations with repetitions.",
        "重複順列の数を返します。",
    ),
    (
        "PHI",
        "Returns the standard normal density.",
        "標準正規分布の密度関数の値を返します。",
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
        "PIVOTBY",
        "Groups values by row and column fields and aggregates each pair.",
        "行と列のフィールドで値をグループ化し、組み合わせごとに集計します。",
    ),
    (
        "PMT",
        "Returns the periodic payment for a loan.",
        "ローンの定期支払額を返します。",
    ),
    (
        "POISSON",
        "Returns the Poisson distribution.",
        "ポアソン分布の値を返します。",
    ),
    (
        "POISSON.DIST",
        "Returns the Poisson distribution.",
        "ポアソン分布の値を返します。",
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
        "PRICE",
        "Returns the price per 100 face value of a security paying periodic interest.",
        "定期利付債の額面 100 あたりの価格を返します。",
    ),
    (
        "PRICEDISC",
        "Returns the price per 100 face value of a discounted security.",
        "割引債の額面 100 あたりの価格を返します。",
    ),
    (
        "PRICEMAT",
        "Returns the price per 100 face value of a security paying at maturity.",
        "満期利付債の額面 100 あたりの価格を返します。",
    ),
    (
        "PROB",
        "Returns the probability that values fall between limits.",
        "値が範囲内に収まる確率を返します。",
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
        "QUARTILE.EXC",
        "Returns a quartile, 0 and 4 excluded.",
        "データの四分位数を返します (0 と 4 を含まない)。",
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
        "RANDARRAY",
        "Returns an array of random numbers.",
        "乱数の配列を返します。",
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
        "RECEIVED",
        "Returns the amount received at maturity of a fully invested security.",
        "全額投資された証券の満期日の受取額を返します。",
    ),
    (
        "REDUCE",
        "Reduces an array to one value with a LAMBDA.",
        "LAMBDA で配列を 1 つの値に集約します。",
    ),
    (
        "REGEXEXTRACT",
        "Extracts text that matches a regular expression.",
        "正規表現に一致する文字列を取り出します。",
    ),
    (
        "REGEXREPLACE",
        "Replaces text that matches a regular expression.",
        "正規表現に一致する文字列を置き換えます。",
    ),
    (
        "REGEXTEST",
        "Checks whether text matches a regular expression.",
        "文字列が正規表現に一致するかどうかを調べます。",
    ),
    (
        "REPLACE",
        "Replaces part of a text string by position.",
        "文字列の指定した位置の文字を置き換えます。",
    ),
    (
        "REPLACEB",
        "Replaces part of a text string, counting bytes.",
        "文字列の指定したバイト位置の文字を置き換えます。",
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
        "RIGHTB",
        "Returns bytes from the end of a text string.",
        "文字列の末尾から指定したバイト数の文字を返します。",
    ),
    (
        "ROMAN",
        "Converts a number to a Roman numeral.",
        "数値をローマ数字に変換します。",
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
        "RRI",
        "Returns the equivalent interest rate for an investment's growth.",
        "投資の成長に相当する利率を返します。",
    ),
    (
        "RSQ",
        "Returns the square of the correlation coefficient.",
        "相関係数の 2 乗を返します。",
    ),
    (
        "SCAN",
        "Applies a LAMBDA to each value, returning each step.",
        "各値に LAMBDA を適用し、途中の結果を配列で返します。",
    ),
    (
        "SEARCH",
        "Finds one text string within another (not case-sensitive).",
        "文字列の中から別の文字列を検索します (大文字と小文字を区別しない)。",
    ),
    (
        "SEARCHB",
        "Finds one text within another, counting bytes, ignoring case.",
        "文字列の中から別の文字列をバイト単位で検索します (大文字と小文字を区別しない)。",
    ),
    (
        "SEC",
        "Returns the secant of an angle.",
        "角度のセカントを返します。",
    ),
    (
        "SECH",
        "Returns the hyperbolic secant.",
        "双曲線正割を返します。",
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
        "SERIESSUM",
        "Returns the sum of a power series.",
        "べき級数の和を返します。",
    ),
    (
        "SHEET",
        "Returns the sheet number of a reference.",
        "参照されているシートの番号を返します。",
    ),
    (
        "SHEETS",
        "Returns the number of sheets in a reference.",
        "参照に含まれるシートの数を返します。",
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
        "SINGLE",
        "Returns one value by implicit intersection.",
        "暗黙的な共通部分によって 1 つの値を返します。",
    ),
    (
        "SINH",
        "Returns the hyperbolic sine.",
        "双曲線正弦を返します。",
    ),
    (
        "SKEW",
        "Returns the skewness of a distribution.",
        "分布の歪度を返します。",
    ),
    (
        "SKEW.P",
        "Returns the skewness of a population.",
        "母集団の歪度を返します。",
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
        "SQRTPI",
        "Returns the square root of a number times pi.",
        "数値に π を掛けた値の平方根を返します。",
    ),
    (
        "STANDARDIZE",
        "Returns a normalised value from a distribution.",
        "正規化された値を返します。",
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
        "STDEVA",
        "Estimates standard deviation from a sample, counting text and logical values.",
        "文字列や論理値も含めて標本の標準偏差を推定します。",
    ),
    (
        "STDEVP",
        "Returns the standard deviation of a whole population.",
        "母集団全体の標準偏差を返します。",
    ),
    (
        "STDEVPA",
        "Returns the population standard deviation, counting text and logical values.",
        "文字列や論理値も含めて母集団の標準偏差を返します。",
    ),
    (
        "STEYX",
        "Returns the standard error of predicted y values.",
        "回帰の標準誤差を返します。",
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
        "SUMX2MY2",
        "Returns the sum of the differences of squares of two arrays.",
        "2 つの配列の要素の 2 乗の差を合計します。",
    ),
    (
        "SUMX2PY2",
        "Returns the sum of the sums of squares of two arrays.",
        "2 つの配列の要素の 2 乗の和を合計します。",
    ),
    (
        "SUMXMY2",
        "Returns the sum of squared differences of two arrays.",
        "2 つの配列の要素の差の 2 乗を合計します。",
    ),
    (
        "SWITCH",
        "Compares a value with a list and returns the matching result.",
        "値を一覧と比較し、一致した値に対応する結果を返します。",
    ),
    (
        "SYD",
        "Returns sum-of-years' digits depreciation.",
        "級数法による減価償却費を返します。",
    ),
    (
        "T",
        "Returns text if the value is text, else an empty string.",
        "値が文字列ならその文字列を、そうでなければ空の文字列を返します。",
    ),
    (
        "T.DIST",
        "Returns the Student's t-distribution.",
        "スチューデントの t 分布の値を返します。",
    ),
    (
        "T.DIST.2T",
        "Returns the two-tailed Student's t-distribution.",
        "スチューデントの t 分布の両側確率を返します。",
    ),
    (
        "T.DIST.RT",
        "Returns the right-tailed Student's t-distribution.",
        "スチューデントの t 分布の右側確率を返します。",
    ),
    (
        "T.INV",
        "Returns the inverse of the Student's t-distribution.",
        "スチューデントの t 分布の逆関数の値を返します。",
    ),
    (
        "T.INV.2T",
        "Returns the two-tailed inverse of the Student's t-distribution.",
        "スチューデントの t 分布の両側逆関数の値を返します。",
    ),
    (
        "T.TEST",
        "Returns the probability from a Student's t-test.",
        "スチューデントの t 検定の確率を返します。",
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
        "TANH",
        "Returns the hyperbolic tangent.",
        "双曲線正接を返します。",
    ),
    (
        "TBILLEQ",
        "Returns the bond-equivalent yield of a Treasury bill.",
        "米国財務省短期証券の債券換算利回りを返します。",
    ),
    (
        "TBILLPRICE",
        "Returns the price per 100 face value of a Treasury bill.",
        "米国財務省短期証券の額面 100 あたりの価格を返します。",
    ),
    (
        "TBILLYIELD",
        "Returns the yield of a Treasury bill.",
        "米国財務省短期証券の利回りを返します。",
    ),
    (
        "TDIST",
        "Returns the Student's t-distribution.",
        "スチューデントの t 分布の値を返します。",
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
        "TINV",
        "Returns the two-tailed inverse of the Student's t-distribution.",
        "スチューデントの t 分布の両側逆関数の値を返します。",
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
        "TRIMMEAN",
        "Returns the mean of the interior of a data set.",
        "データの上下を除いた平均値を返します。",
    ),
    (
        "TRIMRANGE",
        "Trims empty rows and columns from the edges of a range.",
        "範囲の外側の空白の行と列を除きます。",
    ),
    (
        "TRUE",
        "Returns the logical value TRUE.",
        "論理値 TRUE を返します。",
    ),
    (
        "TRUNC",
        "Truncates a number to an integer or a number of digits.",
        "数値の小数部を切り捨てて整数または指定した桁数にします。",
    ),
    (
        "TTEST",
        "Returns the probability from a Student's t-test.",
        "スチューデントの t 検定の確率を返します。",
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
        "VALUETOTEXT",
        "Returns a value as text.",
        "値を文字列で返します。",
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
        "VARA",
        "Estimates variance from a sample, counting text and logical values.",
        "文字列や論理値も含めて標本の分散を推定します。",
    ),
    (
        "VARP",
        "Returns the variance of a whole population.",
        "母集団全体の分散を返します。",
    ),
    (
        "VARPA",
        "Returns the population variance, counting text and logical values.",
        "文字列や論理値も含めて母集団の分散を返します。",
    ),
    (
        "VDB",
        "Returns declining-balance depreciation for any period.",
        "倍率法による任意の期間の減価償却費を返します。",
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
        "WEIBULL",
        "Returns the Weibull distribution.",
        "ワイブル分布の値を返します。",
    ),
    (
        "WEIBULL.DIST",
        "Returns the Weibull distribution.",
        "ワイブル分布の値を返します。",
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
        "WRAPCOLS",
        "Wraps a row or column into columns.",
        "行や列の値を指定した数ごとに折り返して列にします。",
    ),
    (
        "WRAPROWS",
        "Wraps a row or column into rows.",
        "行や列の値を指定した数ごとに折り返して行にします。",
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
    (
        "YIELD",
        "Returns the yield of a security paying periodic interest.",
        "定期利付債の利回りを返します。",
    ),
    (
        "YIELDDISC",
        "Returns the annual yield of a discounted security.",
        "割引債の年利回りを返します。",
    ),
    (
        "YIELDMAT",
        "Returns the annual yield of a security paying at maturity.",
        "満期利付債の年利回りを返します。",
    ),
    (
        "Z.TEST",
        "Returns the one-tailed probability of a z-test.",
        "z 検定の片側確率を返します。",
    ),
    (
        "ZTEST",
        "Returns the one-tailed probability of a z-test.",
        "z 検定の片側確率を返します。",
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
