# Word versions, compatibility modes and platforms

**Covers:** how a `.docx` says which Word laid it out (`w:compat`,
`w:compatSetting`), what Word changes between compatibility modes and
between Word for Mac and Word for Windows, and what jade reads, honours,
ignores and writes back for each.

**Derived from:** jade-office `9df8a1e` / jade-core `9fea681` (origin/main,
2026-10-22): the settings reader
(`jade-core/crates/jade-docx-reader/src/settings.rs`), every place the
layout and renderer consult a compat value
(`jade-core/crates/jade-docx-layout`, `jade-core/crates/jade-docx-editor`),
the platform switch (`crates/jade-office-model/src/word/platform.rs`) and
the Word-for-Mac probe notes under `jade-core/tests/word-oracle/`. The
fixture counts come from the 169 `.docx` files in `jade-core/public/fixtures`.
Descriptions of what Word does are the documented meaning of each
element (ECMA-376 Part 1 §17.15.1 and Microsoft's [MS-DOCX]/[MS-OI29500]
notes); jade's behaviour is from its code. Nothing was run.

**Keeping it current:** when a layout rule starts reading a compat value,
add or update its row in "Legacy compatibility options" and name the test.
`grep -rn 'compatibility_mode\|use_east_asian_layout\|balance_single_byte\|no_tab_hanging_indent\|adjust_line_height_in_table\|character_spacing_control\|word_layout_platform()' jade-core/crates/*/src`
lists every consumer. Paths below are under `jade-core/crates/` unless
they start with `crates/`.

## Word versions and the mode they write

| Word release | `compatibilityMode` | Notes |
|---|---|---|
| Word 2003 and older (`.doc`, or `.docx` through the converter) | 11 | Opens in Compatibility Mode; layout emulates Word 2003 |
| Word 2007 (Windows), Word 2008 (Mac) | 12 | First OOXML writers |
| Word 2010 (Windows), Word 2011 (Mac) | 14 | Adds `compatSetting`s: OpenType features, mirror-indent flipping, table-style font override |
| Word 2013, 2016, 2019, 2021, 2024 and Microsoft 365 (Windows and Mac) | 15 | Still 15 today; newer behaviour arrives as extra named `compatSetting`s (e.g. `useWord2013TrackBottomHyphenation`, `allowHyphenationAtTrackBottom`, `allowTextAfterFloatingTableBreak`) rather than a new mode number |
| Missing `compatibilityMode` | — | Word treats the file as an older layout (Word 2007) — see jade's handling below |

Word's *File › Info › Convert* (Windows) / *Convert Document* (Mac) raises
an old file to mode 15 and drops the legacy options that mode 15 no
longer honours.

Fixture corpus: mode 15 ×101, 14 ×61, 12 ×2, absent ×5.

## What jade reads

`DocumentSettings::from_xml` (`jade-docx-reader/src/settings.rs`) reads
from `w:compat`:

- `w:compatSetting w:name="compatibilityMode"` → `compatibility_mode:
  Option<u32>` (`parse_compatibility_mode`). **No other `compatSetting` name
  is read.**
- five legacy children: `useFELayout`, `balanceSingleByteDoubleByteWidth`,
  `noTabHangInd`, `adjustLineHeightInTable`, and
  `characterSpacingControl` (accepted inside or outside `w:compat`).

The editor applies them on load (`jade-docx-editor/src/word_document.rs`,
`Document.no_tab_hanging_indent`, `adjust_line_height_in_table`,
`snap_to_grid_default`). The headless line extractor
(`jade-docx-layout/src/text_extract.rs::apply_document_settings`) applies
the table and tab flags always, but the East Asian rules and the mode only
when `characterSpacingControl` is `doNotCompress`.

## What the mode changes in jade

jade only ever asks "below 15" or "15 and up"; 11, 12 and 14 behave the
same. Most mode rules are traced from Word for Mac and only apply on the
Mac layout platform.

