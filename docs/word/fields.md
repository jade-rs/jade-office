# Word fields and switches

**Covers:** every Word field type and its switches, with whether jade
**parses** it (recognises the field), **evaluates** it (computes a result
rather than showing the one Word saved), and lets a user **insert** it.
Also the general switches (`\*`, `\#`, `\@`, `\!`) and field-wide
behaviour: lock, dirty, nesting, codes view, update, unlink.

**Derived from:** jade-office `9df8a1e` / jade-core `9fea681` (origin/main,
2026-10-22), by reading `parse_field_instruction` and its helpers in
`jade-core/crates/jade-docx-reader/src/parser.rs`, the `RunFieldKind` enum
(`jade-docx-reader/src/document.rs`), the evaluators in the reader, the
editor (`jade-docx-editor/src/word_document.rs`,
`canvas_renderer.rs::resolve_run_field`, `word_document/table_tools.rs`) and
the insert commands of the shells and APIs. Nothing was run.

**Fields stream:** a parallel stream is building a field engine (Insert ›
Field dialog, field codes view, update/lock/unlink, ASK/FILLIN prompts,
document Properties dialog, `crates/jade-gpui-app/src/word_fields.rs`).
Rows and columns it is expected to change are marked **▸F**. When that work
merges, update the marked cells from its code and tests, and drop the
marker. Do not fill them in from the plan.

**Keeping it current:** a new `RunFieldKind` variant, a new arm in
`parse_field_instruction`, a new switch in a field's parser, or a new
insert entry point each needs its cell here updated in the same change.
`grep -n 'RunFieldKind::' jade-core/crates/jade-docx-reader/src/parser.rs`
lists what is recognised.

## How jade handles a field

- **Reading.** Complex fields (`w:fldChar` begin/separate/end with
  `w:instrText`, joined across runs) and `w:fldSimple` both end in
  `parse_field_instruction`, which matches the field name
  case-insensitively. A recognised field becomes a typed `RunFieldKind`;
  anything else is kept as `Unsupported { instruction }` and **Word's saved
  result is shown**. The original field XML is always written back.
- **Evaluating** happens in one of three places:
  1. *load, from document structure* — `resolve_fields_in_paragraph`
     (`document.rs`): SET, REF, QUOTE, MACROBUTTON, GOTOBUTTON, SYMBOL,
     AUTONUM*, SEQ, COMPARE, IF, PRIVATE/ADDIN;
  2. *load, from package metadata* —
     `resolve_package_value_fields_in_elements` (`word_document.rs`):
     DOCPROPERTY and the property fields, DOCVARIABLE, NUMWORDS, NUMCHARS,
     EDITTIME, FILESIZE, CREATEDATE/SAVEDATE/PRINTDATE; numbered REF;
  3. *every paint* — `resolve_run_field` (`canvas_renderer.rs`): PAGE,
     NUMPAGES, SECTIONPAGES, SECTION, STYLEREF (headers/footers), PAGEREF,
     NOTEREF, DATE, TIME, EQ.
  Body fields evaluated at load are **not recomputed after edits** until the
  document is reopened.
- **Writing new fields.** `create_authored_field_element`
  (`word_document.rs`) writes `w:fldSimple w:dirty="true"` for the kinds
  jade inserts (PAGE, NUMPAGES, DATE/TIME, EQ enclosure, FILENAME, `=`,
  STYLEREF, REF, PAGEREF, NOTEREF, SEQ, XE); INDEX is a complex field.

Legend — **Parsed:** T typed · U kept unsupported (saved result shown) · N
non-display (shows nothing) · S structural. **Insert:** UI = both shells
(native `crates/jade-gpui-app`, browser `crates/jade-gpui-web`), native =
native shell only, API = wasm editor API
(`jade-docx-editor/src/canvas_view.rs`) and/or headless/MCP commands only, –
= no way.

## Field types

