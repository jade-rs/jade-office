# Word enumerations: value-by-value support

**Covers:** every value of the OOXML enumerations that decide how Word
characters, borders and drawings look — `ST_Border` (line and art borders),
`ST_Underline`, `ST_HighlightColor`, `ST_ShapeType` (preset geometry),
`ST_TextEffect`, `ST_Em`, `ST_NumberFormat` — plus SmartArt layout
categories and, briefly, `ST_Shd`. For each value: does jade **read** it into
the model, **draw** it distinctly, and let a user **edit** (set) it.

**Derived from:** jade-office `9df8a1e` / jade-core `9fea681` (origin/main,
2026-10-22), by reading the reader's parse functions, the renderer's match
arms and the shells' pickers. Paths below are relative to
`jade-core/crates/` unless they start with `crates/`. Nothing was run.

**Keeping it current:** each section names the function that decides the
answer. When one of those changes — a new match arm, a new UI list — update
its table in the same commit, and add the pinning test to the section's
"Pinned by" line. Value names are ECMA-376 Part 1 (§17.18 for
WordprocessingML, §20.1.10 for DrawingML).

Legend: **Y** yes · **≈** approximated · **→x** shown as x · **–** no.

## Where Word drawing happens

There is one Word renderer: `jade-docx-editor/src/canvas_renderer.rs`
(with `canvas_renderer/table.rs` and `draw_command/executor.rs`). The native
app reaches it through `jade-docx-native-render`, the browser through
`crates/jade-web`. **PDF export** from the native app uses that renderer too,
but **Print** (⌘P) goes through `jade-docx-pdf-renderer`, a separate and much
simpler renderer; its gaps are noted per section.

Editing UI lives in `crates/jade-office-model/src/word/*` (lists of
choices) and the two shells (`crates/jade-gpui-app`, `crates/jade-gpui-web`).
"API only" means the value can be set through the wasm editor API
(`jade-docx-editor/src/canvas_view.rs`) or the headless/MCP command set, but
no shell offers it.

## ST_Border — line styles

Read: `build_border_edge` (`jade-docx-reader/src/parser.rs`) keeps `w:val` as
a raw string on `BorderEdge.val`; every value survives a save. Draw:
`parse_border_kind` (`jade-docx-editor/src/canvas_renderer/table.rs:9216`)
maps the string to one of ten drawn kinds for paragraph, page, table-cell
and character borders. Character borders (`w:bdr`) are always drawn as a
solid box.

| Value | Read | Draw | Edit | Note |
|---|---|---|---|---|
| nil, none | Y | (no line) | Y | |
| single | Y | Y | Y | paragraph Borders button always writes black ¾ pt single |
| thick | Y | Y | Y table pen | |
| double | Y | Y | Y table pen, page borders | Mac and Windows rule spacing differ (`jade-docx-layout/src/border_geometry.rs`) |
| triple | Y | Y | Y table pen, page borders | |
| dotted | Y | Y | Y table pen, page borders | |
| dashed | Y | Y | Y table pen, page borders | |
| dashSmallGap | Y | Y | API only | |
| dotDash | Y | Y | Y table pen, page borders | |
| dotDotDash | Y | Y | API only | |
| dashDotStroked | Y | →dotDash | – | |
| wave | Y | ≈ evenly dashed, not a wave | Y table pen, page borders | |
| doubleWave | Y | →wave | – | |
| thinThickSmallGap | Y | →single | **Y page borders** | the dialog offers a style it cannot draw |
| thickThinSmallGap, thinThickThinSmallGap, thinThickMediumGap, thickThinMediumGap, thinThickThinMediumGap, thinThickLargeGap, thickThinLargeGap, thinThickThinLargeGap | Y | →single | – | |
| threeDEmboss, threeDEngrave, outset, inset | Y | →single | – | |

