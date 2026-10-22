# Word ribbon and dialog map

**Covers:** Word's ribbon tabs (Home, Insert, Draw, Design, Layout,
References, Mailings, Review, View, Developer), its context tabs and main
dialogs, each command mapped to jade's equivalent in the native shell
(`crates/jade-gpui-app`, **N**) and the browser shell
(`crates/jade-gpui-web`, **B**), with keyboard shortcuts for Word for
Windows, Word for Mac and jade, and where Word for Mac and Word for
Windows differ.

**Derived from:** jade-office `9df8a1e` (origin/main, 2026-10-22): the tab
and group builders (`crates/jade-gpui-app/src/main.rs` `ribbon_tabs` /
`ribbon_groups`, `ribbon.rs`, `word_*.rs`, `word_characters/`;
`crates/jade-gpui-web/src/ribbon.rs`, `view.rs`, `view/`, `word_*.rs`), the
key bindings (`crates/jade-gpui-app/src/main.rs` `key_bindings`; browser
`DocView::handle_key` in `crates/jade-gpui-web/src/view.rs`), and the label
keys in `crates/jade-gpui-app/i18n/{en,ja}.tsv` (shared by both shells).
Word's side lists the commands of Word for Microsoft 365 (Windows) and Word
for Mac 16 with their default shortcuts as Microsoft documents them; check
a shortcut against the current build before relying on it. Nothing was
run.

**Keeping it current:** when a shell gains a command, fill its N/B cell and
key; when a shortcut is bound, add it to "Shortcuts". The native test
`main.rs::no_keystroke_is_bound_twice` guards against duplicates. Other
streams (fields, lists/styles, tables, objects, references, mail merge,
content controls) will add commands to rows marked **–** here; update them
from merged code only.

Legend: **N** native shell · **B** browser shell · ✓ present · – absent ·
"ribbon" = button only, no shortcut. Keys are written ⌘ ⌥ ⌃ ⇧ for Mac and
Ctrl/Alt/Shift for Windows. The browser shell accepts Ctrl wherever ⌘ is
listed.

## Tabs at a glance

| Word tab | N | B | jade label key | Notes |
|---|---|---|---|---|
| File (Windows Backstage) / File menu (Mac) | menu bar | title-bar buttons | `menu.file.*` | see "Menus" |
| Home | ✓ | ✓ | `doc.tab.home` | browser has no Clipboard group |
| Insert | ✓ | ✓ | `doc.tab.insert` | browser has no Blank Page / Comment groups |
| Draw | – | – | — | no ink at all |
| Design | ✓ (Page Background only) | – (under Layout) | `doc.tab.design` | no themes, style sets, watermark |
| Layout | ✓ | ✓ | `doc.tab.layout` | |
| References | ✓ (TOC, notes) | ✓ | `doc.tab.references` | no citations, captions, index, TOA |
| Mailings | – | – | — | |
| Review | ✓ | ✓ | `doc.tab.review` | no proofing, compare, protect |
| View | ✓ | ✓ (marks, panes, zoom) | `doc.tab.view` | native: all of Word's groups but Macros; see "View" |
| Developer | – | – | — | |
| Table Design (context) | ✓ | ✓ | `doc.tab.table_design` | shown with the caret in a table |
| Table Layout (context) | ✓ | ✓ | `doc.tab.table_layout` | |
| Header & Footer (context) | ✓ | – (one-line bar) | `story.tab` | shown while a header/footer is open |
| Outlining (context) | ✓ | – | `outline.tab` | shown in Outline view (`word_views/outline.rs`) |
| Picture Format (context) | – (wrap buttons appear on Insert when a picture is selected) | – (same) | `picture.wrap.*` | |
| Shape Format, Text Box, Equation, Chart, SmartArt (context) | – | – | — | |

Right-click (or ⌃-click) opens Word's context menu in the native shell — text, fields, links, tables, pictures and shapes, headers and footers (`word_views/context.rs` over `jade_office_model::word::context_menu`). The browser has none yet.

## Home