| Field | Parsed | Evaluated | Insert | Switches honoured | Switches not honoured |
|---|---|---|---|---|---|
| = (formula) | U | only in table cells, by Insert/Update Formula (`table_tools.rs`); never on load | UI (Table Layout › Formula, Update Fields) | `\#`; SUM AVERAGE COUNT MAX MIN PRODUCT ABS INT SIGN MOD ROUND AND OR NOT TRUE FALSE; ABOVE/BELOW/LEFT/RIGHT, A1 refs | formulas outside tables, bookmarks as operands |
| ADDRESSBLOCK | U | – | – | – | \b \c \d \e \f \l |
| ADVANCE | U | – | – | – | \d \l \r \u \x \y |
| ASK | T | shows nothing; prompt not shown, bookmark not set **▸F** | – **▸F** | (\d \o parsed) | \d \o applied |
| AUTHOR | T (property) | yes, core `creator`; Mac body shows the saved result, header/footer live | – **▸F** | `\*` case | new-value operand |
| AUTONUM | T | yes | – | \s \e, `\*` format and case | — |
| AUTONUMLGL | T | yes | – | \s \e | — |
| AUTONUMOUT | T | yes (headings without a format keep the saved result) | – | — | — |
| AUTOTEXT | U | – | – | – | entry name |
| AUTOTEXTLIST | U | – | – | – | \s \t |
| BARCODE, DISPLAYBARCODE, MERGEBARCODE | U | – | – | – | all |
| BIBLIOGRAPHY | U | – (Insert Bibliography writes plain text) | API (as text) | – | \l \f \m |
| BIDIOUTLINE | U | – | – | – | — |
| CITATION | U | – (Insert Citation writes plain text) | API (as text) | – | \l \f \s \p \v \m \n \t \y |
| COMMENTS | T (property) | yes, core `description` | – | `\*` case | — |
| COMPARE | T | yes; Mac body keeps the saved result | – | = <> < > <= >=, quoted operands, wildcards | — |
| CREATEDATE | T | yes, core `created` (**UTC, not local**) | – | `\@` | \h \l \s |
| DATABASE | U | – | – | – | all |
| DATE | T | yes, host clock each paint (saved result if no clock) | UI (Date and Time dialog); native header/footer menu; API | `\@` | \h \l \s |
| DOCPROPERTY | T | yes: core, app and custom properties | – **▸F** | `\*` case | — |
| DOCVARIABLE | T | yes, `settings.xml` docVars | – | `\*` case | — |
| EDITTIME | T | yes, app `TotalTime` | – | — | — |
| EQ | T for three forms, else U | draws `\o\ad(…)` (distributed), `\o\ac(○,字)` (enclosed), `\o\aX(\s\up…(guide),base)` (ruby) | UI (Enclose Characters) | \o, \a{c d l r}, \s\up, `\* jcN`, `\* hpsN` | \a(array) \b \d \f \i \l \r \x, \s\do |
| FILENAME | U | **never computed** (saved result) | native header/footer menu; API | — | \p |
| FILESIZE | T | yes, original package size | – | \k \m | — |
| FILLIN | T | saved response stays editable; no prompt **▸F** | – **▸F** | (\d \o parsed) | \d \o applied |
| FORMCHECKBOX, FORMDROPDOWN, FORMTEXT | U | – (`w:ffData` not read; no check-box glyph) | – | – | — |
| GOTOBUTTON | T | display text; bookmark target is a link | – | — | — |
| GREETINGLINE | U | – | – | – | \e \f \l |
| HYPERLINK | T | saved text, followed on ⌘/Ctrl-click | UI ⌘K writes a `w:hyperlink` element, not a field | \l \o \t | \m \n |
| IF | T | yes, incl. Word's Japanese syntax-error text; Mac body result | – | operators as COMPARE, optional false text, nested fields | — |
| INCLUDEPICTURE | U | – | – | – | \c \d |
| INCLUDETEXT | U | – | – | – | \c \! |
| INDEX | U | – (inserted body lists XE terms, no page numbers) | API | – | \b \c \d \e \f \g \h \k \l \p \r \s \y \z |
| INFO | T (read form) / U (with new value) | yes, read form | – | `\*` case | new value |
| KEYWORDS | T (property) | yes | – | `\*` case | — |
| LASTSAVEDBY | T (property) | yes, core `lastModifiedBy` | – | `\*` case | — |
| LINK | U | – | – | – | \a \b \d \f \h \p \r \t \u |
| LISTNUM | U | – | – | – | \l \s |
| MACROBUTTON | T | display text; macro never run | – | — | — |
| MERGEFIELD | T | no data source; saved text editable | – | — | \b \f \m \v |
| MERGEREC, MERGESEQ, NEXT, NEXTIF, SKIPIF | U | – | – | – | — |
| NOTEREF | T | yes, note number in the section's note format | API (cross-reference "number") | \h | \f (parsed, no styling), \p |
| NUMCHARS | T | yes, app `Characters` | – | — | `\*`, `\#` |
| NUMPAGES | T | yes, at paint | UI page-number gallery (via PAGE) / native header/footer menu; API | — | `\*` format, `\#` |
| NUMWORDS | T | yes, app `Words` | – | — | `\*`, `\#` |
| PAGE | T | yes, at paint, with the section's `w:pgNumType` format, start and chapter | UI (Insert › Page Number gallery, remove); native header/footer menu; API | section format | `\*` format (section format used instead), `\#` |
| PAGEREF | T | yes, bookmark → laid-out page | API; generated TOC entries | \h | \p, `\*` |
| PRINT | U | – | – | – | \p |
| PRINTDATE | T | yes, core `lastPrinted` (UTC) | – | `\@` | \h \l \s |
| PRIVATE / ADDIN | N | empty | – | — | — |
| QUOTE | T | yes, the literal | – | `\*` case | — |
| RD | U | – | – | – | \f |
| REF | T | yes, bookmark text in document order with SET; `\n \r \w` give the paragraph's list label | API (cross-reference "text") | `\*` case, \h; \n \r \w (all give the full label) | \d \f \p \t; distinct \n / \r / \w |
| REVNUM | T (property) | yes | – | `\*` case | — |
| SAVEDATE | T | yes, core `modified` (UTC) | – | `\@` | \h \l \s |
| SECTION | T | yes, at paint | – | — | `\*` |
| SECTIONPAGES | T | yes, at paint | – | — | `\*` |
| SEQ | T | yes, document order | API (Insert Caption → `SEQ label \* ARABIC`) | \c \h \r \s, bookmark operand, `\*` ARABIC ROMAN roman ALPHABETIC alphabetic Hex Ordinal CardText OrdText DollarText (English) + case | \n (default behaviour anyway); `\* Roman`/`Alphabetic` mixed-case spellings |
| SET | T | assigns, shows nothing | – | — | — |
| STYLEREF | T | headers/footers: first (or last with \l) paragraph in the style on the page; body: saved result | native header/footer menu (always "heading 1"); headless | \l | \n \p \r \t \w |
| SUBJECT | T (property) | yes | – | `\*` case | — |
| SYMBOL | T | yes | – (Insert Symbol inserts the character) | \a \f \j \s \u | \h |
| TA | N | empty | – | — | \b \c \i \l \r \s |
| TC | S | feeds TOC | – | \f \l \n | — |
| TEMPLATE | T (property) / U with \p | yes, app `Template` | – | — | \p (kept as saved result) |
| TIME | T | yes, host clock | UI (Date and Time dialog, time pictures); API | `\@` | — |
| TITLE | T (property) | yes | – | `\*` case | — |
| TOA | U | – | – | – | \c \e \f \g \h \l \p \s |
| TOC | S | regenerated only by Insert/Update (first TOC in the document only) | UI (References › Table of Contents, Update Table); API | \o \h \n \u \w \x \b \c \a \f \l \t \p \s \d | \z (written, no effect) |
| USERADDRESS | U | – **▸F** | – | – | — |
| USERINITIALS | U | – **▸F** | – | – | — |
| USERNAME | U | – **▸F** | – | – | — |
| XE | N | empty; term read for INDEX | API (Mark Entry) | term | \b \f \i \r \t \y |

