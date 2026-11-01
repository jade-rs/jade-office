//! Formula argument tips: inside a function's parentheses while a formula
//! is typed, the function's arguments as Excel shows them under the cell —
//! `SUM(number1, [number2], ...)` — with the one the caret is in marked.

/// The arguments of the functions the tip knows, as Excel names them.
const SIGNATURES: &[(&str, &[&str])] = &[
    ("ABS", &["number"]),
    (
        "ACCRINT",
        &[
            "issue",
            "first_interest",
            "settlement",
            "rate",
            "par",
            "frequency",
            "[basis]",
            "[calc_method]",
        ],
    ),
    (
        "ACCRINTM",
        &["issue", "settlement", "rate", "par", "[basis]"],
    ),
    ("ACOS", &["number"]),
    ("ACOSH", &["number"]),
    ("ACOT", &["number"]),
    ("ACOTH", &["number"]),
    (
        "ADDRESS",
        &["row_num", "column_num", "[abs_num]", "[a1]", "[sheet_text]"],
    ),
    ("AGGREGATE", &["function_num", "options", "ref1", "..."]),
    (
        "AMORDEGRC",
        &[
            "cost",
            "date_purchased",
            "first_period",
            "salvage",
            "period",
            "rate",
            "[basis]",
        ],
    ),
    (
        "AMORLINC",
        &[
            "cost",
            "date_purchased",
            "first_period",
            "salvage",
            "period",
            "rate",
            "[basis]",
        ],
    ),
    ("ANCHORARRAY", &["reference"]),
    ("AND", &["logical1", "[logical2]", "..."]),
    ("ARABIC", &["text"]),
    ("AREAS", &["reference"]),
    ("ARRAYTOTEXT", &["array", "[format]"]),
    ("ASC", &["text"]),
    ("ASIN", &["number"]),
    ("ASINH", &["number"]),
    ("ATAN", &["number"]),
    ("ATAN2", &["x_num", "y_num"]),
    ("ATANH", &["number"]),
    ("AVEDEV", &["number1", "[number2]", "..."]),
    ("AVERAGE", &["number1", "[number2]", "..."]),
    ("AVERAGEA", &["value1", "[value2]", "..."]),
    ("AVERAGEIF", &["range", "criteria", "[average_range]"]),
    (
        "AVERAGEIFS",
        &["average_range", "criteria_range1", "criteria1", "..."],
    ),
    ("BAHTTEXT", &["number"]),
    ("BASE", &["number", "radix", "[min_length]"]),
    ("BESSELI", &["x", "n"]),
    ("BESSELJ", &["x", "n"]),
    ("BESSELK", &["x", "n"]),
    ("BESSELY", &["x", "n"]),
    (
        "BETA.DIST",
        &["x", "alpha", "beta", "cumulative", "[A]", "[B]"],
    ),
    ("BETA.INV", &["probability", "alpha", "beta", "[A]", "[B]"]),
    ("BETADIST", &["x", "alpha", "beta", "[A]", "[B]"]),
    ("BETAINV", &["probability", "alpha", "beta", "[A]", "[B]"]),
    ("BIN2DEC", &["number"]),
    ("BIN2HEX", &["number", "[places]"]),
    ("BIN2OCT", &["number", "[places]"]),
    (
        "BINOM.DIST",
        &["number_s", "trials", "probability_s", "cumulative"],
    ),
    (
        "BINOM.DIST.RANGE",
        &["trials", "probability_s", "number_s", "[number_s2]"],
    ),
    ("BINOM.INV", &["trials", "probability_s", "alpha"]),
    (
        "BINOMDIST",
        &["number_s", "trials", "probability_s", "cumulative"],
    ),
    ("BITAND", &["number1", "number2"]),
    ("BITLSHIFT", &["number", "shift_amount"]),
    ("BITOR", &["number1", "number2"]),
    ("BITRSHIFT", &["number", "shift_amount"]),
    ("BITXOR", &["number1", "number2"]),
    ("BYCOL", &["array", "lambda"]),
    ("BYROW", &["array", "lambda"]),
    ("CEILING", &["number", "significance"]),
    ("CEILING.MATH", &["number", "[significance]", "[mode]"]),
    ("CEILING.PRECISE", &["number", "[significance]"]),
    ("CELL", &["info_type", "[reference]"]),
    ("CHAR", &["number"]),
    ("CHIDIST", &["x", "deg_freedom"]),
    ("CHIINV", &["probability", "deg_freedom"]),
    ("CHISQ.DIST", &["x", "deg_freedom", "cumulative"]),
    ("CHISQ.DIST.RT", &["x", "deg_freedom"]),
    ("CHISQ.INV", &["probability", "deg_freedom"]),
    ("CHISQ.INV.RT", &["probability", "deg_freedom"]),
    ("CHISQ.TEST", &["actual_range", "expected_range"]),
    ("CHITEST", &["actual_range", "expected_range"]),
    ("CHOOSE", &["index_num", "value1", "[value2]", "..."]),
    ("CHOOSECOLS", &["array", "col_num1", "[col_num2]", "..."]),
    ("CHOOSEROWS", &["array", "row_num1", "[row_num2]", "..."]),
    ("CLEAN", &["text"]),
    ("CODE", &["text"]),
    ("COLUMN", &["[reference]"]),
    ("COLUMNS", &["array"]),
    ("COMBIN", &["number", "number_chosen"]),
    ("COMBINA", &["number", "number_chosen"]),
    ("COMPLEX", &["real_num", "i_num", "[suffix]"]),
    ("CONCAT", &["text1", "[text2]", "..."]),
    ("CONCATENATE", &["text1", "[text2]", "..."]),
    ("CONFIDENCE", &["alpha", "standard_dev", "size"]),
    ("CONFIDENCE.NORM", &["alpha", "standard_dev", "size"]),
    ("CONFIDENCE.T", &["alpha", "standard_dev", "size"]),
    ("CONVERT", &["number", "from_unit", "to_unit"]),
    ("CORREL", &["array1", "array2"]),
    ("COS", &["number"]),
    ("COSH", &["number"]),
    ("COT", &["number"]),
    ("COTH", &["number"]),
    ("COUNT", &["value1", "[value2]", "..."]),
    ("COUNTA", &["value1", "[value2]", "..."]),
    ("COUNTBLANK", &["range"]),
    ("COUNTIF", &["range", "criteria"]),
    ("COUNTIFS", &["criteria_range1", "criteria1", "..."]),
    (
        "COUPDAYBS",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    (
        "COUPDAYS",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    (
        "COUPDAYSNC",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    (
        "COUPNCD",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    (
        "COUPNUM",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    (
        "COUPPCD",
        &["settlement", "maturity", "frequency", "[basis]"],
    ),
    ("COVAR", &["array1", "array2"]),
    ("COVARIANCE.P", &["array1", "array2"]),
    ("COVARIANCE.S", &["array1", "array2"]),
    ("CRITBINOM", &["trials", "probability_s", "alpha"]),
    ("CSC", &["number"]),
    ("CSCH", &["number"]),
    (
        "CUMIPMT",
        &["rate", "nper", "pv", "start_period", "end_period", "type"],
    ),
    (
        "CUMPRINC",
        &["rate", "nper", "pv", "start_period", "end_period", "type"],
    ),
    ("DATE", &["year", "month", "day"]),
    ("DATEDIF", &["start_date", "end_date", "unit"]),
    ("DATESTRING", &["serial_number"]),
    ("DATEVALUE", &["date_text"]),
    ("DAVERAGE", &["database", "field", "criteria"]),
    ("DAY", &["serial_number"]),
    ("DAYS", &["end_date", "start_date"]),
    ("DAYS360", &["start_date", "end_date", "[method]"]),
    ("DB", &["cost", "salvage", "life", "period", "[month]"]),
    ("DBCS", &["text"]),
    ("DCOUNT", &["database", "field", "criteria"]),
    ("DCOUNTA", &["database", "field", "criteria"]),
    ("DDB", &["cost", "salvage", "life", "period", "[factor]"]),
    ("DEC2BIN", &["number", "[places]"]),
    ("DEC2HEX", &["number", "[places]"]),
    ("DEC2OCT", &["number", "[places]"]),
    ("DECIMAL", &["text", "radix"]),
    ("DEGREES", &["angle"]),
    ("DELTA", &["number1", "[number2]"]),
    ("DEVSQ", &["number1", "[number2]", "..."]),
    ("DGET", &["database", "field", "criteria"]),
    (
        "DISC",
        &["settlement", "maturity", "pr", "redemption", "[basis]"],
    ),
    ("DMAX", &["database", "field", "criteria"]),
    ("DMIN", &["database", "field", "criteria"]),
    ("DOLLAR", &["number", "[decimals]"]),
    ("DOLLARDE", &["fractional_dollar", "fraction"]),
    ("DOLLARFR", &["decimal_dollar", "fraction"]),
    ("DPRODUCT", &["database", "field", "criteria"]),
    ("DROP", &["array", "rows", "[columns]"]),
    ("DSTDEV", &["database", "field", "criteria"]),
    ("DSTDEVP", &["database", "field", "criteria"]),
    ("DSUM", &["database", "field", "criteria"]),
    (
        "DURATION",
        &[
            "settlement",
            "maturity",
            "coupon",
            "yld",
            "frequency",
            "[basis]",
        ],
    ),
    ("DVAR", &["database", "field", "criteria"]),
    ("DVARP", &["database", "field", "criteria"]),
    ("ECMA.CEILING", &["number", "significance"]),
    ("EDATE", &["start_date", "months"]),
    ("EFFECT", &["nominal_rate", "npery"]),
    ("ENCODEURL", &["text"]),
    ("EOMONTH", &["start_date", "months"]),
    ("ERF", &["lower_limit", "[upper_limit]"]),
    ("ERF.PRECISE", &["x"]),
    ("ERFC", &["x"]),
    ("ERFC.PRECISE", &["x"]),
    ("ERROR.TYPE", &["error_val"]),
    ("EVEN", &["number"]),
    ("EXACT", &["text1", "text2"]),
    ("EXP", &["number"]),
    ("EXPAND", &["array", "rows", "[columns]", "[pad_with]"]),
    ("EXPON.DIST", &["x", "lambda", "cumulative"]),
    ("EXPONDIST", &["x", "lambda", "cumulative"]),
    (
        "F.DIST",
        &["x", "deg_freedom1", "deg_freedom2", "cumulative"],
    ),
    ("F.DIST.RT", &["x", "deg_freedom1", "deg_freedom2"]),
    ("F.INV", &["probability", "deg_freedom1", "deg_freedom2"]),
    ("F.INV.RT", &["probability", "deg_freedom1", "deg_freedom2"]),
    ("F.TEST", &["array1", "array2"]),
    ("FACT", &["number"]),
    ("FACTDOUBLE", &["number"]),
    ("FALSE", &[]),
    ("FDIST", &["x", "deg_freedom1", "deg_freedom2"]),
    ("FILTER", &["array", "include", "[if_empty]"]),
    ("FILTERXML", &["xml", "xpath"]),
    ("FIND", &["find_text", "within_text", "[start_num]"]),
    ("FINDB", &["find_text", "within_text", "[start_num]"]),
    ("FINV", &["probability", "deg_freedom1", "deg_freedom2"]),
    ("FISHER", &["x"]),
    ("FISHERINV", &["y"]),
    ("FIXED", &["number", "[decimals]", "[no_commas]"]),
    ("FLOOR", &["number", "significance"]),
    ("FLOOR.MATH", &["number", "[significance]", "[mode]"]),
    ("FLOOR.PRECISE", &["number", "[significance]"]),
    ("FORECAST", &["x", "known_ys", "known_xs"]),
    (
        "FORECAST.ETS",
        &[
            "target_date",
            "values",
            "timeline",
            "[seasonality]",
            "[data_completion]",
            "[aggregation]",
        ],
    ),
    (
        "FORECAST.ETS.CONFINT",
        &[
            "target_date",
            "values",
            "timeline",
            "[confidence_level]",
            "[seasonality]",
            "[data_completion]",
            "[aggregation]",
        ],
    ),
    (
        "FORECAST.ETS.SEASONALITY",
        &["values", "timeline", "[data_completion]", "[aggregation]"],
    ),
    (
        "FORECAST.ETS.STAT",
        &[
            "values",
            "timeline",
            "statistic_type",
            "[seasonality]",
            "[data_completion]",
            "[aggregation]",
        ],
    ),
    ("FORECAST.LINEAR", &["x", "known_ys", "known_xs"]),
    ("FORMULATEXT", &["reference"]),
    ("FREQUENCY", &["data_array", "bins_array"]),
    ("FTEST", &["array1", "array2"]),
    ("FV", &["rate", "nper", "pmt", "[pv]", "[type]"]),
    ("FVSCHEDULE", &["principal", "schedule"]),
    ("GAMMA", &["x"]),
    ("GAMMA.DIST", &["x", "alpha", "beta", "cumulative"]),
    ("GAMMA.INV", &["probability", "alpha", "beta"]),
    ("GAMMADIST", &["x", "alpha", "beta", "cumulative"]),
    ("GAMMAINV", &["probability", "alpha", "beta"]),
    ("GAMMALN", &["x"]),
    ("GAMMALN.PRECISE", &["x"]),
    ("GAUSS", &["x"]),
    ("GCD", &["number1", "[number2]", "..."]),
    ("GEOMEAN", &["number1", "[number2]", "..."]),
    ("GESTEP", &["number", "[step]"]),
    (
        "GETPIVOTDATA",
        &["data_field", "pivot_table", "[field1]", "[item1]", "..."],
    ),
    (
        "GROUPBY",
        &[
            "row_fields",
            "values",
            "function",
            "[field_headers]",
            "[total_depth]",
            "[sort_order]",
            "[filter_array]",
        ],
    ),
    ("GROWTH", &["known_ys", "[known_xs]", "[new_xs]", "[const]"]),
    ("HARMEAN", &["number1", "[number2]", "..."]),
    ("HEX2BIN", &["number", "[places]"]),
    ("HEX2DEC", &["number"]),
    ("HEX2OCT", &["number", "[places]"]),
    (
        "HLOOKUP",
        &[
            "lookup_value",
            "table_array",
            "row_index_num",
            "[range_lookup]",
        ],
    ),
    ("HOUR", &["serial_number"]),
    ("HSTACK", &["array1", "[array2]", "..."]),
    ("HYPERLINK", &["link_location", "[friendly_name]"]),
    (
        "HYPGEOM.DIST",
        &[
            "sample_s",
            "number_sample",
            "population_s",
            "number_pop",
            "cumulative",
        ],
    ),
    (
        "HYPGEOMDIST",
        &["sample_s", "number_sample", "population_s", "number_pop"],
    ),
    (
        "IF",
        &["logical_test", "[value_if_true]", "[value_if_false]"],
    ),
    ("IFERROR", &["value", "value_if_error"]),
    ("IFNA", &["value", "value_if_na"]),
    ("IFS", &["logical_test1", "value_if_true1", "..."]),
    ("IMABS", &["inumber"]),
    ("IMAGINARY", &["inumber"]),
    ("IMARGUMENT", &["inumber"]),
    ("IMCONJUGATE", &["inumber"]),
    ("IMCOS", &["inumber"]),
    ("IMCOSH", &["inumber"]),
    ("IMCOT", &["inumber"]),
    ("IMCSC", &["inumber"]),
    ("IMCSCH", &["inumber"]),
    ("IMDIV", &["inumber1", "inumber2"]),
    ("IMEXP", &["inumber"]),
    ("IMLN", &["inumber"]),
    ("IMLOG10", &["inumber"]),
    ("IMLOG2", &["inumber"]),
    ("IMPOWER", &["inumber", "number"]),
    ("IMPRODUCT", &["inumber1", "[inumber2]", "..."]),
    ("IMREAL", &["inumber"]),
    ("IMSEC", &["inumber"]),
    ("IMSECH", &["inumber"]),
    ("IMSIN", &["inumber"]),
    ("IMSINH", &["inumber"]),
    ("IMSQRT", &["inumber"]),
    ("IMSUB", &["inumber1", "inumber2"]),
    ("IMSUM", &["inumber1", "[inumber2]", "..."]),
    ("IMTAN", &["inumber"]),
    ("INDEX", &["array", "row_num", "[column_num]"]),
    ("INDIRECT", &["ref_text", "[a1]"]),
    ("INFO", &["type_text"]),
    ("INT", &["number"]),
    ("INTERCEPT", &["known_ys", "known_xs"]),
    (
        "INTRATE",
        &[
            "settlement",
            "maturity",
            "investment",
            "redemption",
            "[basis]",
        ],
    ),
    ("IPMT", &["rate", "per", "nper", "pv", "[fv]", "[type]"]),
    ("IRR", &["values", "[guess]"]),
    ("ISBLANK", &["value"]),
    ("ISERR", &["value"]),
    ("ISERROR", &["value"]),
    ("ISEVEN", &["number"]),
    ("ISFORMULA", &["reference"]),
    ("ISLOGICAL", &["value"]),
    ("ISNA", &["value"]),
    ("ISNONTEXT", &["value"]),
    ("ISNUMBER", &["value"]),
    ("ISO.CEILING", &["number", "[significance]"]),
    ("ISODD", &["number"]),
    ("ISOMITTED", &["argument"]),
    ("ISOWEEKNUM", &["date"]),
    ("ISPMT", &["rate", "per", "nper", "pv"]),
    ("ISREF", &["value"]),
    ("ISTEXT", &["value"]),
    ("JIS", &["text"]),
    ("KURT", &["number1", "[number2]", "..."]),
    ("LAMBDA", &["[parameter1]", "...", "calculation"]),
    ("LARGE", &["array", "k"]),
    ("LCM", &["number1", "[number2]", "..."]),
    ("LEFT", &["text", "[num_chars]"]),
    ("LEFTB", &["text", "[num_bytes]"]),
    ("LEN", &["text"]),
    ("LENB", &["text"]),
    (
        "LET",
        &["name1", "name_value1", "calculation_or_name2", "..."],
    ),
    ("LINEST", &["known_ys", "[known_xs]", "[const]", "[stats]"]),
    ("LN", &["number"]),
    ("LOG", &["number", "[base]"]),
    ("LOG10", &["number"]),
    ("LOGEST", &["known_ys", "[known_xs]", "[const]", "[stats]"]),
    ("LOGINV", &["probability", "mean", "standard_dev"]),
    ("LOGNORM.DIST", &["x", "mean", "standard_dev", "cumulative"]),
    ("LOGNORM.INV", &["probability", "mean", "standard_dev"]),
    ("LOGNORMDIST", &["x", "mean", "standard_dev"]),
    (
        "LOOKUP",
        &["lookup_value", "lookup_vector", "[result_vector]"],
    ),
    ("LOWER", &["text"]),
    ("MAKEARRAY", &["rows", "cols", "lambda"]),
    ("MAP", &["array1", "lambda_or_array2", "..."]),
    ("MATCH", &["lookup_value", "lookup_array", "[match_type]"]),
    ("MAX", &["number1", "[number2]", "..."]),
    ("MAXA", &["value1", "[value2]", "..."]),
    (
        "MAXIFS",
        &["max_range", "criteria_range1", "criteria1", "..."],
    ),
    ("MDETERM", &["array"]),
    (
        "MDURATION",
        &[
            "settlement",
            "maturity",
            "coupon",
            "yld",
            "frequency",
            "[basis]",
        ],
    ),
    ("MEDIAN", &["number1", "[number2]", "..."]),
    ("MID", &["text", "start_num", "num_chars"]),
    ("MIDB", &["text", "start_num", "num_bytes"]),
    ("MIN", &["number1", "[number2]", "..."]),
    ("MINA", &["value1", "[value2]", "..."]),
    (
        "MINIFS",
        &["min_range", "criteria_range1", "criteria1", "..."],
    ),
    ("MINUTE", &["serial_number"]),
    ("MINVERSE", &["array"]),
    ("MIRR", &["values", "finance_rate", "reinvest_rate"]),
    ("MMULT", &["array1", "array2"]),
    ("MOD", &["number", "divisor"]),
    ("MODE", &["number1", "[number2]", "..."]),
    ("MODE.MULT", &["number1", "[number2]", "..."]),
    ("MODE.SNGL", &["number1", "[number2]", "..."]),
    ("MONTH", &["serial_number"]),
    ("MROUND", &["number", "multiple"]),
    ("MULTINOMIAL", &["number1", "[number2]", "..."]),
    ("MUNIT", &["dimension"]),
    ("N", &["value"]),
    ("NA", &[]),
    (
        "NEGBINOM.DIST",
        &["number_f", "number_s", "probability_s", "cumulative"],
    ),
    ("NEGBINOMDIST", &["number_f", "number_s", "probability_s"]),
    ("NETWORKDAYS", &["start_date", "end_date", "[holidays]"]),
    (
        "NETWORKDAYS.INTL",
        &["start_date", "end_date", "[weekend]", "[holidays]"],
    ),
    ("NOMINAL", &["effect_rate", "npery"]),
    ("NORM.DIST", &["x", "mean", "standard_dev", "cumulative"]),
    ("NORM.INV", &["probability", "mean", "standard_dev"]),
    ("NORM.S.DIST", &["z", "cumulative"]),
    ("NORM.S.INV", &["probability"]),
    ("NORMDIST", &["x", "mean", "standard_dev", "cumulative"]),
    ("NORMINV", &["probability", "mean", "standard_dev"]),
    ("NORMSDIST", &["z"]),
    ("NORMSINV", &["probability"]),
    ("NOT", &["logical"]),
    ("NOW", &[]),
    ("NPER", &["rate", "pmt", "pv", "[fv]", "[type]"]),
    ("NPV", &["rate", "value1", "[value2]", "..."]),
    ("NUMBERSTRING", &["value", "type"]),
    (
        "NUMBERVALUE",
        &["text", "[decimal_separator]", "[group_separator]"],
    ),
    ("OCT2BIN", &["number", "[places]"]),
    ("OCT2DEC", &["number"]),
    ("OCT2HEX", &["number", "[places]"]),
    ("ODD", &["number"]),
    (
        "ODDFPRICE",
        &[
            "settlement",
            "maturity",
            "issue",
            "first_coupon",
            "rate",
            "yld",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "ODDFYIELD",
        &[
            "settlement",
            "maturity",
            "issue",
            "first_coupon",
            "rate",
            "pr",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "ODDLPRICE",
        &[
            "settlement",
            "maturity",
            "last_interest",
            "rate",
            "yld",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "ODDLYIELD",
        &[
            "settlement",
            "maturity",
            "last_interest",
            "rate",
            "pr",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "OFFSET",
        &["reference", "rows", "cols", "[height]", "[width]"],
    ),
    ("OR", &["logical1", "[logical2]", "..."]),
    ("PDURATION", &["rate", "pv", "fv"]),
    ("PEARSON", &["array1", "array2"]),
    ("PERCENTILE", &["array", "k"]),
    ("PERCENTILE.EXC", &["array", "k"]),
    ("PERCENTILE.INC", &["array", "k"]),
    ("PERCENTOF", &["data_subset", "data_all"]),
    ("PERCENTRANK", &["array", "x", "[significance]"]),
    ("PERCENTRANK.EXC", &["array", "x", "[significance]"]),
    ("PERCENTRANK.INC", &["array", "x", "[significance]"]),
    ("PERMUT", &["number", "number_chosen"]),
    ("PERMUTATIONA", &["number", "number_chosen"]),
    ("PHI", &["x"]),
    ("PHONETIC", &["reference"]),
    ("PI", &[]),
    (
        "PIVOTBY",
        &[
            "row_fields",
            "col_fields",
            "values",
            "function",
            "[field_headers]",
            "[row_total_depth]",
            "[row_sort_order]",
            "[col_total_depth]",
            "[col_sort_order]",
            "[filter_array]",
            "[relative_to]",
        ],
    ),
    ("PMT", &["rate", "nper", "pv", "[fv]", "[type]"]),
    ("POISSON", &["x", "mean", "cumulative"]),
    ("POISSON.DIST", &["x", "mean", "cumulative"]),
    ("POWER", &["number", "power"]),
    ("PPMT", &["rate", "per", "nper", "pv", "[fv]", "[type]"]),
    (
        "PRICE",
        &[
            "settlement",
            "maturity",
            "rate",
            "yld",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "PRICEDISC",
        &[
            "settlement",
            "maturity",
            "discount",
            "redemption",
            "[basis]",
        ],
    ),
    (
        "PRICEMAT",
        &["settlement", "maturity", "issue", "rate", "yld", "[basis]"],
    ),
    (
        "PROB",
        &["x_range", "prob_range", "[lower_limit]", "[upper_limit]"],
    ),
    ("PRODUCT", &["number1", "[number2]", "..."]),
    ("PROPER", &["text"]),
    ("PV", &["rate", "nper", "pmt", "[fv]", "[type]"]),
    ("QUARTILE", &["array", "quart"]),
    ("QUARTILE.EXC", &["array", "quart"]),
    ("QUARTILE.INC", &["array", "quart"]),
    ("QUOTIENT", &["numerator", "denominator"]),
    ("RADIANS", &["angle"]),
    ("RAND", &[]),
    (
        "RANDARRAY",
        &["[rows]", "[columns]", "[min]", "[max]", "[whole_number]"],
    ),
    ("RANDBETWEEN", &["bottom", "top"]),
    ("RANK", &["number", "ref", "[order]"]),
    ("RANK.AVG", &["number", "ref", "[order]"]),
    ("RANK.EQ", &["number", "ref", "[order]"]),
    ("RATE", &["nper", "pmt", "pv", "[fv]", "[type]", "[guess]"]),
    (
        "RECEIVED",
        &[
            "settlement",
            "maturity",
            "investment",
            "discount",
            "[basis]",
        ],
    ),
    ("REDUCE", &["[initial_value]", "array", "lambda"]),
    (
        "REGEXEXTRACT",
        &["text", "pattern", "[return_mode]", "[case_sensitivity]"],
    ),
    (
        "REGEXREPLACE",
        &[
            "text",
            "pattern",
            "replacement",
            "[occurrence]",
            "[case_sensitivity]",
        ],
    ),
    ("REGEXTEST", &["text", "pattern", "[case_sensitivity]"]),
    (
        "REPLACE",
        &["old_text", "start_num", "num_chars", "new_text"],
    ),
    (
        "REPLACEB",
        &["old_text", "start_num", "num_bytes", "new_text"],
    ),
    ("REPT", &["text", "number_times"]),
    ("RIGHT", &["text", "[num_chars]"]),
    ("RIGHTB", &["text", "[num_bytes]"]),
    ("ROMAN", &["number", "[form]"]),
    ("ROUND", &["number", "num_digits"]),
    ("ROUNDDOWN", &["number", "num_digits"]),
    ("ROUNDUP", &["number", "num_digits"]),
    ("ROW", &["[reference]"]),
    ("ROWS", &["array"]),
    ("RRI", &["nper", "pv", "fv"]),
    ("RSQ", &["known_ys", "known_xs"]),
    ("SCAN", &["[initial_value]", "array", "lambda"]),
    ("SEARCH", &["find_text", "within_text", "[start_num]"]),
    ("SEARCHB", &["find_text", "within_text", "[start_num]"]),
    ("SEC", &["number"]),
    ("SECH", &["number"]),
    ("SECOND", &["serial_number"]),
    ("SEQUENCE", &["rows", "[columns]", "[start]", "[step]"]),
    ("SERIESSUM", &["x", "n", "m", "coefficients"]),
    ("SHEET", &["[value]"]),
    ("SHEETS", &["[reference]"]),
    ("SIGN", &["number"]),
    ("SIN", &["number"]),
    ("SINGLE", &["value"]),
    ("SINH", &["number"]),
    ("SKEW", &["number1", "[number2]", "..."]),
    ("SKEW.P", &["number1", "[number2]", "..."]),
    ("SLN", &["cost", "salvage", "life"]),
    ("SLOPE", &["known_ys", "known_xs"]),
    ("SMALL", &["array", "k"]),
    (
        "SORT",
        &["array", "[sort_index]", "[sort_order]", "[by_col]"],
    ),
    ("SORTBY", &["array", "by_array1", "[sort_order1]", "..."]),
    ("SQRT", &["number"]),
    ("SQRTPI", &["number"]),
    ("STANDARDIZE", &["x", "mean", "standard_dev"]),
    ("STDEV", &["number1", "[number2]", "..."]),
    ("STDEV.P", &["number1", "[number2]", "..."]),
    ("STDEV.S", &["number1", "[number2]", "..."]),
    ("STDEVA", &["value1", "[value2]", "..."]),
    ("STDEVP", &["number1", "[number2]", "..."]),
    ("STDEVPA", &["value1", "[value2]", "..."]),
    ("STEYX", &["known_ys", "known_xs"]),
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
    ("SUMSQ", &["number1", "[number2]", "..."]),
    ("SUMX2MY2", &["array_x", "array_y"]),
    ("SUMX2PY2", &["array_x", "array_y"]),
    ("SUMXMY2", &["array_x", "array_y"]),
    ("SWITCH", &["expression", "value1", "result1", "..."]),
    ("SYD", &["cost", "salvage", "life", "per"]),
    ("T", &["value"]),
    ("T.DIST", &["x", "deg_freedom", "cumulative"]),
    ("T.DIST.2T", &["x", "deg_freedom"]),
    ("T.DIST.RT", &["x", "deg_freedom"]),
    ("T.INV", &["probability", "deg_freedom"]),
    ("T.INV.2T", &["probability", "deg_freedom"]),
    ("T.TEST", &["array1", "array2", "tails", "type"]),
    ("TAKE", &["array", "rows", "[columns]"]),
    ("TAN", &["number"]),
    ("TANH", &["number"]),
    ("TBILLEQ", &["settlement", "maturity", "discount"]),
    ("TBILLPRICE", &["settlement", "maturity", "discount"]),
    ("TBILLYIELD", &["settlement", "maturity", "pr"]),
    ("TDIST", &["x", "deg_freedom", "tails"]),
    ("TEXT", &["value", "format_text"]),
    ("TEXTAFTER", &["text", "delimiter", "[instance_num]"]),
    ("TEXTBEFORE", &["text", "delimiter", "[instance_num]"]),
    ("TEXTJOIN", &["delimiter", "ignore_empty", "text1", "..."]),
    ("TEXTSPLIT", &["text", "col_delimiter", "[row_delimiter]"]),
    ("TIME", &["hour", "minute", "second"]),
    ("TIMEVALUE", &["time_text"]),
    ("TINV", &["probability", "deg_freedom"]),
    ("TOCOL", &["array", "[ignore]", "[scan_by_column]"]),
    ("TODAY", &[]),
    ("TOROW", &["array", "[ignore]", "[scan_by_column]"]),
    ("TRANSPOSE", &["array"]),
    ("TREND", &["known_ys", "[known_xs]", "[new_xs]", "[const]"]),
    ("TRIM", &["text"]),
    ("TRIMMEAN", &["array", "percent"]),
    ("TRIMRANGE", &["range", "[trim_rows]", "[trim_cols]"]),
    ("TRUE", &[]),
    ("TRUNC", &["number", "[num_digits]"]),
    ("TTEST", &["array1", "array2", "tails", "type"]),
    ("TYPE", &["value"]),
    ("UNICHAR", &["number"]),
    ("UNICODE", &["text"]),
    ("UNIQUE", &["array", "[by_col]", "[exactly_once]"]),
    ("UPPER", &["text"]),
    ("VALUE", &["text"]),
    ("VALUETOTEXT", &["value", "[format]"]),
    ("VAR", &["number1", "[number2]", "..."]),
    ("VAR.P", &["number1", "[number2]", "..."]),
    ("VAR.S", &["number1", "[number2]", "..."]),
    ("VARA", &["value1", "[value2]", "..."]),
    ("VARP", &["number1", "[number2]", "..."]),
    ("VARPA", &["value1", "[value2]", "..."]),
    (
        "VDB",
        &[
            "cost",
            "salvage",
            "life",
            "start_period",
            "end_period",
            "[factor]",
            "[no_switch]",
        ],
    ),
    (
        "VLOOKUP",
        &[
            "lookup_value",
            "table_array",
            "col_index_num",
            "[range_lookup]",
        ],
    ),
    ("VSTACK", &["array1", "[array2]", "..."]),
    ("WEEKDAY", &["serial_number", "[return_type]"]),
    ("WEEKNUM", &["serial_number", "[return_type]"]),
    ("WEIBULL", &["x", "alpha", "beta", "cumulative"]),
    ("WEIBULL.DIST", &["x", "alpha", "beta", "cumulative"]),
    ("WORKDAY", &["start_date", "days", "[holidays]"]),
    (
        "WORKDAY.INTL",
        &["start_date", "days", "[weekend]", "[holidays]"],
    ),
    ("WRAPCOLS", &["vector", "wrap_count", "[pad_with]"]),
    ("WRAPROWS", &["vector", "wrap_count", "[pad_with]"]),
    ("XIRR", &["values", "dates", "[guess]"]),
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
    (
        "XMATCH",
        &[
            "lookup_value",
            "lookup_array",
            "[match_mode]",
            "[search_mode]",
        ],
    ),
    ("XNPV", &["rate", "values", "dates"]),
    ("XOR", &["logical1", "[logical2]", "..."]),
    ("YEAR", &["serial_number"]),
    ("YEARFRAC", &["start_date", "end_date", "[basis]"]),
    (
        "YIELD",
        &[
            "settlement",
            "maturity",
            "rate",
            "pr",
            "redemption",
            "frequency",
            "[basis]",
        ],
    ),
    (
        "YIELDDISC",
        &["settlement", "maturity", "pr", "redemption", "[basis]"],
    ),
    (
        "YIELDMAT",
        &["settlement", "maturity", "issue", "rate", "pr", "[basis]"],
    ),
    ("Z.TEST", &["array", "x", "[sigma]"]),
    ("ZTEST", &["array", "x", "[sigma]"]),
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
        let missing: Vec<_> = SIGNATURES
            .iter()
            .map(|(name, _)| *name)
            .filter(|name| !known.iter().any(|known| known == name))
            .collect();
        assert!(missing.is_empty(), "MISSING {}", missing.join(" "));
    }
}