| Word group › command | Win | Mac | N | B | jade key / handler |
|---|---|---|---|---|---|
| Clipboard › Paste | Ctrl+V | ⌘V | ✓ ⌘V, ribbon | ⌘V (browser paste event) | `doc.paste` |
| Clipboard › Paste options (keep source / merge / text only) | ribbon | ribbon | ✓ ribbon | – | `chars.paste.keep_source/merge/text_only` |
| Clipboard › Paste Special | Ctrl+Alt+V | ⌃⌘V | ✓ ⌃⌘V | – | `chars.paste.title_menu` |
| Paste and Match Formatting | — | ⌥⇧⌘V | ✓ ⌥⇧⌘V | – | `PasteMatchFormatting` |
| Clipboard › Cut / Copy | Ctrl+X / Ctrl+C | ⌘X / ⌘C | ✓ | ✓ (keys only) | `doc.cut`, `doc.copy` |
| Clipboard › Format Painter | Ctrl+Shift+C / Ctrl+Shift+V | ⌘⇧C / ⌘⇧V | ✓ | ✓ (Editing group) | `painter.title` |
| Font › font, size | Ctrl+Shift+F, Ctrl+Shift+P | ⌘⇧F, ⌘⇧P | ✓ ribbon | ✓ ribbon | `ribbon.font_family`, `ribbon.font_size` |
| Font › grow / shrink | Ctrl+Shift+> / < | ⌘⇧> / < | ✓ ribbon | ✓ ribbon | `doc.grow_title`, `doc.shrink_title` |
| Font › Change Case | Shift+F3 | ⌘⌥C | ✓ ribbon (9 modes incl. 半角/全角/カタカナ/ひらがな) | ✓ | `chars.case.title_tip` |
| Font › Clear Formatting | Ctrl+Space | ⌃Space | ✓ ribbon | ✓ | `doc.clear_format_title` |
| Font › Phonetic Guide (ルビ, Japanese UI) | ribbon | ribbon | ✓ | ✓ | `chars.ruby.title` |
| Font › Enclose Characters (囲い文字) | ribbon | ribbon | ✓ | ✓ | `chars.enclose.title` |
| Font › Bold / Italic / Underline | Ctrl+B / I / U | ⌘B / I / U | ✓ | ✓ | `doc.bold_title` … |
| Font › underline style and colour | ribbon | ribbon | ✓ | ✓ | `chars.font.underline` |
| Font › Strikethrough | ribbon | ⌘⇧X | ✓ ribbon | ✓ | `doc.strike_title` |
| Font › Subscript / Superscript | Ctrl+= / Ctrl+Shift+= | ⌘= / ⌘⇧= | ✓ ribbon (⌘= is zoom in jade) | ✓ | `doc.subscript_title`, `doc.superscript_title` |
| Font › Text Effects | ribbon | ribbon | in Font dialog | in Font dialog | `chars.font.title` |
| Font › Highlight | ribbon | ribbon | ✓ (15 colours, no white) | ✓ | `doc.highlight_title` |
| Font › Font Color | ribbon | ribbon | ✓ (auto, theme, standard, more) | ✓ | `doc.font_color_title` |
| Font › Character Shading / Border (Japanese UI) | ribbon | ribbon | ✓ | ✓ | `chars.shading`, `chars.border` |
| Font › Font dialog | Ctrl+D | ⌘D | ✓ ⌘D | ✓ ⌘D | `chars.font.launcher` |
| Paragraph › Bullets / Numbering | Ctrl+Shift+L (bullets) | ⌘⇧L (bullets) | ✓ ribbon (⇧⌘L is align left in jade) | ✓ | `doc.bullets`, `doc.numbering` |
| Paragraph › Multilevel List, Define New List | ribbon | ribbon | – | – | — |
| Paragraph › Decrease / Increase Indent | Ctrl+Shift+M / Ctrl+M | see note | ✓ ⌘[ / ⌘] | ✓ ribbon | `doc.indent_less_title`, `doc.indent_more_title` |
| Paragraph › Asian Layout (縦中横, 組み文字, 割注, 文字の拡大/縮小) | ribbon | ribbon | ✓ | ✓ | `chars.asian.title` |
| Paragraph › Sort | ribbon | ribbon | – (tables only) | – | — |
| Paragraph › Show/Hide ¶ | Ctrl+Shift+8 | ⌘8 | View › Marks | View › Marks | `doc.marks` |
| Paragraph › Align left / center / right / justify | Ctrl+L / E / R / J | ⌘L / E / R / J | ✓ ⇧⌘L / ⇧⌘E / ⇧⌘R, justify ribbon | ✓ ribbon | `doc.align_left_title` … |
| Paragraph › Distributed (均等割り付け) | Ctrl+Shift+J | ⌘⇧J | – | – | — |
| Paragraph › Line spacing | Ctrl+1 / 2 / 5 | ⌘1 / 2 / 5 | ✓ ribbon (1.0, 1.15, 1.5, 2.0) | ✓ | `doc.line_spacing_title` |
| Paragraph › Shading | ribbon | ribbon | ✓ (4 swatches) | ✓ | `doc.shading_title` |
| Paragraph › Borders | ribbon | ribbon | ✓ (bottom, top, box, none) | ✓ | `ribbon.paragraph_borders` |
| Paragraph › Paragraph dialog | ribbon / Alt key tips | ⌥⌘M | ✓ ⌥⌘M | ✓ ⌥⌘M | `paragraph.button_title` |
| Styles › gallery | Ctrl+Alt+1/2/3 (headings), Ctrl+Shift+N (Normal) | ⌘⌥1/2/3, ⌘⇧N | ✓ ribbon | ✓ | `doc.style.*`, `ribbon.styles_more` |
| Styles › Styles pane, Create/Modify Style | Ctrl+Alt+Shift+S | ribbon | – | – | — |
| Editing › Find | Ctrl+F | ⌘F | ✓ ⌘F (bar) | ✓ ⌘F (bar) | `doc.find.*` |
| Editing › Replace | Ctrl+H | ⌃H | ✓ ⌃H | ✓ ⇧⌘H | `doc.find.*` |
| Editing › Advanced Find and Replace (More ▼: wildcards, ^ codes, format, style, direction, scope, prefix/suffix, width, kana, punctuation, spaces) | ribbon | Edit menu | ✓ ⌥⇧⌘H, Edit menu | ≈ find bar's ⋯ row (no format/scope) | `find.*` |
| Editing › Go To | Ctrl+G / F5 | ⌘⌥G | – (⌘G is Find Next in jade) | – | — |
| Editing › Select All / Select Paragraph | Ctrl+A | ⌘A | ✓ ⌘A, ⌥⌘A | ✓ ⌘A, triple-click | — |
| Undo / Redo (Quick Access) | Ctrl+Z / Ctrl+Y | ⌘Z / ⌘Y | ✓ ⌘Z / ⇧⌘Z, ribbon | ✓ ⌘Z / ⇧⌘Z / ⌘Y | `doc.undo_title`, `doc.redo` |