## General switches

| Switch | jade |
|---|---|
| `\* MERGEFORMAT` | not interpreted; results keep the formatting of Word's saved result runs, which is what MERGEFORMAT asks for. jade writes it on DATE/TIME. |
| `\* CHARFORMAT` | not applied |
| `\* Upper`, `Lower`, `FirstCap`, `Caps` | DOCPROPERTY and the property fields, DOCVARIABLE, REF, QUOTE, SEQ, AUTONUM* only (`parse_field_text_case`); not MERGEFIELD, DATE, STYLEREF, PAGE… **▸F** |
| `\* ARABIC`, `ROMAN`/`roman`, `ALPHABETIC`/`alphabetic`, `Hex`, `Ordinal`, `CardText`, `OrdText`, `DollarText` | SEQ and AUTONUM* only; exact case (the mixed-case `Roman`, `Alphabetic` are not recognised); text forms need an English `w:lang` **▸F** |
| `\* DBCHAR`, `SBCHAR`, `KANJINUM1/2/3`, `CIRCLENUM`, `GB1`–`GB4`, `Aiueo`, `Iroha`, `Ganada`, `Chosung`, … | not supported for fields (the list formatter has several of these, unused here) **▸F** |
| `\#` numeric picture | `=` formulas in tables only **▸F** |
| `\@` date-time picture | yes (`jade-docx-reader/src/field_datetime.rs::format_word_date`): yyyy yy, MMMM MMM MM M, dddd ddd dd d, HH H hh h, mm m, ss s, AM/PM, quoted literals, Japanese `aaaa`/`aaa` (weekday), `ggg`/`gg`/`g` (era), `ee`/`e` (era year). Not: `EEEE`/`E`, `O…`, `午前/午後`, 元年 (year 1 prints as 1); `am/pm` prints in capitals |
| `\!` (lock result of included text) | not handled **▸F** |

