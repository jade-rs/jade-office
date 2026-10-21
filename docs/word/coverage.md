# Word feature coverage map

**Covers:** for every row of the Word part of the reference feature list
(rows 1–506, the yardstick described in [`docs/PARITY.md`](../PARITY.md)),
the automated test that exercises it in jade, or **none** where no test does.
Row labels are jade's own short names; the row number is the key into the
reference list.

**Derived from:** jade-office `9df8a1e` with jade-core `9fea681` (origin/main,
2026-10-22). Every `#[test]` function under `crates/*/{src,tests}` and
`jade-core/crates/*/{src,tests}` was listed (4,109 outside the Excel and
PowerPoint crates), matched against each row by keyword, and the matches
were then read and corrected by hand. Where several tests match, at most two
are shown, scenario tests (`crates/jade-web/tests`, `crates/jade-office-model`)
first, then integration tests, then unit tests.

Paths: `jade-web/…`, `jade-office-model/…`, `jade-gpui-app/…` are under
`crates/`; `core/…` and the other `jade-…` crates are under
`jade-core/crates/`.

## Reading the map

- A test listed against a row shows the row is *exercised*, not that every
  aspect of it is: "lists" may be covered by a parse test while the dialog
  that makes one is not.
- **none** means no test names the behaviour. It does not mean the feature
  is absent; see the row's status in the reference list and
  [`docs/GAPS.md`](../GAPS.md).
- Rows 402–410 are documentation rows; they point at the file in this
  directory that answers them.

## Summary

| | rows |
|---|---|
| with at least one test | 316 |
| without a test | 190 |
| — of which jade marks *full* | 45 |
| — of which *partial* / *render* / *roundtrip* | 62 |
| — of which *missing* (nothing to test yet) | 83 |

Rows jade counts as full but that no test pins — the first ones to add tests
for: 11, 54, 59, 61, 63, 67, 70, 73, 74, 79, 80, 82, 83, 103, 110, 113, 140,
210, 211, 213, 225, 345, 347, 349, 350, 353, 360, 366, 367, 370, 371, 372,
379, 382, 389, 391, 396, 398, 399, 400, 444, 445, 470, 473, 483. Most are
dialogs (style, list, template) and document-property fields whose
evaluation shares one code path with fields that are tested
(DOCPROPERTY, INFO).

## Keeping it current

- When a change adds or renames a test for a row, edit that row here in the
  same commit. A feature stream that turns a **none** into a test should name
  the test in its PR.
- To re-derive the candidate list:

  ```sh
  grep -rn -A3 '#\[test\]' --include=*.rs crates jade-core/crates \
    | grep -E 'fn [a-zA-Z0-9_]+' \
    | sed -E 's/^([^ ]+\.rs)[-:][0-9]+[-:][[:space:]]*(pub )?(async )?fn ([a-zA-Z0-9_]+).*/\1::\4/' \
    | grep -vE 'xlsx|pptx|sheet|deck|excel' | sort -u
  ```

  then search it for the row's subject (for example `grep -i ruby`) and
  check the test body before listing it.
- Update the "Derived from" commits and the summary counts.

## Map