Pickers: table pen `Pen::STYLES` (`crates/jade-office-model/src/word/table.rs:226`):
single, dotted, dashed, dotDash, double, triple, thick, wave. Page borders
`PAGE_BORDER_STYLES` (`crates/jade-office-model/src/word/page_setup.rs:929`):
single, double, dotted, dashed, dotDash, triple, thinThickSmallGap, wave.
Engine validation `TableBorderSpec::STYLES` (`jade-docx-editor/src/word_document.rs:772`)
adds none, nil, dashSmallGap, dotDotDash.

Print (`jade-docx-pdf-renderer`) draws no border styles.

Pinned by: `jade-docx-editor/src/canvas_renderer/table.rs::parse_border_kind_recognizes_ooxml_styles`;
`jade-docx-reader/src/parser.rs::paragraph_border_edge_round_trips`,
`parses_page_borders_all_sides`.

## ST_Border — art borders

Word's page-border art (`w:pgBorders` with an art value) is read and kept
verbatim, then **drawn as a plain single line** (the `_ => Single` arm); the
art images and `w:frame`/`w:shadow` on them are not drawn, and no picker
offers them. This applies to every art value:

apples, archedScallops, babyPacifier, babyRattle, balloons3Colors,
balloonsHotAir, basicBlackDashes, basicBlackDots, basicBlackSquares,
basicThinLines, basicWhiteDashes, basicWhiteDots, basicWhiteSquares,
basicWideInline, basicWideMidline, basicWideOutline, bats, birds,
birdsFlight, cabins, cakeSlice, candyCorn, celticKnotwork,
certificateBanner, chainLink, champagneBottle, checkedBarBlack,
checkedBarColor, checkered, christmasTree, circlesLines, circlesRectangles,
classicalWave, clocks, compass, confetti, confettiGrays, confettiOutline,
confettiStreamers, confettiWhite, cornerTriangles, couponCutoutDashes,
couponCutoutDots, crazyMaze, creaturesButterfly, creaturesFish,
creaturesInsects, creaturesLadyBug, crossStitch, cup, decoArch,
decoArchColor, decoBlocks, diamondsGray, doubleD, doubleDiamonds, earth1,
earth2, eclipsingSquares1, eclipsingSquares2, eggsBlack, fans, film,
firecrackers, flowersBlockPrint, flowersDaisies, flowersModern1,
flowersModern2, flowersPansy, flowersRedRose, flowersRoses, flowersTeacup,
flowersTiny, gems, gingerbreadMan, gradient, handmade1, handmade2,
heartBalloon, heartGray, hearts, heebieJeebies, holly, houseFunky,
hypnotic, iceCreamCones, lightBulb, lightning1, lightning2, mapPins,
mapleLeaf, mapleMuffins, marquee, marqueeToothed, moons, mosaic,
musicNotes, northwest, ovals, packages, palmsBlack, palmsColor, paperClips,
papyrus, partyFavor, partyGlass, pencils, people, peopleWaving, peopleHats,
poinsettias, postageStamp, pumpkin1, pushPinNote2, pushPinNote1, pyramids,
pyramidsAbove, quadrants, rings, safari, sawtooth, sawtoothGray, scaredCat,
seattle, shadowedSquares, sharksTeeth, shorebirdTracks, skyrocket,
snowflakeFancy, snowflakes, sombrero, southwest, stars, starsTop, stars3d,
starsBlack, starsShadowed, sun, swirligig, tornPaper, tornPaperBlack,
trees, triangleParty, triangles, triangle1, triangle2, triangleCircle1,
triangleCircle2, shapes1, shapes2, twistedLines1, twistedLines2, vine,
waveline, weavingAngles, weavingBraid, weavingRibbon, weavingStrips,
whiteFlowers, woodwork, xIllusions, zanyTriangles, zigZag, zigZagStitch,
custom.

| | Read | Draw | Edit |
|---|---|---|---|
| all art values | Y (kept) | →single | – |

## ST_Underline (`w:u`)

Read: `parse_underline` (`jade-docx-reader/src/parser.rs`); single/`true`
becomes plain underline, any other token is kept as `underline_kind`; styles
inherit it. Draw: `stroke_underline_with_kind`,
`underline_dash_pattern`, `stroke_wave` (`jade-docx-editor/src/draw_command/executor.rs`);
*Heavy* kinds use twice the stroke; `words` skips spaces. Edit:
`UNDERLINE_STYLES` (`crates/jade-office-model/src/word/font.rs:31`, all 18)
in the ribbon's underline menu and the Font dialog of both shells, plus an
underline colour.