Word for Mac's indent shortcuts are not listed here (not verified); jade
uses the macOS text convention ⌘[ / ⌘].

## Insert

| Word group › command | Win | Mac | N | B | jade key / handler |
|---|---|---|---|---|---|
| Pages › Cover Page | ribbon | ribbon | ✓ (Simple, Banner, Lined) | ✓ | `setup.cover` |
| Pages › Blank Page | ribbon | ribbon | ✓ | – | `doc.blank_page` |
| Pages › Page Break | Ctrl+Enter | ⌘↩ | ✓ ⌘↩ | ✓ ⌘↩ | `setup.break.*` |
| Tables › Table (grid, Insert Table…) | ribbon | ribbon | ✓ grid 10×8 | ✓ | `ribbon.insert_table` |
| Tables › Draw Table | ribbon | ribbon | ✓ | ✓ | `table.draw` |
| Tables › Convert Text to Table | ribbon | ribbon | ✓ | ✓ | `table.to_table` |
| Tables › Excel Spreadsheet, Quick Tables | ribbon | ribbon | – | – | — |
| Illustrations › Pictures | ribbon | ribbon | ✓ | ✓ (picker or drop) | `picture.insert` |
| Illustrations › Shapes, Icons, 3D Models, SmartArt, Chart, Screenshot | ribbon | ribbon | – | – | — |
| Media › Online Video | ribbon | ribbon | – | – | — |
| Links › Link | Ctrl+K | ⌘K | ✓ ⌘K | ✓ ⌘K | `link.button` |
| Links › Bookmark, Cross-reference | ribbon | ribbon | – | – | — |
| Comments › Comment | Ctrl+Alt+M | ⌘⌥A | ✓ ⌘M | Review tab only | `doc.comment` |
| Header & Footer › Header / Footer | ribbon | ribbon | ✓ (in place) | ✓ (one-line bar) | `page.header`, `page.footer` |
| Header & Footer › Page Number (positions, format, remove) | Alt+Shift+P (field) | ⌃⇧P (field) | ✓ gallery | ✓ gallery | `page.number.*`, `setup.numbers.open` |
| Text › Text Box, Quick Parts, WordArt, Drop Cap, Signature Line, Object | ribbon | ribbon | – | – | — |
| Text › Date & Time | Alt+Shift+D (field) | ⌃⇧D (field) | ✓ ⌃⇧D opens the dialog | ✓ ribbon | `chars.date.title_menu` |
| Symbols › Equation | Alt+= | ⌃= | – | – | — |
| Symbols › Symbol | ribbon | ribbon | ✓ ⌥⇧⌘S | ✓ ribbon | `chars.symbol.title_menu` |

## Draw

| Word command | N | B |
|---|---|---|
| Pens, Pencil, Highlighter, Eraser, Lasso Select, Ink to Shape, Ink to Math, Ink Replay, Draw with Touch | – | – |

jade has no ink model; "Draw" in jade is only the table pencil and eraser
(Table Layout › Draw).

## Design

| Word group › command | N | B | jade key |
|---|---|---|---|
| Document Formatting › Themes, Style Sets, Colors, Fonts, Paragraph Spacing, Effects, Set as Default | – | – | — |
| Page Background › Watermark | – | – | — |
| Page Background › Page Color | ✓ | ✓ (under Layout) | `setup.page_color` |
| Page Background › Page Borders | ✓ | ✓ (under Layout) | `setup.borders.open` |

## Layout