## Field-wide behaviour

| Behaviour | jade | |
|---|---|---|
| Lock (`w:fldLock`) | read on both field forms; a locked field shows its saved result, skips evaluation and is written back locked | no lock/unlock command **▸F** |
| Dirty (`w:dirty`) | never read; written `true` on fields jade creates; `w:updateFields` kept only | **▸F** |
| Nested fields | yes: inner results become outer operands, IF/COMPARE operands deferred until known, results with spaces quoted | |
| Show field codes (⌥F9 / Alt+F9) | – | **▸F** |
| Update one / all (F9, update before print) | – (only Update Table of Contents, Update table formulas, DATE/TIME and page fields at paint) | **▸F** |
| Unlink (⌘⇧F9 / Ctrl+Shift+F9) | – | **▸F** |
| Insert › Field dialog | – | **▸F** |
| Editing next to fields | typing, deleting, splitting and joining paragraphs keep field codes whole (many tests in `jade-docx-editor/src/commands/{insert_text,delete_text}.rs`) | |
| Print (`jade-docx-pdf-renderer`) | treats PAGE, NUMPAGES, PAGEREF, REF, SEQ, DATE, CREATEDATE, SAVEDATE, PRINTDATE, TIME as supported and warns for the rest | |

## Tests that pin fields

All in `jade-core/crates/` unless noted; file :: test.

- Parsing and load-time evaluation — `jade-docx-reader/src/parser.rs` ::
  `parses_page_field_instruction_from_fld_simple`,
  `parses_numpages_field_instruction_from_fld_simple`,
  `parses_numwords_and_numchars_as_dedicated_statistics_fields`,
  `parses_edittime_as_a_dedicated_package_statistic_field`,
  `parses_filesize_units_from_simple_and_fragmented_instructions`,
  `parses_sectionpages_instruction_split_across_instr_text_runs`,
  `parses_template_without_path_switch_as_a_package_property_field`,
  `parses_read_only_info_as_a_package_property_field`,
  `parses_builtin_title_field_through_document_property_path`,
  `parses_and_resolves_if_and_compare_field_instructions`,
  `if_field_resolves_bookmark_operand_and_omitted_false_text`,
  `field_lock_false_and_omitted_keep_live_values_and_attribute_order_is_irrelevant`,
  `parses_hyperlink_field_instruction_and_preserves_editable_result_text`,
  `macrobutton_uses_instruction_display_text_instead_of_stale_saved_result`,
  `gotobutton_uses_display_text_and_links_only_bookmark_destinations`,
  `parses_mergefield_name_without_hiding_its_editable_result`,
  `parses_fillin_prompt_default_and_ask_once_without_hiding_response`,
  `parses_ask_contract_without_displaying_its_saved_result`,
  `symbol_field_resolves_hex_unicode_instead_of_a_stale_saved_result`,
  `shift_jis_symbol_field_decodes_single_and_double_byte_values`,
  `set_assignments_update_following_ref_fields_in_document_order`,
  `preserves_unknown_simple_field_instruction_and_cached_result`,
  `models_private_and_addin_as_non_display_fields`,
  `parses_ref_field_and_resolves_bookmark_text`,
  `ref_fields_apply_word_capitalization_switches_to_bookmark_text`,
  `noteref_fields_keep_results_and_hyperlinks_in_simple_and_complex_forms`,
  `parses_eq_overstrike_distributed_text_field`, `parses_eq_overstrike_ruby_field`,
  `eq_overstrike_of_an_enclosure_is_an_enclosed_character`,
  `captures_toc_field_switches_and_generates_outline_from_headings`,
  `toc_f_and_l_switches_collect_hidden_tc_fields`,
  `autonum_counts_paragraphs_once_and_keeps_table_order_and_heading_resets`,
  `seq_general_number_formats_match_word_case_and_sequence_rules`,
  `parses_date_field_with_picture_switch`,
  `parses_package_backed_date_fields_without_collapsing_them_to_date`,
  `extract_field_format_switch_handles_quoted_unquoted_and_missing`.