| Value | Read | Draw | Edit |
|---|---|---|---|
| single | Y | Y | Y |
| words | Y | Y | Y |
| double | Y | Y | Y |
| thick | Y | Y | Y |
| dotted, dottedHeavy | Y | Y | Y |
| dash, dashedHeavy | Y | Y | Y |
| dashLong, dashLongHeavy | Y | Y | Y |
| dotDash, dashDotHeavy | Y | Y | Y |
| dotDotDash, dashDotDotHeavy | Y | Y | Y |
| wave, wavyHeavy | Y | Y (zigzag) | Y |
| wavyDouble | Y | Y (two zigzags) | Y |
| none | Y | — | Y |

Print draws every kind as a single line. The wasm API
(`apply_underline_kind`) takes only single, words, double, thick, dotted,
dash and wave.

Pinned by: `jade-skia-painter/tests/renders_draw_commands.rs::dashed_underline_paints_dashes`;
`jade-docx-editor/src/commands/apply_underline.rs::applies_underline_to_selection`.

## ST_HighlightColor (`w:highlight`)

Read as a raw string; drawn by `word_highlight_to_css`
(`jade-docx-editor/src/canvas_renderer.rs`, case-sensitive); Print has the
same table, case-insensitive. Edit: `HIGHLIGHTS`
(`crates/jade-gpui-app/src/ribbon.rs:41`, same in `crates/jade-gpui-web/src/ribbon.rs`)
— 15 swatches plus "no colour".

| Value | Colour drawn | Read | Draw | Edit |
|---|---|---|---|---|
| black | #000000 | Y | Y | Y |
| blue | #0000FF | Y | Y | Y |
| cyan | #00FFFF | Y | Y | Y |
| green | #00FF00 | Y | Y | Y |
| magenta | #FF00FF | Y | Y | Y |
| red | #FF0000 | Y | Y | Y |
| yellow | #FFFF00 | Y | Y | Y |
| white | #FFFFFF | Y | Y | **–** (no swatch) |
| darkBlue | #000080 | Y | Y | Y |
| darkCyan | #008080 | Y | Y | Y |
| darkGreen | #008000 | Y | Y | Y |
| darkMagenta | #800080 | Y | Y | Y |
| darkRed | #800000 | Y | Y | Y |
| darkYellow | #808000 | Y | Y | Y |
| darkGray | #808080 | Y | Y | Y |
| lightGray | #C0C0C0 | Y | Y | Y |
| none | — | Y | — | Y |

Pinned by: `jade-docx-editor/src/commands/set_highlight_color.rs::highlight_command_updates_exported_document_xml`.

## ST_ShapeType — preset geometry

ECMA-376 defines 187 presets. jade carries **all 187** as guide formulas in
`jade-pptx-layout/src/preset_shapes.txt` (evaluated by
`jade-pptx-layout/src/preset_shapes.rs`), but **Word does not use that
table** — no Word crate depends on `jade-pptx-layout`. Word shapes instead
go through the `TextBoxShape` enum (`jade-docx-reader/src/document.rs:2975`,
`from_preset_name`), and each is traced by hand in `trace_text_box_shape`
(`jade-docx-editor/src/canvas_renderer.rs`) at its **default adjust values**:
`a:avLst` adjustments and `a:custGeom` are not read for Word.

Word for Mac and Windows share one preset set; the shells offer **no Insert
› Shapes command** (the wasm `insert_text_box` and headless `InsertShape`
accept every mapped preset). VML shapes (`v:shape` without a DrawingML
choice) are drawn as rectangles; `v:oval`, `v:roundrect`, `v:line` are not
read.

**Drawn in Word (112 presets)** — read Y, draw Y (default geometry), edit
API only:

arc, bentArrow, bentConnector2, bentConnector3, bentConnector4, bevel,
blockArc, can, chevron, chord, cloud, cloudCallout, corner, cube,
curvedConnector2, curvedConnector3, curvedConnector4, curvedRightArrow,
diagStripe, diamond, donut, doubleWave, downArrow, ellipse,
flowChartAlternateProcess, flowChartCollate, flowChartConnector,
flowChartDecision, flowChartDelay, flowChartDisplay, flowChartDocument,
flowChartExtract, flowChartInternalStorage, flowChartMagneticDisk,
flowChartMagneticDrum, flowChartManualInput, flowChartManualOperation,
flowChartMerge, flowChartMultidocument, flowChartOffpageConnector,
flowChartOr, flowChartPredefinedProcess, flowChartPreparation,
flowChartProcess, flowChartPunchedTape, flowChartSort,
flowChartSummingJunction, flowChartTerminator, foldedCorner, frame,
halfFrame, heart, hexagon, homePlate, irregularSeal1, irregularSeal2,
leftArrow, leftBrace, leftBracket, leftRightArrow, lightningBolt, line,
mathPlus, moon, noSmoking, notchedRightArrow, octagon, parallelogram,
pentagon, pie, plaque, plus, quadArrow, rect, ribbon, ribbon2, rightArrow,
rightBrace, rightBracket, round1Rect, round2DiagRect, round2SameRect,
roundRect, rtTriangle, smileyFace, snip1Rect, snip2DiagRect, snip2SameRect,
snipRoundRect, star4, star5, star6, star7, star8, star10, star12, star16,
star24, star32, straightConnector1, stripedRightArrow, sun, teardrop,
trapezoid, triangle, upArrow, upDownArrow, uturnArrow, wave,
wedgeEllipseCallout, wedgeRectCallout, wedgeRoundRectCallout.

(bentConnector2/4 draw as the bent connector, curvedConnector2/4 as the
curved one; straightConnector1 and line pick plain, one-arrow or two-arrow
from the line ends.)

**Drawn as a rectangle in Word (75 presets)** — read (kept on save), draw
→rect, edit –:

accentBorderCallout1–3, accentCallout1–3, actionButtonBackPrevious,
actionButtonBeginning, actionButtonBlank, actionButtonDocument,
actionButtonEnd, actionButtonForwardNext, actionButtonHelp,
actionButtonHome, actionButtonInformation, actionButtonMovie,
actionButtonReturn, actionButtonSound, bentConnector5, bentUpArrow,
borderCallout1–3, bracePair, bracketPair, callout1–3, chartPlus, chartStar,
chartX, circularArrow, cornerTabs, curvedConnector5, curvedDownArrow,
curvedLeftArrow, curvedUpArrow, decagon, dodecagon, downArrowCallout,
ellipseRibbon, ellipseRibbon2, flowChartInputOutput, flowChartMagneticTape,
flowChartOfflineStorage, flowChartOnlineStorage, flowChartPunchedCard,
funnel, gear6, gear9, heptagon, horizontalScroll, leftArrowCallout,
leftCircularArrow, leftRightArrowCallout, leftRightCircularArrow,
leftRightRibbon, leftRightUpArrow, leftUpArrow, lineInv, mathDivide,
mathEqual, mathMinus, mathMultiply, mathNotEqual, nonIsoscelesTrapezoid,
pieWedge, plaqueTabs, quadArrowCallout, rightArrowCallout, squareTabs,
swooshArrow, upArrowCallout, upDownArrowCallout, verticalScroll.

The cheapest fix for both lists is to route Word shapes through the shared
formula table, which would also bring adjust handles and `custGeom`.

Pinned by: `jade-docx-reader/src/parser.rs::recognizes_every_flowchart_preset_geometry`;
`jade-docx-reader/tests/generated_fixtures.rs::shape_preset_fixture_keeps_common_textless_and_text_shapes`.

## ST_TextEffect (`w:effect`, animated text)

Read by `normalize_text_effect` (`jade-docx-reader/src/parser.rs`) into
`RunProperties.effect`; the renderer never looks at it; only Clear
Formatting removes it. (Word 2007 and later no longer show these
animations either, so not drawing them matches current Word.)

