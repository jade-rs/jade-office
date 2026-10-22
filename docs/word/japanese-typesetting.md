# Japanese typesetting options

**Covers:** each Japanese typesetting option Word offers (段落 › 体裁,
文字体裁, ページ設定 › 文字数と行数, 拡張書式, フォント), the OOXML element
that stores it, the JIS X 4051 rule it corresponds to where there is one,
where jade reads, lays out, draws and edits it, and the tests that pin it.

**Derived from:** jade-office `9df8a1e` / jade-core `9fea681` (origin/main,
2026-10-22), by reading the reader, `jade-docx-layout`, `jade-text-layout`,
the editor renderer and the shells. Element names are ECMA-376 Part 1
(§17.3.1 paragraph properties, §17.3.2 run properties, §17.3.3 ruby,
§17.6 sections, §17.15.1 settings). JIS X 4051 rules are named by topic and
character class (the class numbering W3C *Requirements for Japanese Text
Layout* uses). Nothing was run.

**What "source" means here:** Word does not implement JIS X 4051; its
rules were fitted by jade to Word's own output (Word for Mac 16.11x probes
and the fixtures under `jade-core/tests/word-oracle`), not derived from the
standard. **No jade code cites JIS X 4051.** The JIS column says where
Word's option touches a JIS rule, so a gap against the standard can be
told apart from a gap against Word.

**Keeping it current:** when a rule's code moves or a test is added, update
its row; when Word is re-traced (new probe folder under
`jade-core/tests/word-oracle/`), note the folder in the row. Paths are
under `jade-core/crates/` unless they start with `crates/`. Shorthands:
`LW` = `jade-docx-layout/src/line_wrap.rs`, `LM` =
`jade-docx-layout/src/line_metrics.rs`, `CR` =
`jade-docx-editor/src/canvas_renderer.rs`, `P` =
`jade-docx-reader/src/parser.rs`, `S` = `jade-docx-reader/src/settings.rs`.

## Summary

| Option (Word UI) | Element | Read | Laid out | Drawn | Editable | Tests |
|---|---|---|---|---|---|---|
| 禁則処理を行う | `w:kinsoku` (pPr, settings) | Y | Y | — | Y (体裁 / 文字体裁) | Y |
| 禁則文字 標準 / 高レベル / ユーザー設定 | `w:strictFirstAndLastChars`, `w:noLineBreaksBefore/After` | Y | Y (sets differ, below) | — | Y | Y |
| 句読点のぶら下げを行う | `w:overflowPunct` | Y | Y | — | Y | Y |
| 行頭の記号を 1/2 の幅にする | `w:topLinePunct` | Y | Y (opening brackets) | — | Y | Y |
| 文字間隔の調整 (間隔を詰めない / 句読点のみ / 句読点とかな) | `w:characterSpacingControl` | Y | ≈ (kana by a fixed tenth) | — | Y | Y |
| 日本語と英字の間隔を自動調整 | `w:autoSpaceDE` | Y | Y | — | Y | Y |
| 日本語と数字の間隔を自動調整 | `w:autoSpaceDN` | Y | Y | — | Y | Y |
| 英単語の途中で改行する | `w:wordWrap` | Y | Y | — | Y | Y |
| 文字数と行数 (グリッド) | `w:docGrid` | Y | Y | Y | Y | Y |
| グリッドに合わせる | `w:snapToGrid` (pPr; rPr not modelled) | ≈ | Y | — | Y | Y |
| 均等割り付け (段落) | `w:jc="distribute"` | Y | Y | Y | **–** | Y |
| 均等割り付け (文字列) | `w:fitText` | Y | Y | Y | **–** | Y |
| ルビ | `w:ruby` | ≈ (`hpsBaseText`, `lid` ignored) | Y | Y | Y | Y |
| 傍点 | `w:em` | Y | — (no line growth) | ≈ | Y | Y |
| 縦書き | `w:textDirection` (section, cell) | ≈ (no pPr, text box) | Y | Y | Y | Y |
| 縦中横 | `w:eastAsianLayout w:vert` | Y | Y | Y | Y | Y |
| 組み文字 | `w:eastAsianLayout w:combine` | Y | Y | Y | Y | Y |
| 割注 | `w:combine` + `w:combineBrackets` | Y | Y (one line only) | Y (one line only) | Y | Y |
| 囲い文字 | `EQ \o\ac(○,字)` field | Y | — | Y (one style) | Y | Y |
| 右インデント幅を自動調整 | `w:adjustRightInd` | Y | **–** | — | – | parse only |
| 文字の配置 (上 / 中央 / ベースライン / 下 / 自動) | `w:textAlignment` | Y | ≈ (spare leading only) | — | – | Y |
| 文字幅 / 文字間隔 / カーニング | `w:w`, `w:spacing`, `w:kern` | Y | Y | Y | Y | Y |