| Word group › command | N | B | jade key |
|---|---|---|---|
| Page Setup › Margins (presets, Custom) | ✓ Normal, Narrow, Moderate, Wide, Custom… | ✓ | `page.margins`, `setup.margins.custom` |
| Page Setup › Orientation | ✓ | ✓ | `page.portrait`, `page.landscape` |
| Page Setup › Size | ✓ + More Paper Sizes… | ✓ | `page.size`, `setup.paper.more` |
| Page Setup › Columns | ✓ One/Two/Three/Left/Right, More Columns… | ✓ | `page.columns` |
| Page Setup › Breaks (page, column, text wrapping; next page, continuous, even, odd) | ✓ | ✓ | `page.breaks`, `setup.break.*`, `page.break.*` |
| Page Setup › Line Numbers | ✓ | ✓ | `setup.lines` |
| Page Setup › Hyphenation | – | – | — |
| Page Setup › Text Direction (East Asian UI) | ✓ Horizontal/Vertical, options | ✓ | `setup.direction` |
| Page Setup › 原稿用紙設定 (Japanese UI) | – | – | — |
| Page Setup › Page Setup dialog (文字数と行数) | ✓ | ✓ | `setup.open` |
| Paragraph › Indent / Spacing boxes | ≈ Indent −/+ buttons | – | `doc.indent_less`, `doc.indent_more` |
| — › Left-to-Right / Right-to-Left | ✓ | ✓ | `setup.ltr`, `setup.rtl` |
| — › Snap to Grid | ✓ | ✓ | `setup.snap` |
| Arrange › Position, Wrap Text | ✓ wrap buttons for a selected picture | ✓ | `picture.wrap.*` |
| Arrange › Bring Forward, Send Backward, Selection Pane, Align, Group, Rotate | – | – | — |

The native Layout tab's "Page Break" button (`doc.page_break`) inserts a
page break at the caret; 空白のページ stays under 挿入.

## References

| Word group › command | Win | Mac | N | B | jade key |
|---|---|---|---|---|---|
| Table of Contents › Table of Contents | ribbon | ribbon | ✓ (`TOC \o "1-3" \h \z \u`) | ✓ | `doc.toc` |
| Table of Contents › Add Text | ribbon | ribbon | – | – | — |
| Table of Contents › Update Table | F9 | F9 | ✓ ribbon | ✓ | `doc.toc_update` |
| Footnotes › Insert Footnote | Ctrl+Alt+F | ⌘⌥F | ✓ ⌥⌘F | ✓ ⌥⌘F | `note.footnote` |
| Footnotes › Insert Endnote | Ctrl+Alt+D | ⌘⌥E | ✓ ⌥⌘D | ✓ ⌥⌘D | `note.endnote` |
| Footnotes › Next Footnote, Show Notes, Footnote dialog | ribbon | ribbon | – | – | — |
| Citations & Bibliography › Insert Citation, Manage Sources, Style, Bibliography | ribbon | ribbon | – | – | — |
| Captions › Insert Caption, Table of Figures, Update, Cross-reference | ribbon | ribbon | – | – | — |
| Index › Mark Entry, Insert Index, Update | Alt+Shift+X (mark) | ⌘⌥⇧X (mark) | – | – | — |
| Table of Authorities › Mark Citation, Insert, Update | Alt+Shift+I (mark) | — | – | – | — |

Several of these exist as editor or headless commands without a shell
button (captions, cross-references, index, citations as text) — see
[fields.md](fields.md).

## Mailings

| Word command | N | B |
|---|---|---|
| Envelopes, Labels | – | – |
| Start Mail Merge, Select Recipients, Edit Recipient List | – | – |
| Highlight Merge Fields, Address Block, Greeting Line, Insert Merge Field, Rules, Match Fields, Update Labels | – | – |
| Preview Results, record navigation, Find Recipient, Check for Errors | – | – |
| Finish & Merge | – | – |

Merge fields in a received document are drawn with Word's saved results.

## Review

| Word group › command | Win | Mac | N | B | jade key |
|---|---|---|---|---|---|
| Proofing › Spelling & Grammar | F7 | ⌥⌘L | – | – | — |
| Proofing › Thesaurus | Shift+F7 | ⌃⌥⌘R | – | – | — |
| Proofing › Word Count | Ctrl+Shift+G | ⌘⇧G | status bar | status bar | — |
| Speech › Read Aloud; Accessibility; Language; Translate | ribbon | ribbon | – | – | — |
| Comments › New Comment | Ctrl+Alt+M | ⌘⌥A | ✓ ⌘M | ✓ ribbon | `review.new_comment` |
| Comments › Delete, Previous, Next, Show Comments, Reply, Resolve | ribbon | ribbon | ✓ (pane cards; Show Comments) | ✓ | `review.*` |
| Tracking › Track Changes | Ctrl+Shift+E | ⌘⇧E | ✓ ⇧⌘T, ribbon | ✓ ribbon | `doc.track` |
| Tracking › Display for Review, Show Markup, Reviewing Pane | ribbon | ribbon | – | – | — |
| Changes › Accept / Reject (and move to next), all | ribbon | ribbon | ✓ | ✓ | `review.accept`, `review.reject`, `doc.accept_all`, `doc.reject_all` |
| Changes › Previous / Next | ribbon | ribbon | ✓ | ✓ | `review.previous`, `review.next` |
| Compare › Compare, Combine | ribbon | ribbon | – | – | — |
| Protect › Block Authors, Restrict Editing | ribbon | ribbon | – | – | — |

## View