| Value | Read | Draw | Edit |
|---|---|---|---|
| blinkBackground | Y | – | – |
| lights | Y | – | – |
| antsBlack | Y | – | – |
| antsRed | Y | – | – |
| shimmer | Y | – | – |
| sparkle | Y | – | – |
| none | Y | — | – |

The Font dialog's effects (outline, shadow, emboss, engrave, glow) are
separate properties (`w:outline`, `w:shadow`, `w:emboss`, `w:imprint`,
`w14:*`), not this enumeration.

Pinned by: `jade-docx-reader/src/parser.rs::parses_text_effect_run_property`.

## ST_Em (`w:em`, emphasis marks)

Read by `normalize_emphasis_mark`; drawn by `draw_emphasis_marks`
(`jade-docx-editor/src/canvas_renderer.rs`); edited from the Font dialog
(`EMPHASIS_MARKS`, `crates/jade-office-model/src/word/font.rs:53`). Print does
not draw them. Vertical text puts the marks to the right of the column.

| Value | Read | Draw | Edit | Note |
|---|---|---|---|---|
| dot | Y | Y | Y | filled dot above |
| comma | Y | ≈ | Y | drawn as a dot, not Word's comma (、) shape |
| circle | Y | Y | Y | open circle above |
| underDot | Y | Y | Y | filled dot below |
| none | Y | — | Y | |

Pinned by: `jade-docx-reader/src/parser.rs::parses_emphasis_mark_run_property`;
`jade-docx-editor/src/canvas_renderer.rs::emphasis_mark_center_offsets_skips_nonpositive_advances`.

## ST_NumberFormat (`w:numFmt`)

One formatter, `format_number_value` (`jade-docx-reader/src/numbering.rs`),
serves list labels, page numbers (`w:pgNumType`) and note marks
(`jade-docx-editor/src/number_format.rs`). Value names match
case-insensitively. Custom formats (`w:numFmt w:val="custom" w:format=…`)
are not read.

| Value | Read | Draw | Note |
|---|---|---|---|
| decimal, decimalHalfWidth | Y | Y | |
| decimalZero | Y | Y | 01–09 |
| decimalFullWidth, decimalFullWidth2 | Y | Y | |
| upperRoman, lowerRoman | Y | Y | |
| upperLetter, lowerLetter | Y | Y | AA, BB… after Z, as Word |
| ordinal, cardinalText, ordinalText | Y | ≈ | English list levels only; page and note numbers →decimal |
| hex | Y | Y | |
| chicago | Y | Y | \*, †, ‡, §, then doubled |
| numberInDash | Y | Y | - 1 - |
| decimalEnclosedCircle, decimalEnclosedFullstop, decimalEnclosedParen | Y | Y | to 20, then decimal |
| decimalEnclosedCircleChinese | Y | Y | to 10, then decimal |
| aiueo, iroha | Y | Y | half-width kana |
| aiueoFullWidth, irohaFullWidth | Y | Y | |
| ideographDigital, japaneseDigitalTenThousand, taiwaneseDigital | Y | Y | digit by digit (一〇) |
| japaneseCounting | Y | Y | 十一, 百五, 一万 |
| japaneseLegal | Y | Y | 壱, 弐, 参… |
| chineseCounting, taiwaneseCounting, taiwaneseCountingThousand | Y | Y | |
| chineseCountingThousand | Y | ≈ | uses the Japanese counting routine |
| chineseLegalSimplified, ideographLegalTraditional | Y | Y | |
| ideographEnclosedCircle | Y | Y | |
| ideographTraditional, ideographZodiac, ideographZodiacTraditional | Y | Y | stems, branches, 60-cycle |
| koreanDigital, koreanCounting, koreanLegal, ganada, chosung | Y | Y | |
| russianLower, russianUpper | Y | Y | |
| hebrew1, hebrew2, arabicAlpha, arabicAbjad | Y | Y | |
| hindiNumbers, thaiNumbers | Y | Y | native digits |
| bullet | Y | Y | the level's own text is used for bullets |
| none | Y | Y | empty |
| koreanDigital2, vietnameseCounting, hindiVowels, hindiConsonants, hindiCounting, thaiLetters, thaiCounting, bahtText, dollarText, custom | Y | **→decimal** | |

