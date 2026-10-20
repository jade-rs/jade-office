# Gap inventory

What an everyday Office user would reach for and does not find yet, taken
from the code on 2026-10-20 (`docs/COMPATIBILITY.md` lists what works). Each
row says how often a user hits it, and whether jade-core's engine already
does it, so only the shells lack a button or key (**engine: yes**), or the
engine needs work first (**engine: no**).

**Blocker** marks what is required before a release: every High row, and
the Format Cells dialog, conditional-format editing and chart editing. Table
cell merge/split and SmartArt resizing were required and are done. Every
PowerPoint blocker is done, marked **Done** with the pull request that did
it; copy and paste across slides needed the engine (jade-rs/jade-core#111).
Every Word blocker is done the same way; tracked changes typed a key at a
time needed the engine (jade-rs/jade-core#108).
Excel's printing, CSV, Paste Special, Find and Replace and the native Number
Format dropdown are done, marked the same way; the engine gave them a number
under Text shown as General and transposed formulas (jade-rs/jade-core#118,
#119). Thai tone marks
(#56), Korean (#57) and Arabic (#58) are tracked as issues, not blockers.

## Excel

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#71) | Format Cells dialog (⌘1): number, alignment, font, border, fill tabs | High | partly — `format_at`/`set_format` cover font, fill, number format, alignment, wrap, borders; not indent, rotation, shrink-to-fit, pattern fills, super/subscript, protection flags |
| **Done** (#75) | Conditional formats: create, edit, delete rules | High | no writer; display also misses formula rules, top 10, above average, duplicates, dates, `cellIs` against a cell, icon sets |
| **Done** (#89) | Charts: select, move, resize, edit type, data, titles, axes, legend | High | reads and draws bar/line/area/pie/doughnut/scatter; no chart authoring or anchor move/resize |
| **Done** (#70) | Hidden sheets show as tabs | High (fidelity) | sheets carry visibility; the shells don't filter |
| **Done** (#72) | Edits to a protected sheet are not blocked | High (fidelity) | protection not modelled |
| **Done** (#95) | Print and export to PDF | High | no — jade-office-model paginates; Skia records the PDF |
| **Done** (#94) | CSV import and export; Save As writes xlsx whatever the name | High | no — jade-office-model reads and writes it |
| **Done** (#93) | Paste Special (values, formats, transpose) | High | yes (`transpose_formula`, jade-core#119) |
| **Done** (#92) | Find and Replace (only Find, no options) | High | yes (`find`) |
| **Done** (#91) | Number-format dropdown in the native app (the browser has one) | High | yes |
| | Find All (the matches listed); Look in Values or Notes; within the selection | Medium | yes (`find`) |
| | Paste Special: operations (Add, Multiply…), column widths, Paste Link; Paste Options after a paste | Medium | partly |
| | CSV: other delimiters and encodings (Text Import Wizard), leading zeros kept as text | Medium | — |
| | Format Painter | Medium | yes (`copy_format`) |
| | Clear Formats / Clear All | Medium | partly |
| | Font family, vertical alignment, indent controls; custom/theme colours | Medium | yes for most |
| | Sort dialog: several keys, case, left to right | Medium | yes (`SortRequest`) |
| | Filter: custom, dates, top N, reapply (only ticked values today) | Medium | yes (`AutoFilterChange`) |
| | Data validation: create and edit; reference bounds; input messages | Medium | no writer |
| | Named ranges: define and manage; the name box rejects names | Medium | no |
| | Insert/delete cells shifting right or down | Medium | no (whole rows and columns only) |
| | Go To (F5) and Go To Special | Medium | partly |
| | Hyperlinks: insert and edit (⌘K) | Medium | no writer |
| | Function insert, autocomplete, argument tips | Medium | no |
| | Fill continuing month and weekday names (Jan…, 月曜…) | Medium | no |
| | Hide, unhide, copy sheets; tab colours | Medium | partly |
| | Row/column grouping (outline) | Medium | no |
| | Remove duplicates, text to columns, flash fill | Medium | no |
| | Trace precedents, error checking, show formulas | Medium | no |
| | Excel tables: create, resize, style, total row | Low–Medium | no |
| | Page setup: margins, orientation, paper size, scaling, print titles — read from the sheet and set in a dialog (printing uses Excel's defaults with the sheet's print area, breaks, header and footer) | Medium | partly — print area, breaks, headers/footers read; `pageSetup`/`pageMargins` not |
| | Insert pictures and shapes | Low–Medium | no |
| | Calculation mode and F9 | Low | yes |
| | Threaded comments, multi-sheet selection, Merge Across, split view, cell styles gallery | Low | no |

## Word

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#78) | New blank document, new from template | High | no |
| **Done** (#78) | Save as / export PDF (native prints only; the browser has neither) | High | yes (`export_pdf`) |
| **Done** (#82) | Copy and paste keeping formatting (plain text today, even within a document) | High | partly |
| **Done** (#82) | Insert picture (and moving it in the browser; wrap options) | High | yes (`insert_image`) |
| **Done** (#79) | Page setup: margins, orientation, size, columns, section breaks | High | yes |
| **Done** (#79) | Headers and footers: edit; page numbers | High | yes (`set_header_footer_text`) |
| **Done** (#81) | Comments: type the text, reply, resolve, delete, pane; none in the browser | High | yes |
| **Done** (#81) | Tracked changes: accept or reject one, next and previous; none in the browser | High | partly |
| **Done** (#76) | Find and Replace in the browser; options (case, whole word) in both | High | yes |
| **Done** (#74) | Native keys: Home/End, word and line jumps, word delete, Tab (the browser has them) | High | yes |
| | Headers and footers: first-page and odd/even variants, a story of more than one line, a header per section | Medium | partly (`SetHeaderFooterVariants`) |
| | Page setup per section (the commands act on the default one) | Medium | partly |
| | Paste with formatting from other apps (HTML, RTF); a copy keeps its formatting within the app only | Medium | no |
| | PDF in the browser is a picture of each page: its text cannot be selected or searched | Medium | partly (`jade-docx-pdf-renderer`) |
| | Shift+Enter line break | Medium | partly |
| | Space before/after, paragraph dialog (first-line/hanging indent, keep with next) | Medium | yes (`set_paragraph_spacing`) |
| | Fonts beyond a fixed list; custom colours; Font dialog | Medium | partly |
| | Modify and create styles; character styles | Medium | partly |
| | List types, bullet glyphs, number formats, restart numbering | Medium | partly |
| | Hyperlinks: insert, follow | Medium | yes |
| | Page Break (native runs Blank Page), ⌘Enter | Medium | yes |
| | Footnotes and endnotes | Medium | yes |
| | Table of contents: update; none in the browser | Medium | yes |
| | Table column resizing by drag, AutoFit, table properties | Medium | yes |
| | Word count; spelling | Medium | no |
| | Save failures only printed to the console (native) | Medium | — |
| | Shapes, text boxes, symbols, equations, fields, bookmarks | Low–Medium | yes for most |
| | Japanese word selection on double-click; drag-and-drop text | Low | no |
| | Navigation pane, outline/draft views | Low | no |

## PowerPoint

| | Gap | Hit | Engine |
|---|---|---|---|
| **Done** (#73) | Slides: new, duplicate, delete, reorder | High | yes |
| **Done** (#73) | Layouts; hide a slide (hidden slides still play) | High | yes |
| **Done** (#77) | Text: caret movement, selection, typing in place (native types at the end only) | High | yes (`apply_text_input`) |
| **Done** (#80) | Character formatting: font, size, bold, italic, colour | High | yes (`format_text_range`) |
| **Done** (#80) | Paragraph formatting: alignment, bullets, numbering, spacing | High | yes (`format_paragraphs`) |
| **Done** (#84) | Insert text box, shape, picture | High | yes |
| **Done** (#86) | Copy, paste, duplicate shapes; select several | High | yes (`copy_shapes`, jade-core#111) |
| **Done** (#85) | Speaker notes: edit | High | yes (`set_slide_notes`) |
| | Paste into text in the native app; copy and cut slide text | Medium | — |
| | Cut shapes; paste pictures and shapes from other apps | Medium | partly |
| | Table cells typed in a box over the cell, not in place | Medium | partly |
| | Notes pane: caret put by a click, selection, formatting | Low | yes |
| | Fill and outline; rotate and flip; z-order; group | Medium | yes |
| | Ungroup, align and distribute, effects | Low–Medium | no |
| | Table styles, borders, shading | Medium | no |
| | Chart data and type | Medium | no |
| | Slide master, themes, backgrounds | Medium | no |
| | Transitions and animations (not read, played or edited) | Medium | no |
| | Slide show: presenter view, notes, timer, jump to slide | Medium | — |
| | Comments | Medium | no |
| | Find and Replace for slides | Medium | partly |
| | Export PDF and images; print in the browser | Medium | partly (`Deck::pdf`) |
| | Slide size, sections | Low | no |
| | Outline and slide sorter views | Low | — |