## Option by option

### 禁則処理 — line-start and line-end prohibition

- **Element:** `w:kinsoku` (paragraph, inherited from styles and settings);
  `w:strictFirstAndLastChars`, `w:noLineBreaksBefore`, `w:noLineBreaksAfter`
  with `w:lang` (settings).
- **JIS X 4051:** 行頭禁則 (closing brackets cl-02, hyphens cl-03,
  dividing punctuation cl-04, middle dots cl-05, full stops cl-06, commas
  cl-07, iteration marks cl-09, prolonged sound mark cl-10, small kana
  cl-11), 行末禁則 (opening brackets cl-01), 分割禁止 (cl-08 inseparable
  characters such as ——, ……).
- **jade:** read `P` (`apply_paragraph_line_break_option`), `S`; model
  `EastAsianLineBreakingSettings` (`jade-docx-reader/src/document.rs`).
  Layout `LW` `kinsoku_enabled`, `enforce_line_start_rules`,
  `forbids_line_start/end`, `hang_single_forbidden_line_start_cluster`.
  Excel and PowerPoint use `jade-text-layout/src/kinsoku.rs` +
  `wrap.rs`. Edit: `jade-docx-editor/src/commands/set_typography.rs`
  (paragraph and document levels standard / strict / custom);
  dialog `crates/jade-gpui-app/src/word_characters/typography.rs` (same in
  `crates/jade-gpui-web`); model `crates/jade-office-model/src/word/typography.rs`.
- **Default sets:** exactly Word's lists, in
  `jade-docx-layout/src/word_kinsoku.rs`, which the 体裁 dialog
  (`crates/jade-office-model/src/word/typography.rs`) shows too:

  | Set | Characters |
  |---|---|
  | 行頭禁則文字 標準 | `!%),.:;?]}¢°’”‰′″℃、。々〉》」』】〕゛゜ゝゞ・ヽヾ！％），．：；？］｝｡｣､･ﾞﾟ￠` |
  | 高レベル adds | `ぁぃぅぇぉっゃゅょゎゕゖァィゥェォッャュョヮヵヶーｧｨｩｪｫｬｭｮｯｰ` |
  | 行末禁則文字 (both) | `$([\{£¥‘“〈《「『【〔＄（［｛｢￡￥` |

  A `ja` custom set replaces the default; custom sets for other languages
  are merged into it regardless of the run's language. There are no
  Chinese or Korean default sets. Excel and PowerPoint use
  `jade-text-layout/src/kinsoku.rs` and always behave as strict.
- **分割禁止:** Word does not break between two inseparable characters
  (`—`, `―`, `…`, `‥`, `〳`–`〵`), so `――` and `……` stay on one line
  (`forbids_break_between`, `LW`).
- **Pinned by:** `jade-docx-layout/src/word_kinsoku.rs` ::
  `strict_is_standard_plus_the_small_kana`; `LW` ::
  `kinsoku_uses_words_own_standard_and_strict_lists`,
  `a_two_em_dash_or_leader_is_not_split_across_lines`;
  `jade-text-layout/src/kinsoku.rs` ::
  `default_tables_cover_the_ecma_japanese_standard_sets`,
  `default_tables_keep_ascii_word_punctuation_breakable`;
  `jade-text-layout/tests/wraps_lines.rs` ::
  `a_closing_bracket_does_not_start_a_line`,
  `an_opening_bracket_does_not_end_a_line`,
  `turning_kinsoku_off_breaks_wherever_it_fits`; `LW` ::
  `kinsoku_off_does_not_move_forbidden_start_character`,
  `japanese_custom_kinsoku_sets_replace_the_default_tables`,
  `small_kana_and_choon_start_a_line_unless_kinsoku_is_strict`; `S` ::
  `parses_no_line_breaks_plural_tags_and_splits_entries`;
  `jade-docx-editor/tests/typography.rs` ::
  `the_document_kinsoku_level_and_compression_go_into_settings`,
  `the_paragraph_options_are_written_read_back_and_undone`.