| Behaviour | Mode < 15 | Mode ≥ 15 | Platform | Where |
|---|---|---|---|---|
| Paragraph-final 。 may hang past the margin | hangs | wraps with the preceding character (except in justified paragraphs, which hang in either mode) | both | `jade-docx-layout/src/line_wrap.rs` `paragraph_end_punctuation_can_hang` |
| Hanging punctuation on ragged (non-justified) lines | `overflowPunct` hangs | kept inside the margin | Mac | `line_wrap.rs` `allows_hanging_punctuation_at` |
| Table indent measures to | the first cell's **text** (cell margin + spacing hang left of `tblInd`) | the cell edge | both | `jade-docx-editor/src/canvas_renderer/table.rs` `pre_2013_table_text_inset_px` |
| Percentage width and autofit of such tables | sized on text width plus both hanging margins | text width | Mac | `table.rs` `word_pre_2013_table_sizing_width_px` |
| Right-aligned table | last cell's text at the margin | cell edge at the margin | Mac | `table.rs` right/end alignment |
| Leading border and nested-table borders | inside the indent | `tblInd` measured to the border's outer edge; nested tables keep parent half-strokes outside cell margins | Mac (needs explicit 15) | `table.rs` `word_modern_nested_border_frame_enabled` |
| Paragraph before a square-wrapped float's anchor | wraps around the float (look-ahead) | no look-ahead | Mac | `canvas_renderer/float_lookahead.rs` |

Not mode-dependent in jade: contextual spacing, space-before/after
collapse, East Asian line height, autofit (beyond the Mac sizing width),
hyphenation (not implemented).

**Missing mode:** treated as modern for punctuation hanging, table inset
and float look-ahead, but *not* for the mode-15 border-frame rules (which
need an explicit `>= 15`). Word treats a missing mode as old, so jade's
"modern" choice for the first three is a known divergence to check against
a Word trace before changing.

Pinned by: `jade-docx-layout/src/line_wrap.rs` ::
`justified_paragraph_terminal_period_hangs_under_word_2013_layout` (modes
12/14/15 × Mac/Windows × left/justified),
`original_trust_notice_word_2013_left_lines_keep_punctuation_inside`,
`trust_article_14_2_paragraph_terminal_period_does_not_hang`;
`jade-docx-editor/src/canvas_renderer/table.rs` ::
`pre_2013_tables_align_first_cell_text_with_the_indent`,
`modern_mac_tables_put_the_leading_border_outside_the_indent`,
`mac_legacy_right_alignment_uses_the_trailing_text_edge`,
`repeated_nested_table_paints_at_the_editable_content_frame_origin`
(modes 12/14/15/none); `jade-docx-editor/src/canvas_renderer.rs` ::
`mode_14_wraps_the_paragraph_before_a_square_float_anchor`.
**No test parses `compatibilityMode` itself.**

## Named compatibility settings (`w:compatSetting`)

| Name | Word's meaning | jade |
|---|---|---|
| compatibilityMode | layout version (above) | read; used as above |
| overrideTableStyleFontSizeAndJustification | table style's font size and alignment override the Normal style (mode 14+) | ignored; kept |
| enableOpenTypeFeatures | apply OpenType ligatures/number forms set on runs | ignored; kept (ligature settings are not drawn) |
| doNotFlipMirrorIndents | mirror indents don't swap in right-to-left paragraphs | ignored; kept |
| differentiateMultirowTableHeaders | multi-row headers are separate header rows for accessibility | ignored; kept |
| useWord2013TrackBottomHyphenation | hyphenate at the bottom of a page/column as Word 2013 did | ignored (no hyphenation); kept |
| allowHyphenationAtTrackBottom | allow hyphenating the last line of a page/column | ignored; kept |
| allowTextAfterFloatingTableBreak | text may continue beside a floating table that breaks across pages | ignored; kept |

## Legacy compatibility options (`w:compat` children)

"Kept" means the element survives a save untouched; jade never changes
`w:compat` in an existing file.

