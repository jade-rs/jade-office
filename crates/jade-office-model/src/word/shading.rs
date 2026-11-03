//! 線種とページ罫線と網かけの設定 › 網かけ: the 種類 list — なし, 塗りつぶし,
//! the percentages from 5 % to 95 %, 横縞, 縦縞, 右下がり斜線, 右上がり斜線,
//! 格子, 斜め格子 and their 細 variants (ECMA-376 Part 1 §17.18.78 `ST_Shd`)
//! — its 色 and the 背景の色 under it, as both shells' dialogs pick them.

use super::paragraph_options::ShadingSpec;

/// Every pattern, with its label: an i18n key (`word.…`) or the text itself.
pub const SHADING_PATTERNS: [(&str, &str); 37] = [
    ("clear", "word.shading.clear"),
    ("solid", "word.shading.solid"),
    ("pct5", "5%"),
    ("pct10", "10%"),
    ("pct12", "12.5%"),
    ("pct15", "15%"),
    ("pct20", "20%"),
    ("pct25", "25%"),
    ("pct30", "30%"),
    ("pct35", "35%"),
    ("pct37", "37.5%"),
    ("pct40", "40%"),
    ("pct45", "45%"),
    ("pct50", "50%"),
    ("pct55", "55%"),
    ("pct60", "60%"),
    ("pct62", "62.5%"),
    ("pct65", "65%"),
    ("pct70", "70%"),
    ("pct75", "75%"),
    ("pct80", "80%"),
    ("pct85", "85%"),
    ("pct87", "87.5%"),
    ("pct90", "90%"),
    ("pct95", "95%"),
    ("horzStripe", "word.shading.horz_stripe"),
    ("vertStripe", "word.shading.vert_stripe"),
    ("diagStripe", "word.shading.diag_stripe"),
    ("reverseDiagStripe", "word.shading.reverse_diag_stripe"),
    ("horzCross", "word.shading.horz_cross"),
    ("diagCross", "word.shading.diag_cross"),
    ("thinHorzStripe", "word.shading.thin_horz_stripe"),
    ("thinVertStripe", "word.shading.thin_vert_stripe"),
    ("thinDiagStripe", "word.shading.thin_diag_stripe"),
    ("thinReverseDiagStripe", "word.shading.thin_reverse_diag_stripe"),
    ("thinHorzCross", "word.shading.thin_horz_cross"),
    ("thinDiagCross", "word.shading.thin_diag_cross"),
];

/// 色: the pattern's colours, 自動 (`None`, black) first.
pub const PATTERN_COLORS: [Option<&str>; 8] = [
    None,
    Some("000000"),
    Some("7F7F7F"),
    Some("C00000"),
    Some("2F5496"),
    Some("00B050"),
    Some("FFC000"),
    Some("7030A0"),
];

/// The list's spelling of a pattern read back from a document (the reader
/// keeps `w:val` lowercased): `horzstripe` is `horzStripe`.
pub fn canonical_pattern(value: &str) -> String {
    SHADING_PATTERNS
        .iter()
        .find(|(token, _)| token.eq_ignore_ascii_case(value))
        .map_or_else(|| value.to_string(), |(token, _)| token.to_string())
}

fn pattern_of(shading: Option<&ShadingSpec>) -> String {
    shading.map_or("clear".to_string(), |shading| shading.pattern.clone())
}

/// 背景の色 picked: the pattern and its colour stay; nothing at all with
/// neither a fill nor a pattern.
pub fn with_fill(shading: Option<&ShadingSpec>, fill: Option<&str>) -> Option<ShadingSpec> {
    let pattern = pattern_of(shading);
    if fill.is_none() && pattern == "clear" {
        return None;
    }
    Some(ShadingSpec {
        pattern,
        color: shading.and_then(|shading| shading.color.clone()),
        fill: fill.map(str::to_string),
    })
}

/// 種類 picked: the fill and the colour stay (なし takes the colour away).
pub fn with_pattern(shading: Option<&ShadingSpec>, pattern: &str) -> Option<ShadingSpec> {
    let fill = shading.and_then(|shading| shading.fill.clone());
    if pattern == "clear" && fill.is_none() {
        return None;
    }
    Some(ShadingSpec {
        pattern: pattern.to_string(),
        color: (pattern != "clear")
            .then(|| shading.and_then(|shading| shading.color.clone()))
            .flatten(),
        fill,
    })
}

/// 色 picked: the pattern's colour. With なし still chosen there is nothing
/// for it to colour, so the pattern stays as it is.
pub fn with_color(shading: Option<&ShadingSpec>, color: Option<&str>) -> Option<ShadingSpec> {
    let mut shading = shading.cloned()?;
    if shading.pattern != "clear" {
        shading.color = color.map(str::to_string);
    }
    Some(shading)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_list_spells_what_documents_read_back() {
        assert_eq!(canonical_pattern("horzstripe"), "horzStripe");
        assert_eq!(canonical_pattern("THINDIAGCROSS"), "thinDiagCross");
        assert_eq!(canonical_pattern("pct25"), "pct25");
        assert_eq!(canonical_pattern("nil"), "nil");
    }

    #[test]
    fn fill_pattern_and_colour_are_picked_on_their_own() {
        let striped = with_pattern(None, "diagCross").unwrap();
        assert_eq!((striped.color.clone(), striped.fill.clone()), (None, None));
        let red = with_color(Some(&striped), Some("C00000")).unwrap();
        assert_eq!(red.color.as_deref(), Some("C00000"));
        let filled = with_fill(Some(&red), Some("FFFF00")).unwrap();
        assert_eq!(
            (filled.pattern.as_str(), filled.color.as_deref(), filled.fill.as_deref()),
            ("diagCross", Some("C00000"), Some("FFFF00"))
        );
        let cleared = with_pattern(Some(&filled), "clear").unwrap();
        assert_eq!((cleared.color.clone(), cleared.fill.as_deref()), (None, Some("FFFF00")));
        assert_eq!(with_fill(Some(&cleared), None), None);
        assert!(with_pattern(None, "clear").is_none());
        assert!(with_color(None, Some("000000")).is_none());
    }
}