### ぶら下げ — hanging punctuation

- **Element:** `w:overflowPunct` (omitted = on).
- **JIS X 4051:** ぶら下げ組 of full stops (cl-06) and commas (cl-07) past
  the line end.
- **jade:** `LW` `allows_hanging_punctuation`, `allows_hanging_punctuation_at`,
  `paragraph_end_punctuation_can_hang`; per-language tables (ja, zh-Hans,
  zh-Hant, ko). Depends on compatibility mode and platform — see
  [compatibility-modes.md](compatibility-modes.md): in mode 15 Word for Mac
  keeps punctuation inside ragged lines, and a paragraph-final 。 hangs only
  in justified paragraphs or modes below 15.
- **Pinned by:** `LW` :: `hanging_punctuation_keeps_kuten_on_same_line`,
  `hanging_punctuation_does_not_overflow_multiple_characters`,
  `overflow_punctuation_off_keeps_forbidden_start_on_next_line`,
  `omitted_overflow_punctuation_defaults_to_hanging`,
  `justified_paragraph_terminal_period_hangs_under_word_2013_layout`,
  `mac_paragraph_end_period_hangs_whole_when_half_is_not_enough`.

### 行頭の記号を 1/2 の幅にする

- **Element:** `w:topLinePunct`.
- **JIS X 4051:** 行頭の始め括弧類 (cl-01) set solid / half-width at the
  line start.
- **jade:** `LW` `top_line_punctuation_adjustment` (−50 % advance at line
  start) on `is_top_line_half_width_target`: the opening brackets
  `「『（［｛〈《【〔〖〘‘“` — under kinsoku the only marks that can start a
  line — and the closing marks, which reach a line start only with kinsoku
  off. Not yet compared with a Word trace.
- **Pinned by:** `LW` :: `top_line_punctuation_halves_an_opening_bracket_at_a_line_start`,
  `top_line_punctuation_half_width_allows_more_glyphs_on_line_start`; `CR` ::
  `top_line_punctuation_adjustment_applies_when_enabled`,
  `top_line_punctuation_adjustment_is_zero_when_disabled`.

### 文字間隔の調整 — punctuation and kana compression

- **Element:** `w:characterSpacingControl` = `doNotCompress` |
  `compressPunctuation` | `compressPunctuationAndJapaneseKana` (settings,
  inside or outside `w:compat`).
- **JIS X 4051:** 約物の詰め (half-width brackets and punctuation) in line
  adjustment (追い込み).
- **jade:** `compressPunctuation`: Word for Windows compresses mid-line
  punctuation by a fixed 18 % (`LW` `punctuation_compression_adjustment`,
  gated in `LM`); Word for Mac squeezes only justified lines
  (`LW` `compress_mac_punctuation_on_completed_line`).
  `compressPunctuationAndJapaneseKana` compresses punctuation the same way
  and kana (hiragana, katakana) by a tenth of their cell on Windows, standing
  in for the face's proportional kana widths; Word for Mac squeezes its
  justified lines' punctuation as for `compressPunctuation`. There is no JIS-style
  追い込み/追い出し priority order; line end is squeeze plus single-mark
  hanging.
- **Pinned by:** `S` :: `parses_east_asian_line_break_settings`,
  `parses_character_spacing_control_outside_compat`; `LW` ::
  `punctuation_compression_adjustment_applies_for_compress_punctuation_setting`,
  `mac_squeeze_admission_needs_justified_compress_punctuation`,
  `compressing_kana_narrows_kana_and_punctuation_on_windows`,
  `do_not_compress_line_squeezes_auto_spacing_before_hanging_its_final_mark`.

### 和欧間・和数間の自動調整

- **Element:** `w:autoSpaceDE` (Japanese ↔ Latin), `w:autoSpaceDN`
  (Japanese ↔ digits); omitted = on.
- **JIS X 4051:** 和欧文間の空き (quarter-em between Japanese and Western
  text, cl-25 / cl-23 against cl-15–19).