- TOC switches — `jade-docx-reader/src/document.rs` ::
  `parse_toc_switches_reads_outline_range_and_flags`,
  `parse_toc_switches_accepts_locale_semicolon_style_mapping`.
- Date pictures — `jade-docx-reader/src/field_datetime.rs` ::
  `formats_japanese_weekdays_and_eras`, `formats_word_picture_tokens_and_literals`.
- Paint-time results — `jade-docx-editor/src/canvas_renderer.rs` ::
  `locked_page_fields_preserve_saved_empty_and_formatted_values`,
  `resolve_run_field_pageref_resolves_bookmark_page_with_cached_fallback`,
  `resolve_run_field_datetime_uses_clock_with_cached_fallback`,
  `resolve_run_field_if_and_compare_prefer_recomputed_values`,
  `sectionpages_run_field_resolves_against_the_current_physical_section`,
  `section_run_field_resolves_one_based_active_section_index`.
- Package values and round trip — `jade-docx-editor/src/word_document.rs` ::
  `document_property_fields_apply_word_capitalization_switches`,
  `filesize_resolves_original_package_bytes_and_decimal_units`,
  `load_generated_docvariable_fields_prefers_settings_values_in_every_story`,
  `numbered_ref_shows_the_bookmarked_paragraphs_current_number`,
  `locked_simple_and_complex_field_wrappers_survive_text_edits_and_roundtrip`,
  `page_number_gallery_creates_live_fields_for_every_role_and_alignment`.
- Insertion — `jade-docx-editor/src/commands/insert_field.rs` ::
  `fields_replace_arbitrary_unicode_run_ranges_and_round_trip`;
  `jade-docx-editor/src/commands/insert_toc.rs` ::
  `update_table_of_contents_regenerates_existing_toc`;
  `jade-docx-editor/tests/date_time.rs` :: `a_date_field_keeps_its_picture`;
  `crates/jade-gpui-app/src/word_story.rs` :: `the_footer_takes_fields_and_is_saved_while_open`;
  `crates/jade-web/tests/references_documents.rs` ::
  `the_table_of_contents_is_updated_from_the_headings`;
  `jade-docx-headless-mcp/src/server.rs` :: `export_preserves_field_codes`.

## Gaps found

- No Insert › Field dialog, field-codes view, update-all, lock/unlock or
  unlink (all **▸F**); body REF/SEQ/IF results go stale after edits.
- `\*` number formats are ignored on PAGE, NUMPAGES, SECTIONPAGES, PAGEREF
  and NOTEREF; `\#` works only in table formulas; Japanese `\*` formats
  (DBCHAR, KANJINUM…, CIRCLENUM) are not supported.
- CREATEDATE/SAVEDATE/PRINTDATE show UTC.
- FILENAME is never computed; USER* fields and LISTNUM are not evaluated;
  legacy form fields are not drawn.
- Update Table of Contents updates only the first TOC.
- REF `\n`, `\r`, `\w` all give the same label; REF `\p`, `\f`, `\t`,
  `\d` are ignored.
- Mail-merge fields have no data source (see the mail-merge rows of the
  reference list).