| Element | Word's meaning (when present) | jade | Where / test |
|---|---|---|---|
| useFELayout | always run the East Asian layout path | **honoured**: East Asian line boxes from the face's 1.3× box on Windows (Mac always), Mac half-cell grid tracking, kinsoku/line-break rules in the editor, default `snapToGrid` for paragraphs | `jade-docx-layout/src/line_metrics.rs`, `line_wrap.rs`, `text_extract.rs`, `jade-docx-editor/src/canvas_renderer.rs`; `line_wrap.rs::compat_use_fe_layout_and_balance_flags_change_wrap_result` |
| balanceSingleByteDoubleByteWidth | widen single-byte characters to balance with double-byte ones | **honoured**: Windows widens SBCS glyphs (ratio 0.23), no numeric auto-space; Mac ½-em spaces next to CJK and ¼-em trailing auto-space | `line_metrics.rs`, `line_wrap.rs`, `constants.rs`; same test |
| noTabHangInd | no implicit tab stop at a hanging indent | **honoured** | `text_extract.rs`, `canvas_renderer.rs`; `settings.rs::parses_no_tab_hanging_indent_compatibility_switch` |
| adjustLineHeightInTable | apply the document grid's line pitch inside table cells | **honoured** (pitch dropped in cells without it) | `line_metrics.rs::doc_grid_for_table_cells`; `text_extract.rs::table_cell_lines_take_the_line_grid_only_with_adjust_line_height_in_table`, `canvas_renderer.rs::table_cell_lines_ignore_the_line_grid_unless_adjust_line_height_in_table` |
| characterSpacingControl | doNotCompress / compressPunctuation / compressPunctuationAndJapaneseKana | **honoured in part** — see [japanese-typesetting.md](japanese-typesetting.md) | `settings.rs::parses_character_spacing_control_outside_compat` |
| doNotExpandShiftReturn | don't stretch a justified line that ends in a manual line break | ignored; kept (whether jade stretches such lines is not traced) | — |
| spaceForUL | reserve space below the line for underlines | ignored; kept | — |
| ulTrailSpace | underline trailing spaces | ignored; kept | — |
| doNotLeaveBackslashAlone | show `\` as `¥` | ignored; kept | — |
| doNotUseHTMLParagraphAutoSpacing | fixed spacing for HTML auto-spacing | ignored; kept | — |
| doNotUseEastAsianBreakRules | no East Asian line-break rules | ignored; kept | — |
| useAsianBreakRules | Asian first/last-character rules | ignored; kept | — |
| doNotSnapToGridInCell | no grid snap in table cells | ignored; kept | — |
| doNotWrapTextWithPunct | no hanging punctuation with a character grid | ignored; kept | — |
| autoSpaceLikeWord95 | Word 95 full-width spacing | ignored; kept | — |
| noSpaceRaiseLower | raised/lowered text doesn't grow the line | ignored; kept | — |
| suppressTopSpacing, suppressSpBfAfterPgBrk, suppressBottomSpacing | drop extra spacing at page top / after a break / at bottom | ignored; kept | — |
| growAutofit | autofit tables may grow into the margins | ignored; kept | — |
| layoutRawTableWidth, layoutTableRowsApart, alignTablesRowByRow, doNotBreakConstrainedForcedTable, doNotVertAlignCellWithSp, useWord2002TableStyleRules | legacy table layout | ignored; kept | — |
| footnoteLayoutLikeWW8 | Word 6/97 footnote placement | ignored; kept | — |
| splitPgBreakAndParaMark | paragraph mark after a page break moves to the next page | ignored; kept | — |
| selectFldWithFirstOrLastChar | selecting a field's edge selects the field | ignored; kept | — |
| applyBreakingRules | legacy Ethiopic/Amharic breaking | ignored; kept | — |
| any other child (noLeading, spacingInWholePoints, doNotUseIndentAsNumberingTabStop, forgetLastTabAlignment, noExtraLineSpacing, wpJustification, shapeLayoutLikeWW8, useWord97LineBreakRules, …) | older emulations | ignored; kept | — |

Most common in the corpus besides `useFELayout` (×105):
doNotExpandShiftReturn ×43, spaceForUL/ulTrailSpace/doNotLeaveBackslashAlone/
balanceSingleByteDoubleByteWidth ×41, adjustLineHeightInTable ×38,
applyBreakingRules ×6. These are the Japanese-Word defaults, so
`spaceForUL` and `doNotExpandShiftReturn` are the next ones worth
checking against Word traces.

## Saving

- jade writes an existing package back part by part; `settings.xml` is
  rewritten only by commands that change a root-level setting (even/odd
  headers, track changes, gutter/mirror/book-fold, protection, Japanese
  typography), and those rewrites keep `w:compat` verbatim
  (`jade-docx-editor/src/commands/set_typography.rs` carries the mode over
  explicitly). `w:compat`'s schema position is checked by
  `jade-docx-editor/tests/word_schema.rs`.
- New documents (`crates/jade-office-model/src/word/new.rs`) are mode 15
  with no legacy options; a Japanese blank sets `compressPunctuation`, an
  English blank `doNotCompress`. No test checks the mode written.
- There is **no "Convert" / upgrade command** and no compatibility-mode
  indicator in either shell; the headless `set_document_setting` cannot
  touch `w:compat` children.

## Word for Mac vs Word for Windows

Word for Mac and Word for Windows read the same file and still break lines
differently. jade lays out like one or the other:

| Host | Platform | How chosen |
|---|---|---|
| Native app | Mac on macOS, Windows elsewhere | `native_word_layout_platform()` in `crates/jade-office-model/src/word/platform.rs`, set at start-up (`crates/jade-gpui-app/src/main.rs`) |
| Browser | the visitor's OS | `?word_platform=mac\|windows`, else `navigator.userAgentData.platform` / `navigator.platform` / user agent (`crates/jade-web/src/web.rs`; called from `crates/jade-gpui-web/src/lib.rs`) |
| Tools | flag | `word_glyph_sweep --platform mac`; `JADE_PERF_WORD_PLATFORM=mac` |

The choice is a thread-local read by `jade-docx-layout`
(`line_metrics.rs::set_word_layout_platform`); set it before the first
document opens — cached layouts are not redone.

What differs (as jade models it, from Word for Mac 16.11x and Word for
Windows traces):

| Area | Word for Mac | Word for Windows |
|---|---|---|
| Latin line height | hhea ascent + lineGap / descent per face (table of known faces) | OS/2 `usWin` ascent/descent + external leading |
| East Asian line height | always the face box ×1.3 | only with `useFELayout` |
| Auto-space widths (JA↔Latin, JA↔digits) | exact, from the Latin face's average width | rounded to whole 96-dpi pixels; no numeric auto-space under `balanceSingleByteDoubleByteWidth` |
| Superscript/subscript size | 0.65 (Latin) / 0.6 (Japanese), rounded to ½ pt | 0.66 |
| Punctuation compression (`compressPunctuation`) | squeezes justified lines only | −18 % on mid-line punctuation |
| Justification | spaces may shrink to 0.2 em; U+3000 not stretched | spaces shrink to 1/6 |
| Hanging punctuation in mode 15 | not on ragged lines | as `overflowPunct` says |
| Line grid with no `w:type` | inactive | active if `charSpace` is set |
| Grid line | text centred in the pitch; at-least lines round up to whole steps | — |
| Space before at a page/column top | dropped (except at a section start) | kept |
| Double borders | each rule and the gap equal `w:sz` | legacy rule geometry |
| Missing Japanese glyph in Noto Sans (lang ja) | falls back to MS Mincho | — |
| Tables | absolute widths may pass the margin; autofit shrinks to widest word; `tblpPr` before `tblStyle` makes the table inline | — |

Pinned by: `crates/jade-office-model/src/word/platform.rs::a_setting_names_the_platform_or_follows_the_browser`;
`jade-docx-layout/src/border_geometry.rs::mac_double_border_stripes_and_gap_share_the_authored_width`;
`jade-docx-layout/src/font_resolver.rs::mac_japanese_noto_sans_uses_word_cjk_fallback_without_changing_latin`;
the `mac_*` tests in `jade-docx-layout/src/line_wrap.rs` and
`line_metrics.rs`; the Mac glyph oracle in
[`docs/COMPATIBILITY.md`](../COMPATIBILITY.md).

## Gaps found

- Only `compatibilityMode` is read among `compatSetting`s; only five legacy
  options affect layout. `spaceForUL` and `doNotExpandShiftReturn` (present
  in a quarter of the corpus) are not modelled.
- Modes 11/12/14 are not told apart, and a missing mode is treated as
  modern in three of four rules.
- No parse test for `compatibilityMode`; no test that new documents are
  mode 15.
- No compatibility-mode indicator or Convert command (reference row 463).
- No hyphenation, so the hyphenation compat settings have nothing to
  change.