- **jade:** `LW` `implicit_latin_east_asian_gap_width`,
  `implicit_numeric_east_asian_gap_width`, `drop_line_start_auto_space`,
  `east_asian_justification_gap`. Widths differ by platform (Mac: from the
  Latin face's average width, exact; Windows: whole pixels) and by
  `balanceSingleByteDoubleByteWidth` (Windows: no numeric gap). The gap is
  dropped at a line start and not added after Japanese punctuation.
- **Pinned by:** `LW` ::
  `implicit_latin_gap_defaults_to_enabled_when_auto_space_de_missing`,
  `implicit_latin_gap_respects_auto_space_de_disabled`,
  `implicit_numeric_gap_respects_auto_space_dn_disabled`,
  `wrapped_line_starting_with_latin_drops_the_auto_space_before_it`,
  `no_auto_space_after_east_asian_punctuation`,
  `word_for_mac_keeps_numeric_auto_spacing_for_default_font_hint`; `P` ::
  `parses_paragraph_auto_space_de_dn_flags`.

### 英単語の途中で改行する

- **Element:** `w:wordWrap` (omitted = Latin words may break).
- **JIS X 4051:** 欧文の分割 (hyphenation of Western words) — not
  implemented; jade only chooses between breaking anywhere and at spaces.
- **jade:** `LW` `allows_mid_word_break`, `prevent_mid_word_breaks`.
- **Pinned by:** `LW` :: `word_wrap_off_avoids_breaking_inside_ascii_word`,
  `word_wrap_missing_defaults_to_allow_mid_word_break_for_east_asian_layout`;
  `jade-text-layout/tests/wraps_lines.rs` ::
  `allowing_mid_word_breaks_fills_the_line`.

### 文字数と行数 — document grid

- **Element:** `w:sectPr/w:docGrid` (`w:type` = `lines` | `linesAndChars` |
  `snapToChars`, `w:linePitch`, `w:charSpace`).
- **JIS X 4051:** 基本版面 (characters per line × lines per page, 字送り,
  行送り).
- **jade:** read `P` `parse_doc_grid_attributes`; line grid `LM`
  `doc_grid_has_active_line_grid` (Mac: a missing `w:type` is inactive),
  `resolve_line_height_points`, tiny-pitch fallback (body height ÷ 30.5);
  character grid `LW` `apply_doc_grid_character_settings`
  (`linesAndChars` adds `charSpace` to each advance),
  `resolve_snap_to_chars_cell_width`. In cells the line pitch applies only
  with `adjustLineHeightInTable`. Edit: Page Setup › 文字数と行数
  (`crates/jade-office-model/src/word/page_setup.rs`,
  `crates/jade-gpui-app/src/word_page_setup.rs`), written by
  `set_section_doc_grid`. **Gap:** `snapToChars` cell width comes from the
  run's own East Asian em, not the docDefaults pitch Word uses.
- **Pinned by:** `P` :: `parses_doc_grid`; `LM` ::
  `default_doc_grid_type_is_inactive`,
  `mac_missing_doc_grid_type_does_not_activate_saved_grid_settings`,
  `exact_spacing_overrides_doc_grid_pitch`; `LW` ::
  `doc_grid_modes_lines_lines_and_chars_and_snap_to_chars_diverge`,
  `snap_to_chars_applies_doc_grid_char_space_to_cell_width`;
  `crates/jade-office-model/src/word/page_setup.rs` ::
  `chars_and_lines_set_the_pitches`.

### グリッドに合わせる

- **Element:** `w:pPr/w:snapToGrid` (default from `useFELayout`);
  `w:rPr/w:snapToGrid` is kept on save but **not modelled**.
- **jade:** `LM` `resolve_effective_snap_to_grid`,
  `word_grid_line_centres_text`. Edit: Layout › Snap to Grid.
- **Pinned by:** `LM` :: `effective_snap_to_grid_defaults_on_when_grid_exists`;
  `jade-docx-editor/tests/section_layout_and_stories.rs` ::
  `paragraphs_leave_and_rejoin_the_grid`;
  `crates/jade-web/tests/section_setup_documents.rs` :: `a_paragraph_leaves_the_grid`.

### 均等割り付け

- **Paragraph:** `w:jc w:val="distribute"` — every line, including the
  last, is spread to the full width (a lone character is centred): `LW`
  `distribute_characters_to_width`. **No UI sets it**, and the Paragraph
  dialog shows it as 両端揃え (`crates/jade-office-model/src/word/paragraph.rs`).
- **Characters:** `w:rPr/w:fitText` (`w:val` width, `w:id` group) and cell
  `w:tcFitText`: `LW` `fit_text_group_adjustment` (one width per `w:id`
  group). No UI.
- **Legacy form:** `EQ \o\ad(text,guide)`, drawn by
  `CR` `apply_eq_distributed_text_scale`.
- **JIS X 4051:** 均等割り (spreading a heading over a set width).
- **Pinned by:** `LW` :: `distributed_final_line_expands_character_gaps_across_runs`,
  `distributed_line_of_one_character_centres_it`,
  `fit_text_id_shares_one_target_width_across_formatting_runs`; `P` ::
  `parses_eq_overstrike_distributed_text_field`.

### ルビ

- **Element:** `w:ruby` / `w:rubyPr` (`w:rubyAlign`, `w:hps`, `w:hpsRaise`,
  `w:hpsBaseText`, `w:lid`), `w:rt`, `w:rubyBase`; legacy `EQ \o\aX(\s\up…)`.
- **JIS X 4051:** ルビ — モノルビ / グループルビ placement, overhang onto
  neighbouring kana, ruby wider than its base.
- **jade:** read `P` `parse_ruby_properties` (reads `hps`, `rubyAlign`,
  `hpsRaise`; **`hpsBaseText` and `lid` ignored**). Layout
  `jade-docx-layout/src/ruby.rs` (`ruby_distribution` for each
  `rubyAlign`; `rightVertical` falls back to centre; odd half-point raises
  truncated), `LW` `ruby_group_adjustment` (a wider ruby widens its base).
  Drawn `CR` `draw_ruby_text`. Edit: Phonetic Guide dialog
  (`crates/jade-gpui-app/src/word_characters/ruby.rs`, both shells),
  per character or grouped. No overhang onto neighbours; ruby in vertical
  text is laid out as centred.
- **Pinned by:** `jade-docx-layout/src/ruby.rs` ::
  `distribution_matches_word_for_every_alignment`; `LW` ::
  `ruby_widens_its_base_and_raises_the_line_ascent`,
  `ruby_base_spreads_across_formatting_runs`; `P` ::
  `parses_ruby_raise_and_guide_fonts`;
  `jade-docx-editor/tests/ruby.rs` :: `a_ruby_is_put_on_changed_and_taken_off`;
  `crates/jade-office-model/src/word/asian.rs` :: `bases_split_per_character_and_join_whole`.

### 傍点 — emphasis marks

- **Element:** `w:em` (`dot`, `comma`, `circle`, `underDot`).
- **JIS X 4051:** 圏点 (centred over each character, on the right in
  vertical text).
- **jade:** drawn by `CR` `draw_emphasis_marks`, centred per character;
  `comma` is drawn as a dot. Layout does not grow the line for them. Set
  in the Font dialog. Print (`jade-docx-pdf-renderer`) does not draw them.
- **Pinned by:** `P` :: `parses_emphasis_mark_run_property`; `CR` ::
  `parse_emphasis_mark_maps_known_tokens`,
  `emphasis_mark_center_offsets_centers_non_blank_glyphs`.

### 縦書き

- **Element:** `w:textDirection` on the section and on table cells
  (`lrTb`, `tbRl`, `btLr`, `lrTbV`, `tbRlV`, `tbLrV`).
- **JIS X 4051:** 縦組 (vertical composition, rotated Latin, vertical
  punctuation forms).
- **jade:** `LW` `layout_text_vertically`, `layout_section_text_vertically`,
  `layout_cell_text_vertically`; drawn by `CR` and
  `canvas_renderer/table.rs` `render_vertical_paragraph`. Edit: Layout ›
  Text Direction (turns the page) and the cell text-direction cycle.
  `tbLrV` stacks glyphs like `tbRlV` with its lines running left to right.
  **Not supported:** paragraph-level `w:textDirection`; vertical
  text boxes (`bodyPr@vert`); kinsoku and hanging between columns; vertical
  glyph substitution (`vert`/`vrt2`).
- **Pinned by:** `P` :: `parses_section_text_direction`; `LW` ::
  `vertical_layout_groups_lines_into_columns`,
  `vertical_columns_fit_the_advances_used_by_painting`;
  `canvas_renderer/table.rs` ::
  `rotated_table_text_baseline_follows_text_direction_axis`;
  `jade-docx-layout/src/text_extract.rs` ::
  `a_tb_lr_v_cell_stacks_glyphs_down_and_columns_left_to_right`;
  `crates/jade-web/tests/section_setup_documents.rs` :: `vertical_text_turns_the_page`.

### 縦中横

- **Element:** `w:eastAsianLayout w:vert="1"` (`w:vertCompress` to fit the
  line), grouped by `w:id`.
- **JIS X 4051:** 縦中横 (upright horizontal run in vertical text).
- **jade:** `LW` (the run takes one upright cell in vertical layout); drawn
  by `canvas_renderer/table.rs` `paragraph_line_tate_chu_yoko`; set from
  Home › Asian Layout (`crates/jade-gpui-app/src/word_characters/ribbon.rs`).
- **Pinned by:** `CR` :: `tate_chu_yoko_paints_its_run_as_one_upright_group`;
  `jade-docx-editor/tests/run_format.rs` ::
  `the_font_dialog_writes_every_property_and_reads_back`.

### 組み文字・割注

- **Element:** `w:eastAsianLayout w:combine="1"` (組み文字) and with
  `w:combineBrackets` (割注: none, round, square, angle, curly).
- **JIS X 4051:** 割注 (two-line inline note, which may run across lines).
- **jade:** `jade-docx-layout/src/combined_characters.rs`
  `combined_run_properties` (two half-size lines, scaled to the wider half
  plus full-size brackets) gives the run its width in both the headless
  layout and the renderer, which draws it with `draw_combined_characters`;
  割注 cannot continue onto the next line. The `EQ \o(\s\up…,\s\do…)` form Word may write for
  組み文字 is not recognised. Set from Home › Asian Layout.
- **Pinned by:** `CR` :: `combined_characters_stack_their_halves`;
  `jade-docx-layout/src/text_extract.rs` ::
  `combined_characters_take_their_wider_half_and_brackets`.

### 囲い文字

- **Element:** field `EQ \o\ac(○,字)` (enclosures ○◯〇□△◇).
- **jade:** `P` `parse_eq_enclosed_field`; drawn by
  `jade-docx-editor/src/draw_command/executor.rs` `draw_enclosed_character`
  with the character shrunk to ⅔; inserted from the Enclose Characters
  dialog. Word's two styles (文字のサイズを合わせる / 外枠のサイズを合わせる)
  are not offered — one fixed style.
- **Pinned by:** `P` :: `eq_overstrike_of_an_enclosure_is_an_enclosed_character`;
  `jade-docx-editor/tests/date_time.rs` ::
  `an_enclosed_character_is_written_as_word_writes_it`.

### 右インデント幅を自動調整 / 文字の配置

- `w:adjustRightInd` (adjust the right indent to the grid): read, inherited
  and written back, **no layout effect**. Pinned by `P` ::
  `parses_adjust_right_indent_with_explicit_off_and_inherited_absence`.
- `w:textAlignment` (top / center / baseline / bottom / auto): only moves
  the line's spare leading (`CR` `text_alignment_top_leading_share`); runs
  of different sizes in one line are not aligned to each other. No UI.
  Pinned by `CR` :: `text_alignment_top_leading_share_maps_tokens`.

### 文字幅・文字間隔・カーニング

- **Element:** `w:w` (scale %), `w:spacing` (twips), `w:kern` (threshold).
- **JIS X 4051:** 長体・平体 (scaled faces), 字間 (tracking).
- **jade:** `jade-docx-layout/src/font_resolver.rs` (scale clamped to
  1–1000 %, pair kerning above the threshold), `LW`; set from the Font
  dialog and Home › Asian Layout › Character Scale.
- **Pinned by:** `jade-docx-layout/src/font_resolver.rs` ::
  `resolve_run_style_applies_text_scale_and_kerning_threshold`; `LW` ::
  `wrap_text_applies_text_scale_x`, `wrap_text_applies_pair_kerning_when_enabled`.

## Gaps found

1. Kinsoku: no zh/ko default sets.
2. `compressPunctuationAndJapaneseKana` compresses kana by a fixed tenth,
   not by the face's proportional widths.
3. `topLinePunct`'s target set has not been compared with a Word trace.
4. Vertical text: no paragraph or text-box vertical text, no
   kinsoku/hanging between columns, no vertical glyph forms.
5. 割注 cannot wrap.
6. Ruby ignores `hpsBaseText`/`lid` on read; no overhang; vertical ruby
   centred.
7. `w:jc="distribute"` and `w:fitText` cannot be set from the UI.
8. `adjustRightInd` has no effect; `textAlignment` only moves spare
   leading; run-level `snapToGrid` is not modelled.
9. 囲い文字 has one style; `comma` emphasis is drawn as a dot.