Edit: page numbers — `PAGE_NUMBER_FORMATS`
(`crates/jade-office-model/src/word/page_setup.rs:862`): decimal,
numberInDash, lower/upperLetter, lower/upperRoman, decimalFullWidth,
japaneseCounting. Lists — the shells only toggle the built-in bullet and
numbered presets; custom multilevel lists (API only) accept decimal,
bullet, lower/upperLetter, lower/upperRoman, chineseCountingThousand. Note
formats — API only (`set_note_numbering`).

Pinned by (all `jade-docx-reader/src/numbering.rs` unless noted):
`japanese_counting_uses_multiplicative_kanji_units`,
`japanese_legal_uses_financial_digits_and_units`,
`formats_decimal_full_width_numbers`, `letters_roll_over_by_repeating_like_word`,
`chicago_doubles_its_four_symbols`, `hebrew_and_arabic_formats`,
`english_ordinal_uses_language_aware_suffixes`,
`stems_branches_and_the_sexagenary_cycle`;
`jade-docx-editor/src/number_format.rs::page_numbers_and_note_marks_use_every_list_number_format`.

## SmartArt layout categories

jade has **no SmartArt layout engine**. A SmartArt graphic is drawn from the
drawing Word caches beside it (`word/diagrams/drawingN.xml`, `dsp:drawing`),
found by `WordDocument::resolve_diagrams`
(`jade-docx-editor/src/word_document.rs`) and parsed as an ordinary shape
group (`jade-docx-reader/src/parser/diagram.rs`). Its shapes are limited to
the Word preset list above. The layout definition, data model and colours
parts are kept unchanged on save but never interpreted, so the graphic
cannot be edited or re-laid-out; if the cached drawing is missing, a
placeholder is shown.

| Category (Word's gallery) | Read (kept) | Draw | Edit |
|---|---|---|---|
| List | Y | from cached drawing | – |
| Process | Y | from cached drawing | – |
| Cycle | Y | from cached drawing | – |
| Hierarchy | Y | from cached drawing | – |
| Relationship | Y | from cached drawing | – |
| Matrix | Y | from cached drawing | – |
| Pyramid | Y | from cached drawing | – |
| Picture | Y | from cached drawing (pictures as in the cache) | – |

Pinned by: `jade-docx-editor/src/word_document.rs::smartart_renders_the_cached_drawing_and_round_trips_its_parts`;
`jade-docx-reader/src/parser.rs::preserves_smartart_as_a_visible_placeholder`.

## ST_Shd (`w:shd w:val`), in brief

Read lowercased into `Shading.val`; drawn by `resolved_background_color`
(`jade-docx-reader/src/document.rs`) for paragraphs, runs and cells.

| Values | Draw |
|---|---|
| clear, nil | fill colour |
| solid | pattern colour (fill if none) |
| pct5 … pct95 (all percentages) | one flat blend of pattern over fill — no dot pattern |
| horzStripe, vertStripe, reverseDiagStripe, diagStripe, horzCross, diagCross and their thin* forms | **fill colour only**, pattern not drawn |

Edit: only solid fills, written as `w:val="clear"`.

## Gaps found

- Double-line border families (thin-thick etc.), 3-D, inset/outset and all
  art borders draw as a single line; the page-border dialog offers
  thinThickSmallGap, which it then draws as single.
- 75 of 187 preset shapes are rectangles in Word, and no Word shape honours
  its adjust values; there is no shape gallery in either shell.
- `comma` emphasis is drawn as a dot; Print draws no emphasis, no
  underline kinds and no border styles.
- White highlight cannot be chosen.
- Ten number formats fall back to decimal; custom number formats are not
  read.
- Shading patterns other than percentages are not drawn.
- SmartArt has no layout engine.