| Word group › command | Win | Mac | N | B | jade key |
|---|---|---|---|---|---|
| Views › Read Mode | ribbon | — (Windows only) | ✓ screen-sized columns, ← → / Esc | – | `views.read` |
| Views › Print Layout | ribbon | ribbon | ✓ | ✓ (the only view) | `views.print` |
| Views › Web Layout | ribbon | ribbon | ✓ wrapped to the window | – | `views.web` |
| Views › Outline (Outlining tab: level, promote/demote, move up/down, expand/collapse, Show Level) | ribbon | ribbon | ✓ | – | `views.outline`, `outline.*` |
| Views › Draft | ribbon | ribbon | ✓ text bands, dotted page breaks | – | `views.draft` |
| Immersive › Focus | ribbon | ribbon | ✓ full screen, Esc | – | `views.focus` |
| Immersive › Immersive Reader | ribbon | ribbon | – (review stream) | – | — |
| Page Movement › Vertical / Side to Side | ribbon | ribbon | ✓ | vertical only | `views.vertical`, `views.side_to_side` |
| Show › Ruler | ribbon | ribbon | ✓ | – | `word.ruler.show` |
| Show › Gridlines | ribbon | ribbon | ✓ (settings' drawing grid) | – | `views.gridlines` |
| Show › Navigation Pane | ribbon | ⌃⌘N | ✓ | ✓ | `nav.title` |
| Show › formatting marks | Ctrl+Shift+8 | ⌘8 | ✓ View › Marks | ✓ View › Marks | `doc.marks` |
| Show › hide white space (double-click between pages) | double-click | double-click | ✓ | – | `views.white_space` |
| Zoom › Zoom dialog, 100%, One Page, Multiple Pages, Page Width | ribbon | ribbon | ✓ (fits follow the window) | ≈ −/100%/+/page width | `views.zoom.*`, `views.one_page`, `views.multiple_pages`, `views.page_width` |
| Window › New Window, Split | ribbon | ribbon | ✓ (`名前:2`; two panes) | – | `views.new_window`, `views.split` |
| Window › Arrange All, View Side by Side, Switch Windows | ribbon | ribbon | – | – | — |
| Editing / Reviewing / Viewing (top right) | ribbon | ribbon | ✓ strip's right | – | `modes.*` |
| (jade) previous / next page | — | — | ✓ PageUp/PageDown, ribbon | – | `doc.previous`, `doc.next` |
| Macros | Alt+F8 | ⌥F8 | – | – | — |

## Developer

| Word command | N | B |
|---|---|---|
| Visual Basic, Macros, Record Macro, Macro Security, Add-ins | – (macro-enabled files open and save; macros never run) | – |
| Controls › Rich Text, Plain Text, Picture, Building Block Gallery, Check Box, Combo Box, Drop-Down, Date Picker, Repeating Section | – | – |
| Controls › Legacy Tools, Design Mode, Properties, Group | – | – |
| Mapping › XML Mapping Pane; Protect; Templates › Document Template | – | – |

## Context tabs

### Table Design

| Word command | N | B | jade key |
|---|---|---|---|
| Table Style Options (Header Row, Total Row, Banded Rows, First Column, Last Column, Banded Columns) | ✓ | ✓ | `table.look.*` |
| Table Styles gallery, Modify / Clear | ✓ gallery, clear | ✓ | `table.more_styles`, `table.style_clear` |
| Shading | ✓ | ✓ | `table.shading` |
| Border Styles, Line Style, Line Weight, Pen Color | ✓ (8 line styles) | ✓ | `table.pen_style`, `table.pen_width`, `table.pen_color` |
| Borders menu, Borders and Shading dialog | ✓ (10 sets) | ✓ | `table.border_sets`, `table.borders_dialog` |
| Border Painter / Draw Table | ✓ Draw Table | ✓ | `table.draw` |

### Table Layout

| Word command | N | B | jade key |
|---|---|---|---|
| Select (cell, column, row, table) | ✓ | ✓ | `table.select` |
| View Gridlines | – | – | — |
| Properties | ✓ | ✓ | `table.properties` |
| Draw Table, Eraser | ✓ | ✓ | `table.draw`, `table.eraser` |
| Delete (cells, columns, rows, table) | ✓ | ✓ | `ribbon.row_delete`, `ribbon.column_delete`, `ribbon.table_delete`, `table.delete_cells` |
| Insert Above / Below / Left / Right | ✓ (⇧⌘↩ inserts a row below) | ✓ | `ribbon.row_above` … |
| Insert Cells (dialog) | ✓ | ✓ | `table.insert_cells` |
| Merge Cells, Split Cells, Split Table | ✓ | ✓ | `ribbon.merge`, `ribbon.unmerge`, `table.split_cells`, `ribbon.table_split` |
| AutoFit (contents, window, fixed) | ✓ | ✓ | `table.autofit` |
| Height / Width, Distribute Rows / Columns | ✓ | ✓ | `table.height`, `table.width`, `table.distribute_*` |
| Alignment (9-way), Text Direction, Cell Margins | ✓ | ✓ | `table.align.*`, `table.text_direction`, `table.cell_margins` |
| Sort, Repeat Header Rows, Convert to Text, Formula | ✓ | ✓ | `table.sort`, `table.repeat_header`, `table.to_text`, `table.formula` |
| (jade) Update table formulas | ✓ | ✓ | `table.update_formulas` |

### Header & Footer

| Word command | N | B | jade key |
|---|---|---|---|
| Header, Footer, Page Number | ✓ | ✓ (Insert tab) | `page.header`, `page.footer`, `page.number` |
| Date & Time, Document Info (file name…), Quick Parts › Field | ✓ Page, Page Count, Date, Heading Reference (STYLEREF), File Name | – | `story.field.*` |
| Pictures, Online Pictures | – | – | — |
| Go to Header / Footer, Previous / Next section, Link to Previous | ✓ | – | `story.goto.*`, `story.previous`, `story.next`, `story.link` |
| Different First Page, Different Odd & Even Pages, Show Document Text | ✓ (first two) | – | `setup.header_footer.first_page`, `…odd_even` |
| Header from Top / Footer from Bottom | ✓ (−/+ steps) | – | `story.distance.*` |
| Close Header and Footer | ✓ | – | `story.close` |

### Picture Format, Shape Format, Equation, Chart, SmartArt

| Word command | N | B |
|---|---|---|
| Picture: Wrap Text (7 options) | ✓ (`picture.wrap.*`, on Insert while a picture is selected), move by drag, resize by corner | ✓ |
| Picture: Remove Background, Corrections, Color, Artistic Effects, Transparency, Compress, Change, Reset, Styles, Border, Effects, Alt Text, Position, Align, Group, Rotate, Crop, Size | – | – |
| Shape Format, Text Box, WordArt, Equation, Chart Design/Format, SmartArt Design/Format | – | – |

## Menus (jade native) and Word's menu bar

Word for Mac has a menu bar; Word for Windows has the File (Backstage) tab
and Alt key tips instead. jade's native shell has a macOS menu bar
(`crates/jade-gpui-app/src/main.rs` `menus`):

| jade menu | Items |
|---|---|
| jade | Quit |
| File | New Document ⌘N, New from Template… ⇧⌘P, Open… ⌘O, Open Folder… ⇧⌘O, Close Tab ⌘W, Save ⌘S, Save As… ⇧⌘S, Print… ⌘P, Export as PDF… ⌥⌘P |
| Edit | Undo, Redo, Cut, Copy, Paste, Paste Special… ⌃⌘V, Paste and Match Formatting ⌥⇧⌘V, Select All, Select Paragraph ⌥⌘A, Find and Replace… ⌘F, Find Selection ⌘E, Find Next ⌘G, Replace… ⌃H, Replace All ⌥⇧⌘F, Advanced Find and Replace… ⌥⇧⌘H, Go To ⌥⌘G |
| Format | Bold, Italic, Underline, Font… ⌘D, Asian Typography… ⌥⇧⌘K, Align Left/Center/Right, Increase/Decrease Indent, Paragraph… ⌥⌘M, Copy/Paste Formatting |
| Insert (one menu) | Page Break ⌘↩, Column Break ⇧⌘↩, Link… ⌘K, Bookmark, Footnote ⌥⌘F, Endnote ⌥⌘D, index and citation marks, Symbol… ⌥⇧⌘S, Date and Time… ⌃⇧D, the field commands |
| Table | Insert Row Below ⌥⌘↩, Delete Row ⇧⌘⌫ |
| Review | Add Comment ⌘M, Track Changes ⇧⌘T |
| Tools | AutoCorrect… ⌃⌥⌘A, Word Options… ⌘, (View: marks by kind; Edit: overtype, Insert key, paste defaults; Ribbon & Toolbar: tabs, Quick Access Toolbar, a custom group), Overtype (Insert) |
| View | Explorer ⌥⌘E, Details Panel ⌥⌘I, Appearance (System, Light, Dark, Dark Pages, toggle), Next/Previous Page, Zoom In/Out/Actual Size/Fit |

Word for Mac menus with no jade counterpart: Format › Tabs…, Borders and
Shading…, Columns…, Bullets and Numbering…, Style…, Document…; Insert ›
Break, Page Numbers…, Field…, Bookmark…, Caption…, Cross-reference…,
Index and Tables…, Text Box, Picture, Shape, Chart, SmartArt; Tools ›
Spelling and Grammar, Thesaurus, Word Count, AutoCorrect, Track Changes
options, Merge Documents, Protect Document, Mail Merge, Envelopes, Labels,
Macro, Templates and Add-ins, Customize Keyboard. The browser shell has no
menu bar: Undo, Redo, Download (save), PDF export and a theme toggle sit in
the title bar.

## Main dialogs

| Word dialog | Win | Mac | N | B |
|---|---|---|---|---|
| Font (Font, Advanced) | Ctrl+D | ⌘D | ✓ | ✓ |
| Paragraph (Indents and Spacing, Line and Page Breaks, Asian Typography) | ribbon | ⌥⌘M | ✓ (alignment, indents, special, spacing, line spacing, widow, keep with next, keep lines, page break before) | ✓ |
| Asian Typography (体裁) | ribbon | Format menu | ✓ ⌥⇧⌘K | ✓ |
| Tabs | ribbon | Format menu | – | – |
| Borders and Shading (paragraph, page) | ribbon | Format menu | page borders ✓; table ✓; paragraph – | same |
| Bullets / Numbering / Define New Multilevel List | ribbon | Format menu | – | – |
| Styles pane, Create / Modify Style | Ctrl+Alt+Shift+S | Format menu | – | – |
| Page Setup (Margins, Paper, Layout, 文字数と行数) | ribbon | Format › Document | ✓ (Grid, Margins, Paper, Other) | ✓ |
| Columns | ribbon | Format menu | ✓ | ✓ |
| Page Number Format | ribbon | Insert menu | ✓ | ✓ |
| Insert Table, Table Properties, Convert, Sort, Formula, Split / Insert Cells | ribbon | Table menu | ✓ | ✓ |
| Insert Picture | ribbon | Insert menu | ✓ system dialog | ✓ picker |
| Layout (Position, Text Wrapping, Size), Format Picture/Shape pane | ribbon | ribbon | – | – |
| Hyperlink | Ctrl+K | ⌘K | ✓ (bar) | ✓ (bar) |
| Bookmark, Cross-reference, Caption, Index, Table of Figures, Table of Authorities | ribbon | Insert menu | – | – |
| Table of Contents (custom) | ribbon | Insert menu | – (one fixed TOC) | – |
| Footnote and Endnote | ribbon | Insert menu | ≈ (note text bar; no format/placement options) | ≈ |
| Field | ribbon (Quick Parts) | Insert › Field… | – | – |
| Date and Time | ribbon | Insert menu | ✓ ⌃⇧D | ✓ |
| Symbol (Symbols, Special Characters) | ribbon | Insert menu | ✓ ⌥⇧⌘S | ✓ |
| Phonetic Guide, Enclose Characters | ribbon | ribbon | ✓ | ✓ |
| Combine Characters, Two Lines in One, Fit Text | ribbon | ribbon | menu choices (no dialog); Fit Text – | same |
| Paste Special | Ctrl+Alt+V | ⌃⌘V | ✓ | – |
| Find and Replace (More options, Go To) | Ctrl+H / F5 | ⌃H | ✓ dialog with More ▼ (⌥⇧⌘H), Go To ⌥⌘G | ≈ bar with buttons and the ⋯ options row |
| Word Count | Ctrl+Shift+G | ⌘⇧G | – (status bar) | – (status bar) |
| Spelling and Grammar, Thesaurus | F7 | ⌥⌘L | – (review stream) | – |
| Language (set proofing language, detect automatically) | ribbon | Tools › Language | ✓ Review › Language | – |
| Track Changes Options, Compare, Combine | ribbon | Tools menu | – | – |
| Restrict Editing / Protect Document, Encrypt | ribbon / Backstage | Tools / Review | – | – |
| Mail Merge Recipients, Match Fields, Address Block, Greeting Line, Envelopes, Labels | ribbon | Tools / Mailings | – | – |
| Content Control Properties, legacy form field options | ribbon | ribbon | – | – |
| Document Properties | Backstage | File › Properties | – | – |
| Options (Windows) / Preferences (Mac), Customize Ribbon / Quick Access Toolbar | Backstage | ⌘, | ✓ Word Options ⌘, | – |
| AutoCorrect (AutoCorrect, AutoFormat As You Type) | Backstage | Tools menu | ✓ ⌃⌥⌘A | – (Word's defaults apply as you type) |
| Zoom | ribbon | View menu | ✓ | – |
| Customize Keyboard | Backstage | Tools menu | – | – |
| Templates and Add-ins, Macros | ribbon | Tools menu | – | – |
| Print, Save As | Ctrl+P / F12 | ⌘P / ⌘⇧S | ✓ (system print panel, save panel) | – print; save downloads |

## Shortcuts

Where jade's binding differs from Word's or collides with another Word
command:

| Action | Word Win | Word Mac | jade N | jade B | Note |
|---|---|---|---|---|---|
| Align left / center / right | Ctrl+L / E / R | ⌘L / E / R | ⌘L / ⌘E / ⌘R | – | Find Selection moved to ⌃⌘E |
| Justify | Ctrl+J | ⌘J | ⌘J | – | |
| Bullets | Ctrl+Shift+L | ⌘⇧L | ⇧⌘L | – | |
| Track Changes | Ctrl+Shift+E | ⌘⇧E | ⇧⌘E, ⇧⌘T | – | |
| New Comment | Ctrl+Alt+M | ⌘⌥A | ⌘M | – | |
| Insert Endnote | Ctrl+Alt+D | ⌘⌥E | ⌥⌘D | ⌥⌘D | jade follows the Windows letter |
| Subscript / Superscript | Ctrl+= / Ctrl+Shift+= | ⌘= / ⌘⇧= | ⌘= / ⇧⌘= | – | zoom is ⌥⌘= / ⌥⌘- |
| Column break | Ctrl+Shift+Enter | ⌘⇧↩ | ⇧⌘↩ | – | Insert Row Below is ⌥⌘↩ |
| Redo | Ctrl+Y | ⌘Y | ⇧⌘Z | ⇧⌘Z, ⌘Y | |
| Go To | Ctrl+G / F5 | ⌘⌥G | – (⌘G = Find Next) | ⌘G in find bar | |
| Indent / outdent | Ctrl+M / Ctrl+Shift+M | not verified | ⌘] / ⌘[ | – | |
| Date field | Alt+Shift+D | ⌃⇧D | ⌃⇧D opens the dialog | – | Word inserts the field directly |
| Font size box | Ctrl+Shift+P | ⌘⇧P | ⇧⌘P opens the Font dialog | – | New from Template is ⌥⌘N |
| Grow / shrink font | Ctrl+Shift+> / < | ⌘⇧> / < | – | – | |
| Show ¶ | Ctrl+Shift+8 | ⌘8 | – | – | |
| Line spacing 1 / 2 / 1.5 | Ctrl+1 / 2 / 5 | ⌘1 / 2 / 5 | – | – | |
| Heading 1–3, Normal | Ctrl+Alt+1–3, Ctrl+Shift+N | ⌘⌥1–3, ⌘⇧N | – | – | |
| Clear formatting | Ctrl+Space | ⌃Space | – | – | |
| Update / toggle / lock / unlink field | F9 / Alt+F9 / Ctrl+F11 / Ctrl+Shift+F9 | F9 / ⌥F9 / ⌘F11 / ⌘⇧F9 | – | – | see [fields.md](fields.md) |
| Spelling | F7 | ⌥⌘L | – | – | |
| Word count | Ctrl+Shift+G | ⌘⇧G | – | – | |

Matching Word: ⌘B/I/U, ⌘D (Font), ⌘K (link), ⌥⌘M (Paragraph, Mac), ⌥⌘F
(footnote), ⌘↩ (page break), ⇧↩ (line break), ⌘⇧C/⌘⇧V (copy/paste
format), ⌃⌘V (Paste Special, Mac), ⌥⇧⌘V (match formatting, Mac), ⌃H
(Replace, Mac), ⌘F, ⌘A, ⌘Z, ⌘S, ⇧⌘S (Save As, Mac), ⌘P.

Word for Mac's own: ⌘, Preferences (Word Options), Insert (overtype).

jade-only: ⌥⇧⌘H Advanced Find and Replace, ⌃⌥⌘A AutoCorrect, ⌥⌘A select paragraph, ⌥⇧⌘K Asian Typography, ⌥⇧⌘S Symbol,
⌥⌘P Export PDF, ⌘9 zoom to fit, PageUp/PageDown page steps, theme keys
⌃⌥⌘S/L/D/P, ⌥⌘E / ⌥⌘I panels.

## Word for Mac vs Word for Windows

| Area | Word for Windows | Word for Mac |
|---|---|---|
| File commands | File tab (Backstage): Info, New, Open, Save As, Print, Share, Export, Options | File menu in the macOS menu bar; Preferences in the Word menu |
| Keyboard access to the ribbon | Alt key tips | none (menu bar instead) |
| Menu bar | none | Format, Insert, Tools, Table, Window menus duplicate many ribbon commands and reach dialogs the ribbon hides (Tabs…, Field…, Index and Tables…) |
| Modifier | Ctrl | ⌘ (⌃ for a few: Replace ⌃H, Paste Special ⌃⌘V, date ⌃⇧D) |
| Distinct shortcuts | Comment Ctrl+Alt+M, Endnote Ctrl+Alt+D, Go To Ctrl+G, Spelling F7, Save As F12 | Comment ⌘⌥A, Endnote ⌘⌥E, Go To ⌘⌥G, Spelling ⌥⌘L, Save As ⌘⇧S |
| Views | Read Mode, Print Layout, Web Layout, Outline, Draft, Focus, Immersive Reader | Print Layout, Web Layout, Outline, Draft, Focus, Immersive Reader (no Read Mode) |
| Macros | VBA editor and macro recorder | VBA editor and recorder, some Windows-only objects unavailable |
| Layout engine | as described in [compatibility-modes.md](compatibility-modes.md) — line heights from OS/2 metrics, Windows punctuation compression | hhea metrics, Mac squeeze rules |

jade's native shell follows the Mac conventions (menu bar, ⌘, macOS text
keys), and the browser shell accepts Ctrl for ⌘ so Windows users get the
same commands.

## Gaps found

- Whole tabs missing: Draw, Mailings, Developer (other streams add them).
- Design has only Page Color and Page Borders (no themes, style sets,
  watermark).
- No Shapes, SmartArt, Chart, Text Box, WordArt, Equation insertion; no
  Picture/Shape Format tabs.
- No Bookmark, Cross-reference, Caption, Index, Citation buttons although
  the engine has commands for several.
- Right-click menus: text, table, picture, shape, header and footer, a link
  and a field, from one list
  (`crates/jade-office-model/src/word/context_menu.rs`) drawn by
  `crates/jade-gpui-app/src/word_views/context.rs` and
  `crates/jade-gpui-web/src/view/word_context_menu.rs` — the browser leaves
  Paste to ⌘V and the field and link items to the native shell). No Bring
  Forward / Send Backward items yet.
- Word's own shortcuts for headings, line spacing, ¶, grow/shrink font and
  clear formatting are unbound.