| Row | Area (own label) | Covering test(s) |
|---|---|---|
| 1 | typing, Backspace, Delete | `jade-web/tests/new_documents.rs::a_blank_document_opens_and_takes_typing` <br> `jade-web/tests/edits_documents.rs::typing_deleting_and_undo_edit_the_text_and_repaint_the_page` |
| 2 | IME composition | `jade-web/tests/edits_documents.rs::a_composition_replaces_itself_until_committed` <br> `jade-docx-native-render/tests/composition.rs::a_paragraph_reads_back_as_text` |
| 3 | select word / paragraph / all | `jade-web/tests/edits_documents.rs::a_double_click_selects_a_word_and_a_triple_click_the_paragraph` <br> `jade-web/tests/edits_documents.rs::select_all_takes_every_paragraph` |
| 4 | multi-range selection | **none** |
| 5 | Enter splits, joins paragraphs | `jade-web/tests/edits_documents.rs::enter_splits_the_paragraph_and_the_caret_follows` <br> `jade-docx-editor/tests/word_schema.rs::joining_paragraphs_moves_a_floating_shape_as_authored` |
| 6 | Shift+Enter, Tab | `jade-web/tests/edits_documents.rs::shift_enter_breaks_the_line_inside_the_paragraph` <br> `jade-web/tests/edits_documents.rs::tab_goes_from_cell_to_cell_and_adds_a_row_at_the_end` |
| 7 | cut / copy | `jade-web/tests/clipboard_documents.rs::a_paragraph_copied_with_its_mark_keeps_its_style` <br> `jade-web/tests/edits_documents.rs::a_selection_is_highlighted_copied_and_replaced_by_typing` |
| 8 | paste, source formatting | `jade-web/tests/clipboard_documents.rs::a_paste_keeps_the_runs_formatting` |
| 9 | paste, merge formatting | `jade-office-model/src/word/clipboard.rs::merging_keeps_emphasis_and_text_only_keeps_nothing` |
| 10 | paste as text | `jade-office-model/src/word/clipboard.rs::merging_keeps_emphasis_and_text_only_keeps_nothing` <br> `jade-office-model/src/word/clipboard.rs::the_plain_text_joins_paragraphs_with_newlines` |
| 11 | Paste Special | **none** |
| 12 | Format Painter | `jade-web/tests/painter_documents.rs::the_format_at_a_caret_paints_characters_and_paragraph` <br> `jade-web/tests/painter_documents.rs::a_selection_inside_a_paragraph_paints_only_characters` |
| 13 | undo / redo | `jade-web/tests/finds_documents.rs::replace_all_is_one_undo_step` <br> `jade-web/tests/edits_documents.rs::a_long_paste_undoes_and_redoes_whole` |
| 14 | symbols | `core/jade-docx-reader/src/parser.rs::parses_symbol_char_into_run_text_and_font` <br> `jade-office-model/src/word/insert.rs::pictures_and_codes` |
| 15 | date & time insert | `jade-docx-editor/tests/date_time.rs::a_date_field_keeps_its_picture` <br> `jade-docx-reader/src/parser.rs::parses_date_field_with_picture_switch` |
| 16 | Latin / East Asian / CS fonts | `jade-gpui-app/src/word_characters.rs::complex_script_text_has_a_fallback_face` <br> `jade-docx-layout/src/line_metrics.rs::word_east_asian_font_line_height_follows_the_requested_face` |
| 17 | font size, grow / shrink | `core/jade-docx-editor/src/commands/adjust_font_size.rs::grows_each_run_relative_to_its_own_size` <br> `core/jade-docx-editor/src/commands/set_font_size.rs::exports_font_sizes_up_to_words_1638pt_limit` |
| 18 | bold, italic | `jade-web/tests/formats_documents.rs::bold_toggles_on_the_selection` <br> `jade-docx-native-render/tests/formatting_and_clipboard.rs::bolding_a_selection_changes_the_page` |
| 19 | underline styles & colour | `jade-skia-painter/tests/renders_draw_commands.rs::dashed_underline_paints_dashes` <br> `jade-docx-editor/src/commands/apply_underline.rs::applies_underline_to_selection` |
| 20 | strike, double strike | `core/jade-docx-editor/src/commands/apply_strike.rs::applies_strike_to_selection` <br> `core/jade-docx-editor/src/commands/apply_strike.rs::double_strike_updates_exported_document_xml` |
| 21 | super- / subscript | `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_run_superscript_baseline` |
| 22 | font & theme colours | `jade-office-model/src/word/font.rs::the_theme_palette_tints_and_shades_as_word_writes_them` <br> `jade-docx-native-render/tests/formatting_and_clipboard.rs::a_font_colour_is_drawn_once_the_selection_is_gone` |
| 23 | highlighter | `jade-docx-editor/src/canvas_renderer.rs::text_highlight_vertical_bounds_expand_to_font_em_box` <br> `jade-docx-editor/src/commands/set_highlight_color.rs::highlight_command_updates_exported_document_xml` |
| 24 | run shading & border | `jade-docx-reader/src/parser.rs::parses_run_level_character_shading` <br> `jade-docx-editor/src/canvas_renderer.rs::character_border_side_rects_frame_the_box` |
| 25 | caps, small caps | `core/jade-docx-reader/src/parser.rs::parses_caps_run_property` |
| 26 | hidden text | `core/jade-docx-editor/src/canvas_renderer.rs::run_is_hidden_text_only_when_vanish_is_true` |
| 27 | character scale | `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_run_text_scale` <br> `jade-docx-layout/src/line_wrap.rs::wrap_text_applies_text_scale_x` |
| 28 | character spacing | `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_run_character_spacing` <br> `jade-docx-native-render/tests/style_context.rs::a_style_character_spacing_sets_the_glyphs_closer` |
| 29 | raised / lowered | `jade-docx-layout/src/line_wrap.rs::inline_object_keeps_authored_lowered_baseline_in_shared_layout` |
| 30 | kerning | `jade-docx-layout/src/line_wrap.rs::benchmark_kerning_pair_cache_hot_path` <br> `jade-docx-reader/src/styles.rs::parses_run_kerning_and_text_scale_in_styles` |
| 31 | ligatures, OpenType | **none** |
| 32 | outline, shadow, glow | `jade-docx-reader/src/parser.rs::parses_text_effect_run_property` <br> `jade-docx-editor/src/canvas_renderer.rs::glow_and_text_outline_paint_a_halo_and_a_stroke` |
| 33 | clear formatting | `jade-docx-editor/src/headless_api.rs::clear_formatting_keeps_the_fonts_the_styles_give` <br> `jade-docx-editor/src/commands/clear_formatting.rs::clear_formatting_exports_and_reopens_without_direct_font_size` |
| 34 | paragraph alignment | `core/jade-docx-native-render/tests/search_and_paragraph_format.rs::alignment_reaches_the_page_and_can_be_undone` |
| 35 | distributed alignment | `core/jade-docx-editor/src/canvas_renderer.rs::distributed_final_line_paint_and_selection_use_shared_character_advances` |
| 36 | left / right indent | `jade-web/tests/paragraphs_documents.rs::an_english_document_measures_in_millimetres_and_hanging_indents` <br> `jade-docx-native-render/tests/search_and_paragraph_format.rs::indenting_reaches_the_page` |
| 37 | indent in characters | `jade-web/tests/paragraphs_documents.rs::the_dialog_writes_what_was_changed_in_characters` <br> `jade-docx-editor/src/commands/adjust_paragraph_indent.rs::zero_point_indent_steps_in_character_units` |
| 38 | first-line / hanging | `jade-web/tests/paragraphs_documents.rs::an_english_document_measures_in_millimetres_and_hanging_indents` <br> `core/jade-docx-editor/src/commands/adjust_paragraph_indent.rs::hanging_step_creates_a_one_character_hanging_indent_from_nothing` |
| 39 | space before / after | `core/jade-docx-headless-mcp/src/server.rs::paragraph_space_before_and_after_round_trips` |
| 40 | contextual spacing | `core/jade-docx-reader/src/parser.rs::parses_paragraph_contextual_spacing_flag` |
| 41 | line spacing (single, multiple) | `jade-web/tests/paragraphs_documents.rs::the_ribbon_line_spacing_reads_back` <br> `jade-docx-editor/src/commands/set_paragraph_line_spacing.rs::applies_and_undoes_line_spacing` |
| 42 | line spacing exact / at least | `core/jade-docx-editor/src/canvas_renderer.rs::large_numbering_ascent_preserves_authored_exact_line_spacing` <br> `core/jade-docx-editor/src/canvas_renderer.rs::mac_exact_line_puts_its_baseline_at_four_fifths` |
| 43 | tab stops & leaders | `jade-docx-native-render/tests/pdf_export.rs::tab_leader_pixels_are_stable_across_repaint_and_pdf_export` <br> `jade-docx-editor/src/draw_command/mod.rs::test_tab_leader_character` |
| 44 | paragraph borders | `jade-docx-reader/src/parser.rs::paragraph_border_edge_round_trips` <br> `jade-docx-reader/src/document.rs::paragraph_borders_default_is_empty` |
| 45 | paragraph shading | `jade-docx-native-render/tests/search_and_paragraph_format.rs::shading_and_borders_can_be_cleared` <br> `jade-docx-reader/src/parser.rs::paragraph_shading_round_trips` |
| 46 | widow / orphan | `jade-docx-reader/src/parser.rs::parses_paragraph_widow_control_flags` <br> `jade-docx-layout/src/line_metrics.rs::widow_control_keeps_two_lines_on_each_page` |
| 47 | keep with next | `jade-docx-reader/src/parser.rs::parses_paragraph_keep_next_flag` <br> `jade-docx-editor/src/canvas_renderer.rs::keep_next_group_may_split_inside_a_later_member` |
| 48 | keep lines together | `jade-docx-reader/src/parser.rs::parses_paragraph_keep_lines_flag` <br> `jade-docx-editor/src/canvas_renderer.rs::keep_lines_counts_both_sides_bands_not_wrapped_lines` |
| 49 | page break before | `jade-docx-reader/src/parser.rs::parses_paragraph_page_break_before_flag` <br> `jade-docx-editor/src/canvas_renderer.rs::page_break_before_advances_only_when_not_at_page_top` |
| 50 | no line numbers | `jade-docx-reader/src/parser.rs::parses_paragraph_suppress_line_numbers_flag` <br> `jade-docx-editor/src/word_document.rs::apply_style_paragraph_defaults_propagates_suppress_line_numbers` |
| 51 | no hyphenation | `jade-docx-reader/src/parser.rs::parses_soft_hyphen_into_run_text` <br> `jade-docx-editor/src/word_document.rs::soft_hyphen_round_trips_as_word_run_element` |
| 52 | drop cap | `core/jade-docx-editor/src/commands/set_drop_cap.rs::drop_cap_kinds_and_lines_round_trip_byte_exact_history` <br> `core/jade-docx-layout/src/line_wrap.rs::prepared_drop_caps_preserve_unicode_text_and_reserve_exact_line_bands` |
| 53 | bullets | `jade-docx-editor/src/shortcuts.rs::resolves_word_point_step_break_and_bullet_shortcuts` |
| 54 | picture bullets | **none** |
| 55 | number formats | `jade-docx-reader/src/numbering.rs::japanese_counting_uses_multiplicative_kanji_units` <br> `jade-docx-editor/src/canvas_renderer.rs::unspecified_endnote_format_uses_words_lower_roman_default` |
| 56 | start value | `core/jade-docx-reader/src/numbering.rs::zero_start_override_formats_zero_then_one` <br> `core/jade-docx-editor/src/commands/insert_footnote.rs::inserted_footnote_caret_accounts_for_a_multi_digit_start_number` |
| 57 | restart / continue | `core/jade-docx-editor/src/canvas_renderer.rs::note_ordinal_restarts_follow_num_restart` <br> `core/jade-docx-reader/src/numbering.rs::continues_numbering_across_num_id_without_start_override` |
| 58 | multilevel lists | `jade-docx-editor/src/commands/apply_multilevel_list.rs::invalid_custom_multilevel_definitions_are_atomic` |
| 59 | legal (isLgl) numbering | **none** |
| 60 | headings linked to list | `jade-docx-editor/src/headless_api.rs::insert_after_heading_resolves_numbering_prefixed_article_heading` |
| 61 | number alignment & indent | **none** |
| 62 | number suffix | `core/jade-docx-editor/src/canvas_renderer.rs::numbering_space_suffix_can_start_body_before_left_indent` |
| 63 | list styles | **none** |
| 64 | change list level | `jade-docx-editor/src/commands/adjust_paragraph_indent.rs::hanging_step_leaves_list_levels_alone` <br> `jade-docx-editor/src/headless_api.rs::undoing_a_list_level_step_restores_the_saved_level` |
| 65 | paragraph styles | `jade-web/tests/formats_documents.rs::the_gallery_applies_a_heading_and_normal` <br> `jade-docx-native-render/tests/formatting_and_clipboard.rs::a_paragraph_style_and_a_list_are_applied` |
| 66 | character styles | `jade-docx-reader/tests/generated_fixtures.rs::character_styles_fixture_lists_gallery_styles_and_hides_hyperlink` <br> `jade-docx-reader/src/parser.rs::parses_run_styles` |
| 67 | linked styles | **none** |
| 68 | table styles | `jade-docx-reader/src/styles.rs::default_table_style_requires_the_default_marker` <br> `jade-docx-editor/src/canvas_renderer/table.rs::package_borderless_table_style_does_not_invent_a_grid` |
| 69 | new style | `jade-docx-editor/src/commands/set_paragraph_style.rs::cascades_new_style_run_properties_onto_runs` <br> `jade-docx-editor/src/headless_api.rs::restyled_runs_take_the_new_styles_theme_fonts` |
| 70 | modify style | **none** |
| 71 | based-on chain | `jade-docx-editor/src/word_document.rs::fill_missing_indent_inherits_per_property_group` <br> `jade-docx-editor/src/commands/set_font_family.rs::concrete_font_family_replaces_inherited_theme_slots` |
| 72 | next-paragraph style | `jade-docx-editor/src/commands/insert_paragraph_break.rs::enter_at_the_end_of_a_heading_starts_its_next_style` <br> `jade-docx-editor/src/headless_api.rs::the_next_style_turns_off_a_list_a_pending_change_records` |
| 73 | update style from selection | **none** |
| 74 | auto-update style | **none** |
| 75 | gallery order | `jade-web/tests/formats_documents.rs::the_gallery_applies_a_heading_and_normal` <br> `jade-web/tests/tables_documents.rs::style_options_and_a_gallery_style_apply_and_save` |
| 76 | themes | `jade-office-model/src/word/font.rs::the_theme_palette_tints_and_shades_as_word_writes_them` <br> `jade-docx-editor/tests/word_schema.rs::a_theme_created_for_a_document_without_one_has_a_full_format_scheme` |
| 77 | theme colours | `jade-office-model/src/word/font.rs::the_theme_palette_tints_and_shades_as_word_writes_them` <br> `jade-docx-reader/src/theme.rs::parses_theme_color_scheme_slots` |
| 78 | theme fonts | `jade-docx-reader/src/theme.rs::parses_theme_fonts` <br> `jade-docx-editor/src/word_document.rs::theme_fonts_resolve_from_doc_defaults` |
| 79 | theme effects | **none** |
| 80 | style sets | **none** |
| 81 | attach template | `jade-office-model/src/word/new.rs::a_template_opens_as_a_document` <br> `jade-web/tests/new_documents.rs::a_template_opens_as_a_document` |
| 82 | AutoText / building blocks | **none** |
| 83 | set as default | **none** |
| 84 | insert table | `jade-docx-editor/src/commands/insert_table.rs::insert_table_supports_undo_and_redo` <br> `jade-docx-editor/src/headless_api.rs::insert_table_cell_rejects_merged_cells` |
| 85 | draw table | `jade-web/tests/tables_documents.rs::the_pencil_splits_and_the_eraser_merges` |
| 86 | rows & columns in/out | `jade-docx-editor/src/commands/table_structure.rs::insert_row_supports_undo_and_redo` <br> `jade-docx-headless-editor/src/lib.rs::property_insert_delete_column_roundtrip_keeps_merged_shape` |
| 87 | insert cells with shift | `jade-web/tests/tables_documents.rs::cells_insert_and_delete_with_a_shift` |
| 88 | merge cells | `jade-web/tests/tables_documents.rs::the_pencil_splits_and_the_eraser_merges` <br> `jade-docx-editor/src/commands/merge_table_cells.rs::merge_horizontal_cells_single_row` |
| 89 | split cells | `jade-web/tests/tables_documents.rs::the_pencil_splits_and_the_eraser_merges` <br> `jade-docx-editor/src/commands/unmerge_table_cells.rs::unmerge_restores_independent_cells` |
| 90 | split table | `jade-docx-editor/src/canvas_renderer/table.rs::split_table_paragraph_obeys_keep_lines_and_shared_widow_policy` <br> `jade-docx-editor/src/commands/split_table.rs::splitting_at_the_first_row_puts_a_paragraph_in_front` |
| 91 | nested tables | `jade-web/tests/tables_documents.rs::a_table_inside_a_cell_is_edited_as_a_table` <br> `jade-docx-diff/tests/document.rs::a_nested_table_is_compared_inside_its_cell` |
| 92 | text → table | `jade-web/tests/tables_documents.rs::text_and_tables_convert_both_ways` |
| 93 | table → text | `jade-web/tests/tables_documents.rs::text_and_tables_convert_both_ways` |
| 94 | sort rows | `jade-web/tests/tables_documents.rs::rows_sort_and_formulas_compute` |
| 95 | cell formulas | `jade-web/tests/tables_documents.rs::rows_sort_and_formulas_compute` |
| 96 | paragraphs in cells | `jade-docx-layout/src/text_extract.rs::table_body_element_emits_cell_paragraph_lines` <br> `jade-docx-editor/src/canvas_renderer/table.rs::assign_row_paragraph_indices_reserves_skip_render_cells` |
| 97 | select cell / row / column / table | `jade-web/tests/tables_documents.rs::cells_rows_columns_and_the_table_select_from_the_menu_and_the_mouse` |
| 98 | table alignment & indent | `jade-docx-reader/src/parser/table.rs::parses_table_indent_into_margin_left` <br> `jade-docx-editor/src/canvas_renderer/table.rs::ignored_table_indentation_does_not_add_a_legacy_text_inset` |
| 99 | fixed / % width | `jade-docx-reader/tests/generated_fixtures.rs::table_no_wrap_autofit_fixture_preserves_absolute_and_percentage_constraints` <br> `jade-docx-editor/src/headless_api.rs::set_table_width_command_rejects_invalid_spec` |
| 100 | column width, row height | `jade-web/tests/tables_documents.rs::borders_drag_to_resize_columns_and_rows` <br> `jade-docx-reader/tests/generated_fixtures.rs::yokohama_table_row_heights_fixture_keeps_word_row_rules` |
| 101 | row height rule | `jade-docx-reader/tests/generated_fixtures.rs::yokohama_table_row_heights_fixture_keeps_word_row_rules` <br> `jade-docx-editor/src/word_document.rs::row_height_nodes_receive_unique_non_root_ids` |
| 102 | AutoFit to contents | `jade-web/tests/tables_documents.rs::borders_text_direction_and_autofit` <br> `jade-docx-reader/tests/generated_fixtures.rs::table_no_wrap_autofit_fixture_preserves_absolute_and_percentage_constraints` |
| 103 | AutoFit to window | **none** |
| 104 | distribute rows / columns | `core/jade-docx-editor/src/commands/table_structure.rs::distributing_rows_and_columns_evens_them_out_like_word` |
| 105 | cell margins | `jade-docx-reader/src/parser/table.rs::parses_table_cell_margins` <br> `jade-docx-editor/src/canvas_renderer/table.rs::split_prepared_row_uses_resolved_zero_cell_padding` |
| 106 | cell spacing | `jade-docx-reader/tests/generated_fixtures.rs::table_cell_spacing_fixture_preserves_table_default_and_row_overrides` <br> `jade-docx-reader/src/parser/table.rs::parses_row_cell_spacing_override` |
| 107 | cell vertical alignment | `core/jade-docx-reader/src/parser/table.rs::parses_cell_vertical_alignment` <br> `core/jade-docx-editor/src/canvas_renderer/table.rs::cell_vertical_align_share_maps_tokens_to_offset_fraction` |
| 108 | cell text direction | `jade-web/tests/tables_documents.rs::borders_text_direction_and_autofit` <br> `jade-docx-reader/src/parser.rs::parses_section_text_direction` |
| 109 | table borders | `jade-web/tests/tables_documents.rs::borders_text_direction_and_autofit` <br> `jade-docx-reader/tests/generated_fixtures.rs::table_border_conflict_fixture_preserves_visible_and_hidden_opponents` |
| 110 | cell shading | **none** |
| 111 | header / total row styles | `jade-docx-editor/src/canvas_renderer.rs::nested_table_header_rows_are_not_repeated_like_word` <br> `jade-docx-reader/src/document/table_conditional.rs::banding_skips_header_rows_and_honors_band_size_zero` |
| 112 | banding | `core/jade-docx-reader/src/document/table_conditional.rs::banding_skips_header_rows_and_honors_band_size_zero` <br> `core/jade-docx-reader/src/document/table_conditional.rs::band_regions_report_group_edges` |
| 113 | first / last column | **none** |
| 114 | repeat header rows | `jade-docx-reader/tests/generated_fixtures.rs::table_repeating_header_fixture_parses_empty_on_and_explicit_false` <br> `jade-docx-reader/src/parser/table.rs::parses_repeating_table_header_on_off_values` |
| 115 | row may break across pages | `jade-docx-reader/src/parser/table.rs::parses_cant_split_row_property` <br> `jade-docx-editor/src/canvas_renderer/table.rs::split_prepared_row_splits_at_block_boundaries` |
| 116 | floating tables | `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_embeds_floating_png_images` <br> `jade-docx-native-render/tests/header_and_objects.rs::a_floating_object_can_be_moved_and_undone` |
| 117 | insert / replace picture | `jade-web/tests/pictures_documents.rs::a_picture_goes_in_at_the_caret_and_wraps` <br> `jade-docx-editor/src/commands/insert_image.rs::generated_inline_images_round_trip_at_every_utf8_boundary` |
| 118 | picture size & aspect lock | `jade-office-model/src/word/picture.rs::sizes_come_from_the_headers` |
| 119 | rotate / flip | `core/jade-docx-editor/src/canvas_renderer.rs::zero_rotation_leaves_the_transform_untouched` |
| 120 | crop, crop to shape | `jade-docx-reader/tests/generated_fixtures.rs::image_crop_fixture_preserves_drawingml_source_rectangle` <br> `jade-skia-painter/tests/svg_image.rs::svg_images_use_the_same_source_crop_and_destination_mapping_as_raster_images` |
| 121 | picture corrections | **none** |
| 122 | background removal | **none** |
| 123 | picture border & effects | **none** |
| 124 | SVG | `jade-skia-painter/tests/image_opacity.rs::svg_images_respect_the_saved_global_opacity_and_normalized_source` <br> `jade-skia-painter/tests/svg_image.rs::svg_images_use_the_same_source_crop_and_destination_mapping_as_raster_images` |
| 125 | insert shapes | `jade-docx-extract/tests/context_flags.rs::marks_textbox_and_shape_paragraphs` <br> `jade-docx-document-core/tests/delete_text_box.rs::insert_shape_offsets_are_rendered_offsets` |
| 126 | shape fill / line / effects | `core/jade-docx-editor/src/commands/format_text_box.rs::shape_fill_and_outline_apply_undo_redo_atomically` |
| 127 | edit points | **none** |
| 128 | connectors | `jade-docx-editor/src/word_document.rs::connector_writer_preserves_authored_end_side_and_type` <br> `jade-docx-editor/src/canvas_renderer.rs::connector_line_ends_keep_the_authored_side_and_geometry` |
| 129 | text boxes | `jade-docx-document-core/tests/delete_text_box.rs::a_missing_text_box_is_an_error` <br> `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_simple_text_box` |
| 130 | WordArt | `core/jade-docx-editor/src/word_document.rs::existing_word_art_renders_as_an_effected_text_box_and_round_trips` <br> `core/jade-docx-editor/src/word_document.rs::generated_word_art_preserves_host_coordinates_and_text_formatting` |
| 131 | SmartArt | `core/jade-docx-editor/src/word_document.rs::smartart_renders_the_cached_drawing_and_round_trips_its_parts` <br> `core/jade-docx-editor/src/canvas_renderer.rs::smartart_text_bodies_paint_with_their_drawingml_run_style` |
| 132 | charts | `core/jade-docx-editor/src/commands/insert_chart.rs::generated_native_charts_preserve_coordinates_history_and_round_trip` <br> `core/jade-docx-editor/src/chart.rs::authored_chart_round_trips_xml_reserved_text_and_gaps` |
| 133 | ink | **none** |
| 134 | reset picture | **none** |
| 135 | shape styles | **none** |
| 136 | wrap: in line | `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_embeds_inline_png_images` |
| 137 | wrap: square | `jade-docx-reader/src/parser.rs::parses_floating_image_wrap_square_and_distances` <br> `jade-docx-layout/src/line_wrap.rs::both_sides_square_wrap_fills_both_gaps_of_a_band` |
| 138 | wrap: tight / through | `jade-docx-reader/src/parser.rs::parses_edited_tight_wrap_polygon_points` <br> `jade-docx-layout/src/line_wrap.rs::tight_polygon_narrows_the_exclusion_toward_its_apex` |
| 139 | wrap: top & bottom | `jade-docx-reader/src/parser.rs::parses_floating_image_wrap_top_and_bottom_and_none` <br> `jade-docx-editor/src/canvas_renderer.rs::top_and_bottom_exclusion_uses_resolved_vertical_position` |
| 140 | wrap: behind / in front | `core/jade-docx-native-render/tests/floating_layers.rs::a_shape_in_front_of_text_covers_the_paragraphs_after_its_anchor` <br> `core/jade-docx-native-render/tests/floating_layers.rs::a_shape_behind_text_lies_under_the_paragraphs_before_its_anchor` |
| 141 | wrap points | `core/jade-docx-reader/src/parser.rs::parses_edited_tight_wrap_polygon_points` |
| 142 | distance from text | `core/jade-docx-reader/src/parser.rs::parses_floating_image_wrap_square_and_distances` <br> `core/jade-docx-editor/src/canvas_renderer.rs::floating_object_exclusion_uses_resolved_position_rotation_and_distances` |
| 143 | horizontal position | `jade-docx-reader/src/parser/table.rs::parses_floating_table_horizontal_position_and_text_distances` <br> `jade-docx-editor/src/canvas_renderer/table.rs::floating_table_horizontal_position_overrides_inline_alignment` |
| 144 | vertical position | `jade-docx-editor/src/canvas_renderer.rs::top_and_bottom_exclusion_uses_resolved_vertical_position` <br> `jade-docx-editor/src/canvas_renderer/table.rs::floating_table_without_vertical_position_defaults_to_page_origin` |
| 145 | move with text | `core/jade-docx-editor/src/commands/transform_object.rs::moves_text_box_anchor_between_paragraphs_and_restores_it_with_undo_redo` |
| 146 | fixed on page | **none** |
| 147 | lock anchor | **none** |
| 148 | allow overlap | `core/jade-docx-reader/tests/generated_fixtures.rs::table_floating_overlap_fixture_preserves_pairwise_never_policy` |
| 149 | z-order | `core/jade-docx-editor/src/canvas_renderer/page_layers.rs::objects_stack_by_relative_height_across_paragraphs` |
| 150 | align / distribute objects | **none** |
| 151 | group / ungroup | `core/jade-docx-editor/src/canvas_renderer.rs::a_deferred_group_shares_one_transform_and_one_font_state` <br> `core/jade-docx-editor/src/canvas_renderer.rs::full_layout_capture_reports_nested_group_shape_text` |
| 152 | selection pane | **none** |
| 153 | objects in table cells | **none** |
| 154 | ruby insert | `jade-docx-editor/tests/ruby.rs::a_ruby_is_put_on_changed_and_taken_off` <br> `jade-docx-reader/src/parser.rs::parses_eq_overstrike_ruby_field` |
| 155 | ruby alignment | `jade-docx-editor/tests/ruby.rs::a_ruby_is_put_on_changed_and_taken_off` <br> `jade-docx-reader/src/parser.rs::parses_eq_overstrike_ruby_field` |
| 156 | ruby formatting | `jade-docx-reader/src/parser.rs::parses_ruby_raise_and_guide_fonts` <br> `jade-docx-layout/src/line_wrap.rs::ruby_base_spreads_across_formatting_runs` |
| 157 | emphasis marks | `jade-office-model/src/word/clipboard.rs::merging_keeps_emphasis_and_text_only_keeps_nothing` <br> `jade-docx-reader/src/parser.rs::parses_emphasis_mark_run_property` |
| 158 | enclosed characters | `jade-docx-editor/tests/date_time.rs::an_enclosed_character_is_written_as_word_writes_it` <br> `jade-docx-reader/src/parser.rs::eq_overstrike_of_an_enclosure_is_an_enclosed_character` |
| 159 | combined characters | `core/jade-docx-editor/src/canvas_renderer.rs::combined_characters_stack_their_halves` |
| 160 | 縦中横 | `jade-docx-editor/src/canvas_renderer.rs::tate_chu_yoko_paints_its_run_as_one_upright_group` |
| 161 | vertical text | `jade-web/tests/section_setup_documents.rs::vertical_text_turns_the_page` <br> `jade-office-model/src/word/page_setup.rs::vertical_text_turns_the_page_and_book_fold_turns_the_sheet` |
| 162 | kinsoku | `core/jade-text-layout/tests/wraps_lines.rs::turning_kinsoku_off_breaks_wherever_it_fits` <br> `core/jade-text-layout/tests/wrap_invariants.rs::a_prohibited_character_ends_a_line_only_when_it_is_forced` |
| 163 | kinsoku sets & level | `core/jade-docx-editor/src/canvas_renderer.rs::paragraph_style_forbids_custom_line_start` <br> `core/jade-docx-editor/tests/typography.rs::the_document_kinsoku_level_and_compression_go_into_settings` |
| 164 | hanging punctuation | `jade-docx-layout/src/line_wrap.rs::hanging_punctuation_keeps_kuten_on_same_line` |
| 165 | auto spacing (overall) | `jade-docx-reader/src/settings.rs::parses_auto_space_de_dn_flags` <br> `jade-docx-reader/src/parser.rs::parses_paragraph_auto_space_de_dn_flags` |
| 166 | JA ↔ Latin spacing | `core/jade-docx-layout/src/line_wrap.rs::implicit_latin_gap_respects_auto_space_de_disabled` <br> `core/jade-docx-layout/src/line_wrap.rs::implicit_latin_gap_leading_only_when_prev_is_east_asia` |
| 167 | JA ↔ number spacing | `jade-docx-layout/src/line_wrap.rs::implicit_numeric_gap_respects_auto_space_dn_disabled` <br> `jade-docx-editor/src/canvas_renderer.rs::implicit_numeric_gap_is_zero_when_auto_space_dn_disabled` |
| 168 | document grid | `jade-web/tests/section_setup_documents.rs::a_paragraph_leaves_the_grid` <br> `jade-office-model/src/word/page_setup.rs::chars_and_lines_set_the_pitches` |
| 169 | snap to grid | `jade-web/tests/section_setup_documents.rs::a_paragraph_leaves_the_grid` |
| 170 | RTL paragraphs | `jade-docx-reader/tests/generated_fixtures.rs::bidi_visual_alignment_fixture_preserves_justification_and_leading_indent` <br> `jade-docx-reader/src/numbering.rs::hebrew_and_arabic_formats` |
| 171 | complex-script font & size | `jade-web/tests/thai_in_documents.rs::thai_is_measured_in_the_thai_face` <br> `jade-skia-painter/tests/font_fallback.rs::thai_after_latin_draws_in_the_thai_face` |
| 172 | proofing language | `jade-docx-editor/src/typed_autoformat.rs::language_and_capitalization_options_are_respected` |
| 173 | full / half width | `core/jade-docx-reader/src/numbering.rs::formats_decimal_full_width_numbers` |
| 174 | paper size | `jade-office-model/src/word/page.rs::papers_are_known_either_way_up` <br> `jade-web/tests/pages_documents.rs::the_page_takes_margins_orientation_paper_and_columns` |
| 175 | orientation | `jade-web/tests/pages_documents.rs::the_page_takes_margins_orientation_paper_and_columns` <br> `jade-docx-native-render/tests/page_box.rs::a_page_turned_to_landscape_paints_as_one_opened_landscape` |
| 176 | margins | `jade-office-model/src/word/page.rs::normal_margins_follow_the_language` <br> `jade-office-model/src/word/page_setup.rs::turning_the_page_turns_the_margins` |
| 177 | mirror margins | `core/jade-docx-reader/src/settings.rs::parses_mirror_margins_toggle` <br> `core/jade-docx-layout/src/text_extract.rs::mirrored_margins_shift_even_page_lines_to_the_verso_margin` |
| 178 | gutter | `jade-docx-reader/src/settings.rs::parses_gutter_at_top_toggle` <br> `jade-docx-reader/src/parser.rs::parses_page_gutter_and_defaults_to_none` |
| 179 | book fold / 2 pages per sheet | `jade-web/tests/section_setup_documents.rs::a_book_fold_shows_half_sheet_pages` <br> `jade-office-model/src/word/page_setup.rs::vertical_text_turns_the_page_and_book_fold_turns_the_sheet` |
| 180 | page vertical alignment | `core/jade-docx-editor/src/canvas_renderer.rs::first_full_render_applies_section_vertical_alignment` <br> `core/jade-docx-reader/src/parser.rs::parses_section_vertical_alignment` |
| 181 | page break | `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::hello_fixture_matches_word_page_break_and_text_origins` |
| 182 | next-page section break | `jade-web/tests/pages_documents.rs::a_section_break_goes_in_at_the_caret` <br> `jade-web/tests/section_setup_documents.rs::breaks_go_in_at_the_caret` |
| 183 | continuous section break | `jade-docx-layout/src/section_transition.rs::continuous_only_advances_when_page_geometry_changes` |
| 184 | odd / even section break | `jade-web/tests/section_setup_documents.rs::breaks_go_in_at_the_caret` |
| 185 | per-section page setup | `jade-office-model/src/word/page_setup.rs::apply_choices_follow_the_sections` <br> `jade-web/tests/pages_documents.rs::a_section_break_goes_in_at_the_caret` |
| 186 | columns | `jade-web/tests/pages_documents.rs::the_page_takes_margins_orientation_paper_and_columns` <br> `jade-office-model/src/word/page_setup.rs::unequal_columns_keep_the_line` |
| 187 | column width & spacing | `jade-office-model/src/word/page_setup.rs::unequal_columns_keep_the_line` |
| 188 | line between columns | `jade-office-model/src/word/page_setup.rs::unequal_columns_keep_the_line` |
| 189 | column break | `jade-web/tests/section_setup_documents.rs::a_column_break_moves_the_text_to_the_next_column` |
| 190 | line numbers | `jade-web/tests/section_setup_documents.rs::columns_line_numbers_and_page_numbers_are_saved` |
| 191 | page borders | `jade-web/tests/section_setup_documents.rs::page_borders_color_and_a_cover_page` <br> `jade-docx-reader/src/parser.rs::parses_page_borders_all_sides` |
| 192 | page colour | `jade-web/tests/edits_documents.rs::dark_paper_turns_the_pages_over_and_back` <br> `jade-docx-native-render/tests/native_document.rs::dark_paper_turns_the_page_over_on_screen_but_not_in_the_pdf` |
| 193 | blank page | `jade-docx-editor/src/commands/insert_blank_page.rs::blank_page_uses_one_or_two_breaks_and_round_trips_history` |
| 194 | cover page | `jade-web/tests/section_setup_documents.rs::page_borders_color_and_a_cover_page` <br> `jade-docx-headless-api/src/lib.rs::cover_page_wire_schema_requires_a_strict_paragraph_list` |
| 195 | header / footer editing | `jade-web/tests/pages_documents.rs::headers_and_footers_take_text_and_a_page_number` |
| 196 | first-page header | `jade-docx-native-render/tests/pdf_export.rs::pdf_keeps_first_page_and_section_specific_header_footer_stories` <br> `jade-docx-reader/src/parser.rs::title_page_explicit_false_disables_first_page_variant` |
| 197 | odd / even headers | `core/jade-docx-reader/src/settings.rs::parses_even_and_odd_headers_toggle` |
| 198 | link to previous | `core/jade-docx-editor/tests/section_layout_and_stories.rs::a_section_unlinks_and_links_its_header` |
| 199 | header / footer distance | `jade-docx-headless-mcp/src/server.rs::header_footer_and_gutter_margins_round_trip_and_undo` |
| 200 | pictures & tables in header | `core/jade-docx-editor/src/word_document.rs::extract_header_lines_maps_three_cell_table_rows_to_segments` |
| 201 | fields in footer | `jade-web/tests/pages_documents.rs::headers_and_footers_take_text_and_a_page_number` <br> `jade-web/tests/section_setup_documents.rs::columns_line_numbers_and_page_numbers_are_saved` |
| 202 | page number position | `jade-web/tests/pages_documents.rs::headers_and_footers_take_text_and_a_page_number` <br> `jade-web/tests/section_setup_documents.rs::columns_line_numbers_and_page_numbers_are_saved` |
| 203 | page number format | `jade-docx-editor/src/word_document.rs::page_number_format_targets_the_named_section` <br> `jade-docx-editor/src/canvas_renderer.rs::format_page_number_supports_common_numeral_formats` |
| 204 | page number start | `jade-docx-reader/src/parser.rs::parses_page_number_type_fmt_and_start` <br> `jade-docx-editor/src/word_document.rs::page_number_format_readback_preserves_section_defaults_and_restarts` |
| 205 | chapter in page number | `core/jade-docx-editor/src/canvas_renderer.rs::displayed_page_number_includes_the_chapter_number` |
| 206 | continue page numbers | `core/jade-docx-editor/src/word_document.rs::page_number_format_readback_preserves_section_defaults_and_restarts` |
| 207 | total page count | `jade-docx-native-render/tests/pdf_export.rs::pdf_retains_the_live_total_page_field_on_early_pages` <br> `jade-docx-editor/src/word_document.rs::extract_text_lines_emits_numpages_placeholder` |
| 208 | STYLEREF in header | `jade-docx-editor/src/canvas_renderer.rs::style_reference_follows_the_page` |
| 209 | insert / edit field | `jade-docx-headless-mcp/src/server.rs::export_preserves_field_codes` <br> `jade-docx-editor/src/commands/insert_field.rs::fields_replace_arbitrary_unicode_run_ranges_and_round_trip` |
| 210 | toggle field codes | **none** |
| 211 | update fields | **none** |
| 212 | lock / unlock field | `core/jade-docx-editor/src/word_document.rs::locked_simple_and_complex_field_wrappers_survive_text_edits_and_roundtrip` <br> `core/jade-docx-reader/src/parser.rs::field_lock_false_and_omitted_keep_live_values_and_attribute_order_is_irrelevant` |
| 213 | unlink field | **none** |
| 214 | nested fields | `core/jade-docx-reader/src/parser.rs::nested_mergefield_cache_becomes_if_operand_without_leaking_result` <br> `core/jade-docx-reader/src/parser.rs::nested_non_display_field_contributes_empty_operand_not_cached_storage` |
| 215 | format switches | `core/jade-docx-reader/src/parser.rs::extract_field_format_switch_handles_quoted_unquoted_and_missing` <br> `core/jade-docx-reader/src/field_datetime.rs::formats_word_picture_tokens_and_literals` |
| 216 | PAGE / NUMPAGES / SECTIONPAGES | `jade-docx-native-render/tests/pdf_export.rs::pdf_retains_the_live_total_page_field_on_early_pages` <br> `jade-docx-editor/src/word_document.rs::extract_text_lines_emits_numpages_placeholder` |
| 217 | DATE / TIME / CREATEDATE / SAVEDATE | `core/jade-docx-editor/tests/date_time.rs::a_date_field_keeps_its_picture` <br> `core/jade-docx-reader/src/parser.rs::parses_package_backed_date_fields_without_collapsing_them_to_date` |
| 218 | AUTHOR / TITLE / DOCPROPERTY | `core/jade-docx-reader/tests/generated_fixtures.rs::document_property_fixture_retains_extended_properties_and_header_field` <br> `core/jade-docx-reader/src/parser.rs::parses_builtin_title_field_through_document_property_path` |
| 219 | REF / PAGEREF / NOTEREF | `jade-docx-reader/src/parser.rs::parses_pageref_field_from_fld_simple` <br> `jade-docx-editor/src/commands/bookmark.rs::note_targets_insert_scoped_noteref_fields_with_exact_history` |
| 220 | SEQ / LISTNUM | `core/jade-docx-reader/src/parser.rs::parses_seq_fields_and_numbers_them_in_document_order` <br> `core/jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_ref_and_seq_use_resolved_value_with_cached_fallback` |
| 221 | STYLEREF | `jade-docx-editor/src/canvas_renderer.rs::style_reference_follows_the_page` |
| 222 | IF / COMPARE | `jade-docx-reader/src/parser.rs::set_assignment_is_available_to_a_following_if_field` <br> `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_if_and_compare_prefer_recomputed_values` |
| 223 | ASK / FILLIN | `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_fillin_does_not_duplicate_editable_response` <br> `jade-docx-reader/src/parser.rs::parses_fragmented_fillin_with_omitted_prompt_and_blank_default` |
| 224 | = formula | `jade-web/tests/tables_documents.rs::rows_sort_and_formulas_compute` |
| 225 | INCLUDETEXT / INCLUDEPICTURE | **none** |
| 226 | document properties | `core/jade-docx-reader/tests/generated_fixtures.rs::document_property_fixture_retains_extended_properties_and_header_field` |
| 227 | footnotes in / out | `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_simple_footnote_text` <br> `jade-docx-document-core/tests/tracked_revisions.rs::deleted_text_in_a_footnote_keeps_its_edge_spaces_in_the_file` |
| 228 | endnotes in / out | `jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_draws_simple_endnote_text` <br> `jade-docx-reader/src/settings.rs::parses_document_wide_endnote_position` |
| 229 | convert notes | `jade-docx-editor/src/commands/insert_footnote.rs::converts_imported_notes_both_ways_as_one_undoable_package_edit` |
| 230 | note number format & start | `jade-docx-editor/src/canvas_renderer.rs::unspecified_endnote_format_uses_words_lower_roman_default` |
| 231 | note numbering restart | `jade-docx-editor/src/canvas_renderer.rs::note_ordinal_restarts_follow_num_restart` <br> `jade-docx-editor/src/commands/bookmark.rs::page_restarted_noteref_preserves_cache_until_layout_resolves_it` |
| 232 | note placement | `jade-docx-reader/src/settings.rs::parses_document_wide_endnote_position` <br> `jade-docx-pdf-renderer/src/lib.rs::note_positions_stay_inside_page_and_spill_forward` |
| 233 | note separators | `core/jade-docx-editor/src/canvas_renderer/note_tests.rs::imported_note_markers_preserve_the_authored_separator_and_inline_text` <br> `core/jade-docx-editor/src/canvas_renderer.rs::endnote_separator_measures_authored_story_without_mutating_body_state` |
| 234 | bookmarks | `jade-web/tests/links_documents.rs::an_internal_link_leads_to_its_bookmark` <br> `jade-docx-native-render/tests/pdf_export.rs::pdf_resolves_forward_bookmarks_to_the_actual_page_and_position` |
| 235 | hyperlinks | `jade-web/tests/links_documents.rs::command_k_on_a_link_edits_it` <br> `jade-docx-native-render/tests/pdf_export.rs::pdf_keeps_external_link_actions` |
| 236 | cross-references | `core/jade-docx-editor/src/commands/bookmark.rs::unrelated_edit_preserves_existing_complex_cross_reference_field` <br> `core/jade-docx-editor/src/commands/bookmark.rs::adjacent_cross_references_keep_independent_fields_after_export_and_reload` |
| 237 | captions | `jade-docx-reader/tests/generated_fixtures.rs::focused_toc_edge_fixtures_preserve_fragmented_instructions_and_caption_modes` <br> `jade-docx-editor/src/commands/bookmark.rs::newly_authored_caption_number_reference_needs_no_reload` |
| 238 | captions with chapter | **none** |
| 239 | citation sources | `jade-docx-editor/src/headless_api.rs::query_citation_sources_returns_the_word_catalog` <br> `jade-docx-editor/src/commands/insert_citation.rs::snapshot_batches_rebase_citations_and_following_edits` |
| 240 | insert citation | `jade-docx-editor/src/headless_api.rs::query_citation_sources_returns_the_word_catalog` <br> `jade-docx-editor/src/commands/insert_citation.rs::snapshot_batches_rebase_citations_and_following_edits` |
| 241 | bibliography style | **none** |
| 242 | source manager | **none** |
| 243 | next / previous note | **none** |
| 244 | show notes | **none** |
| 245 | automatic TOC | `jade-web/tests/references_documents.rs::the_table_of_contents_is_updated_from_the_headings` <br> `jade-docx-reader/tests/generated_fixtures.rs::focused_toc_direct_outline_fixture_requires_u_for_direct_formatting` |
| 246 | manual TOC | **none** |
| 247 | TOC levels | `jade-docx-reader/src/parser.rs::parses_paragraph_outline_level` <br> `jade-docx-reader/src/document.rs::toc_f_after_l_resets_the_tc_level_filter` |
| 248 | TOC custom styles | `jade-docx-reader/tests/generated_fixtures.rs::focused_tc_field_fixture_filters_hidden_entries_by_type_and_level` <br> `jade-docx-reader/src/parser.rs::toc_f_and_l_switches_collect_hidden_tc_fields` |
| 249 | TOC leaders & page numbers | `jade-docx-reader/src/document.rs::parse_toc_switches_defaults_and_hide_page_numbers` <br> `jade-docx-editor/src/canvas_renderer.rs::original_toc_dot_leaders_use_requested_pitch_and_word_glyph_grid` |
| 250 | update TOC | `jade-web/tests/references_documents.rs::the_table_of_contents_is_updated_from_the_headings` <br> `jade-docx-editor/src/commands/insert_toc.rs::update_table_of_contents_without_toc_errors` |
| 251 | table of figures | `jade-docx-editor/src/commands/insert_toc.rs::insert_table_of_contents_builds_table_of_figures_from_caption_sequence` |
| 252 | mark index entry | `core/jade-docx-editor/src/commands/insert_index.rs::unicode_index_entries_round_trip_at_arbitrary_run_boundaries` <br> `core/jade-docx-editor/src/commands/insert_index.rs::two_entries_marked_at_the_same_position_survive_a_reopen` |
| 253 | index (entries, subentries) | `core/jade-docx-editor/src/commands/insert_index.rs::current_state_batches_follow_prior_structural_coordinates_and_generate_index` |
| 254 | index see / page ranges | **none** |
| 255 | table of authorities | **none** |
| 256 | navigation pane | `jade-docx-editor/src/word_document.rs::navigation_heading_catalog_is_coordinate_stable` <br> `jade-docx-editor/src/headless_api.rs::document_outline_lists_every_navigation_pane_heading` |
| 257 | outline expand / collapse | **none** |
| 258 | promote / demote / move | `core/jade-docx-reader/src/parser.rs::parses_paragraph_outline_level` <br> `core/jade-docx-headless-mcp/src/server.rs::heading_relative_inserts_find_outline_level_headings` |
| 259 | master / subdocuments | **none** |
| 260 | start mail merge | **none** |
| 261 | new recipient list | **none** |
| 262 | pick recipients | **none** |
| 263 | sort / filter recipients | **none** |
| 264 | merge fields | `jade-docx-reader/src/parser.rs::parses_complex_mergefield_instruction_split_across_runs` <br> `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_mergefield_does_not_duplicate_editable_result_text` |
| 265 | address block | **none** |
| 266 | greeting line | **none** |
| 267 | match fields | **none** |
| 268 | merge IF rules | `core/jade-docx-reader/src/parser.rs::nested_mergefield_cache_becomes_if_operand_without_leaking_result` |
| 269 | NEXT / NEXTIF | **none** |
| 270 | preview results | **none** |
| 271 | merge to documents | **none** |
| 272 | merge to printer | **none** |
| 273 | envelopes | **none** |
| 274 | labels | **none** |
| 275 | check merge errors | **none** |
| 276 | rich-text control | `core/jade-docx-reader/src/docx_xml/mapping.rs::maps_block_table_row_and_cell_nodes_inside_sdt_content` <br> `core/jade-docx-editor/src/word_document.rs::editing_block_sdt_content_clears_word_placeholder_state` |
| 277 | plain-text control | **none** |
| 278 | picture control | **none** |
| 279 | check-box control | **none** |
| 280 | combo-box control | **none** |
| 281 | drop-down control | **none** |
| 282 | date-picker control | **none** |
| 283 | building-block control | **none** |
| 284 | repeating section | **none** |
| 285 | control title & tag | **none** |
| 286 | placeholder text | `core/jade-docx-editor/src/word_document.rs::editing_block_sdt_content_clears_word_placeholder_state` |
| 287 | control can't be deleted | **none** |
| 288 | contents can't be edited | **none** |
| 289 | group controls | **none** |
| 290 | legacy text field | **none** |
| 291 | legacy check box | **none** |
| 292 | legacy drop-down | **none** |
| 293 | design mode | **none** |
| 294 | track changes on/off | `jade-web/tests/reviews_documents.rs::tracked_changes_are_drawn_as_markup` <br> `jade-docx-native-render/tests/comments_and_revisions.rs::tracked_changes_are_drawn_as_markup` |
| 295 | tracked insert / delete | `core/jade-docx-document-core/tests/tracked_revisions.rs::typing_where_an_insertion_was_emptied_is_plain` <br> `jade-web/tests/reviews_documents.rs::a_word_typed_a_key_at_a_time_is_rejected_whole` |
| 296 | tracked formatting | **none** |
| 297 | tracked moves & table edits | `core/jade-docx-document-core/tests/tracked_revisions.rs::a_decided_move_leaves_the_file` |
| 298 | show by reviewer | `core/jade-docx-headless-editor/src/lib.rs::execute_headless_queries_filters_revisions_by_author_and_kind` |
| 299 | markup views | `jade-web/tests/reviews_documents.rs::tracked_changes_are_drawn_as_markup` <br> `jade-docx-native-render/tests/comments_and_revisions.rs::tracked_changes_are_drawn_as_markup` |
| 300 | accept / reject | `jade-web/tests/reviews_documents.rs::tracked_changes_are_walked_and_decided_one_at_a_time` <br> `jade-web/tests/reviews_documents.rs::a_word_typed_a_key_at_a_time_is_rejected_whole` |
| 301 | visible changes only | **none** |
| 302 | lock tracking | **none** |
| 303 | comments add / delete | `jade-web/tests/reviews_documents.rs::a_comment_is_written_answered_resolved_and_deleted` <br> `jade-docx-native-render/tests/comments_and_revisions.rs::a_comment_survives_export_and_reload` |
| 304 | comment replies | `jade-web/tests/reviews_documents.rs::a_comment_is_written_answered_resolved_and_deleted` <br> `jade-docx-headless-api/src/lib.rs::reply_to_comment_parses` |
| 305 | resolve / reopen | `jade-web/tests/reviews_documents.rs::a_comment_is_written_answered_resolved_and_deleted` <br> `core/jade-docx-editor/src/headless_api.rs::query_comments_filters_by_author_and_resolved_state` |
| 306 | compare documents | `core/jade-docx-diff/tests/document.rs::a_nested_table_is_compared_inside_its_cell` |
| 307 | compare options | **none** |
| 308 | combine revisions | **none** |
| 309 | side by side | **none** |
| 310 | spelling & grammar | `core/jade-docx-editor/src/canvas_renderer.rs::spelling_waves_follow_vertical_table_text_and_clear_for_export` |
| 311 | thesaurus | **none** |
| 312 | next / previous comment | **none** |
| 313 | accessibility checker | **none** |
| 314 | alt text | **none** |
| 315 | decorative images | **none** |
| 316 | heading structure | `jade-web/tests/formats_documents.rs::the_gallery_applies_a_heading_and_normal` <br> `jade-web/tests/references_documents.rs::the_table_of_contents_is_updated_from_the_headings` |
| 317 | table header row (a11y) | `jade-docx-reader/tests/generated_fixtures.rs::table_repeating_header_fixture_parses_empty_on_and_explicit_false` <br> `jade-docx-reader/src/parser/table.rs::parses_repeating_table_header_on_off_values` |
| 318 | link display text | **none** |
| 319 | screen reader | **none** |
| 320 | keyboard navigation | `jade-web/tests/edits_documents.rs::word_and_document_moves_go_where_a_mac_takes_them` <br> `jade-gpui-app/src/main.rs::every_menu_item_has_a_shortcut` |
| 321 | read aloud | **none** |
| 322 | immersive reader | **none** |
| 323 | zoom | `jade-gpui-app/src/editor.rs::zoom_steps_through_the_stops_and_stops_at_the_ends` <br> `jade-gpui-app/src/editor.rs::zoom_is_clamped_and_never_nan` |
| 324 | dark mode | `jade-web/tests/edits_documents.rs::dark_paper_turns_the_pages_over_and_back` <br> `jade-docx-native-render/tests/native_document.rs::dark_paper_turns_the_page_over_on_screen_but_not_in_the_pdf` |
| 325 | open password | **none** |
| 326 | restrict editing | `jade-docx-editor/tests/word_schema.rs::edit_protection_is_written_where_settings_expects_it` <br> `jade-docx-reader/src/compatibility.rs::protected_text_uses_namespace_and_decoded_text` |
| 327 | restrict formatting | `jade-docx-editor/tests/word_schema.rs::edit_protection_is_written_where_settings_expects_it` <br> `jade-docx-reader/src/compatibility.rs::protected_text_uses_namespace_and_decoded_text` |
| 328 | forms-only protection | **none** |
| 329 | tracked-only protection | **none** |
| 330 | read-only recommended | **none** |
| 331 | mark as final | **none** |
| 332 | signatures | **none** |
| 333 | inspect document | **none** |
| 334 | print preview | **none** |
| 335 | print ranges | **none** |
| 336 | print markup | **none** |
| 337 | duplex / collate | **none** |
| 338 | pages per sheet | **none** |
| 339 | tagged PDF | `jade-web/tests/new_documents.rs::every_page_goes_into_the_pdf` <br> `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::renders_pdf_from_hello_fixture` |
| 340 | compatibility checker | **none** |
| 341 | .doc / RTF / text | **none** |
| 342 | customise ribbon / QAT | **none** |
| 343 | ADVANCE | **none** |
| 344 | ASK | `jade-docx-reader/src/parser.rs::parses_fragmented_ask_with_blank_default` <br> `jade-docx-editor/src/word_document.rs::extract_header_lines_hides_ask_saved_result_at_field_position` |
| 345 | AUTHOR | **none** |
| 346 | AUTONUM | `jade-docx-reader/src/parser.rs::original_autonum_fields_resolve_without_cached_result_runs` |
| 347 | AUTONUMLGL | **none** |
| 348 | AUTONUMOUT | `jade-docx-reader/src/parser.rs::autonumout_unknown_heading_format_preserves_cache_and_advances_counters` |
| 349 | AUTOTEXT | **none** |
| 350 | AUTOTEXTLIST | **none** |
| 351 | BIBLIOGRAPHY | `jade-docx-editor/src/commands/insert_citation.rs::citation_catalog_text_and_bibliography_round_trip_across_unicode_run_boundaries` |
| 352 | CITATION | `jade-docx-editor/src/headless_api.rs::query_citation_sources_returns_the_word_catalog` <br> `jade-docx-editor/src/commands/insert_citation.rs::snapshot_batches_rebase_citations_and_following_edits` |
| 353 | COMMENTS | **none** |
| 354 | COMPARE | `core/jade-docx-reader/tests/generated_fixtures.rs::if_compare_fixture_preserves_fragmented_instructions_and_live_results` |
| 355 | CREATEDATE | `core/jade-docx-reader/src/parser.rs::parses_package_backed_date_fields_without_collapsing_them_to_date` |
| 356 | DATE | `core/jade-docx-editor/tests/date_time.rs::a_date_field_keeps_its_picture` <br> `core/jade-docx-reader/src/parser.rs::parses_date_field_with_picture_switch` |
| 357 | DOCPROPERTY | `jade-docx-reader/tests/generated_fixtures.rs::document_property_fixture_retains_extended_properties_and_header_field` <br> `jade-docx-reader/src/parser.rs::parses_builtin_title_field_through_document_property_path` |
| 358 | DOCVARIABLE | `jade-docx-reader/tests/generated_fixtures.rs::document_variable_fixture_retains_settings_values_and_fragmented_fields` <br> `jade-docx-editor/src/word_document.rs::document_variable_fields_apply_word_capitalization_switches` |
| 359 | EQ | `core/jade-docx-reader/src/parser.rs::parses_eq_overstrike_ruby_field` <br> `core/jade-docx-reader/src/parser.rs::parses_eq_overstrike_distributed_text_field` |
| 360 | FILENAME | **none** |
| 361 | FILESIZE | `jade-docx-editor/src/word_document.rs::filesize_resolves_original_package_bytes_and_decimal_units` <br> `jade-docx-reader/src/parser.rs::parses_filesize_units_from_simple_and_fragmented_instructions` |
| 362 | FILLIN | `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_fillin_does_not_duplicate_editable_response` <br> `jade-docx-reader/src/parser.rs::parses_fragmented_fillin_with_omitted_prompt_and_blank_default` |
| 363 | = (formula) | `jade-web/tests/tables_documents.rs::rows_sort_and_formulas_compute` |
| 364 | HYPERLINK | `core/jade-docx-reader/src/parser.rs::parses_hyperlink_field_instruction_and_preserves_editable_result_text` <br> `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_emits_external_hyperlink_annotations` |
| 365 | IF | `jade-docx-reader/src/parser.rs::resolves_if_with_empty_quoted_operands` <br> `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_if_and_compare_prefer_recomputed_values` |
| 366 | INCLUDEPICTURE | **none** |
| 367 | INCLUDETEXT | **none** |
| 368 | INDEX | `core/jade-docx-editor/src/commands/insert_index.rs::current_state_batches_follow_prior_structural_coordinates_and_generate_index` |
| 369 | INFO | `core/jade-docx-reader/src/parser.rs::parses_read_only_info_as_a_package_property_field` |
| 370 | KEYWORDS | **none** |
| 371 | LASTSAVEDBY | **none** |
| 372 | LISTNUM | **none** |
| 373 | NOTEREF | `jade-docx-editor/src/commands/bookmark.rs::note_targets_insert_scoped_noteref_fields_with_exact_history` <br> `jade-docx-editor/src/canvas_renderer.rs::noteref_uses_layout_note_ordinals_and_retains_relative_position_cache` |
| 374 | NUMCHARS | `jade-docx-reader/src/parser.rs::parses_numwords_and_numchars_as_dedicated_statistics_fields` <br> `jade-docx-editor/src/word_document.rs::numwords_and_numchars_prefer_extended_properties_over_stale_results` |
| 375 | NUMPAGES | `jade-docx-editor/src/word_document.rs::extract_text_lines_emits_numpages_placeholder` <br> `jade-docx-reader/src/parser.rs::parses_numpages_field_instruction_from_fld_simple` |
| 376 | NUMWORDS | `jade-docx-reader/src/parser.rs::parses_numwords_and_numchars_as_dedicated_statistics_fields` <br> `jade-docx-editor/src/word_document.rs::numwords_and_numchars_prefer_extended_properties_over_stale_results` |
| 377 | PAGE | `jade-web/tests/pages_documents.rs::headers_and_footers_take_text_and_a_page_number` <br> `jade-web/tests/section_setup_documents.rs::columns_line_numbers_and_page_numbers_are_saved` |
| 378 | PAGEREF | `jade-docx-reader/src/parser.rs::parses_pageref_field_from_fld_simple` <br> `jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_pageref_resolves_bookmark_page_with_cached_fallback` |
| 379 | PRINTDATE | **none** |
| 380 | QUOTE | `core/jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_quote_prefers_instruction_literal_over_saved_result` <br> `core/jade-docx-editor/src/word_document.rs::extract_header_lines_prefers_quote_literal_over_saved_result` |
| 381 | REF | `core/jade-docx-reader/src/parser.rs::parses_ref_field_and_resolves_bookmark_text` <br> `core/jade-docx-editor/src/word_document.rs::header_ref_resolves_bookmark_from_main_document_story` |
| 382 | REVNUM | **none** |
| 383 | SAVEDATE | `core/jade-docx-reader/src/parser.rs::parses_package_backed_date_fields_without_collapsing_them_to_date` |
| 384 | SECTION | `jade-docx-editor/src/canvas_renderer.rs::section_run_field_resolves_one_based_active_section_index` <br> `jade-docx-editor/src/canvas_view.rs::section_field_placeholder_is_one_header_footer_caret_unit` |
| 385 | SECTIONPAGES | `jade-docx-reader/src/parser.rs::parses_sectionpages_instruction_split_across_instr_text_runs` <br> `jade-docx-editor/src/canvas_renderer.rs::header_footer_substitutes_sectionpages_for_the_current_section` |
| 386 | SEQ | `core/jade-docx-reader/src/parser.rs::parses_seq_fields_and_numbers_them_in_document_order` |
| 387 | SET | `core/jade-docx-editor/src/canvas_renderer.rs::resolve_run_field_set_never_displays_saved_assignment` <br> `core/jade-docx-reader/src/parser.rs::set_assignment_is_available_to_a_following_if_field` |
| 388 | STYLEREF | `jade-docx-editor/src/canvas_renderer.rs::style_reference_follows_the_page` |
| 389 | SUBJECT | **none** |
| 390 | SYMBOL | `core/jade-docx-reader/src/parser.rs::parses_symbol_char_into_run_text_and_font` <br> `core/jade-docx-reader/src/parser.rs::fragmented_symbol_field_preserves_switch_semantics_and_safe_fallbacks` |
| 391 | TA | **none** |
| 392 | TC | `core/jade-docx-reader/tests/generated_fixtures.rs::focused_tc_field_fixture_filters_hidden_entries_by_type_and_level` <br> `core/jade-docx-reader/src/parser.rs::ordinary_toc_merges_untyped_tc_fields_with_headings_in_document_order` |
| 393 | TEMPLATE | `core/jade-docx-reader/src/parser.rs::parses_template_without_path_switch_as_a_package_property_field` |
| 394 | TIME | `core/jade-docx-reader/src/parser.rs::parses_time_field_without_switch_from_fld_simple` |
| 395 | TITLE | `core/jade-docx-reader/src/parser.rs::parses_builtin_title_field_through_document_property_path` |
| 396 | TOA | **none** |
| 397 | TOC | `jade-docx-reader/tests/generated_fixtures.rs::focused_toc_direct_outline_fixture_requires_u_for_direct_formatting` <br> `jade-docx-reader/src/document.rs::toc_f_after_l_resets_the_tc_level_filter` |
| 398 | USERADDRESS | **none** |
| 399 | USERINITIALS | **none** |
| 400 | USERNAME | **none** |
| 401 | XE | `core/jade-docx-editor/src/commands/insert_index.rs::unicode_index_entries_round_trip_at_arbitrary_run_boundaries` |
| 402 | ribbon & dialog map (docs/word/ribbon-map.md) | **none** |
| 403 | web vs desktop (docs/word/web-vs-desktop.md) | **none** |
| 404 | Japanese typesetting audit (docs/word/japanese-typesetting.md) | **none** |
| 405 | enumeration audit (docs/word/enumerations.md) | **none** |
| 406 | field & switch audit (docs/word/fields.md) | **none** |
| 407 | standard + extension elements on save | `core/jade-docx-editor/tests/word_schema.rs::character_formatting_writes_run_properties_in_schema_order` <br> `core/jade-docx-writer/src/mce.rs::drops_unknown_prefixes_from_the_compatibility_lists` |
| 408 | untouched round trip & fallbacks | `core/jade-docx-editor/tests/round_trip_corpus.rs::every_fixture_saves_without_losing_anything` <br> `core/jade-docx-pdf-renderer/tests/pdf_fixtures.rs::render_pdf_renders_math_as_text_fallback` |
| 409 | coverage map (this file) | **none** |
| 410 | version & compat-mode notes (docs/word/compatibility-modes.md) | **none** |
| 411 | inline equations | `jade-docx-editor/tests/math_font_plan.rs::formula_exact_spacing_keeps_authored_height_and_unbound_plan_has_no_math_leading` <br> `jade-docx-reader/src/parser.rs::parses_inline_math_as_raw_omml` |
| 412 | display equations | `jade-docx-reader/src/parser.rs::parses_math_paragraph_as_block_element` <br> `jade-docx-reader/src/document/math.rs::math_paragraph_preview_truncates_long_text` |
| 413 | built-in equations | `core/jade-docx-editor/src/commands/insert_equation.rs::inline_equation_round_trips_across_arbitrary_unicode_run_boundaries` <br> `core/jade-docx-editor/src/commands/insert_equation.rs::block_equation_splits_paragraph_and_reopens_without_phantom_coordinate` |
| 414 | fractions | `core/jade-docx-editor/src/canvas_renderer/math.rs::fraction_metrics_contain_painted_numerator_and_denominator` <br> `core/jade-docx-layout/src/math_layout/styled.rs::styled_fraction_rules_and_children_fit_bounds_and_errors_propagate` |
| 415 | radicals | `jade-docx-layout/src/math_layout.rs::radical_fallback_keeps_existing_base_only_preview` |
| 416 | scripts & prescripts | **none** |
| 417 | integrals | `jade-docx-editor/tests/math_font_plan.rs::hidden_nary_limits_skip_layout_without_mutating_source` <br> `jade-docx-layout/tests/math_script_font.rs::display_style_keeps_nested_integrals_in_script_forms_without_display_stretch` |
| 418 | large operators | `core/jade-docx-layout/src/math_layout/styled.rs::structured_nary_delimiters_and_scripts_have_measured_utf8_glyph_ranges` |
| 419 | delimiters | `core/jade-docx-layout/src/math_layout/styled.rs::structured_nary_delimiters_and_scripts_have_measured_utf8_glyph_ranges` |
| 420 | function names | **none** |
| 421 | limits | `core/jade-docx-editor/tests/math_font_plan.rs::long_stacked_limits_reserve_left_and_right_advance` |
| 422 | accents & bars | **none** |
| 423 | matrices | **none** |
| 424 | equation arrays | **none** |
| 425 | normal text in math | **none** |
| 426 | math fonts | `jade-docx-layout/tests/math_font.rs::math_font_line_box_prefers_os2_typographic_metrics_and_falls_back_to_hhea` <br> `jade-docx-editor/src/canvas_renderer/math.rs::styled_math_font_preserves_authored_family_before_math_fallbacks` |
| 427 | UnicodeMath input | **none** |
| 428 | LaTeX input | **none** |
| 429 | linear ↔ professional | **none** |
| 430 | Math AutoCorrect | **none** |
| 431 | math line breaks | **none** |
| 432 | OMML round trip | `jade-docx-reader/src/parser.rs::parses_inline_math_as_raw_omml` <br> `jade-docx-editor/src/canvas_renderer/math.rs::block_alignment_honors_omml_justification` |
| 433 | math version matrix | **none** |
| 434 | Print Layout | `jade-web/tests/opens_every_document.rs::every_document_opens_and_its_pages_paint` |
| 435 | Web Layout | **none** |
| 436 | Draft | **none** |
| 437 | Outline view | **none** |
| 438 | Read Mode | **none** |
| 439 | Focus | **none** |
| 440 | vertical paging | `jade-web/tests/edits_documents.rs::up_and_down_cross_page_breaks` |
| 441 | side-to-side paging | **none** |
| 442 | one / many pages, page width | `jade-gpui-app/src/editor.rs::a_fitted_zoom_moves_to_the_nearest_stop` |
| 443 | zoom percentage | `jade-gpui-app/src/editor.rs::zoom_is_clamped_and_never_nan` |
| 444 | horizontal ruler | **none** |
| 445 | vertical ruler | **none** |
| 446 | ruler units | `jade-web/tests/paragraphs_documents.rs::an_english_document_measures_in_millimetres_and_hanging_indents` |
| 447 | gridlines | **none** |
| 448 | navigation pane view | `core/jade-docx-editor/src/headless_api.rs::document_outline_lists_every_navigation_pane_heading` |
| 449 | formatting marks | `jade-docx-editor/src/canvas_renderer.rs::try_prepare_seek_start_allows_formatting_marks` |
| 450 | hide white space | **none** |
| 451 | new window | **none** |
| 452 | split window | **none** |
| 453 | editing / reviewing / viewing | `jade-docx-editor/src/canvas_renderer.rs::header_editing_mode_alone_keeps_body_top_stable` |
| 454 | new blank document | `jade-web/tests/new_documents.rs::a_blank_document_opens_and_takes_typing` <br> `jade-office-model/src/word/new.rs::a_blank_document_is_a_package_word_reads` |
| 455 | new from template | `jade-office-model/src/word/new.rs::a_template_opens_as_a_document` <br> `jade-web/tests/new_documents.rs::a_template_opens_as_a_document` |
| 456 | open | `jade-web/tests/opens_every_document.rs::every_document_opens_and_its_pages_paint` |
| 457 | recent files | `jade-gpui-app/src/shell.rs::a_reopened_file_moves_to_the_front_once` <br> `jade-gpui-app/src/shell.rs::the_list_is_capped` |
| 458 | save / save as | `jade-web/tests/edits_documents.rs::a_save_carries_the_edits` <br> `jade-web/tests/section_setup_documents.rs::columns_line_numbers_and_page_numbers_are_saved` |
| 459 | save a copy | **none** |
| 460 | save as .dotx | **none** |
| 461 | templates load / detach | **none** |
| 462 | .doc conversion | **none** |
| 463 | compat mode / convert | `core/jade-docx-reader/src/settings.rs::parses_compat_flags_with_explicit_off_values` |
| 464 | ODT / HTML | **none** |
| 465 | PDF reflow | **none** |
| 466 | AutoRecover files | **none** |
| 467 | recover unsaved | **none** |
| 468 | backup copy | **none** |
| 469 | standard properties | `core/jade-docx-reader/tests/generated_fixtures.rs::document_property_fixture_retains_extended_properties_and_header_field` |
| 470 | custom properties | **none** |
| 471 | statistics | `jade-web/tests/counts_documents.rs::typing_and_selecting_change_the_count_as_word_counts` <br> `jade-office-model/src/word/count.rs::east_asian_characters_are_words_of_their_own` |
| 472 | repair on open | `jade-web/tests/damaged_documents.rs::damaged_documents_fail_to_open_without_crashing` <br> `core/jade-docx-diff/tests/document.rs::a_file_that_is_not_a_docx_is_reported_readably` |
| 473 | .docm without macros | **none** |
| 474 | pen / pencil / highlighter | **none** |
| 475 | pen options | **none** |
| 476 | eraser | **none** |
| 477 | lasso | **none** |
| 478 | move / resize ink | **none** |
| 479 | draw with touch | **none** |
| 480 | ink to shape | **none** |
| 481 | ink editor | **none** |
| 482 | mini toolbar, live preview | **none** |
| 483 | ScreenTips | **none** |
| 484 | user name / initials | `jade-docx-editor/src/lib.rs::author_name_falls_back_when_unset_or_blank` |
| 485 | overtype | **none** |
| 486 | drag & drop | `core/jade-docx-native-render/tests/header_and_objects.rs::the_moves_of_one_drag_undo_together` <br> `jade-web/tests/tables_documents.rs::borders_drag_to_resize_columns_and_rows` |
| 487 | default paste | **none** |
| 488 | AutoCorrect / AutoFormat | `core/jade-docx-editor/src/typed_autoformat.rs::language_and_capitalization_options_are_respected` |
| 489 | show marks & hidden text | `core/jade-docx-editor/src/canvas_renderer.rs::run_is_hidden_text_only_when_vanish_is_true` <br> `core/jade-docx-editor/src/canvas_renderer.rs::try_prepare_seek_start_allows_formatting_marks` |
| 490 | update before print | **none** |
| 491 | print backgrounds / hidden | **none** |
| 492 | image compression | **none** |
| 493 | AutoRecover options | **none** |
| 494 | font embedding | `core/jade-docx-reader/src/font_table.rs::parses_embedded_font_references` |
| 495 | ribbon tabs | `jade-web/tests/section_setup_documents.rs::the_ribbon_shows_and_sets_the_caret_section` <br> `jade-web/tests/paragraphs_documents.rs::the_ribbon_line_spacing_reads_back` |
| 496 | context tabs & menus | **none** |
| 497 | find | `jade-web/tests/finds_documents.rs::the_options_narrow_what_is_found` <br> `core/jade-docx-native-render/tests/search_and_paragraph_format.rs::an_empty_query_finds_nothing` |
| 498 | replace | `jade-web/tests/finds_documents.rs::replace_all_is_one_undo_step` <br> `jade-web/tests/finds_documents.rs::a_failed_batch_takes_back_what_it_did` |
| 499 | match case | `jade-office-model/src/word/find.rs::case_folds_unless_asked_not_to` <br> `jade-web/tests/finds_documents.rs::the_options_narrow_what_is_found` |
| 500 | whole words | `jade-office-model/src/word/find.rs::whole_words_skip_matches_inside_a_word` |
| 501 | wildcards | **none** |
| 502 | wildcard groups | **none** |
| 503 | special characters | **none** |
| 504 | find formatting | **none** |
| 505 | scope & direction | `jade-web/tests/finds_documents.rs::the_options_narrow_what_is_found` |
| 506 | Go To | **none** |
