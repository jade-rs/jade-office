# Word in the browser vs the native app

**Covers:** for each area of Word use — opening, display, fonts, editing,
saving, printing/PDF, clipboard, view and shortcuts — what the browser
build does and what the native app does, and why they differ.

**Derived from:** jade-office `9df8a1e` / jade-core `9fea681` (origin/main,
2026-10-22): the native shell `crates/jade-gpui-app`, the browser shell
`crates/jade-gpui-web` (what GitHub Pages serves at `/`; built by
`scripts/build-web.sh`, deployed by `.github/workflows/pages.yml`), and the
shared layers (`crates/jade-office-model`, `crates/jade-web` as the browser
document backend, `jade-core`). The older `crates/jade-web` canvas page
(`/canvas.html`) only views Word documents and is not covered. This was the
initial source-only comparison; subsequent verified changes are recorded below.
Rows other than those updated below still describe that baseline.

**Keeping it current:** when a command, dialog or behaviour is ported to
`crates/jade-gpui-web`, move its row out of "To port" and fix the area
table. When a browser limit is worked around (e.g. File System Access,
Local Font Access), update the "Why" column. The two shells are separate
copies because they sit on different gpui versions (gpui 0.2 native,
gpui-pre 0.3.9 on wasm); both call the same command layer in
`crates/jade-office-model/src/word/`.

## Why they differ, in short

1. **Browser limits** — no file paths, no installed-font list, clipboard
   readable only inside a paste event, no print dialog, no threads without
   cross-origin isolation (which GitHub Pages cannot set), shortcuts the
   browser keeps (⌘N, ⌘W, ⌘T, ⌘Q, ⌘P, ⌘=).
2. **Back end** — the browser paints with `jade-tiny-skia-painter` (pure
   Rust; Skia does not build for `wasm32-unknown-unknown`), the native app
   with Skia. No `cfg(target_arch = "wasm32")` gate in jade-core or
   `jade-office-model` turns off an editing feature.
3. **Not yet ported** — shell code written for the native app first.

## Area by area

| Area | Native app | Browser | Why |
|---|---|---|---|
| Open | system dialog (⌘O), folder tree (⇧⌘O), drop on window or Dock, Finder *Open With*, command-line paths; templates (⇧⌘P) | file picker (⌘O / Ctrl+O, + button), drop on the page, bundled samples, `?open=` for samples; a `.dotx` opens as a new document | sandbox: no paths, no OS file association, no arbitrary URLs |
| Recent files | 12 paths kept in Application Support | latest opened/saved bytes for 8 names in IndexedDB; restored after reload | browser-local copies, not paths to the original files; same-name files share an entry; storage may be unavailable or cleared |
| Rendering | Skia at the window's scale factor; re-rasterised when it changes; all pages resident | tiny-skia on a WebGPU/WebGL2 canvas; 128-row tiles re-uploaded only when changed; at most 8 pages kept as pixels, others as 1/8-size thumbnails; pixel ratio read once | wasm memory (a 174-page file once ran out); Skia unavailable on wasm |
| Text effects | shadows, blur, glow, SVG | **canvas shadows ignored** (shadow and emboss text effects missing); SVG via resvg | tiny-skia painter has no shadow pass |
| Layout platform | the machine's Word (Mac on macOS) | the visitor's OS, or `?word_platform=mac\|windows` | intentional: lay out like the Word the user has — see [compatibility-modes.md](compatibility-modes.md) |
| Document fonts | installed fonts: Office's faces and system faces found by any name (游明朝, ＭＳ 明朝, Calibri…), indexed on a background thread; Apple Symbols for marks; Arial Unicode/DejaVu for Hebrew and Arabic. `JADE_FONTS=bundled` switches to bundled fonts | **bundled only**: Noto Sans JP Regular/Bold and Noto Sans Thai; 21 Office family names alias to Noto Sans JP | no font enumeration on the web (Local Font Access is Chromium-only and permissioned); Office fonts cannot be redistributed; download size. Result: different metrics, so line breaks can differ from native; Korean, Arabic, Hebrew, Devanagari show as boxes |
| UI fonts and font menu | system fonts; menu lists installed families | Noto Sans JP; menu is the built-in Japanese/Latin list; picking a font writes the name but still draws Noto | same limit |
| Typing, IME, selection | full; double-click takes a run of one script (東京 of 東京へ), triple-click the paragraph; ⌘-drag adds ranges, ⌥-drag selects a block, a selection drags to move (⌥ copies); AutoCorrect and AutoFormat as you type, set in ツール → オートコレクト; 上書き入力 (Insert) | full; Ctrl works as ⌘; shift-click extends; the same double- and triple-click; AutoCorrect and AutoFormat with Word's default settings (no dialog) | shared model `word::selection`, `word::autocorrect`; the gestures and the dialog not ported |
| Ribbon tabs | Home, Insert, Design, Layout, References, Review, View; context: Table Design, Table Layout, Header & Footer, Outlining; the Quick Access Toolbar and the tabs shown set in Word Options; right-click menus | Home, Insert, Design, Layout, References, Review, View (marks, panes, zoom); context: Table Design, Table Layout | the views, the toolbar and the context menus not ported |
| Font, Paragraph, Asian Typography, Ruby, Enclose, Symbol, Date and Time dialogs | yes, with shortcuts | yes; Symbol and Date and Time have no shortcut | not ported |
| Tables | both tabs, Table Properties and table dialogs; ⌥⌘↩ / ⇧⌘⌫ for rows | same tabs, dialogs and row shortcuts, with Ctrl aliases; Tab past the last cell adds a row | — |
| Page setup | ribbon, dialogs, Design tab | same dialogs (文字数と行数, margins, paper, line numbers, columns, page numbers, breaks, cover pages, page colour and borders) | — |
| Headers and footers | edited in place as a full story: several paragraphs, tables, pictures, fields; first-page, odd/even and linked variants; Header & Footer tab | **one-line bar**: left/centre/right zones of the first line only | not ported (needs a second story drawn over the page) |
| Comments and tracked changes | full: new, reply, edit, resolve, delete, pane; ⌘M, ⇧⌘E/⇧⌘T | same features; ⇧⌘E toggles tracking, with Ctrl alias; no ⌘M or ⇧⌘T | ⇧⌘T reopens a browser tab |
| Fields, TOC, notes, links | TOC insert/update, notes (⌥⌘F/⌥⌘D), ⌘K; DATE/TIME show the current time on open | same; DATE/TIME fields take the browser’s local clock on open and F9 | initial layout receives the clock, including header/footer stories; native regressions pass |
| Find and replace | ⌘F, ⌘E, ⌘G, ⌃H, ⌥⇧⌘F; bar without buttons; Advanced Find and Replace (⌥⇧⌘H): wildcards and \\1, ^ codes, format and style, direction, scope, prefix/suffix, width, kana, punctuation, spaces | ⌘F, ⇧⌘H; bar with buttons, previous (⇧⏎, ⌘⇧G); the ⋯ row: wildcards, ^ codes, prefix/suffix, width, kana, punctuation, spaces | ⌘G/⌘E collide with the browser's find; format, style and scope have no browser UI yet |
| Format painter | ribbon (Clipboard group), ⇧⌘C / ⇧⌘V | ribbon (Editing group), ⇧⌘C / ⇧⌘V | — |
| Pictures | system dialog, wrap, move, resize | picker or drop while a document is active (recognised by content), wrap, move, resize | one shared picker |
| Save | ⌘S writes in place atomically; ⇧⌘S Save As | ⌘S / download button: **always a download**; no Save As; the dirty mark clears even if the download is cancelled | sandbox; File System Access API not used (Chromium-only, needs permission) |
| Unsaved work | dot on the tab; no prompt on close; AutoRecover copies | dot on the tab; confirmation before closing a dirty editor tab; reload/navigation warning when any tab is dirty; periodic Word AutoRecover copies in IndexedDB and recovery list on next visit | browser controls the navigation warning text and may require prior user interaction; downloads cannot confirm that the user kept the file; AutoRecover is periodic, not a save to the original file |
| Export PDF | ⌥⌘P: **vector PDF** with selectable text (Skia) | button: **PDF of page images** (text not selectable), painted page by page | tiny-skia cannot write PDF; bounded memory |
| Print | ⌘P: PDF via `jade-docx-pdf-renderer` (a separate, simpler renderer) then the macOS print panel; other OSes report an error | print settings/preview panel; page-image PDF handed to a browser frame for printing | browser controls the final printer/copies/duplex dialog; PDF frame printing depends on the browser |
| Clipboard | writes plain text plus an in-process rich copy; Paste, Cut, Copy buttons, Paste Options, Paste Special (⌃⌘V), Paste and Match Formatting (⌥⇧⌘V) | writes plain text and HTML plus the in-process rich copy; ordinary browser paste imports HTML formatting with a plain-text fallback; Paste Options and Paste Special are available | ordinary paste reads the event synchronously; Paste Special uses the asynchronous browser clipboard API when allowed; browser copy falls back to plain text when rich writing is unavailable; native system copy remains plain text |
| Undo / redo | ⌘Z, ⇧⌘Z, menu, ribbon | ⌘Z, ⇧⌘Z, ⌘Y, title-bar buttons | no menu bar |
| Hyperlinks | ⌘-click opens the default browser | ⌘/Ctrl-click opens a tab (pop-up blockers may apply) | — |
| Dark mode | System / Light / Dark, saved; separate dark-pages toggle | System / Light / Dark and separate dark-pages choice in View; title-bar toggle; saved in localStorage | browser-local preferences; blocked/cleared storage cannot retain them |
| Zoom and views | ⌘= ⌘- ⌘0 ⌘9, View tab (Zoom dialog, One Page, Multiple Pages, Page Width that follow the window), status bar; Read Mode, Web Layout, Outline, Draft, Focus, side-to-side pages, gridlines, hidden white space, Split, New Window | status bar and View tab (−/100%/+/page width); Print Layout only; pixel ratio not re-read after browser zoom | browser owns ⌘= / ⌘-; the views not ported |
| Windows | one workspace window with tabs; Explorer (⌥⌘E), Details panel (⌥⌘I) | one page-filling window; explorer of samples and recent files; no details panel | — |
| Performance | Skia (C++), background font indexing | single-threaded wasm (`wasm-opt -Oz`); keystroke and paint cost tracked by `jade-office-perf` | wasm threads need COOP/COEP headers |
| Spelling | none | none | not implemented |
| UI language | `JADE_LANG`, else the OS's preferred languages | `navigator.language` | same `i18n/*.tsv` tables |

## Commands only the native app has

| Command | Native | Browser | Reason |
|---|---|---|---|
| Print | ⌘P | — | browser limit |
| Save As | ⇧⌘S | — (download) | browser limit |
| Vector PDF | ⌥⌘P | image PDF button | back end |
| Open Folder | ⇧⌘O | — | browser limit |
| New, New from Template, Close, Quit | ⌘N, ⇧⌘P, ⌘W, ⌘Q | activity-bar button, open a `.dotx`, tab ✕ | browser keeps the keys |
| Paste, Cut, Copy buttons; Paste Options; Paste Special; Paste and Match Formatting | ribbon, ⌃⌘V, ⌥⇧⌘V | ⌘V / ⌘C / ⌘X only | browser limit |
| Find Selection, Find Next, Replace, Replace All | ⌘E, ⌘G, ⌃H, ⌥⇧⌘F | find bar buttons | key clash / not ported |
| Align, indent | ⌘L/E/R/J, ⌘] / ⌘[ | same, with Ctrl aliases; ⇧⌘L toggles bullets | shared ribbon commands |
| Insert/delete table row | ⌥⌘↩, ⇧⌘⌫ | same, with Ctrl aliases | shared ribbon commands; ⇧⌘↩ inserts a column break |
| Comment, Track Changes | ⌘M, ⇧⌘T | Review tab | key clash / not ported |
| Symbol, Date and Time, Asian Typography | ⌥⇧⌘S, ⌃⇧D, ⌥⇧⌘K | ribbon | not ported |
| Select Paragraph | ⌥⌘A | triple-click | not ported |
| Next/previous screen | PageDown/PageUp | PageDown/PageUp; Shift selects to the visible edge | implemented |
| Next/previous physical page | Ctrl/Cmd+PageDown/PageUp | Ctrl/Cmd+PageDown/PageUp, including Shift selection | implemented for adjacent pages with laid-out text |
| Formatting marks | View › Marks; by kind in Word Options (spaces, tabs, paragraph marks, hidden text, optional hyphens, anchors) | View › Marks (all kinds) | the options dialog not ported |
| Blank Page, Page Break buttons | Insert, Layout | ⌘↩, Breaks menu | not ported |
| Design tab | Page Color, Page Borders | under Layout | layout choice |
| Header & Footer tab, in-place editing | yes | one-line bar | not ported |
| Theme and dark-pages commands | ⌃⌥⌘S/L/D/P, saved | View tab and title-bar toggle, saved | native shortcuts not ported |
| Details panel | ⌥⌘I | — | not ported |

The browser has a few of its own: Ctrl as ⌘ everywhere, ⌘Y, picture drop
onto the page, `?open=` and `?word_platform=`, tile residency.

## To port

Word features that are full in the native app but whose UI the
browser shell still needs. Some name files that other streams are adding
and that are not on origin/main yet; the stream is in brackets.

| Rows | What | Native source (stream) |
|---|---|---|
| 195, 200, 201 | in-place header/footer editing | `crates/jade-gpui-app/src/word_story.rs` (pages) |
| 11 | Paste Special dialog (needs a clipboard read inside a user gesture) | `crates/jade-gpui-app/src/word_characters/paste.rs` (characters) |
| 209, 223, 344, 362 | Field dialog; ASK and FILLIN prompts | `crates/jade-gpui-app/src/word_fields.rs` (fields) |
| 226, 469, 470 | document Properties dialog | (fields) |
| 43, 444, 445, 446 | Tabs dialog; horizontal and vertical rulers; ruler units | (lists and styles) |
| 44, 52 | Borders and Shading dialog; drop-cap options | (lists and styles) |
| 55, 56, 57, 58, 60, 61, 62, 63 | define new list / numbering / multilevel dialogs | (lists and styles) |
| 69, 70, 71, 72, 73, 74, 75 | New / Modify Style dialogs and the Styles pane | (lists and styles) |
| 81, 82, 83 | attach template, AutoText, Set as Default | (lists and styles) |

| 4, 486 | ⌘-drag ranges, ⌥-drag block selection, drag-and-drop of text | `crates/jade-gpui-app/src/word_views/select.rs` (views) |
| 482 | mini toolbar (live preview is ported) | `crates/jade-gpui-app/src/word_views/mini.rs` (views) |
| 496 | right-click menus | `crates/jade-gpui-app/src/word_views/context.rs` over `word::context_menu` (views) |
| 435–439, 441, 447, 450 | Read Mode, Web Layout, Outline, Draft, Focus, side to side, gridlines, hidden white space | `crates/jade-gpui-app/src/word_views/{layout,outline}.rs`, `word_views.rs` (views) |
| 451–453 | New Window, Split, Editing/Reviewing/Viewing | `crates/jade-gpui-app/src/word_views/{split,modes}.rs` (views) |
| 172 | Language dialog | `crates/jade-gpui-app/src/word_views/language.rs` (views) |
| 342, 485, 487, 489 | Word Options: toolbar, tabs, custom group, overtype, paste defaults, marks by kind | `crates/jade-gpui-app/src/word_views/{prefs,customize,paste,marks}.rs` (views) |
| 488 | AutoCorrect dialog (the corrections themselves run in the browser) | `crates/jade-gpui-app/src/word_views/options.rs` (views) |
| 504, 505 | find formatting, style and scope | `crates/jade-gpui-app/src/word_views/find.rs` (views) |

Add a row here when a native-only dialog lands; remove it when the browser
shell gets it.

## Statements elsewhere that this supersedes

- `docs/GAPS.md` says the browser has no PDF export, comments, tracked
  changes or TOC update — it now has all four.
- `docs/COMPATIBILITY.md` says ribbon formatting and tables are native
  only — only Print still is.
- The header comment of `.github/workflows/pages.yml` swaps `/` and
  `/gpui.html`: `/` is the gpui shell.
- `crates/jade-gpui-web/src/view.rs` says Word documents are viewed; they
  are edited. `crates/jade-gpui-web/src/view/word_characters.rs` lists
  Paste Special, which the browser does not have.

## Browser appearance checks (2026-11-07)

Appearance is also checked in the release browser build: View now offers
System/Light/Dark and an independent dark-pages toggle. Dark chrome with white
paper and light chrome survive page reload; returning to System picks up the
current dark system appearance. Text and thumbnails switch with the paper.
The WASM compile check and release build pass. The native
`dark_paper_turns_the_pages_over_and_back` regression also passes: changing paper
preserves DOCX bytes, and switching back restores the original pixels.
Changing OS appearance while the
page is open and storage-denied sessions remain untested browser cases.
Changing appearance alone leaves the tabs clean: both an existing document and
a new blank document close without a save prompt.

## Browser screen-reader mirror checks (2026-11-07)

The workspace now publishes the active Word tab's text instead of letting each
rendered document overwrite the offscreen mirror. The cache includes the
document entity identity as well as its name, edit generation and caret. When
no Word document is active, both the text box and paragraph live region are
removed from the DOM.

The WASM compile check and release build pass. Release-browser interaction
checks confirm that typing updates the accessibility text, switching between
two Word tabs replaces its label/content, and switching to Excel removes it.
Returning to Word restores it. Closing the last Word tab removes both DOM
elements, and reopening the same sample filename restores its text. These are
accessibility-tree and DOM checks; VoiceOver speech has not been verified.

The comparison ribbon actions now dispatch through their originating window.
Release-browser checks confirm entering the two-document layout, clicking either
pane changes the active tab and accessibility mirror, and closing a paired
document clears comparison even with synchronous scrolling disabled. Synchronous
scrolling moves both panes; turning it off allows independent scrolling, and
resetting the window position realigns the partner to the first pane.
Word now uses the editor/pane dimensions rather than the whole browser window.
Oversized vertical pages align from the left so their start remains reachable.
The rebuilt release passes browser checks for visible line starts at 100% and
page-width fitting within one comparison pane.

## Browser data-loss checks (2026-11-07)

The page's IndexedDB operations now wait for transaction completion, reject
aborts even after a request succeeds, and retry opening the database after a
failed open. The navigation warning follows all editor tabs, including inactive
ones, and is removed when every tab is clean. These checks do not establish
full Word parity or guarantee that a downloaded file was kept by the user.

Run `scripts/test-web-storage.cjs` with `fake-indexeddb` on `NODE_PATH` (the
installation command is at the top of the script). It checks committed recovery
bytes, an aborted replacement retaining the previous copy, removal, an initial
open failure followed by retry, and the navigation warning's lifecycle.

Recovered documents now retain their IndexedDB snapshot and open as unsaved until downloaded. Failed opens retain the recovery candidate; recovered tabs bypass merge/compare imports and get a distinct name when another tab uses the same name. `recovers_documents` verifies that reopening snapshot bytes preserves edits, requires saving, adds no undo entry, and clears the unsaved state only on DOCX export.

The browser now supplies its local clock when opening a Word document. The initial renderer receives the clock before laying out the body, headers and footers, without an extra repaint or rewriting saved field results. `fields_documents` adds separate DATE and TIME checks in each story for changed pixels, unchanged saved XML, clean save state, and empty undo history. All three native tests pass, including separate DATE/TIME checks in the body, header-only and footer-only documents and both Letter/A4 initial-layout paths for the body. The recovery regression, browser storage regression and browser WASM compile check also pass.

Recent files now keep up to eight opened/saved files in IndexedDB. The workspace
loads their bytes on startup, gives files opened during that load precedence,
and updates its in-memory copy after saving so closing/reopening a tab uses the
saved version. A recovered snapshot is not added to persistent recent files
until saved; password-protected files retain the encrypted source for reopening.
The v1→v2 store migration preserves recovery copies and templates. The browser
storage regression checks this migration, concurrent recent writes, bounded
retention, same-name replacement, clock rollback, transaction rollback without
losing older entries, and reading saved bytes from a fresh page context.

Closing a dirty editor tab now offers Download a copy, Close without saving,
and Cancel (Escape). The prompt retains the target tab entity, so another
tab closing cannot redirect it to a shifted index. Pending cell/slide text is
committed before checking the dirty state; rejected cell edits keep the tab
open. Download keeps the tab open because browser download completion is not
observable here. Explicitly closing without saving deletes that tab’s recovery
copy; cancelling retains the document and recovery state.

A release WASM build was exercised in the running browser: Cancel, Escape,
download and discard all preserve the expected tab/content state. The downloaded
DOCX contains the edits; closing a saved tab skips confirmation. Reloading the
page and reopening its recent entry restores the saved text, including after
discarding later edits. Creating and closing a second tab while confirmation is
open retains the first tab as its target. Recovery-store deletion and rejected
cell edits still need running-browser interaction coverage.

Word saves now acknowledge the document only after format conversion and
protection finish successfully. `saves_documents` checks that a failed output
step retains edits, the dirty flag and undo/redo, that a clean document remains
clean after failure, and that a successful RTF conversion contains the edits
when reopened. Opening Compare now takes a non-saving snapshot, so merely
opening its dialog preserves the dirty flag. A browser download can still be
cancelled after the shell hands it the bytes.

## PDF paper layout correction (2026-11-07)

Browser PDF export now renders a separate snapshot at print resolution (144 dpi),
using the document's section pages regardless of Web Layout, collapsed headings,
dark paper or the screen's rendering resolution. Formatting and crop marks are
excluded from both PDF export and printing. Unsaved edits are included, while
caret, undo history, dirty state, resident pages and the current view are retained.
The snapshot uses the host clock already supplied to the editor for DATE/TIME.

`jade-web/tests/prints_documents.rs` covers PDF equality with editing marks on and
off, export after changing view geometry/collapsed content/dark paper, unchanged
editor pixels and saved XML, and undo after output. The existing print settings
scenario covers page ranges, paper scaling and two pages per sheet. PDFs still
contain page images: selectable text, links and accessible tags remain open.

## Output while editing a header or footer (2026-11-07)

The browser's `WordDoc` now provides whole-document snapshot, conversion/save,
PDF and print methods explicitly. Previously, method lookup through its editable
story could save/print only the header or footer, while PDF export of the main
body omitted pending story edits. All output now combines the main body with
those edits without closing the story or consuming its undo history.

Successful saving marks both body and story saved; failed conversion leaves
both dirty. Closing a story already saved commits its body undo step without
marking the document changed again. `story_documents` exercises header and footer
cases, includes the body in reopened DOCX, compares PDF/print with the combined
snapshot, and checks caret, undo and dirty state before and after output.
WASM compilation and the release build pass. A release-browser UI check also
created a document with distinct body/header text, saved with the header still
open, and verified both strings in the downloaded DOCX parts. PDF export retained
both strings on the rendered page while the editor used dark paper. Undo after
saving restored the dirty indicator; saving through the ribbon and returning to
the body left the document clean. Footer behavior is covered by the integration
tests; it has not yet been repeated through the browser UI.

Print-time table-of-contents updates now explicitly target the main body,
including while a header/footer is open. The story remains open with its caret,
content and undo history intact; the body update is independently undoable.
Print preview also takes page labels from the body, so opening a one-page story
does not reduce a multi-page document's sheet list to one page. The
`story_documents` tests cover both roles for these cases. These two print-panel
corrections have integration/WASM coverage; their UI verification remains open.

Selection printing in the browser now uses the active header/footer's selected
content rather than the body's retained selection. Ordinary page-range printing
still uses the combined document. Empty/collapsed selections return an error
instead of silently printing the whole document. The story regression compares
the generated PDF with an independently cropped story DOCX, checks it differs
from the retained body selection's PDF, and verifies both selections, contents,
dirty state and undo availability survive output. These cases have automated
coverage; selection-print preview pagination and browser UI verification remain
open.

Print-panel sheet pagination now comes from the same output-layout preparation
as PDF printing, including selection cropping, hidden-text/markup settings,
custom page ranges and each section's paper dimensions. The schematic page
rectangles use those individual dimensions rather than repeating the first
page's size. Pagination is cached while navigating sheets and recomputed when
settings change. Regression tests compare preview sheet dimensions with PDF
MediaBoxes under screen-only geometry, selection, imposition and mixed
portrait/landscape sections; active header/footer selection previews are also
checked. Integration tests and WASM compilation pass. This remains a schematic
preview: rendering page content, accounting for print-time field updates before
previewing, and browser UI verification remain open.

Print-time field updates are now simulated on an isolated snapshot for the
browser preview. Newly generated TOC entries affect pagination and custom page
range validation before printing, while opening/cancelling the panel leaves the
original document, selection, dirty state and undo history intact. Header/footer
selection previews retain the active story as-is, matching the print action,
which updates body fields only. A regression with 40 newly added headings checks
that preview page counts change, newly valid page ranges succeed, the preview
matches an actual body field update, and repeated updates are idempotent. It
covers the body and both open-story roles. Actual page-content preview and
browser UI verification remain pending.

The browser print panel now paints the actual sheet content from an isolated
print-preview session, including text, tables and images. Sheet navigation
reuses pagination and renders only the requested sheet; replaced or closed
preview images are released. Preview and PDF output share the same sheet
composition. A regression compares the PDF made from preview images byte for
byte with print output for all pages, a selection, two-up scaling and repeated
custom page ranges. Active header/footer selection images and print-time TOC
update images are also compared with their output snapshots. Seven print and
ten story integration tests pass; browser UI verification is tracked below.

Release-browser interaction checks confirm printed text on white paper while
the editor uses dark pages, different content when advancing to sheet two,
two-up content, closing/reopening the panel, and the error for selection
printing with no selected content. These checks do not cover physical printers
or establish rendering equivalence with Microsoft Word. The macOS build script
also finds keg-only Homebrew LLVM archivers outside PATH and preserves explicit
archiver overrides; both detection paths were checked after reproducing a stale
Apple-archiver `ring` link failure.
The release WASM build, bindgen generation and `wasm-opt -Oz` complete with LLVM;
the optimized bundle also passes the visible content/two-up checks with no
browser console errors. Testing used a fresh localhost origin with no-store
responses to avoid reusing an earlier cached WASM bundle.

Native selection printing now rejects absent or collapsed selections in both
preview and PDF output. Validation runs before print-time field updates, so a
rejected selection request cannot update the source document. This closes the
path that silently printed the entire document for an empty selection.
All three native print scenario tests pass, including absent/collapsed
selection rejection with field updates enabled or disabled, preservation of
DOCX bytes, selection and undo availability, and returning to whole-document
printing. Existing preview, page-range, two-up, A3 and selected-content output
checks also pass.
Native header/footer printing still needs separate verification; this change
does not establish full print parity.

Native previews now simulate print-time field updates on an isolated DOCX
snapshot and invalidate cached pagination when the update-fields setting
changes. Four native print tests pass. The added scenario expands a generated
TOC with 60 headings, checks that preview pagination grows to the actual PDF
page count, compares the predicted DOCX with the updated source after printing,
and verifies that previewing preserves source bytes, selection and undo
availability. Switching field updates off restores the original page count.
This does not prove pixel equivalence between native preview and PDF rendering.
Print-time TOC updates now cover every unlocked table in one undo step. An
update failure rolls back all tables and reaches preview and print error
handling in both shells. Locked tables retain their cached results.

Field-block regeneration now retains ordinary text before/after the outer
field, including text in the same run as its begin/end markers. Boundary run
formatting, hyperlinks and paragraph properties are retained when reattaching
that content to the first/last generated paragraph. The editor model is read
back from the updated XML so layout and reopened output agree. One-paragraph
replacement also accounts for prefix text when returning the caret.
When a shared paragraph ends a section, the break stays only on the last
generated paragraph. Host edits and the update-TOC command retain the original
source archive for rollback/undo after the XML package is read back.

Surrounding tracked-move range markers, objects, multiple outer fields
in one paragraph range, and interior section breaks still require further support.
Those cases continue to reject regeneration before removing content.

Bookmarks outside the regenerated field result now retain their XML IDs,
names and endpoints. This includes bookmarks around the whole TOC, around
adjacent text and across neighboring paragraphs. Reloading the model resolves
their positions after the TOC changes length. If regeneration would delete
only one endpoint of a bookmark, it rejects the operation before mutation;
this also checks endpoints in paragraphs outside the replaced block.
Retained bookmark IDs and names are reserved while creating TOC heading
anchors, preventing collisions when the old field paragraphs are removed.

Comments around the regenerated field or neighboring text also retain their
range endpoints, reference runs and comment metadata, including author and
text. Endpoints in neighboring paragraphs survive changes in field length.
An update that would remove only an endpoint or reference rejects before
mutation. Comment and bookmark IDs are checked in separate namespaces, so
an old result bookmark can be removed without detaching an unrelated comment
with the same numeric ID.

Editing-exception markers around the field or neighboring text are retained
as well. Their IDs, editor/group attributes and column attributes survive
regeneration and reopening; document-protection settings remain in force.
Exception endpoints follow changes in field length, and a partial endpoint
deletion is rejected before mutation, including endpoints outside the block.
Permission IDs use their own namespace when checking retained range markers.

Verification: all five native print scenarios, seven browser-backend print
scenarios and eleven story scenarios pass; the browser WASM target compiles.
The added regressions exercise multiple TOCs, locks, one-step undo, rollback,
and update errors reaching preview/output without changing the document.
The real `toc.docx` fixture now previews and prints with its surrounding body
text retained; undo restores the original DOCX bytes and redo restores the
updated output. Engine regressions also cover a shared run, Japanese prefix
offsets, boundary formatting, section breaks and reopening the generated DOCX.
The final engine run passes 45 field tests and 16 TOC-command tests. Caret
offsets also retain nonbreaking and discretionary hyphens in boundary runs.

## Browser Word keyboard checks (2026-11-07)

The browser now handles twelve Word editing shortcuts with either Command or
Control: four paragraph alignments, two indent commands, bullets, revision
tracking, page and column breaks, and table row insertion and deletion.
Shift+Command/Control+Enter inserts a column break, while Command/Control+Enter
inserts a page break. Alt+Shift+Command/Control+Enter remains unassigned.
These shortcuts use the existing editing and review command paths.

Verification: the release WASM build passes. The browser regression script
`scripts/test-web-word-shortcuts.cjs` passes for both modifier families,
checking exported DOCX content, actual tracked insertions, undo restoration,
and the unassigned Enter combination. This verifies the tested Chromium
environment; complete Word compatibility remains open.

## Change recording survives DOCX reopening (2026-11-07)

Starting and stopping change recording now updates `w:trackRevisions` in the
DOCX settings part. Loading a document with recording enabled restores the
recording session, with the current revision-author options. Existing revision
authors and timestamps are retained in their individual revision elements.
Recording commands create a missing settings part, relationship and content
type; existing settings retain their other elements, path and XML prefix.
Undo and redo restore both the recording session and the package setting.
Temporary internal recording suspension continues to affect the session alone.

Engine regressions cover the on/off values, alternate settings path and prefix,
settings creation, exact package restoration on undo/redo, and tracked versus
ordinary insertions after reopening. The browser shortcut regression also
checks the saved switch and reopens the downloaded DOCX before typing again.
Verification: the full editor suite passes 2,409 tests with no failures
(21 existing ignored tests). The release WASM build and the Chromium shortcut
checks pass for both Command and Control, including recording after reopening.

### Locked change recording (2026-11-07)

An enforced `documentProtection` restriction with `edit="trackedChanges"`
now starts recording on DOCX load even when `trackRevisions` is absent or
explicitly off. Starting the restriction also enables and saves recording,
retaining an existing revision author. The command executor refuses recording
stop and revision acceptance/rejection while this restriction is enforced;
the saved-session setter independently refuses stop. Lifting the restriction
permits recording to be stopped again. Protection reads and writes follow the
main document's settings relationship, including nonstandard settings paths.

Regression coverage includes actual tracked insertion after reopening,
rejected stop without package mutation, and undo/redo restoring the protection,
recording session, and package together. Temporary internal tracking suspension
continues to use the session-only setter.

Validation: the editor library suite passed 2,411 tests (21 existing ignored),
and the web application passed `cargo check --target wasm32-unknown-unknown --lib`.

### Password verification for imported protection (2026-11-07)

Imported editing and write protection now retain their declared hash algorithm.
SHA-1, SHA-256, SHA-384 and SHA-512 are verified in both the `hashValue` /
`algorithmName` form and the `hash` / `cryptAlgorithmSid` form. Wrong passwords,
unsupported algorithms and malformed verifiers cannot authorize restriction
removal through either session API or the canvas API. Saving protection retains
its algorithm and attribute form instead of silently replacing it with SHA-512.
New passwords continue to use SHA-512.

The hash procedure is checked against independent fixed vectors including a
Japanese password and a supplementary Unicode character. Algorithm identifiers
follow Microsoft's [OOXML protection documentation](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.writeprotection.cryptographicalgorithmsid),
and the salt/UTF-16LE and hash-then-iterator procedure is consistent with
[Apache POI's write-protection implementation](https://github.com/apache/poi/blob/trunk/poi/src/main/java/org/apache/poi/poifs/crypt/CryptoFunctions.java).

Validation: all 2,413 editor library tests passed (21 existing ignored), and
`cargo check --target wasm32-unknown-unknown --lib` passed for the web application.

### Imported protection metadata validation (2026-11-07)

Password verification now accepts XML whitespace within base64 hashes and salts,
and around numeric iteration counts, in both Word protection attribute forms.
Malformed numeric counts no longer fall back to the default count. A lone
iteration-count attribute is treated as incomplete password metadata rather than
as password-free protection.

The common OOXML validator checks metadata before password hashing: iteration
counts above 10,000,000, salts above 65,536 decoded bytes and hashes above 64
bytes are rejected. These implementation limits keep imported protection from
causing unbounded verification work; higher work factors remain unsupported.
Regression coverage includes independent Unicode password vectors with XML
whitespace, malformed/overflowing counts, incomplete metadata and oversized
base64 values.

Validation: all 2,415 editor library tests passed (21 existing ignored); the web
application also passed `cargo check --target wasm32-unknown-unknown --lib`.

### Protection command/query compatibility (2026-11-07)

The canvas and headless document-protection metadata APIs now expose ISO
`algorithmName` credentials through the optional `algorithmName` field. Existing
`algorithmSid` requests keep the older attribute form. Both forms support
SHA-1, SHA-256, SHA-384 and SHA-512; shared validation checks digest lengths,
base64 values and the work factor before changing the document. All four
editing modes and independent formatting restrictions can be authored through
this API (see the protection-mode update below).

Switching credential forms removes stale attributes from the previous form,
while retaining extension attributes and unrelated settings. Setting, removing,
and undoing protection now follow the Settings relationship target, including
nonstandard paths, and preserve the settings part's relationships.

Regression coverage exercises all four algorithms in both forms through the
serialized headless schema, command and query, then verifies the password,
exports/reopens the package and checks exact settings XML after undo/redo.

Validation: 2,416 editor library tests and all 9 headless API tests passed
(21 existing editor tests ignored). The web application passed
`cargo check --target wasm32-unknown-unknown --lib`.

### Protection modes in the automation API (2026-11-07)

The document-protection command now authors `readOnly`, `comments`,
`trackedChanges` and `forms`, with active or inactive enforcement. The optional
`formatting` field controls direct-formatting restrictions independently. An
`edit` value of `none` with `formatting: true` creates formatting-only protection;
imported formatting-only elements without an `edit` attribute are also visible
through the metadata query. Unknown modes are rejected before any mutation.

Starting enforced tracked-changes protection saves the recording switch in the
same settings XML update and activates recording immediately. Undo restores both
the exact previous XML and the previous revision session; redo restarts the lock.
Existing revision author information is retained. Removing the restriction keeps
the current recording session.

Regression coverage checks every mode with enforcement on/off, alternate settings
paths and namespace prefixes, formatting-only imports, editing guards, saved and
reopened tracking, actual tracked typing, undo/redo and atomic invalid requests.

Validation: 2,418 editor library tests and all 9 headless API tests passed
(21 existing editor tests ignored). The web application passed
`cargo check --target wasm32-unknown-unknown --lib`.

### Protection on documents without a settings part (2026-11-07)

The automation protection command can now create a missing settings part. It
registers the main-document relationship and settings content-type override along
with the XML. Undo restores package metadata and the recording session, removing
the newly created part when it did not exist before; redo recreates it. Removing
protection from a document with no settings remains a no-op.

Interactive editing restrictions and read-only recommendations use the same
writer and can also create the settings part. Saving either preserves the other
restriction and the settings part's existing relationships. Semantic documents
without an OPC package continue to update their in-memory settings only.

Protection and saved change-tracking commands share the settings writer. If
relationship or content-type registration fails, the writer restores the package
state instead of leaving a partially added relationship. Regression coverage
checks all five edit values on a minimal package, exported OPC metadata, reopened
protection, exact exported bytes after undo/redo, and rollback on malformed
content-type XML for both commands.

Validation: 2,421 editor library tests and all 9 headless API tests passed
(21 existing editor tests ignored). The web application passed
`cargo check --target wasm32-unknown-unknown --lib`.

### HTML formatting on ordinary browser paste (2026-11-07)

The DOM paste handler now captures `text/html` alongside `text/plain` while the
clipboard event is active. GPUI transports the HTML in string metadata, including
when an image read requires asynchronous delivery. Word imports the fragment
through the existing HTML clipboard parser and applies the configured paste mode
as one undo step. The caret follows the inserted fragment. HTML-only clipboard
data is accepted; empty HTML falls back to the plain text. Search and comment
fields continue to consume the text, and matching in-process rich copies keep
priority.

Validation: the release WASM build passed. The new Playwright regression
`scripts/test-web-word-clipboard.cjs` reproduced lost formatting on the previous
build using a real system clipboard write and Command-V. It passed against the
updated build, checking saved DOCX bold, italic, color and size, caret position,
atomic undo/redo, HTML-only data and plain-text fallback. The latter two cases use
DOM clipboard events to supply those specific MIME combinations.

### HTML formatting on browser copy (2026-11-07)

Browser Word Copy and Cut now publish `text/html` together with `text/plain`.
The HTML exporter includes paragraph alignment, bold, italic, underline,
strikethrough, superscript/subscript, font family, point size and RGB color.
It preserves consecutive spaces and escapes text and font-family attributes.
Both document colors (`#RRGGBB`) and imported clipboard colors (`RRGGBB`) are
accepted. The HTML importer now honors inherited `white-space:pre-wrap` so
exported spacing survives reimport.

The browser starts the rich write during the input event and observes the
result. Missing rich-write support uses plain text; a rejected rich write
attempts a plain-text write. Browser clipboard permissions and user-activation
rules still apply. Native clipboard output remains plain text. Table structure,
images, list semantics and full paragraph properties are not exported by this
text-and-run representation. This does not establish Word clipboard parity.

Validation: three exporter unit tests and the four existing HTML-paste tests
passed; the release WASM build passed. The clipboard Playwright regression
failed against the previous bundle because its system clipboard lacked HTML.
The updated bundle passed an actual system clipboard copy and paste into a
separate browser editing page, checking bold, italic, RGB color and point size.
It also passed simulated unavailable-ClipboardItem and rejected-rich-write
fallbacks, plus the existing paste/caret/undo/redo checks. Microsoft Word itself
and browsers other than Chrome were not exercised by this check.

### Paste mode caret and external defaults (2026-11-07)

Browser pastes of an in-process copy now place the caret after the inserted
fragment for every default paste mode. Paste Options, Paste and Match Formatting
and Paste Special also place the caret using the model's returned position.
Previously the merge/text-only branches and the Paste Special path discarded
that position, so subsequent typing could precede the inserted text.

Ordinary external HTML paste now uses the "other programs" paste preference,
instead of the preference for copies from the same or another document.

Validation: the release WASM build passed. The clipboard Playwright regression
reproduced typing at the wrong position on the old bundle for both the default
merge mode and the explicit Paste and Match Formatting shortcut. The updated
bundle passed those routes and Paste Special, checking the saved DOCX's text
and character properties, typing after the fragment, atomic undo and redo.
Separate browser contexts with different external-paste settings verified
text-only and merge behavior while keeping other-document settings at
keep-source. The existing rich import/export and write-fallback checks also
passed. This checks Chrome browser behavior; Word parity remains open.

### Clipboard identity for equal text (2026-11-07)

Browser Word no longer identifies an in-process rich copy by plain text alone.
Each rich copy carries a fresh identity attribute on its first HTML paragraph.
Ordinary paste and asynchronous Paste Options/Paste Special reuse the cached
copy only when that identity, the text and the HTML's parsed formatting match.
The latter check handles external editors that retain the attribute while
changing the content or formatting. Otherwise the clipboard's actual HTML or
plain text is used, so copying the same words elsewhere cannot revive stale
bold/italic formatting or use the wrong source's paste preference.

When a browser exposes only plain text, that text is treated as an external
copy, including after a rich-write fallback. If the asynchronous clipboard read
is refused altogether, Paste Options/Paste Special still offer the app's cached
copy as their existing fallback; its freshness cannot be verified in that case.

Validation: the old bundle reproduced stale formatting after an external copy
of the same plain text. The release WASM build and Chrome clipboard regression
passed after the change. Actual clipboard writes covered plain text, external
underlined HTML and modified HTML retaining the identity attribute, through
ordinary paste and Paste and Match Formatting. Saved DOCX checks verified the
external underline and absence of stale bold/italic formatting. Existing rich
import/export, write fallbacks, paste preferences, Paste Special, caret and
atomic undo/redo checks also passed. Other browsers remain unverified.

### Empty paragraphs in HTML paste (2026-11-07)

The shared HTML clipboard parser retains explicit empty paragraphs at the
beginning, between text paragraphs and at the end of a fragment. A closing HTML
paragraph contributes an empty paragraph only when its contents have not
already emitted one, so `<p><br></p>` produces one blank paragraph and outer
containers do not add more. Empty paragraph alignment is retained as well.
Office's `o:p` namespace wrapper is distinguished from an HTML `p`, avoiding
an extra blank paragraph in Office HTML. Paragraph-internal `br` handling was
subsequently corrected as described below; full HTML layout parity is unproven.

Validation: two new parser regressions failed before the change; all six HTML
paste integration tests and all three existing HTML export tests passed after
it. The old browser bundle also reproduced the missing empty paragraphs. The
release WASM build and updated Chrome clipboard regression passed, checking
paragraphs in the saved DOCX after a real clipboard paste, typing at the end,
atomic paste undo and redo. Existing rich clipboard, copy identity, paste mode,
external preference and write-fallback checks remained green.


### Consecutive browser typing undo (2026-11-07)

Adjacent single-character input events now form one undo action when the gap
between events is at most three seconds. Cursor movement and selection,
formatting commands, paste, IME composition and history navigation end the run.
Multi-character insertions remain separate actions. Groups append commands in
application order rather than repeatedly nesting composite commands, so undo
reverses the run and redo restores its original character order. This is a
browser typing improvement, not a claim of matching every Word undo heuristic.

Validation: the previous Chrome bundle left part of `After` after two Undo
operations following a paste. Both new native regressions passed, covering
Unicode typing, caret movement, history branching, paste and IME boundaries;
all 36 document editing tests passed. The release WASM build passed.
The updated Chrome regression also passed: real key events undo and redo as
one run, a pause starts a separate action, and the preceding clipboard paste
remains independently undoable. Saved DOCX contents were checked after these
operations. Existing rich clipboard, empty paragraph, copy identity, paste
mode, external preference and write-fallback checks passed as well. Other
browsers and native-app typing coalescing remain unverified.

### Paragraph-internal HTML line breaks (2026-11-07)

HTML `br` elements now produce line breaks inside the current document
paragraph, retaining its alignment and character formatting. Block boundaries
still produce paragraphs. A lone unmarked `br` remains an empty-block
placeholder, so `<p><br></p>` keeps its existing blank-paragraph behavior.
Copied line breaks carry a `data-jade-line-break` attribute to distinguish
an actual standalone break from that placeholder when pasted back into Jade.
This follows the distinction between a paragraph and the run-level break in
[Microsoft's Open XML documentation](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.break?view=openxml-3.0.1).
Actual Word HTML clipboard output and full HTML layout parity remain unverified.

Validation: the previous parser failed two new regression cases, and the old
Chrome bundle added paragraphs for `br`. All eight HTML paste integration
tests, three HTML export tests and four document clipboard tests now pass, including standalone and
trailing soft breaks, formatting, selection copy, and paste undo/redo.
The release WASM build and Chrome clipboard regression passed. Real clipboard
paste saved three `w:br` elements without increasing the paragraph count;
bold text, the following input position, undo and redo were also checked.
Existing clipboard identity, write fallbacks, paste modes, empty paragraphs
and typing undo checks passed in the same browser run.

### Unicode spaces in HTML paste (2026-11-07)

HTML import now preserves nonbreaking spaces (U+00A0 and U+202F), width-specific
spaces including U+2002/U+2003/U+2009, and Japanese ideographic spaces (U+3000).
Previously NBSP became an ordinary space and other Unicode whitespace was
collapsed, changing text and potentially its wrapping or spacing. Ordinary
HTML spaces, tabs and segment breaks still collapse in normal text; preformatted
text retains its tabs and spaces. The distinction follows the document
whitespace scope in [CSS Text Level 3](https://www.w3.org/TR/css-text-3/#white-space-processing).
This verifies preservation of characters, not complete Word line-layout parity.

Validation: three parser cases failed before the fix, including an updated
Office HTML expectation. All ten HTML paste tests, three HTML export tests and
five document clipboard tests passed after it. These cover leading, trailing
and consecutive Unicode spaces, normal and preformatted text, bold formatting,
HTML roundtrips, paste caret offsets, copy and undo/redo. The release WASM build
passed. The old Chrome bundle also reproduced the changed spaces in saved DOCX.
The updated Chrome clipboard regression passed on retry, verifying exact DOCX
characters, bold formatting, following input placement and undo/redo alongside
the existing clipboard checks. The first run passed the new assertions but
stopped later when an unrelated DOCX download was canceled. Other browsers and
Word's own HTML import remain unverified.

### HTML attribute boundaries in paste (2026-11-07)

The clipboard parser now reads attribute names and complete values in order.
Quoted greater-than signs no longer terminate tags, and text such as
`style='font-weight:bold'` inside a title is not treated as an actual style
attribute. Unquoted values retain slashes, and an apostrophe inside an unquoted
value does not swallow subsequent markup. This addresses text contamination,
incorrect formatting and truncated font names in copied HTML. The boundary
rules follow the [HTML tokenizer's attribute value states](https://html.spec.whatwg.org/multipage/parsing.html#attribute-value-(double-quoted)-state);
this remains a limited clipboard importer rather than a complete HTML parser.

Validation: all three new parser regressions failed on the previous code and
all thirteen HTML paste integration tests pass after the fix. The old Chrome
bundle reproduced the spurious italic formatting from title text. The release
WASM build and updated Chrome clipboard regression passed, checking clean text,
bold formatting, absence of title-derived italics, DOCX save and atomic paste
undo/redo alongside the existing clipboard checks. Other browsers and Word's
own HTML import remain unverified.

Additional native validation for attribute boundaries: all six document
clipboard tests and all three HTML export tests passed, covering formatting,
selection copy, paste caret placement and undo/redo.

### Paragraph navigation shortcuts (2026-11-07)

In browser editing, Ctrl/Cmd+Up now reaches the current paragraph's start,
or the previous paragraph's start when already there. Ctrl/Cmd+Down reaches
the next paragraph's start, stopping at the current story's end. These keys
previously jumped to the document boundaries. Ctrl/Cmd+Home/End retain document
navigation. Shift extends the selection from its existing anchor.
Cmd+Shift+Down selects to the current paragraph's text end as described in the
paragraph selection correction below.
The paragraph shortcuts follow [Microsoft's Word keyboard shortcuts](https://support.microsoft.com/en-us/accessibility/word/keyboard-shortcuts-in-word).

Movement follows paragraph identities within the same body, note or text box,
including table cells and empty paragraphs. It skips other stories interleaved
in the rendered paragraph list. Three new document tests passed, covering
Japanese offsets, empty paragraphs, selection extension, boundaries, footnotes,
endnotes, text boxes and unchanged DOCX output. All 36 existing editing tests
and the release WASM build also passed. PageUp/PageDown editing navigation
is covered separately below.

Chrome reproduced the old document-end jump before the fix. The updated
shortcut regression passed with both Meta and Control, checking paragraph
movement into a table cell, movement to the current paragraph's start, input
placement, saved DOCX and undo. Existing paragraph formatting, history, breaks,
table-row and tracked-change persistence checks passed in the same run.
Other browsers and actual Word interoperability remain unverified.

### Screen navigation shortcuts (2026-11-07)

PageUp/PageDown now move the editing caret by the scroll viewport's height.
Shift+PageUp/PageDown extend the selection to the visible top or bottom edge,
following [Microsoft's Word keyboard shortcuts](https://support.microsoft.com/en-us/accessibility/word/keyboard-shortcuts-in-word).
Previously these keys left the caret in place. The view converts viewport
coordinates through its zoom and page grid into document positions. Vertical
movement retains the preferred column across short lines and stays within the
current body, note or text box. Header/footer editing translates the target
into its active story's coordinate system.

Validation: four document navigation tests and all 36 existing editing tests
passed, covering preferred columns, selection anchors, empty paragraphs, notes,
text boxes, unchanged DOCX output and invalid coordinates. The release WASM
build passed. Chrome reproduced the old stationary caret; with the new bundle,
PageDown moved 18 paragraphs in a 600px viewport and 38 in a 1000px viewport.
PageUp, screen-edge selection, saved DOCX and undo are covered by the browser
regression. Zoom, alternate page arrangements, headers/footers, other browsers
and comparison with Word itself have not been browser-verified. Ctrl/Cmd plus
PageUp/PageDown navigation is covered below.

### Physical page navigation shortcuts (2026-11-07)

Ctrl/Cmd+PageUp/PageDown now reach the first text line on the previous or next
physical page, following [Microsoft's Word keyboard shortcuts](https://support.microsoft.com/en-us/accessibility/word/keyboard-shortcuts-in-word).
The target is resolved from laid-out lines, including paragraphs that continue
across pages, rather than from the scroll viewport's height or paragraph starts.
Shift retains the selection anchor. Targets stay in the current story; a page
without an eligible line leaves the caret unchanged. Header/footer navigation,
blank-page handling and Word's context-dependent browse targets remain gaps.

A new regression exposed a boundary-affinity error while extending a selection:
the focus at a wrapped page boundary was drawn at the previous page's end.
Affinity resolution now measures the selection's focus as well as collapsed
carets. All five document navigation tests and all 36 existing editing tests
passed, including a long Japanese paragraph spanning multiple pages, selection
anchors, first/last document boundaries and unchanged DOCX output.

The release WASM build passed. The old Chrome bundle reproduced Ctrl+PageDown
leaving the caret in the first paragraph. The updated browser regression passed
at both 600px and 1000px viewport heights, testing Control and Meta with page-up,
page-down and Shift selection, exact saved DOCX text and undo. Existing screen
movement and selection checks also passed. Blank pages, multi-column layouts,
notes and text boxes have not been browser-verified for these shortcuts; other
browsers and actual Word interoperability remain unverified.

### Mac paragraph-end selection (2026-11-07)

Cmd+Shift+Down now selects through the current paragraph's text end, excluding
its paragraph break, following [Microsoft's Word for Mac shortcut reference](https://support.microsoft.com/en-us/accessibility/word/keyboard-shortcuts-in-word).
Previously it reused the next-paragraph movement, so replacing selected text
also deleted the paragraph break and joined the following paragraph. The
editing model now has a distinct paragraph-text-end motion, used for this Mac
shortcut. Plain Cmd+Down and the Control shortcuts retain their current paths.

Six document navigation tests and all 36 existing editing tests passed. The
new case covers Japanese byte offsets, selected text, replacing the selection,
preserving an empty following paragraph, undo and unchanged exported DOCX.
Footnote and endnote cases also verify selection ends in the current note.
The old Chrome bundle reproduced the paragraph join in saved DOCX with two
ordinary body paragraphs; a body-to-table boundary alone did not expose it.

The release WASM build passed. Updated Chrome checks at 600px and 1000px
viewport heights verified the exact paragraph list after replacement and undo,
alongside the existing screen and physical-page navigation checks. The shortcut
regression also passed with Meta and Control, including a body-to-table case,
paragraph formatting, history, breaks, table rows and tracked-change persistence.
Repeated boundary presses, other browsers and actual Word interoperability
remain unverified for this correction.

### Vertical arrows within notes and text boxes (2026-11-07)

Up/Down and Shift+Up/Down now restrict hit testing to the current editing
story. Footnotes, endnotes and text boxes previously used unrestricted hit
testing, allowing movement or selection to escape into nearby body text.
Body movement continues to include body paragraphs and table cells. Each note
and each text box is identified separately, so neighboring stories are excluded.

The existing six document navigation tests now cover top/bottom boundaries
with and without Shift in footnotes, endnotes and text boxes, movement within
wrapped Japanese text, selection anchors, undo and unchanged DOCX export.
The old implementation failed the new boundary assertions for both notes and
text boxes. All six navigation tests and all 36 editing tests passed after
the correction. The release WASM build also passed. Chrome regressions at
600px and 1000px viewport heights passed for screen/physical-page movement,
Shift selection, saved DOCX content and undo. Direct Chrome verification now
also covers footnotes, endnotes and VML text boxes using two separate stories
of each type: top/bottom boundaries with and without Shift, internal paragraph
movement, selection replacement, exact saved text isolation and undo. Actual
Word interoperability remains outstanding.


### Clicking text inside text boxes (2026-11-07)

A click on text inside a text box now places the caret. Previously the object
handler intercepted every click within the box, so typing after the click did
not edit its text. The handler now allows text hits belonging to that box to
reach text selection, while retaining object selection at its border and the
bottom-right resize handle.

The old Chrome bundle reproduced the missing edit in saved DOCX. The release
WASM build passed with the correction. The browser regression is
`scripts/test-web-word-story-arrows.cjs`; it checks text insertion, vertical
movement and selection, saved DOCX isolation from the body and a second text
box, and undo. Resize interaction was subsequently verified as described below.
An exploratory border-click-and-type check found that the old text selection
could still receive typing while the object outline was selected. The input
routing correction below addresses this case. Rotated, grouped and linked text
boxes remain unverified.

A separate exploratory drag check exposed a VML transform/export problem:
dragging the fixture's inline VML text box exported a new DrawingML inline
box alongside the original VML box, without changing the original VML style.
The duplicate export is corrected by the change below. The fixture specifies
compatibility mode 15 so saving a transformed object does not pause the test at
the older-format compatibility checker.

### Transforming VML text boxes without duplicate export (2026-11-07)

Object edits rebuild a modeled VML text box as DrawingML. The run writer now
consumes the original `w:pict` in that path, rather than preserving it as an
additional unmodeled child. Previously both drawings were exported, and the
reader could take the stale VML frame on reopening. Text edits that preserve
drawings retain their existing VML preservation path.

The new `objects_documents` regression failed before the correction with two
`w:txbxContent` elements instead of one. It now verifies inline resizing and
floating movement/resizing, one exported box, reopened frame geometry, Japanese
text and bold formatting, undo and redo. Undo restores the original frame;
the transformed document remains DrawingML rather than recovering the original
VML bytes. All 22 object, picture and shape document tests passed.

The release WASM build passed. The old Chrome bundle also reproduced duplicate
text after resizing. With the correction, the story-arrow browser regression
passed for footnotes, endnotes and text boxes. Its new resize case verifies a
20px width and 10px height increase in saved DOCX, exactly one copy of the box's
text, unchanged second-box geometry and text, and undo restoring the original
frame and text. Arbitrary inline dragging, unsupported VML extensions, rotated
or grouped VML shapes, other browsers and actual Word interoperability remain
unverified. The object-selection text-input correction is described below.

### Text input while an object frame is selected (2026-11-07)

Selecting a picture or text-box frame now removes the remembered document
caret from the Word input context. Direct text insertion, IME composition and
plain or rich text paste no longer replace the previous text selection while
that frame is selected. Clicking text inside the box clears object selection
and restores normal text editing. Find and comment fields keep their existing
input paths.

The old Chrome bundle reproduced the regression: after selecting a paragraph
range, clicking the border and typing deleted the middle paragraph. The release
WASM build passed. The story-arrow browser regression passed for all three
story types. For text boxes it verifies unchanged saved DOCX after direct
input, Japanese IME composition/commit through CDP, and plain/HTML browser
paste events; clicking content resumes insertion and undo. The existing resize,
other-box isolation and story-navigation checks also passed.

This covers text input routing, not complete object keyboard behavior. The
Delete/Backspace and Escape correction is described below. Arrow movement,
object clipboard operations, IME behavior on physical input methods, other
browsers and actual Word interoperability remain unverified.


### Deleting a selected object without deleting remembered text (2026-11-07)

The browser now routes Delete and Backspace on a selected frame to the object
command, rather than deleting the previous text selection. Escape clears the
frame selection and returns input to the remembered caret. Existing dialog,
find, comment and specialized object-mode keyboard handling keeps priority.

Deleting a text box or picture also removes its matching raw drawing container.
Previously, a drawing sharing a run with ordinary text could survive in the
saved package after disappearing from the model. The deletion command now
journals the paragraph and XML so undo restores the original drawing bytes,
including VML, and redo removes the object again.

The native object, picture and shape suites passed all 24 tests. New cases
verify host text and other paragraphs survive deletion, saved DOCX reopens
without the object, and undo restores the original document XML. The old Chrome
bundle reproduced the keyboard bug by deleting the selected middle paragraph
while leaving its text box intact.

The release WASM build and the Chrome story-arrow regression passed for all
three story types. Text-box checks cover both Delete and Backspace after a
remembered text range, only the selected box disappearing from saved DOCX,
undo/redo, Escape resuming text input, and the existing typing/IME/paste and
resize checks. Other-box text and geometry remain unchanged. The arrow-key movement correction is described below. Object clipboard
operations, drawings nested in other stories,
unsupported drawing extensions, other browsers and actual Word interoperability
remain unverified.


### Moving a selected object with arrow keys (2026-11-07)

The browser's selected-frame keyboard path now invokes the shared object
transform command for Left/Right/Up/Down, using the desktop editor's 7.5px step
and 1px step with Alt. After a move the selected frame is refreshed from layout,
so consecutive keys use the current object position. Undo/redo also refreshes
that frame. Without the history refresh, moving right after undoing four nudges
started at the old frame coordinates (158,102.5 instead of 151.5,96 in the
floating-box fixture).

The browser regression has a floating VML box variant and reads saved page
positions from both VML and DrawingML. It checks consecutive right/down and
Alt-left/up operations, unchanged text and sizes for both boxes, four undo and
redo operations, and a fresh movement after undo. This complements the existing
inline-box text editing, deletion and resize cases.

The release WASM build and the floating-box Chrome regression passed, including
keyboard and ribbon Undo/Redo and subsequent movement from the restored frame.
Inline-object keyboard semantics, multiple selected objects, movement across
pages, protected documents, other browsers and actual Word interoperability
remain unverified by this regression.

The existing Chrome story-arrow run also passed for footnotes, endnotes and
inline text boxes, including frame typing/IME/paste isolation, Delete/Backspace,
Escape, saved resize geometry and undo. The script syntax and worktree whitespace
checks passed. This change reuses the existing object transform command; it does
not establish complete Word object-editing parity.


### Copying and pasting a selected object (2026-11-07)

The web editor now copies a selected picture or text-box frame as an object.
Cut removes that frame, and paste inserts the detached drawing at a collapsed
body caret, replaces a selected body-text range, or inserts beside the selected
frame. Forward and reversed body selections, including selections across
paragraphs, are replaced as one undo/redo step. Invalid endpoints are rejected
before text or package content changes; a failed import restores both. A
remembered selection inside a text box is not replaced by these operations.
Ordinary external text cannot reuse a stale object copy. The Home paste and Paste Special paths also recognize an
object copied by this editor.

The detached copy retains its drawing XML and recursively copies related
package parts with fresh destination paths and relationship IDs. This includes
SVG originals and PNG fallbacks, chart data and embedded workbooks. Drawing IDs
are renewed, checking saved VML IDs as well so that another paste after reopening
does not reuse an earlier ID. The clipboard retains the detached data after a
cut; package changes and document content are restored together by undo and redo.

The native `objects_documents`, `pictures_documents` and `shapes_documents`
suites pass 29 tests. New cases copy a VML box sharing a host-text run, an SVG
with fallback and an editable chart into another document, save and reopen each,
and compare complete DOCX bytes across undo and redo. Replacement cases cover
Japanese text, forward/reversed and cross-paragraph selections, invalid UTF-8
and out-of-range endpoints, and exact document/selection restoration. The Chrome
`OBJECT_CLIPBOARD=1 STORY_KINDS=textbox` regression passes frame copy/cut/paste,
content and geometry retention, remembered-range isolation, clipboard identity
and selection replacement with undo/redo through the browser clipboard,
including merge-formatting and Paste Special commands. The new replacement
regression failed against the previous WASM and passes after rebuilding. The
full footnote/endnote/text-box story regression and the existing rich-text
clipboard regression also pass.

This is an in-process object clipboard identified by a small HTML marker and a
replacement-character text representation. It does not export a reconstructible
object to Word or another browser session. Copying or pasting objects in other
stories, multiple selected objects, and migration of source document defaults,
themes and fonts remain gaps.
Linked VML shapes, SmartArt and OLE dependency graphs have not been verified by
these new cases. Full Word clipboard interoperability remains open.


### Retaining named styles in copied text boxes (2026-11-07)

Object copies now carry the paragraph and character styles referenced inside
text boxes, along with their `basedOn`, `link` and `next` dependencies. Import
allocates separate IDs for destination collisions and rewrites both the drawing
and the imported style definitions, including cyclic references. Existing style
definitions and unresolved destination references retain their meaning. Imported
styles do not become document defaults, and unreferenced source styles are not
imported. A destination without a styles part receives one and the associated
package relationship; unlinked pre-existing style parts are preserved without
activating them. When creating that part, the destination’s implicit Word
styles become explicit so its existing font and paragraph defaults are retained.
Package snapshots restore the style table together with the object during undo
and redo.

The native object/picture/shape suites pass 30 tests. The new text-box case checks
inherited bold and character-style italic immediately after paste and after
DOCX reopening, with colliding styles, no destination styles part, and an
unlinked old styles part. It checks dependency remapping, unchanged effective
host formatting and exact DOCX bytes across undo/redo. The browser WASM release
build also passes. The browser object-clipboard regression passes frame
copy/cut/paste, remembered-range isolation, geometry, clipboard identity,
selected-text replacement and undo/redo.

Source document defaults, theme-dependent colors and
fonts, and embedded font parts still require migration to fully retain source
appearance across documents. This change does not establish full Word clipboard
interoperability.

### Retaining numbering in copied text boxes (2026-11-07)

Object clipboard data now includes the numbering instances used by text-box
paragraphs and their styles, the referenced abstract definitions and picture
bullets. The dependency walk also imports styles referenced only by list levels
or numbering-style links. It follows dependencies until stable, including cycles.
Unused numbering instances stay in the source document.

Paste allocates fresh IDs in the three independent numbering ID spaces and
rewrites paragraph, style and numbering references together. Existing destination
list definitions and unresolved references retain their IDs. Start overrides,
level definitions and their raw XML are retained. Picture-bullet image parts and
relationships travel with the clipboard and resolve immediately after paste.
Destinations without a linked numbering part receive one; an unlinked old part
is preserved without being activated. Undo and redo restore the entire package.

The native object/picture/shape suites pass 31 tests. The new numbered-text-box
case checks the start override of 9, decimal formatting, a picture bullet and its
image bytes, style dependencies and collisions, an unresolved host list ID, and
existing destination list formatting. It checks documents with a linked part,
no numbering part, or an unlinked old part, both immediately and after DOCX
reopening, with exact package-byte restoration across undo/redo. Existing
picture-bullet images and relationships survive alongside the imported ones.
The style-copy regression also checks that a destination style's numbering
association survives pasting an object with styles but no numbering definitions.
The WASM release build and browser object-clipboard regression pass, including
frame copy/cut/paste, geometry and clipboard identity, selected-text replacement,
and undo/redo.

Document defaults, theme-dependent formatting and embedded fonts still require
migration for full source-appearance retention. Native Word clipboard exchange,
list continuation choices, and Word interoperability remain broader parity work.

### Retaining source theme colors and fonts in copied objects (2026-11-07)

Object copying resolves source theme references in the detached drawing and its
imported style and numbering definitions. WordprocessingML theme fonts become
explicit faces, and text, underline, border and shading colors become explicit
RGB colors with their tint/shade applied. DrawingML scheme colors become RGB
colors while retaining transform children such as alpha; theme font tokens are
resolved in DrawingML font elements too. Unresolvable references and their
fallback values are retained. Copying leaves the source document intact and
pasting leaves the destination's theme intact. The followed-hyperlink theme
color lookup also now matches the theme parser's normalized slot names.

The native object/picture/shape suites pass 33 tests. The two new cases copy
between documents with different theme colors and fonts, checking direct text,
style and numbering XML, tint/shade, followed-hyperlink underline color,
unresolved-reference fallback retention, DrawingML alpha transforms, source and
destination theme preservation, reopening, and exact package undo/redo.
The WASM release build and browser object-clipboard regression also pass,
covering frame copy/cut/paste, geometry, selected-text replacement and history.

Full source-appearance retention still requires document-default migration,
embedded fonts, custom color-scheme mappings, theme format-matrix references,
and the independent theme contexts of chart and diagram dependency parts.
These changes do not establish complete Microsoft Word parity.


### Retaining source document defaults in copied text boxes (2026-11-07)

Object copy now creates a private paragraph base style carrying the source
`docDefaults`, including unknown XML and namespace declarations. Imported root
paragraph styles inherit that base. Paragraphs without a valid explicit style
receive the source default paragraph style, or the private base if none exists.
Name collisions are remapped without importing unrelated source styles or
changing the destination's default styles. Documents without a styles part use
the same built-in Word defaults as the reader.

The base also supplies zero paragraph before/after spacing where the source
omits it, and automatic text color where the source has no default color. This prevents those destination defaults
from leaking into the copied text. Named styles and direct formatting retain
their precedence over the copied defaults. Source theme resolution also applies
to the generated base.

The regression covers distinct source and destination font sizes, fonts,
colors, alignments and paragraph spacing; named style inheritance; missing style
references; source and destination style-name collisions; absent styles parts;
host-default preservation; reopening; and exact package undo/redo.

The native object/picture/shape suites pass 34 tests. The WASM release build
and browser object-clipboard regression pass, including copy/cut/paste,
geometry, text replacement and undo/redo.

This is partial source-default migration. Other omitted run/paragraph fields,
Word's toggle-style semantics, automatic color in different backgrounds,
cyclic style inheritance, table defaults, stylesWithEffects, embedded fonts and
independent chart/diagram themes still require work. Complete Word parity has
not been established.


### Automatic text color and theme metadata in style inheritance (2026-11-07)

Style parsing now retains an explicit automatic run color as `#auto`, matching
the existing direct-formatting representation. Previously it became `None`, so
automatic colors inherited a parent's RGB color instead of overriding it.
Paragraph and character styles now distinguish automatic color from absence.

Run-property merging also keeps the color and theme reference together. Theme
name/tint/shade survive inheritance, while a later RGB or automatic color clears
the overridden theme reference. A copied object's private default style now uses
`w:color w:val="auto"` for an absent source default color, retaining automatic
semantics rather than freezing black into the saved document.

This follows the distinction between absent and automatic color described in
[Microsoft's WordprocessingML Color reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.color?view=openxml-3.0.1).
Background-sensitive automatic-color rendering still requires work; this change
preserves the authored choice and fixes inheritance and clipboard contamination.

The reader style suite passes 57 tests, including automatic color overriding
inherited RGB, explicit RGB overriding automatic character formatting, and theme
metadata replacement through the complete run-property cascade.
The native object/picture/shape suites also pass 34 tests, including explicit
automatic color retention in the copied base and saved package. The WASM
release build and browser object-clipboard regression pass.

### Background-aware automatic text color (2026-11-07)

The canvas renderer now retains whether a cached run color is automatic,
including an omitted color after style resolution. It selects black or white
against supported solid RGB backgrounds when creating draw commands, rather
than permanently converting automatic text to black during style caching.
In the body text path, run highlights take precedence over character shading,
followed by paragraph shading, the containing shape/cell, and the page background. Explicit run colors
remain authored colors on run, paragraph, shape and cell fills. The dark-preview remap still applies to
text on the viewer's page background.

Floating and inline text boxes and table cells scope their background state and
restore it on exit. Unfilled nested tables inherit the enclosing cell's
background; repeated table headers use the same resolution. WordArt also
normalizes `#auto` before constructing a CSS fill.

The black/white choice uses linear-sRGB contrast, a consumer policy consistent
with the automatic-color semantics in the
[OOXML Color reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.color?view=openxml-3.0.1).
Exact agreement with Word's decision boundary is not established. Gradient,
picture and partially transparent backgrounds, table run highlights/character
shading, and all vertical-text/background combinations still need dedicated coverage. This
extends the automatic-color inheritance work above; it does not establish full
Word color-rendering parity.

Validation: 872 canvas-renderer tests pass (18 ignored), including background
precedence, preservation of fixed colors, reuse of cached styles across different
backgrounds, dark filled shapes, restoration of body color, and dark nested cells
in both normal and repeated-header painting.
The WASM release build and browser text-box object-clipboard regression also
pass, including copy/cut/paste, text replacement and undo/redo.

### Paragraph shading inside tables (2026-11-07)

Horizontal table paragraphs now retain their resolved shading through layout and
page fragments. Both normal cells and repeated header cells paint a shading band
for each line, within the cell clip and paragraph indents, before selection and
text. Automatic text color resolves against this band ahead of the cell fill;
explicit run colors remain unchanged. The next paragraph and nested table keep
their own background context.

Regression coverage checks dark and light paragraph fills, automatic and fixed
text colors, nested tables, and ordinary/repeated-header rendering. Uncommon
vertical column geometry is boxed to retain the existing cached-paragraph memory
budget. Rotated paragraph shading and table run highlights/character shading
remain separate gaps; this change does not establish Word rendering parity.

Validation: 873 canvas-renderer tests pass (18 ignored), including the existing
cached-paragraph memory budget. The WASM release build passes.
`scripts/test-web-word-table-shading.cjs` loads a DOCX in Chrome and checks the
rendered pixels for navy/white automatic text, fixed black text on navy, and
yellow/black automatic text. The fixture uses 24-point text to give stable solid
text pixels despite antialiasing. Run with Playwright and pngjs on `NODE_PATH`,
`PLAYWRIGHT_CHROMIUM_EXECUTABLE` pointing to Chrome, and the web shell on port
8765 (or pass its URL as the first argument).

### Run backgrounds inside tables (2026-11-07)

Horizontal table text now paints run highlights and character shading. Marker
highlights take precedence over character shading; `none` falls through to the
character fill, matching the body renderer. Automatic text color uses that run
background before paragraph, cell and page backgrounds. Fixed colors on authored
run backgrounds remain unchanged, and discretionary hyphens use their final run's
background when choosing automatic text color.

Ordinary cells, repeated header cells, and the `lrTbV` horizontal writing path
share this resolution. The ordinary span path uses the body renderer's clipped
font-em highlight bounds, including baseline shifts; the `lrTbV` path fills each
glyph's unrotated line box before rotating its ink. Fully vertical/rotated writing
paths and exact Word highlight geometry still need further work.

Validation: the native canvas-renderer suite passed 874 tests (18 ignored),
including run-background precedence and fixed/automatic colors in ordinary and
repeated-header cells, with `lrTbV` coverage. The release WASM build succeeded.
The browser pixel regression verified navy/yellow paragraph fills, explicit
black text, cyan highlight precedence over red character shading, and automatic
white text on green character shading with `highlight=none`.

### Vertical table background and color resolution (2026-11-07)

Vertical table paragraphs now resolve text color for each styled glyph/group,
including character highlights and shading, instead of reusing the first run's
color for the whole paragraph. Upright and turned columns paint run backgrounds
in their own coordinate systems. Paragraph shading follows the vertical column
bands rather than the horizontal paragraph painter. Repeated headers use the
same path. The full-string rotated visual path now paints styled spans with
their own fonts, scale, backgrounds and text colors, preserving layout advances.

The background precedence remains highlight, character shading, paragraph,
cell and page. Exact Word highlight extents and mixed formatting inside a single
vertical combined-character group still need comparison with Word.

Validation: 874 native canvas-renderer tests passed (18 ignored). The run-fill
matrix covers all seven table writing directions, normal and repeated rows, and
paragraph fallback after `highlight=none`. The release WASM build and browser
pixel regression passed for all seven directions. These checks establish color
and background rendering, not complete vertical geometry: the browser fixture
also exposes vertically stacked paragraph flow and clipped end text in the
full-string rotated path, which remain layout work.

### Rotated text follows wrapped columns (2026-11-07)

The rotated column path now paints each layout column separately instead of
painting the entire paragraph as one unbroken string. This covers the `btLrV`
compatibility direction in cells and rotated `tbRlV` section text. Each column
retains source glyph offsets and run formatting through the shared span painter;
its transform uses the same origin, writing axis, and advances as selection.
The old empirical full-paragraph anchor offsets are no longer used. The cell
layout also no longer adds an extra line of wrapping capacity before shrinking
the visible column height, which could place final glyphs outside that height.

The native canvas-renderer suite passed 874 tests (18 ignored), including a new
multi-column regression using the actual cell and section layout adapters. It
checks complete text, per-column painting, and selection bounds. Spelling waves
in the rotated path now use the shared continuous span painter while retaining
the decorated UTF-8 range and export suppression.

The release WASM build succeeded. A browser regression with a short cell and a
red final `H` reproduced the old failure (0 red pixels), then passed with the
new column painter (128 red pixels, all inside the navy column bands). Its
screenshot shows all eight letters across four columns. This confirms that
fixture's clipping correction; it does not establish exact Word typography.
Vertical multi-paragraph cell flow and mixed combined-character formatting
remain open comparisons.

The existing browser background/color regression also passed for all seven
table writing directions after this change. Native and browser checks retain
fixed colors, automatic contrast, highlight precedence, and character shading.

### Vertical table paragraphs advance across columns (2026-11-07)

Consecutive vertical paragraphs in a table cell now share a writing-axis origin
and occupy separate horizontal column bands, following the existing direction's
column order. Previously each paragraph advanced downward by its full height.
The editable body and repeated header painter use the same band placement;
selection coordinates receive the same horizontal offsets. Left and right cell
padding are reserved independently. Cell content height and refreshed fragment
height use the maximum height within a paragraph band rather than summing its
paragraphs. Nested tables and page-fragment spacing end a band and retain their
existing block flow.

The native canvas-renderer suite passed 875 tests (18 ignored). The new regression
covers all five stacked table directions, multiple columns per paragraph,
nonoverlapping paint and selection bounds, repeated-header paint equivalence,
asymmetric padding, centered vertical alignment, and a spacing boundary.

This corrects the internal paragraph-flow defect; exact Word direction mappings,
paragraph alignment/spacing in the writing axis, and mixed vertical text with
nested tables still need native Word comparisons. The installed Word application
could not be inspected during this run because the desktop was locked.

The updated browser fixture asserts a common top edge for navy and yellow
paragraph bands in stacked directions. Before rebuilding, `tbRlV` failed this
check (navy y=135, yellow y=164). The rebuilt screenshot places all four paragraph
bands side by side on one page. Upright Latin glyph spacing still appears crowded
in that screenshot and remains an open typography issue.

The release WASM build succeeded. All seven table-direction browser background
and color cases passed, including the new shared-top assertion for the five
stacked directions. The short rotated-cell wrap regression also passed, retaining
the final red `H` inside its column background. Root and core whitespace checks
and the browser script syntax check passed.

### Upright Latin table-cell advances (2026-11-07)

Upright `tbRlV` and `tbLrV` table cells now keep a Latin glyph's measured
horizontal ink width separate from its writing-axis advance. The cell adapter
uses a font-size em fallback for Latin advances, so narrow letters no longer
advance by their horizontal width and overlap. Column wrapping, justification,
painting, and selection consume that same advance; centering still uses measured
ink width. Both upright directions restore the ink width and use the upright
font baseline. CJK measurement and explicit tate-chu-yoko groups retain their
existing advances.

The OpenType specification defines vertical advance heights separately from
horizontal advances ([vmtx](https://learn.microsoft.com/en-us/typography/opentype/spec/vmtx)).
The em fallback here is an internal crowding fix, not a measured claim of Word
parity. Font-specific vertical advances and origins, vertical kerning, and
native Word comparisons remain open.

Native validation passed 714 layout tests (12 ignored) and 876 canvas-renderer
tests (18 ignored). Regressions cover narrow and wide measured ink, scaled runs,
source ranges, wrapping, both upright directions, and matching paint/selection
positions. The browser fixture `JADE_TABLE_UPRIGHT=1` renders four 24pt Arial
lowercase l glyphs and checks for separate ink bands at an em pitch. The old
WASM failed with one merged ink band instead of four.

The release WASM build succeeded. In the rebuilt browser, both upright
fixtures show four separate ink bands at y=355, 387, 419, and 451 (32px pitch
for 24pt text); the `tbRlV` screenshot was visually inspected. All seven table
directions passed background/foreground and paragraph-band checks. The existing
short rotated-column fixture also passed, retaining the final red H inside its
background (128 red pixels; 78 white-ink columns). No browser page errors were
reported. Evidence logs: `/tmp/jade-upright-cell-layout-all.log`,
`/tmp/jade-upright-cell-native.log`, `/tmp/jade-upright-cell-wasm.log`,
`/tmp/jade-upright-cell-browser-before.log`, and
`/tmp/jade-upright-cell-browser-final.log`.

### Native upright cells use font vertical advances (2026-11-07)

The native Skia painter now exposes the selected glyph's positive OpenType
`vmtx` advance, scaled to the current font size. It resolves the same fallback
face used for painting. The table measurement context passes this value through
to upright cell layout for Latin and CJK glyphs. Column wrapping, painting, and
selection therefore use the font advance while retaining horizontal ink bounds.
Capture routing preserves the optional measurement hook.

Backends without font vertical metrics retain the previous em fallback for Latin
and existing CJK measurements. Missing glyphs, empty strings, and multi-scalar
graphemes return no font advance; combined tate-chu-yoko cells retain their
existing geometry. This change covers native Skia table cells. The browser
canvas backend still uses the fallback. Fontdue and native shaping support was
added in the follow-up below. Vertical origins, substitutions, kerning,
multi-scalar shaping, and comparison against native Word remain open.

The font-table regression edits the bundled Noto Sans JP vertical advances to
0.75 em, then checks 24px at 32px and 12px at 16px, through a routed painter.
It also checks missing glyphs, multi-scalar input, and a face with its vertical
metrics table removed. Layout regressions check font advances, invalid-value
fallback, text scaling, CJK, and wrapping. The renderer regression exercises
both upright directions with font-provided and fallback advances and checks
that painting and selection geometry agree.

Validation passed 715 layout tests (12 ignored), 876 canvas-renderer tests
(18 ignored), and both Skia font-table regressions. The release WASM target
passed `cargo check`; the existing browser behavior remains on the fallback
path and was not rerun for this native measurement change. Root and core
whitespace checks passed. Evidence logs: `/tmp/jade-font-vertical-layout-all.log`,
`/tmp/jade-font-vertical-editor.log`, `/tmp/jade-font-vertical-skia-final.log`,
and `/tmp/jade-font-vertical-wasm.log`.

### Shared measurement backends expose upright advances (2026-11-07)

`FontdueMeasurer` and `ShapingMeasurer`, together with their line-wrap contexts,
now expose the same optional upright-advance measurement as the Skia painter.
The shaping backend selects the face through its existing per-character fallback
and continues to record uncovered characters. Fontdue uses its selected font and
returns no advance for uncovered glyphs. Neither backend invents vertical metrics
for a font without them or treats a multi-scalar grapheme as one mapped glyph.
Both `WrapBackend` variants preserve the specialized measurement.

A real-font integration regression sets Noto Sans JP vertical advances to
0.75 em, checks two font sizes, then passes scaled Latin and Japanese runs through
shared table-cell layout. Initially the Fontdue case incorrectly wrapped every
character into a separate column at an exactly full two-character height:
scaling in f32 produced an advance slightly above 12px. Reading design units at
scale one and converting to pixels in f64 fixes the boundary without widening
layout tolerances. The same fixture verifies missing-metrics fallback. Another
case starts with Noto Sans Thai and checks that Japanese text uses the registered
JP face's vertical advance while Thai without vertical metrics returns none.

This extends the shared layout API; it does not add full vertical shaping to
these backends or establish Word visual parity. Browser font metrics, vertical
origins, vertical substitutions, and kerning remain outstanding.

Validation: native-shaping layout unit tests passed (733 passed, 13 ignored),
the new vertical-font integration suite passed all four cases, and existing
Noto Japanese and shaping-backend integration suites passed four cases with
one existing slow reference test ignored. The default-feature configuration
also passed both applicable vertical-font cases. Both repository diff checks
passed.

### Native upright cells select vertical glyph forms (2026-11-07)

Upright table/section character painting now uses an explicit painter operation.
Skia shapes a single Unicode scalar in the top-to-bottom direction to select
font vertical alternates, then paints that glyph at the layout-owned origin and
baseline. Rotated text and tate-chu-yoko retain their horizontal drawing paths.
Backends without this operation use the existing text fallback. Capture records
and replays the upright operation rather than flattening it to horizontal text.

The native vertical-advance measurement also reads the substituted glyph's
vertical metrics. A real-font test changes only the vertical bracket alternate's
advance and verifies that the original bracket uses that value. A raster test
compares an upright fullwidth opening parenthesis with the font's independently
encoded vertical presentation form, verifies the horizontal form differs, and
checks deferred replay. Multi-scalar input continues to use ordinary painting;
this does not implement combined-grapheme vertical shaping or vertical origins.
Browser canvas vertical alternates and the other measurement backends' use of
substituted-glyph metrics remain separate follow-up work. Word visual parity
has not been established.

The PDF regression initially extracted U+FE35 from an upright U+FF08: Skia's
PDF glyph mapping selected the presentation character despite the original
UTF-8 in the text blob. Substituted glyphs now use the existing outline plus
original-Unicode text-layer path for vector PDF recording. Both ordinary and
editor recording modes preserve U+FF08, verified by PDF text extraction.

Validation: all five native vertical-font integration tests passed; painter
unit tests passed 12 cases; editor canvas-renderer tests passed 876 cases with
18 existing ignored tests. The release WASM target check passed. Skia's library
test target compiled successfully but contains no unit tests.

### GPUI Web upright cells select vertical glyph forms (2026-11-07)

The default Web shell uses the TinySkia painter with registered font bytes.
That painter now selects single-scalar vertical glyphs with top-to-bottom
Rustybuzz shaping and measures the selected glyph's positive vertical advance.
The cache stores glyph IDs and design-unit advances per actual fallback face,
so changing font size does not reuse a stale pixel advance. Missing coverage,
missing vertical metrics, and multi-scalar strings retain their existing
fallback behavior. Drawing preserves transforms, shadows, blur, alpha, and
deferred capture/replay.

All 30 TinySkia unit tests passed, including two new real-font tests for glyph
substitution/effects/replay and substituted-glyph metrics across sizes and font
fallback. The release WASM build and wasm-bindgen packaging succeeded. The
browser fixture `JADE_TABLE_UPRIGHT_ALTERNATES=1` compares fullwidth opening
parentheses against Noto Sans JP's independently encoded vertical presentation
form. Both `tbRlV` and `tbLrV` matched, with 98 near-white glyph pixels inside
the largest connected navy paragraph band in each direction. Selecting that
band excludes page thumbnails and surrounding page-white pixels. An initial
comparison against the previous WASM failed before the update.
The existing `JADE_TABLE_UPRIGHT=1` browser checks also passed in both
directions, retaining four separate Latin glyphs at an em pitch. Logs:
`/tmp/jade-tiny-vertical-tests-final.log`,
`/tmp/jade-tiny-vertical-wasm-build.log`,
`/tmp/jade-tiny-vertical-browser-{rl,lr}.log`, and
`/tmp/jade-tiny-vertical-latin-{rl,lr}.log`.

This covers the default GPUI Web renderer; the separate Canvas2D painter still
uses its generic upright-text fallback. Vertical glyph origins, combined
grapheme shaping, and comparison against Word remain open. Matching the font
presentation form is a regression check, not evidence of full Word parity.

### OpenType layout measures the substituted vertical glyph (2026-11-07)

The shared native-shaping measurer now selects the top-to-bottom glyph before
reading its vertical advance, matching the native and GPUI Web painting paths.
Previously it read the original cmap glyph's advance; fonts with different
vertical-alternate metrics could therefore wrap columns differently depending
on the measurement backend. Each immutable fallback face caches design-unit
advances independently of font size. The existing shape-plan cache separates
vertical and horizontal directions.

A real-font integration test leaves the original fullwidth opening parenthesis
at one em and changes only its vertical alternate to three quarters of an em.
Before the fix, a 24pt bracket measured 32px rather than 24px. After the fix,
both direct selection and character fallback from a Thai font give the correct
advance at 24pt, 12pt, and 24pt again. Through `WrapBackend`, four brackets now
occupy two columns of two characters in a two-advance-high cell; cluster widths
and source byte ranges are checked as well. All five vertical-font integration
tests passed, and native-shaping library tests passed 733 cases with 13 existing
ignored tests. Logs: `/tmp/jade-shaping-vertical-before.log`,
`/tmp/jade-shaping-vertical-final.log`, and `/tmp/jade-shaping-vertical-lib.log`.

The lightweight Fontdue measurer still reads cmap glyph metrics without
OpenType substitution. Vertical origins, multi-scalar shaping, and visual
agreement with Word remain unverified; this change does not establish parity.

### Composed upright graphemes retain their source text (2026-11-07)

Native Skia, GPUI Web's TinySkia painter, and the native-shaping layout measurer
now accept a single extended grapheme that shapes to one nonzero vertical glyph.
This includes decomposed kana (`か` plus U+3099) and Latin (`a` plus U+0301)
when supported by the selected font. Glyph selection and design-unit advances
use the complete grapheme. TinySkia caches these results by face and source
text without allocating a string on cache hits. Unsupported or multi-glyph
clusters keep the generic fallback; the Thai face covering `a` does not borrow
another face's metrics merely because that other face can compose the accent.

Real-font tests compare composed and decomposed vertical rendering, including
TinySkia effects and captured-command replay. Noto Sans JP also substitutes the
composed kana's vertical form, so the reference uses vertical shaping on both
encodings. Layout tests verify column breaks, per-grapheme widths, and complete
source byte ranges at repeated font sizes. Native PDF extraction preserves both
source scalars in recording and editor-recording modes, assigning the glyph's
width to the first scalar and zero to subsequent scalars in the invisible text.

Validation: 30 TinySkia library tests, six native Skia integration tests, 733
native-shaping layout library tests (13 existing ignored), and six layout
integration tests passed. Logs: `/tmp/jade-grapheme-tiny-final.log`,
`/tmp/jade-grapheme-skia-final.log`, and `/tmp/jade-grapheme-layout-final.log`.

This covers one-glyph composition rather than general multi-glyph vertical
shaping. Fontdue and Canvas2D remain on their existing fallback paths. Vertical
origins and visual agreement with Word are still unverified.

The release WASM build also passed. With
`JADE_TABLE_UPRIGHT_GRAPHEMES=1`, `scripts/test-web-word-table-shading.cjs`
opens DOCX files containing two decomposed kana and compares them with two
precomposed kana in the actual GPUI Web shell. Both `tbRlV` and `tbLrV` passed:
paragraph bounds and all 290 white text pixels matched exactly. Logs:
`/tmp/jade-grapheme-wasm.log`, `/tmp/jade-grapheme-browser-rl.log`, and
`/tmp/jade-grapheme-browser-lr.log`. This compares equivalent encodings in Jade;
it is not a Word visual oracle.

### Fontdue vertical measurement follows the drawn glyph (2026-11-07)

The Fontdue layout backend now shapes upright graphemes top-to-bottom before
reading the selected glyph's vertical advance. It previously used the original
cmap glyph, and rejected every multi-scalar grapheme. That produced 32px instead
of 24px for a 24pt parenthesis whose vertical alternate alone has a three-quarter
em advance, and returned no metric for composable decomposed kana or Latin.
Both failures were reproduced in `/tmp/jade-fontdue-vertical-before.log`.

Each registered face now retains shared source bytes and its collection index,
and caches design-unit results by source grapheme independently of point size.
Missing base characters use the first registered covering face, matching the
TinySkia painter's fallback. Uncovered text, missing vertical metrics, and
multi-glyph clusters retain the existing em/generic fallback. Horizontal
Fontdue measurement is unchanged. Rustybuzz is now a required layout dependency
for vertical glyph selection; the `native-shaping` feature still controls the
full-run shaping backend. The GPUI Web painter already depends on Rustybuzz.

The same real-font integration checks now exercise Fontdue and the full shaping
backend: substituted advances, character fallback, two-character column breaks,
repeated sizes, composed graphemes, and complete source byte ranges. The normal
configuration passed 715 library tests (12 existing ignored) plus four vertical
integration tests. With `native-shaping`, 733 library tests (13 existing ignored)
and six integration tests passed. Logs: `/tmp/jade-fontdue-vertical-default.log`
and `/tmp/jade-fontdue-vertical-final.log`. These checks close the previously
recorded Fontdue cmap-metric gap; they do not establish Word visual parity or
general multi-glyph vertical shaping.

The default-feature layout crate also passed `cargo check` for
`wasm32-unknown-unknown` with the required vertical shaper enabled. Log:
`/tmp/jade-fontdue-vertical-wasm.log`.

### Print properties are identified by XML namespace (2026-11-07)

Print preprocessing now recognizes WordprocessingML `background` and `vanish`
by their namespace URI and local name, including alternate/default prefixes and
both Transitional and Strict namespaces. The previous literal `<w:…` search
missed these properties and could remove comments, foreign elements, or truncate
an element containing `>` inside a quoted attribute. The replacement parses XML
events and removes original byte ranges, preserving every unrelated XML byte.
Malformed XML encountered during preprocessing now returns an error instead of
silently dropping trailing content. Unchanged packages retain their original
ZIP bytes.

Three public-API regression tests reproduced the failures before the change
(`/tmp/jade-print-namespaces-before.log`) and now pass, together with all 171
shared-model library tests (`/tmp/jade-print-namespaces-final.log`). Coverage
includes document backgrounds, hidden text in document/styles parts, comments,
CDATA, foreign namespaces, quoted delimiters, and mismatched elements. These
checks verify package transformation; they do not establish Word print-output
or general feature parity.

The shared model also passed a `wasm32-unknown-unknown` check with the Web
`tiny-skia` backend (`/tmp/jade-print-namespaces-wasm.log`).

### Hidden text inherits paragraph and character styles (2026-11-07)

The styles reader now parses `w:vanish` in document defaults and style run
properties. Hidden-text declarations in a style toggle the inherited state;
direct run formatting sets an absolute state. The effective resolver preserves
this distinction across based-on paragraph styles and character styles, including
an explicit direct `w:vanish w:val="0"` that makes inherited hidden text visible.
See [Microsoft's Vanish reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.vanish?view=openxml-3.0.1).

Headless line layout now uses that effective visibility before collecting run
text. Styles are loaded for visibility even when all runs explicitly select East
Asian fonts; the existing font-default fallback conditions remain in place.
The new package regression checks inherited hiding, character-style toggles,
and direct visibility overrides with and without explicit East Asian fonts.
It failed before the change by retaining the hidden text. Reader regressions
exercise 48 combinations of defaults, paragraph inheritance, character styles,
and direct formatting, plus run-property scope boundaries.

The reader's 620 library tests and two new integration tests passed
(`/tmp/jade-hidden-style-final.log`). Layout passed 716 tests with 12 existing
ignored tests (`/tmp/jade-hidden-layout-final.log`), and the shared model passed
the Web `tiny-skia` WASM check (`/tmp/jade-hidden-style-wasm.log`). These checks
cover effective run visibility, rather than establishing complete Word parity
or hidden paragraph-mark pagination equivalence.

Native-shaping layout also passed 734 tests with 13 existing ignored tests
(`/tmp/jade-hidden-layout-native.log`). Both print integration suites passed,
six tests total (`/tmp/jade-hidden-style-model.log`).

### Toggle formatting follows the style cascade (2026-11-07)

The style resolver now applies parity to all twelve supported OOXML toggle
properties: bold/italic (including complex-script variants), caps, small caps,
single strike, outline, shadow, emboss, imprint, and hidden text. A true style
declaration reverses the inherited state; a false style declaration leaves it
unchanged. Direct formatting remains an absolute on/off override. Underline and
double strike retain their non-toggle behavior. The styles parser also now
reads the eight previously omitted toggle tags rather than silently dropping
complex-script emphasis, case changes, and text effects in styles/defaults.
See Microsoft's references for [Bold](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.bold?view=openxml-3.0.1)
and [Shadow](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shadow?view=openxml-3.0.1).

Headless line layout uses the complete cascade for toggle state even when
explicit East Asian fonts suppress font-default fallback. The ribbon readout
now calls the same effective resolver as canvas rendering, replacing its own
absolute-override cascade.

Twelve regression tests cover 576 combinations of defaults, based-on paragraph
styles, character styles and direct formatting. Before the fix, eleven tests
failed; the hidden-text test already passed (`/tmp/jade-style-toggle-before.log`).
All twelve now pass, together with 620 reader library tests and both previous
hidden-text regressions (`/tmp/jade-style-toggle-reader.log`). A layout regression
also reproduced bold remaining enabled after two style toggles; it now passes
with and without explicitly authored East Asian fonts. All 717 layout tests
passed, with 12 existing ignored tests (`/tmp/jade-style-toggle-layout.log`).
The shared model's Web `tiny-skia` WASM check passed after the ribbon change
(`/tmp/jade-style-toggle-wasm-final.log`). These checks cover formatting cascade
semantics; full Word rendering and feature equivalence remain unverified.

Editor tests matching `style` passed: 169 tests, three existing ignored tests
(`/tmp/jade-style-toggle-editor.log`). Native-shaping layout passed 735 tests,
with 13 existing ignored tests (`/tmp/jade-style-toggle-native.log`).

### Non-toggle visual formatting survives style inheritance (2026-11-07)

Styles and document defaults now retain double strike, text highlighting, and
superscript/subscript baseline settings. These values use absolute overrides:
repeated double-strike declarations stay enabled, and explicit false, `none`,
or `baseline` declarations reset inherited formatting. Microsoft's
[DoubleStrike reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.doublestrike?view=openxml-3.0.1)
identifies double strike as a non-toggle property.

Headless layout also carries these three resolved properties when explicitly
specified East Asian fonts suppress font fallback. This preserves style-driven
visual formatting in the input used by print/PDF layout.

The new reader regression failed before the parser fix
(`/tmp/jade-visual-before.log`). Both new tests now pass, covering document
defaults, based-on styles, character-style resets, direct resets, paired and
self-closing tags, and exclusion of properties outside run-property scopes.
The reader's 620 library tests and 14 previous cascade regressions also pass
(`/tmp/jade-visual-reader.log`). All 717 layout tests pass, with 12 existing
ignored tests, including explicit East Asian font coverage for these properties
(`/tmp/jade-visual-layout.log`). The shared model's `tiny-skia` WASM check passes
(`/tmp/jade-visual-wasm.log`). These checks verify the parsed cascade and layout
input; visual comparison with Word remains outstanding.

Native-shaping layout also passes all 735 tests, with 13 existing ignored tests
(`/tmp/jade-visual-native.log`). The final reader regressions additionally cover
both superscript and subscript (`/tmp/jade-visual-reader-final.log`).

### Raised/lowered text inherits styles and supports explicit reset (2026-11-07)

The styles parser now reads `w:position` from run defaults and paragraph and
character styles, using the same half-point conversion as direct formatting.
Zero is retained as an explicit baseline-shift reset instead of being discarded
as an absent property. The font-dialog command writes this reset as
`<w:position w:val="0"/>`, allowing inherited positioning to be canceled.
Headless layout carries the resolved shift even with explicitly authored East
Asian fonts. The property changes baseline position without changing font size;
omission inherits the previous style level, as documented in Microsoft's
[Position reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.position?view=openxml-3.0.1).

Both new reader regressions failed before the fix
(`/tmp/jade-position-before.log`). They now pass for positive, negative, zero,
and absent values, character-style resets, based-on paragraph styles, paired
and self-closing tags, and invalid/out-of-scope declarations. All 620 reader
library tests and 18 cascade regression tests pass
(`/tmp/jade-position-reader.log`). The shared model's `tiny-skia` WASM check
passes (`/tmp/jade-position-wasm.log`). Full Word feature and visual equivalence
remain unverified.

All three font-dialog integration tests pass, including export/reload of the
zero reset and undo restoring inherited positioning
(`/tmp/jade-position-editor.log`). Native-shaping layout passes 735 tests, with
13 existing ignored tests; explicit East Asian font coverage checks both the
inherited negative shift and a direct zero reset
(`/tmp/jade-position-layout.log`).

### Character spacing preserves an explicit standard reset (2026-11-07)

The font-dialog command now retains zero character spacing in both the model
and exported `w:spacing`, so selecting standard spacing cancels an inherited
expanded or condensed value. Undo restores the inherited value. The reader
handles both paired and self-closing spacing elements, rejects non-finite
values, and reads paragraph-mark run spacing without confusing it with
paragraph spacing. Style-derived spacing also reaches headless layout when a
run has an explicitly authored East Asian font. Microsoft documents character
spacing in twentieths of a point, with omitted values inherited from the style
hierarchy in its
[Spacing reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacing?view=openxml-3.0.1).

The first two new reader regressions failed before the fix
(`/tmp/jade-spacing-before.log`). All three now pass, including paragraph-mark
scope, alongside 620 reader library tests and 18 previous cascade regressions
(`/tmp/jade-spacing-reader.log`). All four font-dialog integration tests pass,
covering exported zero values, reload, and undo
(`/tmp/jade-spacing-editor.log`). The shared model's `tiny-skia` WASM check
passes (`/tmp/jade-spacing-wasm.log`). Full Word feature and visual equivalence
remain unverified.

Native-shaping layout passes all 735 tests, with 13 existing ignored tests
(`/tmp/jade-spacing-layout.log`). Its style-cascade regression verifies both
inherited expanded spacing and a direct zero reset, with and without explicit
East Asian fonts.

### Style text color and underline survive headless layout (2026-11-07)

Headless layout now takes text color (including its theme reference), underline
on/off, underline kind, and underline color from the complete effective run
cascade. The previous font-fallback merge lost underline kind/color even in
ordinary paragraphs; suppressing that fallback for explicit East Asian faces
also lost inherited text color and underline. The new overlay retains the
existing font selection while carrying these visual properties into print/PDF
layout input. Direct RGB color clears inherited theme color, and direct
underline-off clears the inherited underline declaration. Microsoft describes
underline inheritance and the separate kind/color attributes in its
[Underline reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.underline?view=openxml-3.0.1).

The extended regression failed before the fix because the inherited wave
underline kind was absent (`/tmp/jade-color-before.log`). It now verifies
paragraph-style color and wave underline, character-style theme color and
double underline, direct RGB and underline-off overrides, and preservation of
an explicit East Asian font. Native-shaping layout passes 735 tests, with 13
existing ignored tests (`/tmp/jade-color-layout.log`). The shared model's
`tiny-skia` WASM check also passes (`/tmp/jade-color-wasm.log`). These checks
verify the resolved layout properties; rendered comparison with Word and full
feature equivalence remain outstanding.

### Run shading inherits styles and preserves an explicit removal (2026-11-07)

The styles reader now parses character shading from document run defaults,
paragraph styles, and character styles. Shading stays separate from paragraph
shading and paragraph-mark run shading. The document reader also handles
paired `w:shd` elements, matching its self-closing-element handling. Headless
layout carries resolved shading even when explicit East Asian faces suppress
font-default fallback. Microsoft's
[Shading reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shading?view=openxml-3.0.1)
describes omitted run shading as inheriting the previous style level.

The font-dialog command therefore writes `w:shd w:val="nil"` for “no shading”
instead of deleting the declaration and revealing inherited shading. The
formatting query reports this explicit reset as `none`; undo restores the
previous inherited background. Both new reader regressions failed before the
fix (`/tmp/jade-shading-before.log`). They now pass for defaults, based-on
styles, character-style removal, direct removal and replacement, paired and
self-closing elements, and paragraph/mark scope separation. All 620 reader
library tests and 23 cascade regression tests pass
(`/tmp/jade-shading-reader.log`).

All five font-dialog integration tests pass, including export/reload of the
explicit shading removal and undo restoring the inherited fill
(`/tmp/jade-shading-editor.log`). Native-shaping layout passes 735 tests, with
13 existing ignored tests; its style-visual regression checks inherited fill
and explicit removal both with and without East Asian font declarations
(`/tmp/jade-shading-layout.log`). The final shared-model `tiny-skia` WASM check
passes (`/tmp/jade-shading-wasm-final.log`). Full Word visual and feature parity
remain unverified.

### Character borders inherit styles and preserve removal (2026-11-07)

Run borders now inherit document defaults, paragraph styles, based-on styles,
and character styles. The style reader keeps border kind, thickness, spacing,
and color within run-property scope. Headless layout carries this resolved
border even with explicit East Asian fonts. Microsoft's
[Border reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.border?view=openxml-3.0.1)
describes omitted run borders as inheriting the previous style level.

Turning the border off now writes `w:bdr w:val="nil"`, so inherited borders
remain off after export/reload; undo restores the inherited border. The new
inheritance regression failed before the fix (`/tmp/jade-border-before.log`).
All 620 reader library tests, 25 cascade regression tests, and six font-dialog
integration tests pass (`/tmp/jade-border-reader.log`,
`/tmp/jade-border-editor.log`). Native-shaping layout passes 735 tests, with
13 existing ignored tests (`/tmp/jade-border-layout.log`); the regression checks
both inherited and removed borders with and without explicit East Asian fonts.
The shared-model `tiny-skia` WASM check passes (`/tmp/jade-border-wasm.log`).
Full Word visual and feature parity remain unverified.

### Style kerning and text scale survive explicit East Asian fonts (2026-11-07)

Headless layout now carries resolved kerning thresholds and horizontal text
scale through the complete style cascade even when a run declares its East
Asian font explicitly. Direct threshold zero and scale 100 remain explicit
overrides. Microsoft documents style inheritance for both
[CharacterScale](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.characterscale?view=openxml-3.0.1)
and [Kern](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.kern?view=openxml-3.0.1).

The extended regression failed before the fix because the inherited 12-point
kerning threshold was absent (`/tmp/jade-scale-before.log`). It now checks
that threshold, 80% text scale, direct overrides, and preservation of the
explicit font, with and without font-default suppression. All 735 native-shaping
layout tests pass, with 13 existing ignored tests
(`/tmp/jade-scale-layout.log`), and the shared-model `tiny-skia` WASM check
passes (`/tmp/jade-scale-wasm.log`). These checks verify the resolved layout
properties; rendered comparison with Word remains outstanding.

### Emphasis marks inherit styles and preserve explicit removal (2026-11-07)

Emphasis marks now inherit document run defaults, paragraph styles, based-on
styles, and character styles. Both paired and self-closing declarations share
token normalization with direct formatting. The explicit `none` token remains
in the model so it cancels inherited marks, while an omitted declaration still
inherits. Microsoft's
[Emphasis reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.emphasis?view=openxml-3.0.1)
describes style inheritance when the declaration is absent.

The font-dialog command now writes `w:em w:val="none"` when marks are turned
off, and formatting queries retain that explicit choice after export/reload.
Undo restores the inherited mark. Headless layout also carries resolved marks
when a run explicitly declares an East Asian font. The existing renderer
recognizes `none` as drawing no mark.

The new inheritance regression failed before the fix
(`/tmp/jade-emphasis-before.log`). All 620 reader library tests and 27 cascade
regressions pass (`/tmp/jade-emphasis-reader.log`), including direct and
character-style removal, replacement, based-on inheritance, paired elements,
and scope isolation. All seven font-dialog integration tests pass
(`/tmp/jade-emphasis-editor.log`). Native-shaping layout passes 735 tests,
with 13 existing ignored tests (`/tmp/jade-emphasis-layout.log`); its visual
property regression checks inherited and removed marks with and without an
explicit East Asian font. The shared-model `tiny-skia` WASM check passes
(`/tmp/jade-emphasis-wasm.log`). Rendered comparison with Word and full
feature parity remain outstanding.

### Style typography reaches headless shaping (2026-11-07)

Word 2010 typography stored in styles was discarded during both style parsing
and headless run extraction. Styles now read `w14:ligatures`, `w14:numForm`,
`w14:numSpacing`, and `w14:cntxtAlts` from run properties, including document
defaults. The resolved typography reaches layout with and without explicit
East Asian run fonts. Each setting inherits independently; a character style
or direct setting can disable ligatures or contextual alternates without
removing inherited numeric formatting. The existing OpenType feature resolver
then receives those settings. Microsoft's
[ligatures reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.office2010.word.ligatures?view=openxml-3.0.1)
describes the corresponding Word 2010 rendering property.

The new layout regression failed before the fix
(`/tmp/jade-typography-before.log`). It now checks empty and paired XML,
document/paragraph/character/direct precedence, explicit resets, East Asian
font suppression, and feature-bearing shaping descriptors. All 620 reader
library tests and two new typography cascade tests pass
(`/tmp/jade-typography-reader.log`). Native-shaping layout passes 736 tests,
with 13 existing ignored tests (`/tmp/jade-typography-native-layout.log`);
default layout passes 718 tests, with 12 ignored
(`/tmp/jade-typography-layout.log`). The shared-model `tiny-skia` WASM check
passes (`/tmp/jade-typography-wasm.log`). Pixel comparison against Word,
additional style text effects, and full feature parity remain outstanding.

### Multiple OpenType stylistic sets survive editing (2026-11-07)

Run formatting now retains every enabled `w14:styleSet`, including sets inherited
from document defaults and paragraph or character styles. Disabled entries and
IDs outside 1–20 are ignored. An explicitly empty `w14:stylisticSets` cancels
inherited sets. Microsoft's [example](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/4bf1dfaa-74a2-4c3b-a1bb-e3f719c94bf8)
enables sets 1 and 4 together; its [CT_StyleSet definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/fdd6f560-2d05-492e-bde2-87fae8428dae)
defines the valid IDs and disabled-entry behavior.

The headless formatting API accepts a list while retaining the legacy single-set
field. The list takes precedence, is normalized on edit, and produces all matching
OpenType shaping features. Both font dialogs offer independent set checkboxes.
Clearing every checkbox writes an explicit empty declaration, and undo restores
the previous selection. Glyph changes still depend on the font's supported sets.

The reader regression failed before the fix (`/tmp/jade-sets-before.log`). All
620 reader library tests, two set regressions, and two typography cascade tests
pass (`/tmp/jade-sets-reader-final.log`). Native-shaping layout passes 736 tests,
with 13 existing ignored tests, and all three OpenType integration tests pass
(`/tmp/jade-sets-layout.log`). Eight run-format integration tests verify editing,
export/reload, reset, and undo (`/tmp/jade-sets-editor.log`); five shared font-model
tests pass (`/tmp/jade-sets-model.log`). The shared-model `tiny-skia` WASM check
passes (`/tmp/jade-sets-wasm-final.log`). The complete Web GPUI library check also
passes with Rust 1.99 (`/tmp/jade-sets-gpui-web-199.log`); Rust 1.93 stops in the
existing `gpui-pre` dependency's use of `std::hint::cold_path`. Native GPUI build,
font-dialog visual inspection, and rendered comparison against Word remain
outstanding.

The header/footer regression also checks list-only patches and explicit empty
resets across headers and footers in default, first-page, and even-page variants
(`/tmp/jade-sets-hf-final.log`). Its old reset assertions were updated to require
explicit `none`, `nil`, and zero values for emphasis, shading, borders, spacing,
and position, preserving the inheritance behavior covered above.

### Style glow/outline and opacity (2026-11-07)

The reader now parses `w14:glow` and `w14:textOutline` in document defaults,
paragraph/character styles, and direct formatting through the same parser.
Each declared effect replaces the inherited effect as a unit: omitted radius
or width uses zero, and omitted glow alpha uses full opacity. Empty declarations
and `noFill` can cancel inherited effects. Layout carries these resolved effects
even when East Asian font handling bypasses style font selection.

The old reader inverted `w14:alpha`; an opacity of 40000 became 0.6 instead
of 0.4. Reading and editing now use opacity consistently. Clearing an effect
writes a zero-radius glow or a zero-width `noFill` outline so saving/reloading
cannot expose the inherited effect. The renderer suppresses zero radius,
zero opacity, and zero width instead of forcing a visible minimum.

Primary references: [CT_Glow](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/c20861c4-7aeb-4a1f-b246-e405a423e398),
[CT_SRgbColor](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/10a451b3-6054-4e0a-b1b2-b68fe9316533),
and [CT_TextOutlineEffect](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/5b065f17-63c7-4d54-83e3-6f3c161f3afa).
The three reader regressions failed before the fix (`/tmp/jade-effect-before.log`)
and now pass alongside all 620 reader library tests (`/tmp/jade-effect-reader.log`).

Theme effect colors, color transforms beyond glow alpha, gradient outlines,
outline alpha/dashes, and rendered comparison
with Word remain outstanding. This change does not establish visual parity.

Verification: all nine run-format integration tests pass
(`/tmp/jade-effects-editor.log`), including effect reset/reload/undo and preserving
40% opacity in a radius-only direct-format edit. The layout cascade regression
passes (`/tmp/jade-effects-layout.log`), as do the disabled-effect renderer test,
two glow paint-recording tests, and the header/footer font patch regression
(`/tmp/jade-effects-renderer.log`, `/tmp/jade-effects-glow.log`,
`/tmp/jade-effects-hf.log`). The complete Web GPUI library WASM check passes with
Rust 1.99 (`/tmp/jade-effects-web.log`). These are model/paint-operation checks;
Word-rendered image comparison remains unverified.


### Partial edits preserve inherited glow and outline (2026-11-07)

Changing only glow radius or outline width previously consulted direct run
properties, so a style-supplied color could be lost and the effect disabled.
Changing only color could also replace an inherited radius, width, or opacity
with editor defaults. The format command now resolves each selected run's
paragraph/character style cascade and materializes only the effect group being
edited before applying the patch. Unedited effects retain inheritance, and
unselected text retains its original direct properties. Header/footer XML
patches use the same normalization with their paragraph style and document
styles, including first/even variants.

Ten run-format integration tests pass (`/tmp/jade-inherited-effects-editor.log`).
The new regression covers radius-only, width-only, and color-only edits,
export/reload, selection boundaries, and undo restoring the original direct
properties. Word-rendered image comparison remains outstanding.

All 42 header/footer regressions pass (`/tmp/jade-inherited-effects-hf-final.log`).
The new package round-trip test selects across paragraph-style and character-style
runs in all six header/footer role/variant combinations, changes radius and width,
and verifies each run retains its own color/opacity while unselected runs retain
inheritance. The complete Web GPUI library WASM check also passes with Rust 1.99
(`/tmp/jade-inherited-effects-web.log`).

### Font dialog reads effective styles (2026-11-07)

The body font-state query now resolves document defaults, paragraph styles,
character styles, and direct overrides through the same style resolver used for
formatting. Both shell character models delegate to this shared query. Header
and footer queries also resolve their paragraph and character styles, including
empty paragraphs. Reading state leaves the original direct properties intact.
Explicit false and zero values override inherited settings. Empty body formatting
runs retain their own properties. The built-in complex-script size is now read
from its 11 pt default rather than falling back to the ordinary 10.5 pt size.

Regression coverage includes UTF-8 run boundaries, paragraph-end lookup,
paragraph/character glow inheritance, direct off/zero overrides, empty paragraphs,
and an unchanged font dialog producing an empty patch. Header/footer coverage
uses all six role/variant combinations. Word-rendered comparison and mixed
selection dialog states remain outstanding.

Validation: all five format-state tests and ten run-format integration tests pass
(`/tmp/jade-format-state-final.log`), as do all 42 header/footer regressions
(`/tmp/jade-format-state-hf-final.log`). The complete Web GPUI library WASM check
passes with Rust 1.99 (`/tmp/jade-format-state-web-final.log`). The existing direct
noProof on/off XML is retained when resolving modeled styles.


### Mixed body selections in the font dialog (2026-11-07)

Both character models now open the font dialog on the common effective values
of all runs intersecting the body selection. Different values remain unset in
the dialog snapshot, separately tracked from implicit defaults. A half-open
selection excludes the next run at its end and the next paragraph at offset
zero; reversed ranges and selected empty paragraphs are handled as well.

Native and Web GPUI font controls show mixed values as empty choices or an
indeterminate check box. Opening and confirming without edits produces an empty
patch. Choosing an explicit off value on a mixed field does produce a patch,
while other mixed properties stay untouched. The glow and outline color pickers
leave radius/width out of their patch so each run keeps its existing setting.

Coverage includes mixed bold/size, implicit versus explicit defaults, UTF-8 run
boundaries, reversed/cross-paragraph ranges, package save/reopen, and undo.
Individual stylistic-set merging, paragraph-mark formatting, and comparison against Word's
rendered dialog remain outstanding.

Validation: the seven `word::font::tests` passed with Rust 1.93.1
(`/tmp/jade-mixed-font-model.log`), and the Web GPUI wasm32 library check
passed with Rust 1.99.0 (`/tmp/jade-mixed-font-web.log`).
The native GPUI `cargo check --tests` also passed with Rust 1.99.0
(`/tmp/jade-mixed-font-native.log`).
All seven `format_state` integration tests passed with Rust 1.93.1,
including range boundaries and save/reopen/undo
(`/tmp/jade-mixed-font-range.log`). Root and core `git diff --check` passed.

### Common selection formatting in canvas stories (2026-11-07)

`runFormatAtSelection` now aggregates the body selection instead of reporting
only its first run. Header/footer selections use the same paragraph byte range
as the editing command, including tab-separated segment prefixes. The new story
query resolves each selected run's paragraph and character styles; mixed fields
are unset in the returned dialog state. A caret retains the previous run query.

Field or other unsupported inline paragraphs still use the editor's existing
whole-paragraph formatting fallback; their state query aggregates the same run
scope. This does not add partial-field editing or selection across header/footer
segments, lines, or pages. Those remain separate gaps.

The regression test exercises all six header/footer role/variant combinations,
mixed bold and size, inherited values, UTF-8 boundaries, reversed and collapsed
ranges, empty formatting runs, field fallback, and save/reopen after changing
only bold.

Validation: all 44 editor `header_footer` tests passed against the final
source with Rust 1.93.1 (`/tmp/jade-mixed-story-font-tests-final.log`), including
the six-variant mixed-selection/save test. The Web GPUI wasm32 library check
passed with Rust 1.99.0 (`/tmp/jade-mixed-story-font-web-fixed.log`). That check
also caught a missing context argument in the slide rotation UI's focus call;
the one-line call was corrected to match the surrounding GPUI API usage.
Root and core `git diff --check` passed. Word UI comparison and cross-segment
header/footer selection are not proved by these checks.


### Paragraph marks in body font selections (2026-11-07)

Body `run_formats_in_range` includes each paragraph mark crossed by the
selection. Ending at the next paragraph's offset zero includes the preceding
mark and excludes the following paragraph. A range ending at the same
paragraph's text length includes text only. Empty-paragraph caret queries use
authored mark properties when present, with the existing empty-run fallback
when they are absent. Nonempty-paragraph end carets retain the last text run.

`SetRunFormatCommand` applies the same patch to selected marks, including
mark-only ranges and intervening empty paragraphs. Mark properties are written
to `w:pPr/w:rPr` and restored on Undo. Other paragraph properties and existing
mark insertion/deletion/move metadata remain intact. Existing text does not
inherit the mark's direct formatting. This follows the separate mark property
location described by [Microsoft's Open XML reference](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.paragraphmarkrunproperties?view=openxml-3.0.1).

Regression coverage includes reversed UTF-8 ranges, mixed text/mark values,
mark-only edits, an intervening empty paragraph, preservation of unedited
sizes and italics, retained mark insertion metadata, text-only exclusion,
save/reopen, and Undo removing newly created mark properties. The load pass
also retains mark-level raw run properties, so spacing, proofing exclusions,
and language declarations survive edits and Undo.

Validation: `format_state` (9 tests) and `run_format` (10 tests) pass with
Rust 1.93.1. The `jade-gpui-web` library check for `wasm32-unknown-unknown`
passes with Rust 1.99.0. These checks do not exercise the Word desktop UI.

This does not establish Word UI equivalence, paragraph-mark glyph painting,
formatting changes tracked on marks, insertion/Enter inheritance, or mark
editing through the other individual character-format commands. Header/footer
mark selection remains a separate gap.

### Individual bold, italic and size commands include marks (2026-11-07)

`SetBoldCommand`, `SetFontSizeCommand`, `ApplyBoldCommand` and
`ApplyItalicCommand` now use the shared `SetRunFormatCommand` implementation.
Selections crossing paragraph boundaries format the selected paragraph marks,
including intervening empty paragraphs. Mark-only selections no longer silently
leave the mark unchanged. Save/reopen, Undo and Redo use the same mark-property
preservation as the font dialog.

The commands retain their previous field scope: `SetBoldCommand` changes the
ordinary bold property; the bold/italic actions change both ordinary and complex
script properties. Explicit font size changes update ordinary and complex script
sizes through the existing shared patch, validate positive finite input, and snap to Word's half-point sizes and
1–1638pt bounds. Text-only selections continue to exclude the paragraph mark.

Regression tests cover reversed multi-paragraph ranges, an empty paragraph,
mark-only ranges, independent ordinary/complex script values, half-point rounding,
unchanged text and the excluded ending paragraph, retained spacing and pagination,
save/reopen, Undo and Redo. This does not establish Word UI equivalence or resolve
mark handling in color, underline, character-style and other commands.

Validation: the final Web/WASM `jade-gpui-web` library check passed, as did
format checks on the four commands and range tests. Native integration and
command-unit tests are still pending in the shared compiler queue; this entry
does not claim their success.

### Grow/shrink includes selected paragraph marks (2026-11-07)

`AdjustFontSizeCommand` now steps selected paragraph marks independently of the
text runs. It resolves each mark's size from its direct properties, paragraph or
character style, and document defaults before using the existing ribbon ladder
or one-point step. Intervening empty paragraphs and mark-only selections are
included; the final paragraph's unselected mark is excluded. The size patch updates
ordinary and complex-script sizes together, matching text runs. Unrelated
mark/paragraph properties are retained. Undo restores the original
paragraph properties, including removing properties created solely by the command.

Two integration regressions exercise reversed ranges, different text/mark sizes,
style inheritance on empty paragraphs, both ladder-grow and point-shrink,
mark-only ranges, save/reopen, and Undo/Redo. The fresh Web/WASM library check
passed in 7m 04s before the follow-up aligning mark size patches with text runs.
The first native run exposed an incorrect expectation that explicit size patches
retain szCs; the shared patch intentionally updates both sizes. That expectation
and grow/shrink mark handling were corrected. The follow-up Web/WASM library
check passed in 5m 57s; native runtime validation remains pending.
Word UI/layout equivalence, header/footer
marks, tracked formatting on marks, and other individual formatting commands
remain unproved or incomplete.

### Explicit font color includes selected paragraph marks (2026-11-07)

`SetFontColorCommand` now delegates to the shared run-format patch. Selected
paragraph marks, including intervening empty paragraphs and mark-only ranges,
receive the explicit color. Text-only ranges leave the mark unchanged, and the
ending paragraph's unselected mark remains excluded. An explicit color removes
previous theme color/tint/shade attributes; Undo restores those attributes and
removes paragraph properties created by the edit. The command also accepts the
model's `#auto` spelling and normalizes it to the lowercase OOXML `auto` value.

Three new integration regressions cover reversed multi-paragraph selections,
mark-only versus text-only selections, theme replacement, retained italic,
spacing and keep-next properties, automatic color serialization, save/reopen,
and Undo/Redo. Formatting and diff checks pass. Native integration tests and a
fresh Web/WASM check are running; their success is not yet established. Highlight,
underline, character styles, tracked mark formatting, header/footer marks and
Word UI/layout equivalence remain separate gaps.

### Underline actions include selected paragraph marks (2026-11-07)

`ApplyUnderlineCommand` now uses the shared run-format patch, matching the font
dialog's handling of selected paragraph marks. The default single underline,
word-only and other explicit kinds, and explicit off apply to marks as well as
text, including intervening empty paragraphs and mark-only ranges. Explicit off
clears the underline kind and color and writes `w:u w:val="none"`; changing the
kind retains an existing underline color. Undo restores complete prior mark
properties and removes newly created paragraph properties.

Two integration regressions cover single, words, double and off across reversed
multi-paragraph ranges, retained mark italic/spacing and paragraph keep-next,
the excluded ending paragraph, mark-only edits, save/reopen and Undo/Redo.
Existing command tests still cover collapsed selections and text ranges.
Formatting and diff checks pass. The queued native range run must include all
19 `format_state` tests, and the queued Web/WASM check must compile this updated
command, before claiming those validations passed. Neither has completed yet.
Highlight, character styles, tracked mark formatting, header/footer marks, and
Word UI/layout equivalence remain unproved or incomplete.


### Highlight includes selected paragraph marks (2026-11-07)

`SetHighlightColorCommand` now patches selected paragraph mark properties in
addition to text. It includes intervening empty paragraphs and mark-only ranges,
leaves text unchanged for mark-only selections, and excludes the unselected
ending mark. Text-only ranges retain their previous scope. The command uses the
same highlight property for text and marks, including the explicit `none` value.
Undo restores the complete original paragraph properties and removes properties
created by the edit, while retaining unrelated mark formatting and pagination.

Two integration regressions cover reversed multi-paragraph selections, mark-only
versus text-only ranges, apply and explicit off, save/reopen, and Undo/Redo. The
range suite now has 21 tests. Formatting and diff checks pass; native runtime
verification and a Web/WASM check of the latest source remain pending. This does
not prove Word highlight rendering, header/footer mark support, tracked mark
formatting, or Word UI/layout equivalence.


### 文字スタイル適用時の段落記号（2026-11-07）

`SetCharacterStyleCommand` が段落をまたぐ選択の段落記号を対象に含めるよう修正した。段落記号だけの選択と空段落でも `w:pPr/w:rPr/w:rStyle` を設定・解除し、終点段落の記号は変更しない。表示名から ID への解決と未定義スタイルの拒否は既存の検証を使い、元の段落プロパティを保存して Undo で復元する。

定義済み文字スタイルを含む DOCX を使い、逆方向の複数段落選択、段落記号だけの選択、保存・再読込後のスタイル継承、既存の斜体と段落設定の保持、解除、Undo／Redo を確認する回帰テストを追加した。変更ファイルの rustfmt とルート／jade-core の差分チェックは成功。追加テストのネイティブ実行と Web/WASM チェックは進行中であり、成功は未確認。

先行する太字・斜体・文字サイズの単体テストは 29 件成功。段落記号の色・下線・ハイライト等を含む統合テストは引き続き実行中。直前の Web/WASM チェックはプレゼン書き出し側の参照型不一致で失敗したため、同箇所の現行修正版で再実行している。Word との全機能同等性は未達・未検証。


### 書式クリアの段落記号とテーマ色（2026-11-07）

`ClearFormattingCommand` が段落をまたぐ選択の段落記号と空段落も処理するよう修正した。元の段落プロパティを保存して Undo で復元する。既存の文字用クリア処理を共有し、ハイライト・言語・段落設定を保持しながら直接の文字書式を解除する。文字色を解除してもテーマ色だけ残っていた点も修正し、文字と段落記号の両方で `theme_color` を消去する。

逆方向の複数段落選択と段落記号から始まる選択について、保存・再読込、終点段落の除外、元々プロパティのない空段落の保持、ハイライト・言語・keepNext の保持、Undo／Redo を確認する統合テストを追加した。format_state は計 22 テスト。rustfmt とルート／jade-core の差分チェックは成功。ネイティブ統合テストと本修正後の Web/WASM チェックは進行中。

直前の文字スタイル修正までの Web/WASM チェックは成功（1 分 09 秒、`/tmp/jade-character-style-mark-web.log`）。文字スタイル等の単体テストは引き続き待機中で、成功は未確認。Word の全機能同等性は未達・未検証。


### 取り消し線・上付き・下付きの段落記号（2026-11-07）

`ApplyStrikeCommand`、`ApplySuperscriptCommand`、`ApplySubscriptCommand` が選択中の段落記号と空段落も更新するよう修正した。取り消し線は通常線・二重線・解除の既存の明示指定を段落記号にも適用する。上付き・下付きの解除は文字と同じ `w:vertAlign w:val="baseline"` を書き、スタイルから継承する上付き／下付きに戻ることを防ぐ。選択終点の段落記号は除外し、Undo は元の段落プロパティ全体を復元する。

統合テストを 2 件追加し、取り消し線 3 操作 × 選択範囲 3 種、上付き／下付き × 有効／解除 × 選択範囲 3 種を確認する。逆方向選択、空段落、既存の斜体・keepNext の保持、保存・再読込、Undo／Redo を含む。format_state は計 24 テスト。rustfmt とルート／jade-core の差分チェックは成功。ネイティブ統合テストは共有ビルドの待機中であり、出力が 24 テストを含むことを確認する必要がある。

直前の書式クリア修正後の Web/WASM チェックは成功（2 分 06 秒、`/tmp/jade-clear-mark-web.log`）。今回の 3 コマンド修正後のチェックは `/tmp/jade-strike-baseline-mark-web.log` で進行中。Word との全機能同等性は未達・未検証。


### 書式解除時の継承と段落記号サイズの復元（2026-11-07）

共通 `SetRunFormatCommand` で標準位置を指定した際に `w:vertAlign` を削除していたため、スタイルから上付き・下付きが再び有効になる問題を修正した。`baseline` を明示して保存する。同様に取り消し線・二重取り消し線・大文字・小型大文字・隠し文字・輪郭・影・浮き出し・彫り込み・コンテキスト代替の解除を明示的な false とし、排他的な書式を有効にする際も反対側の継承を明示的に解除する。文字・選択中の段落記号・空段落の継承解除、終点の除外、保存・再読込、Undo／Redo の回帰テストを追加した。継承元は DOCX パッケージの styles.xml に含め、保存・再読込後もスタイルが存在する状態で確認する。format_state は計 27 テスト。

先行する format_state の実行は 24 件中 21 件成功・3 件失敗（`/tmp/jade-individual-mark-format-tests-final.log`）。取り消し線・上付き・下付きの追加ケースは成功した。失敗から、段落記号の Undo で通常文字のサイズを複雑な用字系のサイズにも出力する保存処理の問題を特定し、`font_size_complex` 自体を保存するよう修正した。複雑な用字系のサイズが未指定の場合と通常文字と異なる場合の両方で元の値を保持する。別の失敗は、既定の 10.5 pt が有効な位置で未指定を期待していたテストを修正した。修正後のネイティブ統合テストは進行中で、成功は未確認。

文字スタイル・サイズ調整・文字色・下線・ハイライトの単体テストは 23 件成功（`/tmp/jade-character-style-mark-unit.log`）。取り消し線等の段落記号対応後の Web/WASM チェックは成功（1 分 13 秒）、標準位置の継承修正後も成功（1 分 40 秒）、複雑な用字系サイズの保存修正後も成功（1 分 07 秒）。boolean 書式解除修正後のチェックは成功（1 分 38 秒、`/tmp/jade-explicit-format-off-web.log`）。コンテキスト代替の解除追加後のチェックも成功（2 分 00 秒、`/tmp/jade-contextual-format-off-web.log`）。最新のネイティブ統合実行は `/tmp/jade-explicit-format-off-native.log` で待機中。変更対象の rustfmt と両リポジトリの差分チェックは成功。Word の実描画との比較および全機能同等性は未達・未検証。


### フォント変更時の段落記号（2026-11-07）

`SetFontFamilyCommand` が選択中の段落記号と空段落にもフォントを適用するよう修正した。欧文フォントの指定は欧文スロットと対応するテーマだけを置換し、日本語・複雑な用字系の設定を保持する。日本語フォント指定は従来の文字用処理と同じ各スロットを更新する。終点段落の記号は除外し、Undo で元の段落プロパティを復元する。

欧文・日本語それぞれについて、逆方向の複数段落選択・段落記号だけの選択・文字だけの選択、既存の書式と段落設定の保持、保存・再読込、Undo／Redo を確認する回帰テストを追加した。format_state は計 28 テスト。先行する継承テストの DOCX フィクスチャで ZIP API の戻り値を誤って扱っていたコンパイルエラーも修正した。最新の Web/WASM チェックは成功（`/tmp/jade-font-family-mark-web.log`）。ネイティブ統合実行は進行中で、28 件の成功はまだ確認していない。変更対象の rustfmt と両リポジトリの差分チェックは成功。Word との全機能同等性は未達・未検証。


### 段落の日本語体裁でスタイル継承を明示的に上書き（2026-11-07）

`SetParagraphTypographyCommand` は禁則処理・単語途中の改行・句読点のぶら下げ・行頭句読点・日本語と英字／数字の間隔自動調整について、Word の既定値に等しい指定を XML から削除していた。スタイル側に反対の値があると保存・再読込後に設定が戻るため、指定された値を `w:val="1"` または `w:val="0"` として保存するよう修正した。未指定項目は従来どおり変更しない。

基底スタイルを継承する子スタイル付き DOCX を生成し、6 項目を両方向に変更する回帰テストを追加した。複数段落と空段落、選択外の保持、既存の斜体・keepNext の保持、保存・再読込、Undo／Redo を確認する。既存テストも明示保存の期待値に更新した。小ファイルの rustfmt と両リポジトリの差分チェックは成功。Web/WASM チェックも成功（1 分 47 秒、`/tmp/jade-typography-inheritance-web.log`）。ネイティブの typography 3 テストもすべて成功（2 分 22 秒、`/tmp/jade-typography-inheritance-native.log`）。

先行する format_state は 28 件中 27 件成功（`/tmp/jade-explicit-format-off-native.log`）。継承解除と段落記号サイズの復元を含む 27 件は成功した。フォント追加テストの 1 件は、読み込み時にテーマ参照で無視される具体名を期待していたため失敗し、テーマ指定あり・なしの両ケースを検証する期待値に修正して再実行している（`/tmp/jade-font-family-mark-native.log`）。Word の実描画との比較と全機能同等性は未達・未検証。


### フォント変更の Undo で元の属性を保持（2026-11-07）

テーマ指定のフォントテストを修正して再実行したところ、28 件中 27 件成功し、残る失敗から Undo がテーマ参照と併記された元の具体名を失う問題を確認した。描画用モデルは読み込み時にテーマで無視される名前を正規化するため、モデルだけから XML を復元しても元に戻らない。共通の文字列書式バックアップで元の `w:rPr` も保存・復元し、フォント変更の段落記号には元の `w:pPr` も保存・復元するよう修正した。本文とノートの XML 更新経路を使う。

既存のフォント回帰テストは欧文・日本語、テーマあり・なし、複数段落・段落記号のみ・文字のみの 12 条件で保存・再読込と XML 一致の Undo／Redo を検証する。修正後の Web/WASM チェックは成功（1 分 06 秒、`/tmp/jade-font-xml-undo-web.log`）。format_state 28 件と run_format 10 件の計 38 件がすべて成功（`/tmp/jade-font-xml-undo-native.log`）。Word の全機能同等性は未達・未検証。


### スタイル由来の段落装飾の解除（2026-11-07）

- リボンの段落網掛け・罫線と段落設定ダイアログで、スタイルから継承した装飾を解除すると、直接指定を削除していたため保存後の再読込で装飾が復活する経路を修正。継承がある場合は網掛けと罫線に明示的な `nil` を設定し、継承がない場合の従来の削除は維持する。
- 実 DOCX の段落スタイルに黄色の網掛けと上罫線・段落間罫線を設定した回帰テストを追加。リボンとダイアログの両経路でモデル・保存後の再読込、対象外段落の保持、元 XML への Undo、Redo、リボンで解除を繰り返した際の履歴を確認する。
- Web/WASM の `jade-gpui-web --lib` チェックは成功（1分46秒）。ネイティブの段落コマンド群は39件すべて成功（追加回帰テストを含む）。初回はテストの参照先、次回は読込時に色が `#FFFF00` に正規化される点の期待値を修正し、再実行で成功を確認した。
- Word 全機能との同等性は未検証。目標は継続中。


### 段落罫線の辺ごとの継承と部分変更（2026-11-07）

- [Microsoft の TopBorder 仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.topborder?view=openxml-3.0.1) に従い、段落罫線は未指定の辺ごとに上位スタイルを継承する。従来は `pBdr` 全体を置き換えていたため、子スタイルや直接指定に一部の辺だけがある文書で、残りの辺が消えていた。
- `ParagraphBorders::inherit_missing_from` を追加し、スタイル階層の解決と文書内の段落へのスタイル適用に利用する。明示的な `nil` / `none` は維持する。
- リボンと段落設定ダイアログが受け取る罫線ボックスは完成形なので、スタイルに罫線がある場合は省略した辺へ `nil` を出力する。「下罫線だけ」を選んだ後、Word が省略された上・左右・段落間・バー罫線をスタイルから補わないようにする。
- 実 DOCX に親・子スタイルと直接指定を用意し、6種類の辺の継承、直接の `nil`、両編集経路の保存 XML、再読込、対象外段落の罫線保持、Undo/Redo を検証する回帰テストを追加。段落コマンド40件、Reader のスタイル関連57件がすべて成功。Web/WASM の `jade-gpui-web --lib` チェックも成功（1分26秒）。初回はテストで比較した `ParagraphProperties` が `PartialEq` 未実装のため、対象外段落の罫線を比較するよう修正して再実行した。
- Word 全機能との同等性は未検証。目標は継続中。


### 日本語段落体裁の項目ごとのスタイル継承（2026-11-07）

- 段落が体裁を1項目でも直接指定すると、スタイル由来の残りの項目を補完しない経路を修正。例えば禁則処理だけの直接指定により、スタイルで無効にした英数字の自動間隔や句読点のぶら下げが既定値へ戻っていた。
- [Microsoft の Kinsoku 仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.kinsoku?view=openxml-3.0.1) が示す項目単位の継承に合わせ、禁則処理・英単語途中の改行・句読点のぶら下げ・行頭句読点の圧縮・和欧文間隔・和数字間隔を個別に補完。明示した true/false は維持する。
- 実 DOCX の親スタイル・子スタイル・段落直接指定に異なる値を設定し、6項目それぞれを直接指定するケースを true/false 両方向で検証する回帰テストを追加。体裁1項目の編集、対象外段落、保存・再読込、Undo/Redo も対象。修正前のテスト失敗で継承漏れを再現済み。修正後は体裁テスト4件（追加テスト内の12ケースを含む）がすべて成功。Web/WASM の `jade-gpui-web --lib` チェックも成功（32.99秒）。
- Word 全機能との同等性は未検証。目標は継続中。

### 段落で直接指定した番号階層の保持（2026-11-07）

- 番号付き段落で `ilvl` だけを直接指定し、`numId` を段落スタイルから継承する場合に、直接指定した階層までスタイルの値で上書きしていた経路を修正。段落の直接指定を保持し、未指定の場合だけスタイルの階層を補完する。
- [Microsoft の NumberingLevelReference 仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberinglevelreference?view=openxml-3.0.1) における段落の階層指定を対象とする。スタイル定義内の階層選択規則は今回変更していない。
- 既存 DOCX のスタイルと番号定義を利用した回帰テストを追加。直接指定した階層 0・2・8、明示的な `numId=0` による解除、直接指定のない通常のスタイル継承を、読込直後と保存後の再読込で検証する。保存 XML の保持も確認する。修正前は階層 0 がスタイル由来の 1 に変わる失敗を再現した。
- 追加回帰テストは成功（1件内で上記5段落を検証）。Web/WASM の `jade-gpui-web --lib` チェックも成功（38.34秒）。両リポジトリの `git diff --check` も成功。初回の修正ビルドは型の参照パス不足を修正し、再実行で成功を確認した。
- Word 全機能との同等性は未検証。目標は継続中。


### 既定の段落スタイルによる番号の継承と解除（2026-11-07）

- `w:pStyle` がない段落で、既定の段落スタイルに設定された番号を読み込まない経路を修正。段落プロパティ自体がない場合も補完し、直接指定した階層と明示的な `numId=0` は保持する。OOXML の既定スタイル属性は [Microsoft の Style.Default](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.style.default?view=openxml-3.0.1) および ISO/IEC 29500-1 §17.7.4.17 を参照。
- 番号解除コマンドも既定スタイルを調べ、継承がある場合は `numId=0` を保存する。これにより保存後の再読込で番号が復活することを防ぐ。
- 実 DOCX の既定スタイルに番号を設定し、プロパティなし・スタイル指定なし・階層のみ直接指定・明示的な番号解除の4段落で回帰テストを追加。番号解除後のモデル・保存 XML・再読込、keepNext の保持、元 XML への Undo と Redo を確認する。修正前は番号未読込の失敗を再現した。
- Web/WASM の `jade-gpui-web --lib` チェックは成功（41.63秒）。番号継承のネイティブ回帰テスト2件もすべて成功（コンパイル3分31秒、実行0.17秒）。両リポジトリの `git diff --check` も成功。
- Word 実描画との比較と全機能同等性は未検証。目標は継続中。


### 複数の既定段落スタイルを文書順で解決（2026-11-07）

- 既定の段落スタイルが複数ある文書で、HashMap の走査順によって異なるスタイルを選ぶ問題を修正。段落スタイルの XML 順序を保持し、最後に定義された既定スタイルを選択する。ISO/IEC 29500-1 §17.7.4.17 の default 属性に関する規則に従う（[ISO/IEC 29500-1:2012 本文の写し](https://iso.ieclist.ink/iec/ISO%20IEC%2029500-1-2012%20PDF.pdf)）。
- 定義順を逆にした2通りで繰り返し解析し、別種の既定スタイルと `default=0` を除外する回帰テストを追加。存在しない段落スタイルから既定スタイルへのフォールバック番号と、シリアライズ・復元後の選択順も検証する。修正前に最後のスタイルを選ばない失敗を再現した。
- 修正後の Reader スタイル関連テスト58件はすべて成功（コンパイル28.96秒、単体テスト0.71秒）。Web/WASM の `jade-gpui-web --lib` チェックも成功（1分14秒）。両リポジトリの差分チェックは成功。
- Word 実描画との比較と全機能同等性は未検証。目標は継続中。


### Word 実機で確認した部分的な番号設定の継承（2026-11-07）

- Microsoft Word for Mac 16.114.3 で6段落の DOCX を開き、AppleScript の `list level number` と `list string` を観測。段落で `numId` だけを直接指定すると、スタイルの階層を継承する。また、子スタイルの空の `numPr` は親の番号設定を継承する。従来はそれぞれ階層と番号が欠落していた。
- Word に合わせ、段落の直接指定を優先しながら未指定の番号 ID・階層を補完し、空のスタイル番号設定からは親スタイルをたどる。子スタイルに階層だけを指定した場合には親の番号 ID を継承しないことも実機で確認し、この挙動は保持する。明示的な `numId=0` も維持する。
- 入力文書は `jade-core/public/fixtures/partial-style-numbering.docx`、Word の観測値・バージョン・SHA-256 と再実行スクリプトは `jade-core/tests/word-oracle/partial-style-numbering/` に保存。6段落の読込・保存後の再読込に加え、空の子スタイルから継承した番号の解除、対象外段落、Undo/Redo を回帰テストにした。修正前は番号 ID だけを直接指定した段落で階層が欠落する失敗を再現済み。
- Web/WASM の `jade-gpui-web --lib` チェックは成功（42.81秒）。番号継承のネイティブ回帰テスト3件もすべて成功（コンパイル3分31秒、実行0.15秒）。Reader のスタイル関連58件も成功（実行0.58秒）。両リポジトリの `git diff --check` は成功。
- 保存したスクリプトによる再確認は AppleEvent のタイムアウト（-1712）で終了し、UI の確認では Mac のロックを検出。観測 JSON はロック前の実行結果を記録している。今回の Word 実機比較は番号設定の6段落が対象で、全機能同等性は未検証。目標は継続中。


### タブ位置の親子スタイル継承と編集後の保持（2026-11-07）

- [Microsoft の Tabs 仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.tabs?view=openxml-3.0.1) が定める加算方式に合わせ、親子スタイルのタブ設定を順に合成する。従来は子にタブ設定が1件でもあると、親から受け継ぐ残りの位置を失っていた。位置ごとの上書きと `clear` は後から出現する設定を優先する。
- タブ位置の解除・再設定も出現順に解決する。親の位置を子が解除し、孫が再設定する場合に、古い `clear` が孫の設定まで消していた編集用の実効値解決を修正した。
- `SetParagraphTabsCommand` は段落・表・既定スタイルから継承する位置を調べ、不要な位置には直接の `clear` を保存する。タブをすべて消した後、保存・再読込でスタイルの位置が復活する問題を防ぐ。
- 実 DOCX を基に3階層のスタイル、同位置の配置変更、リーダー、段落の直接解除、既定スタイルを検証する回帰テストを追加。タブ設定のコマンドと段落設定ダイアログの両方について、空の設定・別位置への置換、対象外段落、keepNext、保存・再読込、Undo/Redo を確認する。修正前に親の位置の消失と、解除した位置の再読込時の復活を再現した。最初のテスト素材には styles のリレーションがなかったため、スタイルが正しく読み込まれる既存パッケージへ変更してから再現を確認した。
- 追加したタブ継承テスト2件（編集テスト内で2経路×2種類の置換を検証）は成功（コンパイル1分33秒、実行0.42秒）。Reader のスタイル関連58件も成功（実行0.71秒）。Web/WASM の `jade-gpui-web --lib` チェックも成功（52.04秒）。既存の段落コマンド40件もすべて成功（実行0.12秒）。両リポジトリの `git diff --check` も成功。今回の修正は仕様に基づくもので、Word 実機とのタブ位置比較と全機能同等性は未検証。目標は継続中。


### 自動の段落間隔から固定値への切り替えと項目単位の保存（2026-11-07）

- 段落前後の間隔を自動から固定ポイントへ変更するとき、表示中の自動間隔と同じ数値でも変更として扱う。従来は数値だけを比較し、同じ値だと自動設定の解除を無視していた。
- 固定値の保存には `beforeAutospacing="0"`／`afterAutospacing="0"` を明記する。[Microsoft の beforeAutospacing 仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.beforeautospacing?view=openxml-3.0.1) にある、属性省略時はスタイルから継承する規則に合わせ、スタイルの自動設定が保存後に復活することを防ぐ。ヘッダー・フッターにも共通の保存処理を適用する。
- 行間・段落前・段落後のうち、編集した項目だけを XML に反映する。行間だけの変更や片側の固定値変更で、未編集の側の自動設定を解除していた問題を修正した。
- スタイル由来と直接指定の自動間隔について、同値・別値・ゼロへの切り替え、未編集側の保持、保存・再読込、Undo/Redo を検証する回帰テストを追加。修正前に同値の切り替え無視と行間変更による自動設定の消失の2件が失敗することを確認した。
- 追加の自動間隔テスト3件はすべて成功（実行0.55秒）。既存の段落コマンド40件も成功（実行0.10秒）。Web/WASM の `jade-gpui-web --lib` チェックも成功（45.57秒）。 ヘッダー・フッターの段落設定テスト1件（6種類の領域を検証）も成功（実行0.11秒）。両リポジトリの `git diff --check` も成功。
- 今回は固定値への編集と保存が対象。数値指定を伴わない自動設定の明示的な解除だけの読込、親子スタイル間のポイント単位と行単位の切り替えは、別途検証・改善が必要。Word 実機との比較と全機能同等性は未検証。目標は継続中。


### 自動間隔の明示的なオフと元の数値の継承（2026-11-07）

- `beforeAutospacing`／`afterAutospacing` の未指定と明示的な `0`・`false`・`off` を区別する。従来は false のみの spacing 要素を捨て、親スタイルの自動間隔を解除できなかった。[Microsoft の仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.beforeautospacing?view=openxml-3.0.1) に従い、属性が未指定ならスタイルから継承する。数値を指定しただけでは、継承した自動設定を解除しない。
- 自動設定によって隠れている元のポイント値・行単位の値をモデルに保持する。子スタイルや段落が自動設定だけをオフにした際に、親が指定した数値を復元する。数値が元からなければ、表示用の自動間隔14ptを固定値として引き継がない。
- スタイルの親子関係と、段落・表・既定値の継承で同じ間隔解決処理を使う。固定ポイントへの編集は自動設定のオフを明示し、保持した元の値も更新する。
- 段落プロパティの空判定でも明示的なオフを有効な書式として扱う。スタイル名や数値を持たず、自動間隔の解除だけを指定した段落が読み込み時に書式なしへ変わる経路も修正した。
- 既存 DOCX を基に、自動→オフ→再び自動の3階層、直接のオフ指定、片側だけのオフ、数値だけの直接指定、保存・再読込を検証する回帰ケースを追加。修正前は子スタイルでオフにした段落が自動の14ptのままになる失敗を確認した。Reader のテストでは空要素・開始終了タグ、3種類の false 表記、JSON の保存・復元、元の数値がない場合も確認する。
- DOCX の自動間隔テスト4件（保存・再読込、固定値編集の Undo/Redo を含む）は成功（実行0.53秒）。Reader の追加2件も成功（実行0.01秒）。既存のスタイル関連58件と段落コマンド40件も成功（実行0.54秒／0.12秒）。Web/WASM の最終チェックも成功（59.25秒）。両リポジトリの `git diff --check` も成功。
- ポイント単位と行単位の数値同士の優先関係は引き続き別途検証が必要。今回の修正は仕様に基づくもので、Word 実機との間隔比較と全機能同等性は未検証。目標は継続中。


### スタイル変更・次の段落への直接の自動間隔の保持（2026-11-07）

- 段落スタイルを変更するときと、段落末尾の改行で `w:next` のスタイルへ移るとき、直接指定した `beforeAutospacing`／`afterAutospacing` を保持する。従来は数値と行間の属性だけを戻していたため、自動間隔の明示的なオン・オフが新しいスタイルの設定へ置き換わっていた。
- 新しいスタイルへ重ねる直接書式には、元の XML にある数値と自動間隔の属性だけを使う。自動間隔が表示していた14ptを、元の数値の代わりにコピーしない。数値だけが直接指定されている場合は、新しいスタイルの自動間隔の設定と組み合わせて解決する。
- DOCX の回帰テストを2件追加。スタイル変更と次の段落への改行の各経路で、直接の両側オフ、数値だけの指定、片側オフ、直接の両側オンの4ケースを確認する。保存・再読込と Undo/Redo も対象にする。修正前には両経路とも直接のオフが自動の14ptになる失敗を確認した。 移行前後のスタイルで元の段落前間隔を4ptと12ptに変え、数値を直接指定していない側は新しいスタイルの値を使うことも確認する。
- 自動間隔の DOCX テスト6件（実行0.58秒）、既存の段落コマンド40件（0.12秒）、改行処理15件（0.76秒）の計61件が成功。Web/WASM チェックも成功（35.75秒）。ルートと今回の段落間隔関連ファイルの `git diff --check` は成功。サブモジュール全体のチェックには、今回の対象外である `jade-pptx-reader/tests/reads_a_slide.rs` の末尾空行の指摘が残る。
- 自動設定の独立した継承は [Microsoft の beforeAutospacing の説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.beforeautospacing?view=openxml-3.0.1) を参照。Word 実機との間隔比較と全機能同等性は未検証。目標は継続中。


### 既定の段落スタイルから次のスタイルへの改行（2026-11-07）

- `w:pStyle` を省略した段落、または未定義のスタイル名を持つ段落で、Enter が既定段落スタイルの `w:next` を使うようにした。従来の処理は直接のスタイル名だけを調べて終了し、読込時には既定スタイルで表示していた段落でも、改行後のスタイルへ移行できなかった。
- 既定スタイルは `Normal` 固定ではなく、StylesPart が解決した文書の既定段落スタイルを参照する。単発と連続の改行に共通の処理を使い、連続改行では既定スタイル→本文→末尾のような `w:next` の連鎖も適用する。
- [Microsoft の next の仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.nextparagraphstyle?view=openxml-3.0.1) が定める、次のスタイルが存在しない・段落スタイルではない場合の無視も確認する。既定スタイルの適用と組み合わせた仕様解釈に基づく修正であり、Word 実機の編集結果との比較は未検証。
- DOCX 回帰テスト4件を追加。段落プロパティ自体がない場合・空の場合、直接書式だけを持つ場合、未定義のスタイル名、段落途中の分割、無効な次スタイルと自己参照を対象にする。変更先のスタイル名・配置・段落間隔、直接書式の保持、保存・再読込と Undo/Redo を確認する。修正前には省略・未定義の2件が失敗し、途中分割と無効参照の2件は成功した。
- 最初の実行を妨げていた共有 PPTX 依存部分の `TextInsets` のフィールド参照も、既存の `left_emu`／`right_emu`／`top_emu`／`bottom_emu` に合わせた。
- 追加4件（実行0.30秒）、既存の改行処理15件（0.76秒）、自動間隔6件（1.12秒）の計25テストが成功。Web/WASM チェックも成功（42.39秒）。ルートとサブモジュールの `git diff --check`、追加テストの rustfmt チェックも成功。
- 全機能同等性は未検証。目標は継続中。

### 段落スタイル適用時のテーマ色参照の解放（2026-11-07）

- 段落スタイルを適用するとき、スタイルが定義する文字色はランから外して継承に戻していたが、`theme_color`（テーマ名・濃淡）の参照だけがランに残っていた。新規適用だけでなく同じスタイルを再適用する場合にも再現した。
- 解決済みの色とテーマ参照を一つの `w:color` 宣言として扱い、スタイルが色またはテーマ参照を持つ場合に両方を解放する。XML 側の除去条件も同じにした。文字色を持たないスタイルでは直接指定したテーマ色を保持する。
- `paragraph_style_colour` に回帰テスト3件を追加。修正前は新規適用と再適用の2件が残存テーマ参照で失敗。修正後は、文字入力と別スタイルへの切替、保存・再読込、Undo/Redo の XML 復元、直接テーマ色の保持を含めて3件成功した。
- 既存 `format_state` 28件を含む計31テストが成功。Web/WASM チェック（1分01秒）、変更ファイルの rustfmt チェック、ルートとサブモジュールの `git diff --check` も成功。
- 今回は編集モデルと DOCX 保存の一貫性を検証した。Word 実機との編集結果比較と全機能同等性は未検証。目標は継続中。

### スタイル由来の組文字・縦中横の読み込みと継承（2026-11-07）

- `w:eastAsianLayout` は本文の直接書式では対応していたが、`styles.xml` の読み込みとラン書式の合成では欠落していた。既定書式・段落スタイル・文字スタイルから組文字や縦中横が指定されていても反映できなかった。
- 既存の本文用属性パーサーをスタイルにも使用し、グループ ID・組文字・括弧・縦中横・圧縮指定を保持する。既定書式→段落スタイル→文字スタイル→直接書式の順で合成し、子スタイルや直接指定の明示的オフも反映する。
- 段落スタイル適用時には、当該スタイルが定義する `w:eastAsianLayout` を直接書式から外して継承に戻す。Undo は元の直接書式を復元し、文字スタイルによる解除は維持する。
- [Microsoft の要素説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.eastasianlayout?view=openxml-3.0.1)を参照。Word 固有の圧縮指定無視や数式内の制限などは別途比較が必要であり、今回すべての描画規則を同等にしたという意味ではない。
- 読み込み回帰テストは修正前に設定欠落で失敗し、修正後は既存分を含むスタイルの59テストが成功。保存・再読込、文字入力、スタイル切替、Undo/Redo、直接書式の保持の統合テストと、実際の描画位置を検証するテストを追加した。
- 読み込み59件、編集・保存6件、描画2件の計67テストが成功。既定書式・段落スタイル・文字スタイルそれぞれから指定した組文字が上下2段に描かれることを記録ペインターで確認した。Web/WASM チェック、変更ファイルの rustfmt チェック、ルートとサブモジュールの `git diff --check` も成功。Word 実機との表示比較と全機能同等性は未検証であり、目標は継続中。

### 組文字・縦中横の編集後に残る古い描画キャッシュ（2026-11-07）

- `w:eastAsianLayout` が段落描画、折り返し、適用済み文字スタイルのキャッシュキーから欠落していた。組文字の括弧を丸から角へ変更しても丸括弧のまま描画され、段落描画と折り返しのキャッシュを消しても文字スタイル側に古い括弧が残ることを再現した。
- グループ ID・組文字・括弧・縦中横・圧縮指定を持つ `EastAsianLayout` に `Hash` を導出し、3種類のキャッシュキーに設定全体を含める。キャッシュ自体は維持する。
- 同じ描画器で書式を順に変更した結果を、各文書を新規描画器で描いた結果と比較する回帰テスト3件を追加。編集処理と同じ変更通知を伴う再描画、段落描画キャッシュのクリア、折り返しキャッシュもクリアする場合を対象に、横書きの組文字・括弧の切替と縦書きの縦中横の設定・解除を検証する。修正前は3件すべてが括弧の変更時に失敗した。
- これは編集後の表示と文書の書式を一致させる修正。[Microsoft の互換性仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/efbc3ed1-1349-4572-93a0-a4e0f71e2a8b)に記載された `fitText` との優先規則、Word の `vertCompress` の扱い、および Word 実機での表示比較は別途検証が必要。
- 修正後は新規3件を含む描画・キャッシュ関連の10テストが成功。Web/WASM チェック（1分48秒）、追加したテスト部分の rustfmt 確認、ルートとサブモジュールの `git diff --check` も成功。Word 実機との比較および全機能同等性は未検証で、改善目標は継続中。

### 組文字と縦中横を同時指定した場合の優先規則（2026-11-07）

- [Microsoft の互換性仕様 §2.1.75](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/efbc3ed1-1349-4572-93a0-a4e0f71e2a8b)では、`combine=true` の場合に Word は `vert` を無視する。両方の指定を持つ文書を組文字だけの文書と比較し、修正前は縦書きで文字列全体が縦中横のグループになる不一致を再現した。
- 縦書きのグループ形成、隣接グループの結合、表セルの縦方向メトリクス、表セルの描画判定で組文字を優先する。保存対象の属性自体は変更しない。
- 本文と表セルについて、横書き・`tbRl`・`tbRlV` それぞれの描画命令を比較する回帰テストを追加。`vertCompress`、`fitText`、数式内の制限は別の確認事項として残る。Word 実機での今回の比較は未実施。
- 修正後は本文・表セルの描画回帰を含む7件、共通レイアウトの縦書き・組文字関連26件の計33テストが成功。Web/WASM チェック（1分32秒）、新規テスト部分の rustfmt 確認、ルートとサブモジュールの `git diff --check` も成功。全機能同等性は未達で改善を継続する。

### 縦中横の vertCompress 互換性（2026-11-07）

- Word for Mac 16.114.3で、本文の tbRl / tbRlV × vertCompress 未指定・0・1の6条件をPDF出力して比較した。各方向の3条件で座標が完全に一致し、10桁の数字の幅は約66.742ptだった。1文字幅への縮小は行われない。MS-OI29500 §2.1.75の属性を無視する規定とも一致する。
- Jadeが vertCompress=true の場合にだけ縦中横を縮小していた処理を削除した。読み込み・保存する属性自体は保持する。
- 本文と表、tbRl / tbRlVで属性オン・オフの描画一致を回帰テストに追加した。修正前の失敗を確認済み。比較文書の生成コードとWordの座標記録は `jade-core/tests/word-oracle/east-asian-compress/` に保存した。Wordで実測した範囲は本文であり、表は共有描画処理の回帰検証である。
- 検証: 縦中横関連の描画テスト3件、Web向けWASM check、変更断片のrustfmt確認とdiff checkが成功した。Word全体との機能同等性は引き続き未達・未検証。

### 組文字と均等割り付けの優先順位（2026-11-07）

- Word for Mac 16.114.3 の実機 PDF で、同一 run に `fitText` と `eastAsianLayout combine` があると組文字が無効になることを確認。共有レイアウト処理と描画効果の両方で `fitText` を優先するよう修正。
- 比較資料: `jade-core/tests/word-oracle/east-asian-combine/`。括弧5種類の対応も実機で一致を確認した。
- 新たに確認した残差: 1文字の組文字は Word では半サイズの下段になる。既存の「2文字未満は通常文字」処理は別途修正が必要。Word との完全一致は未達。
- 検証: 修正前の新規描画テストが失敗することを確認。修正後は組文字の描画4テスト、共有レイアウトの新規1テストが成功。`jade-gpui-web` の WASM check も成功。

### 1文字の組文字と組文字の上下位置（2026-11-07）

- 前項で残差とした1文字の組文字を半サイズに修正。Word 実機 PDF の text matrix により、周囲の通常文字とベースラインが一致すると確認した。「下段」という初期の目視解釈を訂正した。
- 同じ12ptの比較文書では複数文字の組文字は6ptとなり、上段は通常基準線から6.24pt上、下段は2.4pt下、括弧は通常基準線に一致する。共有描画処理を実測値に合わせた。
- 証跡: `jade-core/tests/word-oracle/east-asian-combine/word-observation.json`。この数値確認は Arial / 游明朝、12ptの本文の実測。全フォント・サイズでの完全互換は未検証。
- 検証: 修正前の1文字の共有レイアウトテストが失敗することを確認。修正後は共有レイアウト2テスト、組文字の描画5テスト、縦中横の既存3テストが成功。Wordエディター単体とWeb全体のWASM checkも成功。

### 組文字の文字幅倍率と描画幅（2026-11-07）

- 組文字のレイアウトは文字幅倍率（`w:w`）を反映していたが、描画は無倍率の文字幅で並べていた。50%指定の1文字では、記録ペインター上で確保幅4pxに対して8pxを描き、後続文字に重なることを修正前テストで確認。
- 配置済みグリフの位置と末尾の文字幅から確保された幅を復元し、上下2段と括弧を含む描画全体をその幅に合わせるよう修正。レイアウト用の2段圧縮率をそのまま再適用する二重圧縮を避ける。
- 回帰テストは1・3・4文字、括弧なし／丸括弧、50%／100%／200%の18条件。canvas変換を計算した実際の描画右端と後続文字の開始位置を比較する。
- Word実機はMacのロックで今回の操作ができず、倍率指定と組文字を併用した実描画比較は未検証。今回の確認対象は共有レイアウトと描画幅の整合性であり、全機能同等性は引き続き未達。
- 検証: 修正前の新規テストの失敗を確認し、修正後は組文字の描画6テスト（新規1テスト内で18条件）が成功。Web全体のWASM checkも成功（1分37秒）。Word比較用の `jade-core/tests/word-oracle/east-asian-combine/combine-scale.docx` を追加したが、実機の観測記録とは区別している。

### 表内の組文字の計測と描画（2026-11-07）

- 本文の組文字テストを表内に拡張すると、1文字・50%で描画4pxに対して後続文字まで8px空く不一致を再現。表の行計測では組文字の半サイズ・2段分の圧縮を適用していなかった。
- 表の行折り返しと列の内容幅計測に、本文と共通の `combined_run_properties` を適用。元の文字範囲・編集用の文字列は維持する。行の変更は横書きセルに限定し、縦書きセルの既存経路は維持。
- 計測を直した後、丸括弧付きのケースでも失敗。表の横書き描画は組文字のエフェクト処理を通らず、通常の1段で描画していた。組文字の横書き描画を本文の共通コマンド実行処理につないだ。
- 本文／表内 × 1・3・4文字 × 括弧なし／丸括弧 × 50%／100%／200% の36条件で、変換後の描画右端と後続文字の開始位置、1文字／複数文字の基準線の数、括弧の描画数を検証。既存の組文字関連6テストとWeb全体のWASMチェック（最終コードで1分14秒）が成功。
- `combine-table-scale.docx` にWord実機比較用の18条件を追加。MacがロックされているためWord出力は未取得。文字間隔の計測と描画の扱い、縦書きセルの組文字描画は引き続き確認が必要で、Wordとの完全互換は未達。

### 組文字の文字間隔と確保幅（2026-11-07）

- 組文字の上下2段を1行のグリフ位置で表す際、段の境界にある余分な文字間隔を分母から除いていた。さらに文字幅倍率だけを圧縮し、文字間隔を圧縮しなかったため、計測幅が広い段の幅と一致しなかった。修正前テストでは3文字・−0.75pt・50%が期待7pxに対して約5.636pxとなった。
- 圧縮前の文字列全体の幅を分母にし、文字幅と文字間隔の両方に同じ圧縮率を適用。共有描画処理では元の文字間隔を復元し、各段の文字間に挿入してから確保幅に合わせる。間隔の指定をグリフ自体の引き伸ばしに置き換えていた問題も修正した。
- 共有レイアウトの新規テストは3・4文字 × −0.75pt／1.5pt／12pt × 50%／100%／200%の18条件。既存を含むレイアウト3テストとWeb全体のWASM check（41秒）が成功。
- 描画の回帰テストを本文／表内 × 1・3・4文字 × 括弧なし／丸括弧 × 50%／100%／200% × 文字間隔0pt／1.5ptの72条件に拡張。括弧なしの段内間隔、後続文字への重なりがないこと、間隔なしの描画右端、基準線数と括弧数を検証し、既存を含む組文字関連6テストが成功。
- `combine-spacing.docx` にWord実機比較用の36段落を追加。DOCXのZIPとXMLを検査した。Word実機出力は未取得で、今回の幅・間隔の整合性の確認とは区別する。括弧付きの組文字と後続文字の境界間隔は単純な倍率計算と一致せず、正確なWordの規則は実測待ち。縦書きセルや実フォントでのWordとの完全互換も引き続き未検証。

### 均等割り付けと縦中横の優先順位（2026-11-07）

- `fitText` と `eastAsianLayout vert` の併用時、縦書きの行折り返しと描画に縦中横が残っていた。Microsoftの MS-OI29500 §2.1.75 は `fitText` がある場合に `eastAsianLayout` を無視すると規定している。組文字だけでなく縦中横にも同じ優先順位を適用した。
- 縦書き本文・セルのグループ化、縦方向の送り幅の例外、表を含む共有描画判定で `fitText` を確認する。修正前のテストでは「株式会社」が1グループとなり、通常の縦書きと同じ4行への折り返しに失敗した。
- 新規の共有レイアウト1テストが成功。描画では本文／表 × 通常／tbRl／tbRlV × combine／vert／両方の18比較を追加し、組文字関連6テストと縦中横関連4テストが成功（優先順位テストは両方に含まれる）。Web全体のWASM check（44秒）とdiff checkも成功した。
- `jade-core/tests/word-oracle/east-asian-combine/combine-vertical-fit.docx` に縦書きセル2方向 × 4属性条件の8行を追加し、ZIPとXMLを検査した。今回はMacがロックされていてWord実機出力は未取得。縦書きセルの組文字自体の向きや配置は引き続き実測が必要で、完全互換は未達。
- 仕様参照: https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/efbc3ed1-1349-4572-93a0-a4e0f71e2a8b

### 組文字・縦中横の解除とスタイル継承（2026-11-07）

- 全単体テストを実行し、共有レイアウト721件成功・12件ignored。エディターは2445件成功・21件ignored・1件失敗だった。失敗はヘッダー／フッターの装飾解除テストが古い「要素削除」を期待していたもの。継承を打ち消す明示的なオフ値と、保存・再読込後の状態を検証するよう修正し、単独実行で成功した。
- 同じ確認で、組文字／縦中横の解除だけは実装も要素を削除しており、段落スタイル由来の指定を解除できないことを発見。新しい統合テストで修正前の失敗（解除後にも combine=true）を確認した。解除時に `eastAsianLayout` の `combine=0` と `vert=0` を保存し、継承への復帰と明示的な解除を区別する。
- 新規統合テストは組文字スタイル／縦中横スタイルの2条件で、保存・再読込、Undo／Redoを検証する。ヘッダー／フッターと通常の文字書式テストも解除属性の保持を検証するよう更新。関連統合テスト14件、修正後のエディター全単体テスト2446件（21件ignored）が成功した。Web全体のWASM check（38秒）とdiff checkも成功した。
- 前回の `fitText` 回帰テストも強化。フォントの縦送り14pxと横の字面10pxを分けて与え、縦書きセルが通常の縦送りを使い、字面幅を変えないことを確認した。新しい検証は成功した。

### OpenType文字書式の解除とスタイル継承（2026-11-07）

- 合字を「なし」、数字の字体・間隔を「既定」に戻す処理が `w14:ligatures`／`w14:numForm`／`w14:numSpacing` を削除しており、文書の既定値やスタイル由来の設定を解除できなかった。新規統合テストで、解除後にも `standard` が残る失敗を確認した。
- 解除時に `none`／`default` を明示保存するよう、本文・ヘッダー／フッターで共用する文字書式更新を修正した。Microsoftの MS-DOCX §5.1 スキーマに定義された値を使い、モデル上の継承打ち消しと保存するXMLを一致させる。
- `style_typography` の回帰テストで文書の既定値／段落スタイル／文字スタイル × 3項目の9条件を検証する。1項目だけ変更したときの他の2項目の保持、保存・再読込、Undo／Redoを確認する。既存の本文とヘッダー／フッターのテストも明示的な解除値を検証するよう更新した。
- 修正後の関連統合テスト11件（新規テスト内の9条件を含む）とヘッダー／フッターの回帰テスト1件（2役割 × 3種類）が成功。Web全体のWASM check（35秒）、変更箇所のrustfmt checkと両リポジトリのdiff checkも成功した。Word実機での見た目の比較は今回未実施。
- 仕様参照: https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/9704b59f-bc49-4618-ac66-41beb82a0d7f

### 下線の種類・色の独立した継承（2026-11-07）

- `w:u` の種類を指定すると色の継承も消え、色だけのスタイル指定は読み捨てていた。Wordは `val` と `color` の省略を個別に継承するため、スタイル解析と実効書式の合成、本文／表の描画用フォールバックを修正した。
- 下線色を「自動」に戻す処理は属性を削除しており、継承色の打ち消しにならなかった。`w:color="auto"` を明示保存する。色だけの編集では `w:u` に不要な `w:val` を追加せず、継承した下線の種類を維持する。
- `style_underline` の新規統合テストは、文書の既定値／段落スタイル／文字スタイル × 種類のみの変更／色のみの変更／自動色への解除の9条件を扱う。選択範囲外の保持、保存・再読込、Undo／Redoを検証する。色だけを指定した開始・終了タグ形式／空要素形式の読み込み6条件も加えた。リーダーの単体テストには、既定値→段落スタイル→文字スタイルの各属性の継承を追加した。
- 仕様参照: [MS-OI29500 §2.1.100](https://learn.microsoft.com/zh-tw/openspecs/office_standards/ms-oi29500/983b4d5a-53ba-4d79-89cb-fc13303af71b)。今回の確認は明示RGB色と自動色が対象で、下線のテーマ色・濃淡とWord実機での見た目の比較は未検証。
- 検証: 下線関連のリーダー単体テスト7件、本文文字書式の統合テスト10件、新規の下線統合テスト2件が成功。Web全体のWASM check（1分55秒）、新規テストと変更ブロックのrustfmt check、両リポジトリのdiff checkも成功した。
- 追加検証: エディターの既存下線関連単体テスト20件も成功。破線パターン、本文・ヘッダー／フッターの状態、描画、下線コマンド、色だけの指定で下線を有効にしない既存文書の回帰を含む。今回の関連テストは合計39件成功した。

### 下線のテーマ色と濃淡（2026-11-07）

- 下線の `themeColor`／`themeTint`／`themeShade` はモデルに保持されず、読み込み時にキャッシュされた `color` だけを使っていた。本文の開始・終了タグ形式／空要素形式とスタイル解析でテーマを解決し、テーマ参照自体も保持する。共有レイアウト、本文／表の描画用フォールバックへ参照を引き継ぐ。
- 下線の種類だけを変更した場合はテーマ参照を保持し、通常のRGB色や自動色を明示指定した場合は古いテーマ参照を解除する。書式クリアやスタイル変更で下線色を削除する経路でも参照を残さない。保存時にはテーマ属性を書き戻す。
- `themeTint` と `themeShade` の両方がある下線は、Wordと同じく `themeTint` を優先する。文字自体の色の処理は変更していない。仕様参照: [MS-OE376 §2.1.101](https://learn.microsoft.com/id-id/openspecs/office_standards/ms-oe376/70728747-cf2d-4393-afb3-f5117faa27d5)。
- 統合テストは直接指定／文書既定／段落スタイル／文字スタイル × 通常テーマ色／shade／tint／両方／未知のテーマ名 × 空要素／開始・終了タグの40条件を追加。保存・再読込、種類変更、RGB色・自動色への変更、範囲外の保持、Undo／Redoを検証する。キャッシュ色なしの色だけの指定6条件と、リーダーのテーマ参照のみの読み込み・継承・解除、共有レイアウトへの参照引き継ぎも検証する。
- 検証: リーダーの下線関連9件、共有レイアウト1件、エディターの下線関連20件が成功。最終検証ではエディター全体2,446件（21件は既存のignore）、文字書式の統合テスト10件、下線の統合テスト4件が成功し、失敗は0件。Web全体のWASM check（約37秒）、変更ブロックのrustfmt check、両リポジトリのdiff checkも成功した。Word実機での描画比較、テーマ変更操作や配色マッピングを含む完全互換は引き続き未検証。

### 文字テーマ色のtint／shadeの優先順位（2026-11-07）

- 文字色の `themeTint` と `themeShade` が両方あると、両方の変換を順番に適用していた。Wordはtintだけを適用するため、テーマ色の共有解決処理を修正した。下線も同じ処理を利用し、前項で修正済みの優先順位を維持する。
- 仕様参照: [MS-OI29500 §2.1.72(b)](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/8229a077-7fc8-4fba-96cc-c77b6a4fc768)。
- 回帰テストはtintの00／80／FFとshadeの00を併記する境界値を扱う。直接指定／文書既定／段落スタイル／文字スタイル × 空要素／開始・終了タグ × 3値の24条件で、読み込み、保存・再読み込み、一部の太字変更、範囲外の色保持、Undo／Redoを確認する。
- 検証: リーダーのテーマ関連単体テスト12件、文字書式の統合テスト10件、テーマ色と下線の統合テスト5件が成功（計27件、失敗0件）。Web全体のWASM check（約39秒）、追加テストのrustfmt check、両リポジトリのdiff checkも成功した。Word実機での描画比較は未検証。

### 文書のテーマ配色割り当て（2026-11-07）

- `settings.xml` の `w:clrSchemeMapping` は色の解決で無視され、文字色・下線色が既定の配色割り当てで表示されていた。文書設定に割り当てを保持し、本文・各ストーリー・スタイルの読み込みより前に適用する。編集中のスタイル再解析でも設定を適用する。
- 論理的な文字色／背景色、accent1〜6、ハイパーリンク／表示済みリンクを物理的なテーマ色へ一度だけ割り当て、その後に濃淡を解決する。例えばaccent1とaccent2を互いに交換しても循環させない。明示したdark1／light1等の物理参照は変更しない。テーマ参照の名前自体は元のまま保持する。
- 仕様参照: [clrSchemeMapping.Text1](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.colorschememapping.text1?view=openxml-3.0.1)、[clrSchemeMapping.Accent1](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.colorschememapping.accent1?view=openxml-3.0.1)、[MS-OI29500 §2.1.395](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/1be23913-3f88-40ab-9cb2-2790be8c549d)。
- 統合テストは直接指定／文書既定／段落スタイル／文字スタイル × 設定の空要素／開始・終了タグの8条件で、文字色と下線色、保存・再読み込み、部分編集、Undo／Redoを確認する。単体テストには物理色との区別、循環する割り当て、スコープの復元、濃淡を適用する順序を追加した。
- 検証: リーダー全体628件、エディター全体2,446件（21件は既存のignore）、文字書式とテーマ色・下線の統合テスト16件が成功（失敗0件）。Web全体のWASM check（約59秒）、追加テストと変更関数のrustfmt check、両リポジトリのdiff checkも成功した。前回追加した下線テーマ参照によるRun構造体の8バイト増加について、サイズ検査の期待値と説明を更新した。Word実機との描画比較、DrawingML図形の配色割り当てや文書間コピーの網羅的検証は未完了。

### テキストボックスの文書配色のコピーと再解析（2026-11-07）

- 図形をコピーする際、元文書の `w:clrSchemeMapping` を使わず既定のテーマ色で固定していた。図形内の文字と、コピー対象のスタイル・番号定義を具体色へ変換する処理に、元文書の配色割り当てを渡すよう修正した。貼り付け先の設定やテーマは変更しない。
- 図形XMLを編集してテキストボックスを再解析する経路も、文書のテーマと配色割り当てを解析前に適用する。割り当てがない場合も、テーマ参照を古いキャッシュ色のままにせず実際のテーマ色へ解決する。
- 配色の意味は [clrSchemeMapping.Text1](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.colorschememapping.text1?view=openxml-3.0.1) を参照。DrawingMLの図形塗り自体のカスタム配色への対応は別途必要。
- 回帰テストは、直接指定／文書既定／段落スタイル／文字スタイル × 設定の空要素／開始・終了タグの8条件で、文字色・下線色・物理色参照、コピー元の不変性、異なる配色設定の文書への貼り付け、保存・再読み込み、Undo／Redoを確認する。図形XMLの文字変更による再解析は、配色割り当てあり／なしの2条件を扱う。
- 検証: 追加した2件の回帰テスト（計10条件）と既存の図形編集テスト4件が成功（失敗0件）。Web全体のWASM check（約38秒）、変更関数・追加ファイルのrustfmt check、両リポジトリのdiff checkも成功した。Word実機との描画比較と、図形・グラフ・SmartArt全体の配色の網羅的検証は未完了。

### 図形の文書配色割り当て（2026-11-07）

- DrawingMLの図形を表示する際、`w:clrSchemeMapping` が塗り・線のテーマ参照に反映されていなかった。本文・ヘッダー／フッター・注記・図形XMLの再解析・テーマ編集・書式変更履歴の再解析で、図形用の論理色を文書設定から解決する。WordArtのtext1／background1等も物理色とは区別する。
- 元のテーマは変更せず、物理色dk1／lt1等を保持した描画用テーマを作る。アクセント色の交換は元のテーマから一度だけ解決し、その後にDrawingMLの色変換を適用する。文字色の解析スコープには元のテーマを渡し、割り当ての二重適用を避ける。
- 文書間の図形コピーでも同じ割り当てを使って色を固定する。DrawingMLの透明度や色変換の子要素は保持する。
- 既存のテキストボックス回帰テスト10条件に塗り・線・50%透明度の確認を追加。異なる配色の文書への貼り付け、元文書の不変性、保存・再読み込み、Undo／Redo、図形XMLの編集後を検証する。テーマの単体テストでは論理色・物理色・アクセントの交換・リンク色を確認する。
- Word実機との描画比較、およびグラフ・SmartArt独自の配色上書きは未検証。
- テーマ編集でaccent1／accent2を変更した後、割り当て先の色で塗りを更新する追加テストも成功。検証はテーマ関連の単体テスト41件、図形編集と配色の統合テスト7件、Web全体のWASM check（約34秒）、変更関数と追加ファイルのrustfmt check、両リポジトリのdiff checkが成功した（失敗0件）。

### テーマ変更直後の文字色・下線色（2026-11-07）

- テーマ変更は図形色とフォントを更新する一方、文字色・下線色には読み込み時の具体色が残っていた。追加テストで、accent1を変更しても下線が旧テーマの4472C4のまま残ることを再現した。
- テーマ変更時にスタイル／文書既定を現在のテーマと配色割り当てで再解析する。本文・表・テキストボックス・脚注／文末脚注・読み込んだヘッダー／フッターの再着色処理では、保持している文字色／下線色のテーマ参照から表示色を再計算する。段落記号の文字プロパティも対象とする。元のテーマと配色設定をスコープに渡し、DrawingML用テーマによる二重の割り当てを避ける。
- XMLのテーマ参照やキャッシュ色は書き換えず、表示モデルの色を更新する。明示的な具体色にはテーマ参照がないため再着色しない。物理色dark1の参照は文書の論理色割り当てに影響されない。
- 統合テストに、直接指定／文書既定／段落スタイル／文字スタイル × 配色割り当てあり／なしの8条件を追加。テーマ変更直後と保存・再読み込み後の文字色・下線色・図形色、本文／スタイル／設定XMLの不変性、Undo／Redoを確認する。濃淡変換は同じ4種類の指定元 × Tintのみ／TintとShade併記の8条件で、割り当て後の変換とTint優先を検証する。
- 番号定義のリーダーは文字色のテーマ参照を保持・解決しておらず、別途修正が必要。Word実機との描画比較と、全ストーリーでの操作を通した網羅的検証は未完了。
- 検証: テーマ関連の単体テスト22件、図形・配色・下線の統合テスト15件が成功（失敗0件）。Web全体のWASM check（約32秒）、変更関数・追加テストのrustfmt check、両リポジトリのdiff checkも成功した。

### 番号・箇条書きのテーマ色と下線（2026-11-07）

- 番号定義の文字色は `w:val` のみを読み、テーマ色・濃淡・文書の配色割り当てを無視していた。統合テストで、text1をdark2へ割り当てた番号がテーマ色44546Aではなくキャッシュ色AA1122になることを再現した。
- 番号レベルの文字色と下線を本文と同じパーサーで解析する。テーマ参照と濃淡を保持し、下線の種類・色・明示的な解除も読み取る。`w:u` に値がない場合は本文と同様に下線の有無を未指定とする。
- テーマ変更時には番号定義のテーマ参照を、元のテーマと文書の配色割り当てから再解決する。XMLのキャッシュ色や参照名は変更しない。変更直後／保存・再読み込み／Undo・Redoを、濃淡なしとShade・Tint併記の2条件で確認した。箇条書きの下線はdouble／none／0／single／値なしの5条件を検証した。
- 検証: リーダー全体630件、配色の統合テスト6件、Web全体のWASM check（約43秒）、追加統合テストのrustfmt check、両リポジトリのdiff checkが成功（失敗0件）。Word実機との描画比較は未検証。番号インスタンスの `w:lvlOverride/w:lvl` によるレベル全体の上書きは、開始番号のみを扱う既存実装に差分が残る。

### 番号インスタンスのレベル上書き（2026-11-08）

- `w:lvlOverride/w:lvl` のレベル定義を読み捨てていたため、リスト固有の番号形式・表示文字・インデント・文字書式が元の抽象定義のままだった。レベル全体を保持し、番号表示とリスト編集画面の参照先に反映する。同じ抽象定義を使う別インスタンスは変更しない。
- 開始番号の扱いは後述のWord実装仕様の確認で修正した。`w:startOverride` を優先し、指定がなければ抽象定義の `w:start` を使用する。上書きレベル内の `w:start` は保持するが番号計算には使用しない。ローマ数字・箇条書き形式への置換と、番号レベルから段落スタイルへの関連付けにも対応する。
- テーマ色・下線色とテーマフォントの更新対象に上書きレベルを含める。テーマ色の統合テストを抽象定義／上書き定義の両方で実行し、保存・再読み込み、テーマ変更、Undo／Redoを確認した。
- 検証: リーダー全体631件、配色の統合テスト6件、リスト編集の単体テスト3件、Web全体のWASM check（約41秒）、追加統合テストのrustfmt check、両リポジトリのdiff checkが成功。Word実機との描画比較と、上書き定義を編集する全操作の検証は未完了。別実装の抽出専用リーダーにも上書きレベルの対応差分が残る。

### テキスト抽出の番号レベル上書き（2026-11-08）

- 抽出専用リーダーにも `w:lvlOverride/w:lvl` の保持と参照を追加した。修正前は上書きした「Article IV)」が「2.」となり、子レベルからの親番号参照と箇条書きへの変更も元定義を使っていた。回帰テスト3件すべての失敗を確認してから修正した。
- 開始番号とカウンター共有の扱いは後述のWord互換修正を適用する。別インスタンスの書式は変更せず、子レベルの `%1` にも親の上書き形式を使用する。
- 空要素と開始・終了タグの両形式を解析し、空の上書きレベルでは元の番号形式や表示文字を継承しない。これは既存の抽出フォールバックを使う場合の回帰検証であり、Word実機との空レベル表示比較は未検証。
- 検証: 抽出クレート全体19件（既存DOCXスナップショットを含む）、抽出を利用する文書差分クレートの単体テスト15件、Web全体のWASM check（約46秒）、変更ファイルのrustfmt check、両リポジトリのdiff checkが成功した。抽出側は `w:lvlRestart` を未解析で、表示側と異なり常に下位レベルをリセットする差分が残る。

### 番号リセットとWord固有の上書き動作（2026-11-08）

- [Wordの番号リセット仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/140a2cd9-2f26-456d-9760-ae6ecef4e8b5)に合わせ、抽出側も `w:lvlRestart` を解析する。0はリセットなし、省略時は直前またはそれより上位のレベルでリセット、指定時は対象またはそれより上位のレベルでリセットする。前方参照とWordの範囲外の値は無視する。
- 同仕様と[開始値のWord実装仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/8f145055-5422-4df0-933d-e67a81c633cd)により、Wordは上書きレベル内の `w:start`／`w:lvlRestart` を無視することを確認した。前回の実装はこの点で標準仕様の動作を採用していたため、表示・抽出・リスト設定画面を修正した。番号形式などの上書きは維持し、開始値・リセット条件は抽象定義を参照する。明示的な `w:startOverride` は引き続き優先する。
- 抽出カウンターは未使用／リセット済みと開始値0を区別する。同じ抽象定義のリストIDを切り替えて戻った場合も最新の共有番号から続け、親番号の参照を同期する。表示用と変更履歴を反映した確定用の両カウンターに適用する。
- 表示・抽出の両方に、リセット条件6種類、上書き内の開始値とリセットの無視、0からの開始とリストID切り替えの回帰テストを追加した。Word実機との描画比較は未完了。この時点で未解析だった抽出側の `w:isLgl` は、次節のWord互換修正で対応した。

- 検証: リーダー全体634件、抽出・文書差分の単体／統合テスト53件、リスト設定3件、テーマ統合6件の計696件が成功。Web全体のWASM check（約50秒）も成功した。

### 法的番号形式のWord互換修正（2026-11-08）

- 抽出側で `w:isLgl` を解析し、表示用・変更履歴反映後の確定用番号に適用する。空要素と開始・終了タグ、値なし／true／1／on、false／0／offを扱い、インスタンス固有レベルの設定も参照する。
- [MicrosoftのWord実装仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/0953b25e-15cc-4461-a50e-cc5d8585f6ff)では、この設定が親番号だけでなく自分自身にも適用される。表示側とリスト設定のプレビューも合わせ、法的番号形式で従来 `III.01` → `3.01` となっていた結果を `3.1` に修正した。Articleプリセットも同じ設定のため、プレビューを `Section 1.1` に揃える。
- `numFmt=none` は法的番号形式でも抑制を維持し、抽出側も通常の `none` に対応する。親の参照が空でも `%1` などの文字列を出さない。0からの開始と未使用の親番号の初期値にも対応する。
- 表示・抽出の両方に、設定値8種類、親と現在レベル、レベル上書きでの有効化／省略／無効化、noneの例外、0開始の回帰テストを追加した。修正前は追加した各2件が両クレートで失敗することを確認した。Word実機との描画比較は未完了。
- この時点で残っていた抽出側の多言語番号形式の差分は、次節の共通化で改善した。
- 検証: 修正後のリーダー全体636件、抽出・文書差分55件、リスト設定3件の計694件が成功。Web全体のWASM check（約46秒）、変更箇所のrustfmt check、両リポジトリのdiff checkも成功した。


### 表示・抽出の多言語番号形式を共通化（2026-11-08）

- 表示側の番号変換を外部依存のない `jade-number-format` クレートへ移し、リーダーと抽出の両方から利用する。従来の公開関数はリーダーから再公開し、ページ番号・脚注番号・フィールドの呼び出しも維持する。番号形式の一覧は [MicrosoftのNumberFormatValues](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberformatvalues?view=openxml-3.0.1) を参照できる。
- 抽出でも和数字・大字・全角数字・丸数字・韓国語・ベトナム語・ヒンディー語・タイ語など、表示側で対応済みの番号変換を利用する。`w:lang` を保持し、英語指定の cardinalText／ordinal／ordinalText に反映する。未知形式は十進数へフォールバックする。
- `w:numFmt/@w:format` によるカスタム形式と、対応するChoiceの後に現れるdecimalのFallbackを扱う。XML属性の文字参照も復号する。親番号参照と具体的なレベル上書きにも適用し、表示用・変更履歴反映後の確定用で同じ結果を返す。
- 回帰テスト3件は修正前に全件失敗した。15種類の番号形式、0開始、言語4種類、カスタム形式と親参照を検証する。既存DOCXのスナップショット3件は、元文書のdecimalFullWidth指定に合う「第１条」「（１）」などへ更新した。本文などの内容変更はない。
- 検証: 共通変換・リーダー・抽出・文書差分の単体／統合テスト計770件が成功。Web全体のWASM check（約55秒）、変更ファイルのrustfmt check、両リポジトリのdiff checkも成功した。下流の各Cargo.lockに共通クレートを反映した。
- 限界: Word実機との全形式の比較は未完了。英語以外の言語依存cardinalText／ordinalText等は既存の十進数フォールバックを維持する。また、番号文字列が正しくてもWebでフォントが不足する文字の描画差は残る。

### リスト設定の言語・カスタム番号の保持（2026-11-08）

- リスト設定を読み書きする編集モデルが `w:rPr/w:lang` を捨てていたため、番号位置だけを編集しても英語の文字番号などが十進数に変わり得た。言語指定を保持し、階層ごとの番号プレビューにも渡す。
- `mc:AlternateContent` 内のカスタム番号形式を読み取り、十進数のフォールバックで上書きしない。保存時は内部表現の `custom:` をそのまま `w:val` に出さず、`w:val="custom"` と `w:format` に戻す。[NumberingFormatの仕様](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingformat?view=openxml-3.0.1)を参照。
- アウトラインのプレビューに各レベルの開始番号を反映する。英語の親番号、日本語指定の子番号、ゼロ埋めカスタム番号を組み合わせた定義を保存し、実際のリーダーが生成する番号との一致を検証する。
- コア側と、ネイティブ／Web画面が参照する共通モデルの両方に適用する。共通モデルに残っていた正規形式番号のプレビューも、現レベルを含めて十進数にする既存の表示処理に揃える。
- 検証: コアのリストテスト6件、画面側共通モデルのリストテスト5件が成功。言語・カスタム形式の新規テスト2件は修正前の失敗を確認した。Web全体のWASM check（約53秒）、変更関数のrustfmt check、両リポジトリのdiff checkも成功した。
- 限界: 言語指定のうち今回保持するのは `w:lang/@w:val`。番号の文字書式全体の無損失編集、未対応カスタム形式、Word実機との全形式・描画比較は未完了。画面側に残っていたリスト固有レベル上書きの参照差は、後述の修正で対応した。


### リスト固有の上書き定義の参照・編集（2026-11-08）

- ネイティブ／Web画面の共通モデルにも `w:lvlOverride/w:lvl` の参照を反映した。コア側の対応だけでは画面に反映されず、上書きした番号形式・番号文字・配置・インデントが編集画面で元の抽象定義に戻っていた。既存のコア回帰テストを共通モデルで実行し、修正前の失敗を確認した。
- 番号形式の変更とインデント等のレベル編集では、対象リストに完全なレベル上書きがあればそこを更新する。共通の抽象定義と、同じ定義を使う別リストを変更しない。開始番号だけの `w:startOverride` は完全なレベル上書きとして扱わず、番号値を保持する。
- 完全なレベル上書きがないスタイル参照リストでは、`w:numStyleLink` が指す実体の定義を編集する。参照だけの空の抽象定義にレベルを追加して表示に反映されなくなる問題を防ぐ。これらの編集修正はコアと画面共通モデルの双方に適用した。
- [Wordの開始値仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/8f145055-5422-4df0-933d-e67a81c633cd)と[リセット仕様](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/140a2cd9-2f26-456d-9760-ae6ecef4e8b5)に合わせ、上書きレベル内の `w:start`／`w:lvlRestart` を表示設定の参照には使わず、抽象定義の値を維持する。
- 検証: コアと画面共通モデルのリストテスト各8件、計16件が成功。保存したカスタム番号を実際のリーダーで再計算し、別リストと開始番号を維持するケース、スタイル参照とレベル上書きが共存するケースを含む。Web全体のWASM check（約35秒）、変更関数のrustfmt check、両リポジトリのdiff checkも成功した。
- 限界: Word実機との操作比較、リスト編集の全操作に対する保存・Undo／Redo検証、未対応の番号文字書式全体の無損失編集は引き続き未完了。


### リストのレベル編集で既存の文字・段落書式を保持（2026-11-08）

- 既存レベルの番号形式やインデントを編集するとレベル全体を再生成し、編集対象外の文字色・太字・文字サイズ・タブ位置・段落間隔などが失われていた。変更したプロパティだけを元のXMLへ反映するよう、コアと画面共通モデルの両方を修正した。変更がない場合は元のXMLツリーをそのまま保持する。
- 抽象定義とリスト固有のレベル上書きの双方に適用する。フォントと言語の変更では東アジア・複合文字用の設定を保持し、インデント変更では右インデント等を保持しつつ競合する文字単位の指定を除去する。負のぶら下げ値は先頭行インデントとして保存する。
- 互換用Choice／Fallback内の番号形式も更新し、その他の拡張要素を保持する。カスタム番号のFallbackは十進数とする。
- 検証: 新規回帰テスト各6件を追加し、うち各4件は修正前の失敗を確認した。コアと画面共通モデルのリストテスト各14件、計28件が成功。Web全体のWASM check（約34秒）、変更関数のrustfmt check、両リポジトリのdiff checkも成功した。
- 限界: この時点の保持処理は既存レベルの編集コマンドが対象。定義ダイアログでの再生成については次節で改善した。Word実機との操作・描画比較、保存とUndo／Redoの全操作検証は未完了。


### 定義ダイアログで番号の文字・段落書式を引き継ぐ（2026-11-08）

- 「新しいアウトラインの定義」は既存リストの設定を読み込んで新しいリストを生成するが、その過程で番号の文字色・太字・文字サイズ、タブ位置などモデル化していない設定を捨てていた。読み込んだ定義に元のレベルXMLを保持し、新規定義の保存時にも変更項目だけを反映する。リストスタイルの作成も同じ生成処理を使う。
- リスト固有の完全なレベル上書きがある場合は、その上書き側の文字・段落書式を引き継ぐ。上書きレベル内でWordが無視する開始番号とリセット指定は、抽象定義から得た有効値へ正規化する。元のリスト定義は変更しない。
- 定義の比較は従来どおり編集可能な値で行い、保持用XMLは比較に含めない。既存のライブラリ形式は保持元XMLなしで生成できる。
- 新規回帰テストは各クレート2件。既存定義を読み込み、番号文字とインデントを変えて新しいリストへ追加しても元の書式が残ること、固有の上書き側の色と有効な開始・リセット設定が生成XMLに反映されることを検証する。
- 検証: コアと画面共通モデルのリストテスト各16件、計32件が成功。Web全体のWASM check（約40秒）、変更箇所のrustfmt check、両リポジトリのdiff checkも成功した。
- 限界: 保持対象はレベル内のXML。抽象定義自体の未モデル化メタデータ、別文書への移送時の画像・名前空間・スタイル参照の再割り当て、Word実機との比較、全操作のUndo／Redo検証は未完了。


### 開始番号の変更でリスト固有の書式と他階層の上書きを保持（2026-11-08）

- 「番号の設定」「新しいリストを開始」は選択位置以降を新しいリストIDに移すが、従来は抽象定義の参照と変更対象の開始値だけを生成していた。このため既存の `w:lvlOverride/w:lvl` にある番号文字・文字書式・配置や、他階層の `w:startOverride` が消えていた。
- 元リストのレベル上書きを丸ごと複製し、指定階層の開始値だけを更新する処理に変更した。元リストの定義と上書きは変更しない。指定階層に上書きがない場合は新しく追加し、既存の別階層は維持する。コアと画面共通モデルの双方へ適用した。
- 回帰テストでは3階層分の上書き、番号の文字・段落書式、別階層の開始値、繰り返しの開始値変更、無効なリスト参照時の非変更を検証する。保存XMLを実際のリーダーで読み、開始番号「3.」と固有形式「(a)」を確認する。主要な回帰テストは修正前に上書き件数が3件から1件に減って失敗することを確認した。
- 検証: コアと画面共通モデルのリストテスト各18件、計36件が成功。Web全体のWASM check（約34秒）、変更箇所のrustfmt check、両リポジトリのdiff checkも成功した。
- 限界: Word実機との操作比較、段落選択範囲と継続番号の全組み合わせ、開始値変更の保存・Undo／Redoを通した検証は未完了。


### 書式付きリストの保存・Undo／Redoを実編集で検証（2026-11-08）

- ブラウザ版の `EditableDocument` を使い、リスト固有のレベル上書きに太字・文字色・文字サイズ・番号タブを持つDOCXを開いて編集する統合テストを追加した。XML編集関数の単体テストとは別に、実際の編集コマンド、番号計算、DOCX保存・再読込、履歴操作を通す。
- 開始番号変更では、元のリストを残したまま後続段落を新しい開始値へ移し、対象階層の書式と他階層の開始値を保持することを確認する。定義ダイアログ相当の操作では、番号形式と開始値を変えた新しい定義にも文字・段落書式を引き継ぐことを確認する。
- 各操作のUndoで操作前、Redoで操作後のDOCXバイト列へ戻ること、番号表示がその都度一致すること、保存して開き直したXMLにも書式が残ることを検証する。
- 既存の統合テストに残っていた法的番号付けの期待値も更新した。`isLgl` は現在階層も十進数へ変換するため、`Section 1.01` ではなく `Section 1.1` を期待する。番号表示の実装は既に対応済みで、今回の修正はテストと画面共通モデルの説明コメント。根拠: [MicrosoftのisLgl仕様](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.islegalnumberingstyle?view=openxml-3.0.1)。
- 検証結果: `jade-web` の `lists_styles_documents` 統合テスト全17件が成功（新規2件、既存15件）。追加コードのrustfmt check、両リポジトリのdiff checkも成功。今回はテストと説明コメントのみの変更で、実装変更はない。
- 限界: 今回の対象は単一階層にある連続段落。混在階層・表内・スタイル経由の番号付け、編集後の入力を挟む履歴操作、Word実機との比較は別途必要。Wordとの完全互換は未達。


### 法的番号付けの明示的なオフを正しく読み込む（2026-11-08）

- `w:isLgl w:val="off"` を、描画側はオフ、編集画面のモデルはオンとして扱っていた。コアと画面共通モデルの読み込みを修正し、チェック状態・番号プレビュー・編集時の比較を一致させた。`off` は `false`・`0` と同じオフ値として扱う。根拠: [Microsoft OnOffValue](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.onoffvalue?view=openxml-3.0.1)。
- 回帰テストは各クレート2件。要素省略、属性省略、オン／オフ各3表記を基本定義と完全なレベル上書きで確認し、文書リーダーの解釈とモデルのプレビューを比較する。基本定義がオンでも上書き側でオフにできること、既にオフならXMLを変更しないこと、別項目の編集でも明示的なオフと文字色を保持すること、オンにしてから再びオフに戻せることも検証する。
- 修正前は画面共通モデルの新規2件が失敗することを確認。修正後はコアと画面共通モデルのリストテスト各20件、計40件が成功。Web全体のWASM check（約33秒）、変更箇所のrustfmt check、両実装の該当コード一致確認、両リポジトリのdiff checkも成功した。
- 限界: Word実機でのダイアログ操作比較、リスト以外の全On/Off属性の監査は未完了。Wordとの完全互換は未達。

### 読み込んだスタイルのオン／オフ切り替えを修正（2026-11-08）

- 明示的なオフ（`0`・`false`・`off`）を持つスタイルで、クイックスタイル・自動更新・「次の段落と分離しない」をオンに変更しても、元のオフ属性が残る問題を修正した。コアと画面共通モデルに適用。
- 「次の段落と分離しない」をオフにするときは `w:keepNext w:val="0"` を保存する。要素を削除すると親スタイルのオンを継承するため、明示的なオフと区別した。要素省略時の継承は [Microsoft KeepNext](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.keepnext?view=openxml-3.0.1) に記載されている。
- 各クレートに回帰テスト1件を追加。オフの3表記それぞれについて、オン→オフ→オンと変更してXML保存・再読み込みを行い、3設定の値、親スタイルの指定と設定、文字色、段落後の間隔を検証する。修正前に新規テストが失敗することを確認した。
- 修正後は画面共通モデルのスタイルテスト5件、コアのスタイルテスト7件、計12件が成功。Web全体のWASM check（約32秒）、変更箇所のrustfmt check、両実装の変更部分の一致確認、両リポジトリのdiff checkも成功した。
- 限界: 実際のWordでの操作比較、改ページ位置の描画比較、スタイル変更のUndo／Redoを含むUI操作試験は未実施。Wordとの完全互換は未達。

### 段落の文書グリッド設定の読み込みを修正（2026-11-08）

- 段落の `w:snapToGrid` について、設定が段落プロパティの唯一の項目なら無視する問題、開始・終了タグ形式を無視する問題、`off` をオンと誤読する問題を修正した。既存の段落On/Offパーサーを共有し、必要な場合に段落プロパティを作成する。
- 段落記号や通常の文字列の `w:rPr/w:snapToGrid` は段落設定を上書きしないよう区別した。段落側は行間、文字列側は字間に関わる別設定である。根拠: [Microsoft SnapToGrid](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.snaptogrid?view=openxml-3.0.1)。
- 回帰テスト2件を追加。属性省略・オン／オフの各3表記とXMLの両要素形式（14通り）、文字列側の設定が段落の明示的なオフや省略値に漏れないこと（4通り）を確認。修正前に新規読み込みテストが失敗することを確認した。従来テストに残っていた未対応動作の期待値も更新した。
- リーダーのライブラリテスト全634件、Web全体のWASM check（約34秒）、変更したヘルパーと新規テストのrustfmt check、両リポジトリのdiff checkが成功した。
- 限界: Word実機との行位置・ページ数の描画比較、文字列単位のグリッド字間の完全対応は未検証。Wordとの完全互換は未達。


### 文字単位の文書グリッド設定を反映（2026-11-08）

- `w:rPr/w:snapToGrid` を文字プロパティへ追加し、通常の文字列・段落記号・スタイル・文書既定値・番号定義から読み込む。段落の同名設定とは区別する。明示的なオフを継承・保存・入力時の書式引き継ぎで保持し、書式クリア時には解除する。
- オフの文字には文書グリッドの追加字間やセルへのスナップを適用せず、明示的な文字間隔は維持する。折り返しと描画で同じ指定を使い、書式キャッシュにもオン／オフを含める。根拠: [Microsoft SnapToGrid](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.snaptogrid?view=openxml-3.0.1)。
- 回帰テストでは、On/Off各表記と両XML要素形式、スタイル継承、番号定義、オン／オフ混在時の英数字と日本語の幅、Mac／Windowsのグリッド方式、文字間隔の保持、描画キャッシュの区別、オフ→オン→オフのXML保存を確認する。
- 初回の全体検証はリーダー636件・レイアウト723件が成功（レイアウト12件は既存のignore）。追加修正後は関連8件（リーダー4件・レイアウト2件・エディター2件）とWeb全体のWASM check（約48秒）が成功。両リポジトリのdiff checkも成功した。
- 限界: 実際のWordとの文字位置・改行位置・ページ数の比較、編集画面からの設定変更のUI操作試験は未実施。Wordとの完全互換は未達。


### 段落グリッドのオン指定を保存時にも保持（2026-11-08）

- ページ設定から段落のグリッドをオンにすると、従来は内部値と `w:snapToGrid` を削除していた。親スタイルがオフの場合、画面上はオンでも保存・再読み込み後は再びオフになっていた。内部値を `Some(true)`、XMLを `w:val="1"` として明示的に保持する。オフは従来どおり `w:val="0"`。
- 要素の省略はスタイルの設定を継承するため、オンへの変更には明示的な指定が必要。根拠: [Microsoft SnapToGrid](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.snaptogrid?view=openxml-3.0.1)。
- Webの文書操作を通す回帰テストを追加し、修正前にオン指定が保存XMLから欠落して失敗することを確認した。スタイルでオフにした2段落の一方だけをオンにし、保存・再読み込み、Undo／Redo、再度のオフ、未選択段落と既存の `keepNext` の保持を検証する。
- 検証結果: ページ設定の統合テスト全11件（新規1件）が成功。Web全体のWASM check（約41秒）と両リポジトリのdiff checkも成功した。
- 限界: Word実機での設定操作と行位置・ページ数の描画比較は未実施。Wordとの完全互換は未達。

### 番号・箇条書きの太字と斜体のオフ指定を保持（2026-11-08）

- 番号定義の `w:lvl/w:rPr` にある `w:b` と `w:i` は、従来は属性値を読まず常にオンとしていた。本文と同じOn/Offパーサーを使い、`0`・`false`・`off` を明示的なオフとして保持する。省略・`1`・`true`・`on` はオン。
- 番号側のオフ指定は段落の太字・斜体より優先され、未指定の文字サイズなどは段落から引き継ぐ。明示的な `false` が太字を解除する例は [Microsoft Bold](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.bold?view=openxml-3.0.1) に記載されている。
- 回帰テストで7種類の属性指定と2種類のXML要素形式（14通り）、段落プロパティからの設定漏れがないこと、段落側が太字・斜体の場合の番号側のオフを確認する。読み込みの新規テストは修正前に失敗することを確認した。
- 検証結果: リーダーのライブラリテスト全638件、番号書式の描画用マージの回帰テスト1件、Web用ライブラリのWASM checkが成功。初回のWASM検査はネイティブ専用の比較ツールも対象にしたため失敗し、`--lib` で対象を指定し直した。
- 限界: Word実機との番号の字形・配置の描画比較は未実施。複合文字用の太字・斜体を含む番号書式全般の完全対応を示すものではなく、Wordとの完全互換は未達。

### 番号・箇条書きの文字幅と上下位置を反映（2026-11-08）

- 番号定義の `w:w`（文字幅倍率）、`w:position`（上下位置）、`w:kern`（カーニング閾値）が読み捨てられていた。本文と共通の数値パーサーを使い、通常の番号定義と `lvlOverride` の両方から読み込む。位置と閾値の半ポイントをポイントに変換し、位置と閾値のゼロ、倍率100％の明示的な解除値も保持する。非有限値・不正な値は採用しない。
- 番号定義に指定がなければ段落側の文字書式を継承し、指定があれば番号側を優先する。既存の文字幅・基準線・カーニング処理まで値を引き渡す。根拠: [Microsoft CharacterScale](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.characterscale?view=openxml-3.0.1)、[Microsoft Position](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.position?view=openxml-3.0.1)。
- 読み込みの新規テストが修正前に3設定とも欠落して失敗することを確認した。開始・終了タグ形式と空要素形式、通常定義と段落ごとの上書き、解除値、不正値、段落プロパティへの設定漏れがないことを検証する。
- 描画テストでは50％・100％・継承した200％の番号幅、上下位置の指定とゼロによる解除、カーニング閾値の継承と解除、キャッシュ再描画時の拡縮率と基準線を確認する。
- 検証結果: リーダーのライブラリテスト全639件、エディターの番号関連テスト全61件、Web用ライブラリのWASM check（約35秒）、両リポジトリのdiff checkが成功。
- 限界: Word実機との字形・文字位置・改ページの描画比較、実フォントでのカーニング差の比較は未実施。Wordとの完全互換は未達。

### 番号・箇条書きの取り消し線と上付き・下付き（2026-11-08）

- 番号定義の `w:strike`・`w:dstrike`・`w:vertAlign` が読み捨てられていた。単一・二重取り消し線のオン／オフ、上付き・下付き・通常の基準線を読み込み、既存の番号描画に渡す。二重取り消し線は段落側からの継承にも対応する。明示的なオフと `baseline` は継承値を解除する。
- 根拠: [Microsoft DoubleStrike](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.doublestrike?view=openxml-3.0.1)、[Microsoft VerticalTextAlignment](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.verticaltextalignment?view=openxml-3.0.1)。
- 読み込みの回帰テストは修正前に取り消し線の欠落で失敗することを確認した。7種類のOn/Off指定、開始・終了タグと空要素、上付き・下付き・通常の基準線、番号レベルの上書き、段落プロパティからの混入防止を検証する。描画側では継承と解除、基準線の移動、取り消し線の本数、キャッシュ用描画命令とその再生を確認する。
- 検証結果: リーダーのライブラリテスト全640件、エディターの `numbering_` に一致するテスト全56件、Web用ライブラリのWASM check（約59秒）、両リポジトリのdiff checkが成功。WASM検査は初回の誤ったマニフェストパスを修正して実行した。
- 限界: Word実機との字形・位置の描画比較は未実施。番号書式全般の完全対応やWordとの完全互換を示すものではない。

### 番号・箇条書きの大文字表示を反映（2026-11-08）

- 番号定義の `w:caps` が読み捨てられ、生成した小文字の番号やラベルが大文字表示にならなかった。本文と共通のOn/Off読み込みと表示変換を使い、未指定なら段落から継承し、明示的なオフなら解除する。元の番号定義・文字列は保持する。
- 幅の計算、描画、キャッシュ用描画命令、補助ストーリーの記録に同じ大文字の文字列を使う。単語下線の範囲とMac用番号のアセント計算にも表示変換を適用する。`ß → SS` のように文字数が増える変換も幅と描画の不一致を起こさない。根拠: [Microsoft Caps](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.caps?view=openxml-3.0.1)、[Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1)。
- 読み込みテストは修正前にオン値が欠落して失敗することを確認した。7種類のOn/Off値、空要素と開始・終了タグ、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画側はオン・オフ・継承について、幅、文字数の増加、空白区切り、単語下線、キャッシュ再生、元の番号定義の保持を検証する。
- 検証結果: リーダーのライブラリテスト全641件、エディターの `numbering_` に一致するテスト全57件、最終版のWeb用ライブラリのWASM check（約30秒）、両リポジトリのdiff checkが成功。描画テストの共有スタイル複製に関する記述ミスを修正して再実行した。
- 限界: 小型英大文字（`w:smallCaps`）の番号描画は下記の追加対応を参照。Word実機との描画比較は未検証であり、Wordとの完全互換は未達。

### 番号・箇条書きの小型英大文字を反映（2026-11-08）

- 番号定義の `w:smallCaps` を読み込み、未指定時の段落からの継承と明示的な解除を反映する。抽象番号と個別番号のレベル上書きの双方を対象とする。
- 小文字だった部分だけを大文字に変換して2ポイント縮小し、大文字・数字・句読点・後続空白は元のサイズで描画する。最低サイズは0.5ポイント。本文も従来の一律75%縮小から同じサイズ計算に変更し、上付き・下付きのサイズ調整前に適用する。根拠: [Open XML SmallCapsの仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.smallcaps?view=openxml-3.0.1)。元の文字列と番号定義は保持する。
- 番号の幅計算、描画コマンド、キャッシュ再描画、単語下線、補助ストーリーの文字・グリフ範囲、空段落の行高に部分ごとのフォントサイズを反映する。`w:caps` が同時指定された場合は既存本文と同様に大文字表示を優先する。
- 回帰テストはOn/Offの7表記と空要素／開始終了要素、最小サイズ、8／12／24ポイント、継承・解除・大文字指定との組み合わせ、Unicodeの複数字への大文字化、実描画とキャッシュ再描画、単語下線、補助ストーリー、空段落の行高、本文の上付き・下付き文字を対象とする。
- 検証: 読み込み全642件、番号関連59件、小型英大文字関連4件のテストが成功。Web版WASMのライブラリビルド検査と差分の空白検査も成功。追加した本文テストの型指定誤りは修正して再実行した。
- 限界: Word実機との描画比較は未検証。小型英大文字の対応をもってWordとの完全互換とは判断しない。

### 番号・箇条書きの複合文字用書式を反映（2026-11-08）

- 番号定義の `w:bCs`・`w:iCs`・`w:szCs` を読み込み、アラビア文字やヘブライ文字用の太字・斜体・文字サイズを通常文字の書式とは独立して保持する。通常レベルと個別番号のレベル上書きの双方に対応する。根拠: [Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1)、[Microsoft FontSizeComplexScript](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.fontsizecomplexscript?view=openxml-3.0.1)。
- 未指定の項目は段落側から継承し、番号側で明示したオフやサイズは優先する。半ポイントのサイズをポイントに変換して既存の複合文字描画に渡す。
- 読み込みの追加テストは修正前に欠落して失敗することを確認した。On/Offの7表記、空要素と開始終了要素、通常文字との独立性、段落プロパティからの混入防止を検証する。描画側はラテン文字とヘブライ文字／アラビア文字が混ざるラベルについて、継承・解除・サイズ、文字幅、実際の描画命令とキャッシュ再描画を検証する。
- 検証: 読み込み全643件、番号関連60件のテストと、Web版WASMのライブラリビルド検査（約36秒）が成功。
- 限界: Word実機との字形・位置の描画比較は未検証。Wordとの完全互換は未達。

### 番号・箇条書きの言語指定を文字描画へ渡す（2026-11-08）

- 番号定義の `w:lang/@w:val` は番号形式の生成用にだけ保持されていた。文字描画用の `RunProperties.language` にも保持し、読み捨てられていた `w:eastAsia` は独立した `east_asia_language` に保持する。通常の番号レベルと個別番号のレベル上書きの双方に対応する。属性の意味と未指定時の継承は [Microsoft Languages](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.languages?view=openxml-3.0.1) を参照。
- 番号側が指定しない言語の属性は段落側から個別に継承する。これにより、既存のMac向け日本語フォント選択など、言語を使う描画処理まで番号の言語指定が届く。
- 読み込みの追加テストは修正前に文字書式の欠落で失敗することを確認した。言語2属性の独立性、空要素と開始終了要素、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画テストは日本語と中国語の指定・継承について、ラテン文字と漢字が混ざる番号の実際のフォント、幅計算と描画の一致、キャッシュ再描画を検証する。
- 検証: 読み込み全644件、番号関連61件のテスト、Web版WASMのライブラリビルド検査（約36秒）、両リポジトリの差分の空白検査が成功。
- 限界: 今回の変更についてWord実機との描画比較は未実施。スペル・文法チェックを含む言語機能全般の互換性を示すものではなく、Wordとの完全互換は未達。

### 番号・箇条書きの影・輪郭・浮き出し・彫り込み（2026-11-08）

- 番号の文字プロパティとして許可されている `w:shadow`、`w:outline`、`w:emboss`、`w:imprint` を読み込むようにした。通常の番号レベルと個別番号のレベル上書きの双方に対応する。許可される要素は [Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1) を参照。
- 未指定の効果は段落側から継承し、番号側の明示的なオフは維持する。既存の文字効果の描画処理へ渡すことで、通常描画とキャッシュ再描画の双方に反映する。
- 読み込みの追加テストは修正前にプロパティの欠落で失敗することを確認した。4効果それぞれのOn/Offの7表記、空要素と開始終了要素、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画テストは指定・継承・解除、混在文字の幅、塗りと輪郭の描画命令、影の色と方向、描画後の影の解除、キャッシュ再描画を検証する。
- 検証: 読み込み全645件、番号関連62件のテスト、Web版WASMのライブラリビルド検査（約41秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との描画比較は未実施。浮き出し・彫り込みは既存の影による近似描画を使用し、Wordと同じ画素表現を保証しない。Wordとの完全互換は未達。

### 番号・箇条書きの傍点を読み込み・描画（2026-11-08）

- 番号の文字プロパティに許可される `w:em` を読み込むようにした。通常レベルと番号ごとの上書きレベルに対応し、本文と同じ正規化処理を使用する。許可される要素は [Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1) を参照。
- 未指定時は段落側の傍点を継承し、`none` は継承を解除する。番号の文字のインク範囲と文字送りを使って本文と同じ傍点描画へ渡し、描画命令をキャッシュにも残す。傍点のための縦方向の計測は描画時に行い、番号幅の計測は従来どおり文字送りだけを使う。
- 読み込みテストは修正前に `dot` の欠落で失敗することを確認した。5種類の指定と空・省略属性、空要素と開始終了要素、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画側は4種類の傍点と継承・解除、空白を飛ばすこと、色・上下位置・塗りの指定、幅計算と描画の一致、通常描画とキャッシュ再描画の円の位置一致を検証する。
- 検証: 読み込み全646件、番号関連63件のテスト、Web版WASMのライブラリビルド検査（約62秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との描画比較は未実施。既存の傍点処理では `comma` を丸で近似しており、言語ごとの字形差も未検証。Wordとの完全互換は未達。

### 番号・箇条書きの文字の網かけ（2026-11-08）

- 番号の文字プロパティ `w:shd` を通常レベルと番号ごとの上書きレベルで読み込む。許可される要素は [Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1) を参照。
- 番号側で未指定なら段落側の網かけを継承し、空の網かけ指定は継承を解除する。番号描画では本文の装飾処理を通らないため、文字より先に背景を描き、キャッシュにも同じ矩形と色を保存する。文字の縦方向の範囲とフォントのサイズから背景の高さを取り、番号の幅計算は変更しない。
- 読み込みテストは修正前にプロパティ欠落で失敗することを確認した。4種類のトークンと色、空要素と開始終了要素、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画テストは指定・継承・解除、単色と濃度指定、背景が文字より先に描かれること、幅と縦範囲、初回描画とキャッシュ再描画の矩形一致を検証する。
- 検証: 読み込み全647件、番号関連64件のテスト、Web版WASMのライブラリビルド検査（約40秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との描画比較は未実施。網かけのテーマ色の読み込み、縞模様のブラシによる描画、`nil` の色付き指定の扱いは既存の本文と共通の課題として残る。番号の囲み線は別途対応が必要。Wordとの完全互換は未達。

### 番号・箇条書きの文字の囲み線（2026-11-08）

- 番号定義の `w:bdr` を通常レベルと番号ごとの上書きレベルで読み込む。未指定時は段落側から継承し、`nil`／`none` の指定は枠を解除する。番号の文字プロパティに含まれることと、同一属性の文字範囲を一つの枠として扱うことは [Microsoft NumberingSymbolRunProperties](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.numberingsymbolrunproperties?view=openxml-3.0.1)、[Microsoft Border](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.border?view=openxml-3.0.1) を参照。
- 線幅と `w:space` の余白を左右に確保し、番号幅の計測と描画を一致させる。右揃えは枠を含む幅で配置する。スモールキャップスの文字サイズが分かれる場合も余白を重複させず、各文字サイズの縦方向の範囲を覆う一つの枠を描く。既存の本文用の線種描画を使い、キャッシュにも枠を保存する。
- 読み込みテストは修正前にプロパティの欠落で失敗することを確認した。5種類の線種・解除指定、色・線幅・余白、空要素と開始終了要素、通常レベルと上書きレベル、段落プロパティからの混入防止を検証する。描画側は指定・継承・解除、通常文字とスモールキャップス、右揃え、左右の余白が一度だけ加算されること、四辺の位置・太さ、初回描画とキャッシュ再描画の矩形一致を検証する。
- 検証: 読み込み全648件、番号関連65件のテスト、Web版WASMのライブラリビルド検査、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との描画比較は未実施。囲み線による段落の行高・ページ分割への影響、テーマ色、網かけと枠の余白の組み合わせ、細線のピクセル丸めの一致は追加検証が必要。Wordとの完全互換は未達。

### 空の番号付き段落の囲み線を行高に反映（2026-11-08）

- 空の番号付き段落で、番号の囲み線の太さと余白を上下の文字範囲に加えるようにした。通常文字とスモールキャップスの両方を対象とし、`nil`／`none` の枠解除では高さを増やさない。
- 枠を含む自然な高さを改ページ判定の前に求め、自動行間の行高・次の段落の開始位置・ページ末尾での繰り越しに反映する。既存の比例行間・行グリッドの計算を使い、固定行間は拡張しない。行間の指定方法は [Microsoft SpacingBetweenLines.Line](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.line?view=openxml-3.0.1) を参照。
- 追加テストは修正前に枠の上下余白の欠落と自動行高の不足で失敗することを確認した。文字のサイズ別計測とフォントの行ボックス別計測、枠解除、後続段落との重なり防止、ページ末尾での繰り越し、固定行間の維持を検証する。
- 検証: 番号関連67件のテスト、Web版WASMのライブラリビルド検査（約29秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との比較は未実施。本文のある番号付き段落の行高・改ページには別の計測経路があり、今回の修正対象外。番号以外の文字の囲み線との基準統一や細線の丸めも追加検証が必要。Wordとの完全互換は未達。

### 本文のある番号付き段落の囲み線を行高に反映（2026-11-08）

- 本文のある箇条書きでも、番号の囲み線を含む上側・下側の範囲を段落の計測情報に渡す。番号が載る最初の物理行だけの行高・ベースライン・改ページに反映し、後続行は拡張しない。本文のフォント計測が得られない場合は既存の行高を下限として使用する。
- 浮動オブジェクトの左右に分かれた行は一つの物理行として両側に同じ枠の高さを確保する。浮動オブジェクトの先読みで使う段落全体の高さも同じ範囲を使用する。途中から再開する段落には番号の枠の予約を繰り返さない。固定行間では番号の枠を理由に行高を拡張しない。
- 描画側の追加テストはページあり／なしと自動／固定行間の組み合わせで、最初の行の拡張、次行との重なり防止、後続行の高さ、ページ末尾での繰り越しを検証する。共通レイアウト側ではWindows／Macの計測設定と行グリッドの有無、浮動オブジェクトの左右の行高と段落全体の高さを検証する。
- 検証: 共通レイアウト724件（既存の12件は無視）、番号関連68件のテスト、Web版WASMのライブラリビルド検査（約31秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との比較は未実施。縦書きの番号描画、途中再開と枠の組み合わせの専用回帰テスト、枠と網かけの余白の組み合わせ、細線のピクセル丸めは追加検証が必要。Wordとの完全互換は未達。

### 網かけの `nil` をWordの実装に合わせる（2026-11-08）

- 網かけの `w:val="nil"` を `solid` と同じ前景色優先に変更した。前景色と背景色が両方残っている文書では、従来は背景色を描いていた。Microsoftの実装注記は、標準では `nil` と `clear` は同じ意味だが、Wordは `nil` を `solid` として扱うと明記している（[MS-OE376 §2.1.586](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/fabbed98-c314-46a1-8580-2bad718be974)）。`nil` を一律に透明にする変更は採用しない。
- 共通の背景色解決処理を変更したため、本文・段落・表・番号の同処理を使う経路に適用される。値を消さずに保持し、スタイルの継承判定は維持する。`solid` の前景色省略時に背景色へ代替する既存の処理も維持する。
- 追加した回帰テストでは修正前に `#123456` の前景色ではなく `#ABCDEF` の背景色が返ることを確認した。前景色と背景色の有無、大文字表記、番号の実描画とキャッシュ再描画、スタイル継承時の色付き `nil` を検証する。
- 検証: 読み込み725件（単体649件と統合76件）、描画・編集の網かけ関連13件、Web版WASMのライブラリビルド検査（約43秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との比較は未実施。自動色の厳密な解決、テーマ色、網かけパターンのブラシ描画、番号の囲み線との余白の組み合わせは引き続き検証が必要。Wordとの完全互換は未達。

### 網かけの小数の濃度指定（2026-11-08）

- `pct12`・`pct37`・`pct62`・`pct87` を、それぞれ12.5%・37.5%・62.5%・87.5%として扱うようにした。従来はトークンの数字を整数の濃度として解釈していた。各値の意味は [Microsoft ShadingPatternValues](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shadingpatternvalues?view=openxml-3.0.1) を参照。
- 共通の色解決処理で0.5%単位の整数計算を使い、各色チャンネルの丸めを維持する。本文・段落・表・番号で同処理を使う経路に適用される。
- 修正前に12.5%の黒と白の組み合わせが `#DFDFDF` ではなく `#E0E0E0` になることを回帰テストで確認した。4種類すべての期待色、任意のRGB値の混合、番号の初回描画とキャッシュ再描画の色を検証する。
- 検証: 読み込み726件、描画・編集の網かけ関連13件、Web版WASMのライブラリビルド検査（約47秒）、両リポジトリの差分の空白検査が成功。
- 限界: 網かけは平均色による近似であり、Wordと同じパターンのブラシ描画ではない。Word実機との比較、自動色・テーマ色、番号の囲み線との余白の組み合わせも引き続き検証が必要。Wordとの完全互換は未達。

### 段落・文字・番号の網かけのテーマ色（2026-11-08）

- `w:shd` の前景 `themeColor` と背景 `themeFill`、それぞれの `Tint`・`Shade` を読み込み、文書テーマと色の割り当てからRGBへ解決する。テーマ参照を解決できる場合はキャッシュ済みの `color`・`fill` より優先し、解決できない場合は保存済みRGBへ戻る。属性の定義は [Microsoft Shading](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shading?view=openxml-3.0.1)、濃淡の扱いは [MS-OI29500 §2.1.144](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/4f0cccae-70c2-492a-bc94-3ccc39172119) を参照。
- 元の6属性を解決後のRGBとともに保持し、段落の編集・再保存と表紙の段落再生成にも引き継ぐ。テーマやキャッシュ済みRGBがない参照も捨てない。文字データの増加を抑えるため、テーマ参照はボックス化する。
- 修正前に、テーマの `accent1` を暗くする指定が無視され、`#2F5496` ではなくキャッシュ済みの `#FF0000` になることを回帰テストで確認した。属性順序、濃淡、Tint優先、テーマの割り当て、段落・文字・スタイル継承と直接指定、番号の初回描画とキャッシュ再描画、段落の再読込・undo/redo、表紙の属性保持を検証する。
- 検証: 読み込み730件（単体654件と統合76件）、描画・編集の網かけ関連15件、Web版WASMのライブラリビルド検査（約37秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との画像比較と、読み込み後のテーマ切替は未検証。表セル・表スタイルの背景には別の読み込み経路があり、今回の対応には含めない。自動色の厳密な解決と網かけパターンのブラシ描画も引き続き差分として残る。Wordとの完全互換は未達。

### 表セル・表スタイルの網かけのテーマ色（2026-11-08）

- 表の `tblPr`、行例外の `tblPrEx`、セルの `tcPr`、表スタイルの `tcPr` にある `w:shd` を、段落と共通の網かけ解決処理で読み込む。`themeFill`・`themeColor` と濃淡をテーマから解決し、キャッシュ済みRGBより優先する。単色の前景指定と百分率パターンの平均色も反映する。
- 表スタイルの基底スタイルからの継承、先頭行・縞模様の条件付き書式にも適用する。セルが網かけを省略した場合は表スタイルから継承し、表と行例外の網かけはセル間の余白だけに使う既存の優先順位を維持する。[MS-OI29500 §2.1.144](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/4f0cccae-70c2-492a-bc94-3ccc39172119) を参照。
- 修正前の回帰テストで、テーマの濃淡による `#2F5496` ではなく古いキャッシュ値 `#FF0000` が残ることを確認した。開始・終了タグと空要素、表・行例外・セル、テーマ参照だけの単色、百分率パターン、表スタイル継承と条件付き書式を検証する。
- 検証: 読み込み732件（単体656件と統合76件）、テーマ色のセル・余白描画を含む網かけ関連16件、Web版WASMのライブラリビルド検査（約36秒）、両リポジトリの差分の空白検査が成功。
- 限界: 表の表示モデルは解決後の色を保持するため、読み込み後のテーマ切替への追従は別途対応が必要。網かけは平均色による近似で、Word実機との画像比較、自動色の厳密な解決、パターンのブラシ描画は未検証。明示的な解除による継承の抑止は次項で対応。Wordとの完全互換は未達。

### 表の網かけの明示的な解除（2026-11-08）

- 網かけの省略と、`w:shd w:val="clear" w:fill="auto"` などの明示的な解除を区別する。色が解決されない明示的な網かけには表示モデル上の `transparent` を保持し、基底スタイルや表の色が再適用されることを防ぐ。表・行例外・セルの開始／終了タグと空要素、表スタイルの基底・条件付き書式に適用する。省略したセルが表スタイルを継承する規則は [MS-OI29500 §2.1.144](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/4f0cccae-70c2-492a-bc94-3ccc39172119) を参照。
- セルの色を消す編集も、プロパティの削除ではなく明示的な `clear`・`auto` を書き出す。直接指定の有無をそれぞれ検証し、保存・再読み込み、取り消し・やり直しでも解除状態を維持する。
- 修正前に、読み込みで解除指定が消えること、子スタイルで解除しても親の `#123456` が残ることを回帰テストで確認した。描画テストでは明示的に解除したセルと行例外の余白に継承色を描かず、指定を省略した隣のセルだけに表スタイルの色を描くことを確認する。
- 検証: 読み込み734件（単体658件と統合76件）、保存・履歴・描画を含む網かけ関連18件、Web版WASMのライブラリビルド検査（約38秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との画像比較、自動前景色のコントラスト判断、テーマの変更追従、パターンのブラシ描画は引き続き未検証。Wordとの完全互換は未達。

### 模様なしの網かけで前景色を背景に使わない（2026-11-08）

- `clear` の網かけは模様の前景色を描かず、背景の `fill` だけを使う。従来は `fill` がないと `color` へフォールバックしていたため、背景を解除したはずの文字・段落・表・番号などが前景色で塗られた。模様なしの定義は [Microsoft ShadingPatternValues](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shadingpatternvalues?view=openxml-3.0.1)、前景と背景の合成は [Microsoft Shading](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.shading?view=openxml-3.0.1) を参照。
- 共通の色解決処理で `clear`（大文字表記と既存モデルのパターン未指定を含む）には `fill` のみを返す。背景を省略／自動にして前景だけを指定した表セルでも、解除状態を保持する。`nil` を単色として扱う既存のWord固有動作と、縞模様の近似処理は維持する。
- 修正前に、赤い前景だけの網かけが背景色として返ることと、表セルの解除が赤い背景になることを回帰テストで確認した。文字の背景帯、番号の直接指定・テーマ前景とキャッシュ再描画にも「背景を描かない」ケースを追加した。
- 検証: 読み込み735件（単体659件と統合76件）、網かけ関連18件、Web版WASMのライブラリビルド検査（約44秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との画像比較、背景に応じた自動色の決定、パターンのブラシ描画、読み込み後のテーマ切替への追従は未検証。Wordとの完全互換は未達。

### 罫線のテーマ色と濃淡（2026-11-08）

- 共通の罫線読み込みとセル専用の罫線読み込みで `w:themeColor`・`w:themeTint`・`w:themeShade` を解決し、解決できた場合はキャッシュ済みの `w:color` より優先する。共通処理は文字・段落・ページ・表・行例外・表スタイルの罫線に使われる。属性の定義は [Microsoft BorderType](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.bordertype?view=openxml-3.0.1) を参照。
- 修正前に、テーマの濃淡による `#2F5496` ではなくキャッシュの赤 `#FF0000` が使われることを回帰テストで確認した。段落とセルでキャッシュの優先順位、属性順序、キャッシュなし、明度調整、未知のテーマ色へのフォールバックを検証する。番号の罫線描画・キャッシュ再表示のテストにも、古いキャッシュ色と異なるテーマ色を指定した。
- 検証: 読み込み736件（単体660件と統合76件）、罫線関連46件、Web版WASMのライブラリビルド検査（約45秒）、両リポジトリの差分の空白検査が成功。
- 限界: 表示モデルには解決後の罫線色だけを保持する。罫線XMLを再生成する経路でのテーマ参照の維持と、読み込み後のテーマ切替への追従は別途対応が必要。Word実機との画像比較と自動色のコントラスト判断は未検証。Wordとの完全互換は未達。

### 段落罫線のテーマ参照を保存時に維持する（2026-11-08）

- `BorderEdge` に解決後のRGBと別に元のテーマ参照を保持し、テーマやキャッシュ色がない場合も参照と濃淡を失わない。JSONへの保存・復元も対応し、従来のテーマ参照がないデータは引き続き読み込める。属性の定義は [Microsoft BorderType](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.bordertype?view=openxml-3.0.1) を参照。
- 段落の罫線変更でXMLを作り直す処理と、表紙の段落生成に `themeColor`・`themeTint`・`themeShade` を書き戻す。表紙生成では従来落ちていた `between` と `bar` の罫線も書き出す。
- 修正前に、テーマだけを指定した罫線を読むと参照がモデルから消えることを回帰テストで確認した。段落オプションと罫線コマンドの両方で保存・再読み込み・取り消し・やり直しを検証し、表紙生成では6辺それぞれのテーマ属性を検証する。モデルの追加メタデータはボックス化し、64ビット環境の `Run` サイズは3184から3192バイトとなる。
- 検証: 読み込み737件（単体661件と統合76件）、罫線関連48件、Web版WASMのライブラリビルド検査（約38秒）、両リポジトリの差分の空白検査が成功。
- 限界: 表セルのCSS形式の表示モデルはテーマ参照を保持しない。読み込み後のテーマ切替への追従、すべての罫線編集経路のテーマ参照維持、Word実機との画像比較と自動色の判断は未検証。Wordとの完全互換は未達。

### 表・セル罫線の部分変更で既存属性を維持する（2026-11-08）

- 表罫線を書き戻す際に `themeColor`・`themeTint`・`themeShade` と `space` を出力する。一辺の変更によって、変更していない辺のテーマ参照と間隔が落ちる経路を修正した。
- セル罫線はCSS表示モデルから全辺を再生成せず、指定された辺だけXMLを置き換える。未変更のテーマ参照・間隔・影・開始辺／終了辺・斜め罫線とコンテナ属性を保持し、辺のスキーマ順序を維持する。
- 左右の辺を変更する場合は、表の双方向配置に応じて対応する開始辺／終了辺の古い指定をXMLと表示モデルから取り除く。編集前の論理辺が変更後の物理辺を上書きすることを防ぐ。
- 回帰テストで表のテーマ参照・間隔の保存と再読み込み、セルの未変更XMLの維持、左右両方向の表で論理辺を置き換えた後の保存前後の一致を確認した。罫線関連50件、既存の表・ページ・段落罫線など31件、Web版WASMのライブラリビルド検査（約47秒）、両リポジトリの差分の空白検査が成功。
- 限界: セルのCSS表示モデル自体へのテーマ参照保持、表罫線でモデル化していない属性の維持、ページなど別の罫線再生成経路、読み込み後のテーマ切替、Word実機との画像比較は引き続き対応・検証が必要。Wordとの完全互換は未達。

### 表罫線の未変更XMLとセル罫線の表示・保存の一致（2026-11-08）

- 表罫線も、編集対象として指定された辺だけをXMLに書き戻す。モデルに含まれない未変更辺の影・枠属性、コンテナの拡張属性・子要素をそのまま維持する。開始辺／終了辺を使う表は、双方向配置に応じて対象の論理辺を更新し、物理辺を重ねて追加しない。
- セル罫線の編集直後のCSSを、再読み込みと同じ線種・色・太さへ正規化する。太さ未指定時に古い太さが残る問題を修正し、指定された太さは保存する8分の1ポイント単位に揃える。`nil` の非表示指定、大文字混在の線種、ハッシュなしの色も保存前後で一致させる。
- 新しい回帰テストでは、左右両方向の表で未変更XMLと論理辺の保持、内側の罫線の変更、再読み込みしたモデルの一致を検証する。セルでは既存の太い罫線から既定値への変更、非表示、線種・色の正規化、端数の太さの保存前後の一致を検証する。
- 検証: 罫線関連84件、Web版WASMのライブラリビルド検査（約27秒）、両リポジトリの差分の空白検査が成功。
- 限界: 編集対象となった辺のモデル外の属性は引き継がない。ページなど他の罫線再生成経路、セルのテーマ参照を含む表示モデル、テーマ切替への追従、Word実機との描画比較は引き続き対応・検証が必要。Wordとの完全互換は未達。

### Canvas編集APIでWordの全下線種を受け付ける（2026-11-08）

- `apply_underline_kind` の7種類に限定していた検証を、フォント設定で共用する18種類の下線一覧へ統一する。描画済みの太い点線・長い破線・一点鎖線・二重波線などをAPIから指定できるようにした。
- `none` は下線解除として選択範囲・ヘッダー／フッターへの書式コマンドへ渡す。入力待ち状態でも無効化し、一重線へ変更するときも直前の線種を消す。
- 回帰テストで18種類すべての入力待ち書式、選択範囲の編集・取り消し・やり直し、DOCX保存・再読み込みを検証した。下線関連22件、Web版WASMライブラリのビルド検査（約29秒）、両リポジトリの差分の空白検査が成功。
- 限界: ブラウザ上の実操作とWord実機との描画比較は未検証。この変更はCanvas編集APIの制限を解消するもので、Wordとの全機能の互換性を証明するものではない。

### ヘッダー／フッターの書式変更で下線色を保持する（2026-11-08）

- リボンから太字などを変更すると、既存の下線色・テーマ参照が共通のXML再生成処理で消える問題を修正した。下線を変更しない場合は元の `w:u` を保持する。色だけを指定した下線に勝手に線種を追加しない。
- 下線の線種を変更する場合も、既存の色・テーマ色・濃淡属性を保持して `w:val` だけを更新する。明示的な下線解除は従来どおり無効化と色の解除を行う。
- 回帰テストは修正前に下線色の消失で失敗することを確認した。ヘッダーとフッターの通常・先頭・偶数ページ用、段落全体と部分選択、色だけの下線と波線からの太字・二重波線への変更について、編集中とDOCX保存・再読み込み後の色・テーマ参照・濃淡・線種を検証する。
- 検証: ヘッダー／フッター関連44件、Web版WASMライブラリのビルド検査（約50秒）、両リポジトリの差分の空白検査が成功。
- 限界: ブラウザ実操作とWord実機との表示比較、すべての書式属性を通した部分変更の検証は未完了。Wordとの完全互換は未達。

### ヘッダー／フッターの書式編集で既存の変更履歴を保持する（2026-11-08）

- リボンから太字などを変更すると、既存の文字書式の変更履歴 `w:rPrChange` が消える問題を修正した。変更履歴の承認・却下を行わない通常の書式編集では、著者・日時・変更前の書式を含む元の履歴XMLを維持する。部分選択による文字列の分割時には、既存処理に従って履歴IDを振り直す。
- フォント設定からの部分変更でも、新しく生成した書式の後ろへ履歴要素を配置する。リボン編集と共通の処理で、既存の他のプロパティの順序を保ちながら履歴を末尾に置く。
- 回帰テストは修正前に履歴消失で失敗することを確認した。ヘッダーとフッターの通常・先頭・偶数ページ用、段落全体と部分選択、リボンとフォント設定の24通りについて、編集中とDOCX保存・再読み込み後の履歴XML・要素順序・選択範囲の太字を検証する。
- 検証: ヘッダー／フッター関連45件、Web版WASMライブラリのビルド検査（約46秒）、両リポジトリの差分の空白検査が成功。
- 限界: ブラウザ実操作、Word実機での履歴の表示・承認・却下、編集時の新しい変更履歴の記録はこのテストの対象外。Wordとの完全互換は未達。

### ヘッダー／フッターの欧文フォント変更で他の言語の指定を維持する（2026-11-08）

- リボンでArialなどの欧文フォントを選ぶと、日本語・複雑文字のフォント指定やテーマ参照が消える問題を修正した。本文の既存の選択規則に揃え、欧文フォントではASCII・欧文用の指定だけを更新する。日本語など東アジア用のフォントを選んだ場合は、従来どおりすべての言語スロットを更新する。
- フォント要素全体の再生成で失われていた言語ヒントと拡張属性も保持する。変更したスロットのテーマ参照は外し、未変更のスロットのテーマ参照を維持する。属性の部分更新処理はフォント設定画面と共用する。
- 修正前の回帰テストで、Arialへの変更による日本語フォントの消失を確認した。ヘッダーとフッターの通常・先頭・偶数ページ用、段落全体と部分選択、欧文・日本語フォントへの変更の24通りについて、編集中とDOCX保存・再読み込み後の各言語のフォント・テーマ参照・言語ヒント・拡張属性を検証する。
- 検証: ヘッダー／フッター関連46件、Web版WASMライブラリのビルド検査（約28秒）、両リポジトリの差分の空白検査が成功。
- 限界: ブラウザ実操作とWord実機との描画比較は未検証。フォント名から東アジア用かどうかを判定する既存の規則、および利用可能な描画フォントの制約は残る。Wordとの完全互換は未達。

### ヘッダー／フッターの特殊ハイフンを含む選択範囲の編集（2026-11-08）

- 改行しないハイフン `w:noBreakHyphen` と任意ハイフン `w:softHyphen` を含む段落で、部分選択の書式変更が行全体の変更へフォールバックする問題を修正した。本文の読み込みと同じU+2011・U+00ADとして文字位置を数え、選択範囲だけを分割・編集する。
- 分割や文字の挿入でランを作り直す際も、特殊ハイフンをWordprocessingMLの専用要素として書き戻す。タブの専用要素と、前後の空白を保持する既存処理も維持する。
- 修正前の回帰テストで、特殊ハイフン直前の1文字への書式変更が範囲編集として処理されないことを確認した。ヘッダーとフッターの通常・先頭・偶数ページ用、2種類のハイフン、その直前・ハイフン自体・直後の選択の36通りで、太字の適用範囲、既存の斜体、末尾への文字挿入後とDOCX保存・再読み込み後の文字列・専用要素を検証する。
- 検証: ヘッダー／フッター関連47件、Web版WASMライブラリのビルド検査（約43秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・図・改行などを含む段落の範囲編集、およびブラウザ実操作とWord実機での改行位置の比較は対象外。Wordとの完全互換は未達。

### ヘッダー／フッターの文字編集でランの編集識別子と拡張属性を保持する（2026-11-08）

- 文字の挿入・置換・削除で編集対象のランを作り直す際、`w:rsidR`・`w:rsidRPr` や拡張属性が消える問題を修正した。既存のラン要素を維持して子要素だけを更新するよう、大文字・小文字変換の処理と揃えた。削除後も残るランは元の属性を保持する。
- 修正前の回帰テストで、1文字の挿入だけで編集識別子と拡張属性が消えることを確認した。ヘッダーとフッターの通常・先頭・偶数ページ用、文字列内部への挿入、日本語の途中への挿入、ランをまたぐ削除・置換、境界への挿入、全文削除、変更なしの42通りで、編集中とDOCX保存・再読み込み後の文字列・各ランの識別子と拡張属性・太字／斜体／下線・ハイパーリンクのラッパー属性を検証する。
- 検証: ヘッダー／フッター関連48件、Web版WASMライブラリのビルド検査（約28秒）、両リポジトリの差分の空白検査が成功。
- 限界: 編集時の新しい変更履歴や編集識別子の生成、フィールドや図を含む段落の範囲編集、Word実機での比較は対象外。Wordとの完全互換は未達。

### ヘッダー／フッターの複数段落選択に文字書式を適用する（2026-11-08）

- 段落をまたいで選択してもカーソルのある1段落だけに文字書式が適用される問題を修正した。通常の文字段落について、太字・斜体・下線・取り消し線・上付き／下付き・フォント・サイズ・文字色・蛍光ペン・書式のクリアの共通経路を複数段落の選択へ接続した。先頭と末尾の選択外の文字は変更せず、次の段落の先頭で選択を終えた場合はその段落を対象に含めない。
- 選択内の各段落の有効な書式から太字などのオン／オフを判定し、段落をまたぐ選択表示にも対応した。範囲全体を検証してからXMLを更新し、文字位置を対応付けられない複数行の選択がカーソル行全体への変更に置き換わることを防ぐ。既存のヘッダー／フッター専用の履歴に変更前後の文書と選択を保存する。
- ヘッダーとフッターの通常・先頭・偶数ページ用、順方向・逆方向、終端の文字位置2種類の24通りで、空段落を含む選択への太字のオン／オフ、選択外の書式、既存の斜体・ハイパーリンク・空ランの拡張属性、本文の保持を編集中とDOCX保存・再読込後に検証した。不正なUTF-8境界・範囲外・フィールドを含む範囲がXMLを変更しないことと、両ストーリーの複数段落の選択矩形・逆方向・古い描画情報の拒否も検証する。
- 検証: ヘッダー／フッター関連51件、Web版WASMライブラリのビルド検査（約33秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・表・タブ区切りの配置などを含む範囲、段落記号自体の文字書式、複数段落の文字入力／削除・大文字小文字変換・サイズの段階変更・フォント設定ダイアログは今回の拡張対象外。ブラウザ実操作とWord実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの複数段落選択でフォントサイズを拡大・縮小する（2026-11-08）

- 複数段落を選択してサイズを拡大・縮小すると、カーソルの段落全体が変更される問題を修正した。通常の文字段落では、先頭と末尾の部分選択・中間の空段落を含めて選択範囲に適用する。終端が次の段落の先頭ならその段落を変更しない。文字位置を対応付けられない複数行選択は、カーソル行全体への変更に置き換えない。
- 段階的な変更と1ptの変更は、単一段落と共通のサイズ計算を使用する。各ランの直接指定・複合文字用サイズ・段落スタイルから有効なサイズを求め、それぞれを変更する。既存の専用履歴に選択のアンカーも保存し、変更前後の選択範囲を保持する。
- ヘッダーとフッターの通常・先頭・偶数ページ用、順方向・逆方向、終端の文字位置2種類、拡大／縮小と段階変更／1pt変更の96通りで、混在するサイズと継承サイズ、空段落、選択外のサイズ、斜体・ハイパーリンク・段落の間隔・拡張属性の保持を編集中とDOCX保存・再読込後に検証する。不正なUTF-8境界・範囲外・フィールドを含む範囲のXMLが変わらないことも確認する。
- 検証: ヘッダー／フッター関連53件、Web版WASMライブラリのビルド検査（約36秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・表・タブ区切り配置などを含む範囲、段落記号自体の書式、複数段落の文字入力／削除・大文字小文字変換・フォント設定ダイアログは引き続き対象外。ブラウザ実操作とWord実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの複数段落選択を大文字・小文字へ変換する（2026-11-08）

- 単一行に限られていた大文字・小文字変換を、通常の文字段落をまたぐ選択へ拡張した。先頭と末尾の選択外の文字、空段落、各ランの書式・編集識別子・拡張属性・ハイパーリンクを保持する。各段落で変換状態を初期化し、同じ段落ではランをまたぐ単語に変換状態を引き継ぐ。本文の変換と同じUnicode変換処理を使用する。
- `İ` や `ß` などの変換でUTF-8の長さが変わった場合、最終段落の選択終端を更新し、逆方向の選択も維持する。単一行のタブ配置では段落内の位置とセグメント内のカーソル位置の差を差し引く。専用履歴へ変更前後のアンカーも保存する。
- ヘッダーとフッターの通常・先頭・偶数ページ用、順方向・逆方向、終端の文字位置2種類、Upper／Lower／Title／Sentence／Toggleの120通りで、文字列・変換後の選択範囲・既存書式と属性の保持を編集中とDOCX保存・再読込後に検証する。不正なUTF-8境界・範囲外・フィールドを含む範囲でXMLが変わらないことも確認する。
- 検証: ヘッダー／フッター関連55件、Web版WASMライブラリのビルド検査（約28秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・表・タブ区切り配置などを含む複数段落選択は対象外。複数段落の文字入力／削除とフォント設定ダイアログは未対応。ブラウザ実操作、Word実機との変換結果の比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの複数段落選択をフォント設定ダイアログへ反映する（2026-11-08）

- フォント設定の適用は複数段落に対応していたが、初期値の取得がカーソル位置へ戻っていた。通常の文字段落をまたぐ選択では、変更時と同じ範囲を検証して各ランの有効な書式を読み、共通する値だけを表示する。中間の空段落に複数の空ランがある場合も、すべてを混在判定に含める。終端が次の段落の先頭ならその段落は含めない。
- ヘッダーとフッターの通常・先頭・偶数ページ用、順方向・逆方向、終端の文字位置2種類の24通りで、太字・斜体・サイズの混在判定、未変更時の空パッチ、選択範囲への太字のみの変更、既存サイズ・斜体・選択外の継承書式・空ランの拡張属性・ハイパーリンク・編集識別子の保持を編集中とDOCX保存・再読込後に検証する。不正なUTF-8境界と存在しない段落の範囲は取得できないことも確認する。
- 検証: ヘッダー／フッター関連56件、Web版WASMライブラリのビルド検査（約38秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・表・タブ区切り配置などを含む複数段落選択、段落記号自体の文字書式、複数段落の文字入力／削除は引き続き対象外。ブラウザ実操作とWord実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの複数段落選択を削除・置換する（2026-11-08）

- 通常の文字段落をまたぐ選択をDelete／Backspaceで削除し、先頭と末尾の選択外の文字を結合する。文字入力・改段落・貼り付け前の選択削除にも共通処理を使用する。カーソルは選択の先頭へ移し、選択アンカーを解除する。既存の専用履歴へ変更前後の文書・カーソル・アンカーを保存し、入力バッチの失敗時は元の文書と選択へ戻す。
- 範囲全体のUTF-8境界・段落・インライン内容と、段落が同じ親で連続していることを検証してからXMLを変更する。ヘッダー／フッターの改行整形用空白は許容する。対応付けられない複数行選択はカーソルだけの削除・入力へ置き換えない。選択外のラン書式・ハイパーリンク・編集識別子と先頭の段落プロパティを保持し、全体削除後は入力用の空ランに開始位置の文字書式を残す。
- ヘッダーとフッターの通常・先頭・偶数ページ用、順方向・逆方向、終端の文字位置2種類の24通りで、日本語・特殊ハイフン・中間の空段落を含む削除、選択外の文字とXML属性、本文の保持を検証する。両ストーリーの全6種類で段落境界だけの削除・文面全体の削除と、その後の文字挿入もDOCX保存・再読込で検証する。不正なUTF-8境界・範囲外・フィールド・表をまたぐ選択がXMLを変更しないことも確認する。
- 検証: ヘッダー／フッター関連59件、Web版WASMライブラリのビルド検査（約54秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールド・表・タブ区切り配置などを含む複数段落選択は対象外。選択範囲の置換時に継承する全書式のWordとの一致、ブックマーク・コメント・変更履歴の範囲調整、ブラウザ実操作、Word実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの繰り返し文字を選択位置どおりに編集する（2026-11-08）

- 異なる書式のランに同じ文字が続くとき、変更前後の文字列の共通部分から編集位置を推測すると、カーソルと異なるランを変更する場合があった。通常の文字段落の入力・単一範囲削除・Delete／Backspaceには実際のUTF-8文字位置を渡し、編集対象のランを決定する。位置を保持した空ランがある場合は、その入力用書式を優先する。
- 中央／右揃えの単一段落と、XMLの文字列が描画内容と一致するタブ区切り配置について、表示セグメントの位置を段落全体の文字位置に変換する。範囲のUTF-8境界を検証してから更新し、選択外のラン書式・ハイパーリンク・編集識別子を保持する。
- ヘッダー／フッターの通常・先頭・偶数ページ用で、同じ日本語文字が太字・斜体のランに続く場合の先頭・途中・ラン境界の挿入、異なるランでの削除・置換、DOCX保存・再読込、不正な文字境界でXMLを変更しないことを検証する。UIの共通処理でも中央揃えとタブ区切り中央セグメントの削除・挿入を検証する。
- 検証: ヘッダー／フッター関連61件、Web版WASMライブラリのビルド検査（約30秒）、両リポジトリの差分の空白検査が成功。共通依存のPowerPoint処理にあった罫線構造体の初期化漏れも補完した。
- 限界: 表・フィールドなど、表示文字と通常の段落の文字位置を対応付けられない行には既存経路を使用する。選択置換時の全書式の継承、ブラウザ実操作、Word実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの段落先頭からの選択置換で書式を保持する（2026-11-08）

- 複数段落の選択が段落先頭から始まり、末尾段落の文字が残る場合、削除後の入力が末尾の文字書式を引き継いでいた。末尾の文字を連結する前に開始段落の入力用空ランを保持し、次の入力には開始位置の書式を使う。開始段落にランがない場合は段落プロパティ内の文字書式を使用する。
- フォント設定のカーソル位置の照会も、その位置にある入力用空ランを優先する。後ろに表示文字が残る場合も、置換入力に使う書式を表示する。
- ヘッダー／フッターの全6種類、順方向／逆方向、開始段落が空／文字あり、終端が段落先頭／途中の48通りで、削除後の書式照会・入力・DOCX保存と再読込を検証する。末尾の斜体・ハイパーリンク・編集識別子、後続の段落が保持されることも確認する。
- 検証: ヘッダー／フッター関連62件、Web版WASMライブラリのビルド検査（約38秒）、両リポジトリの差分の空白検査が成功。
- 限界: Word実機との書式継承の網羅的比較、ブラウザ実操作、表・フィールドなどをまたぐ選択、コメント・ブックマーク・変更履歴の範囲調整は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターで次に入力する文字の書式を変更する（2026-11-08）

- 文字を選択せずに太字・下線などを変更すると、カーソルの段落内の既存文字全体が変わっていた。文字位置を対応付けられる通常の段落では、カーソル位置に入力用の空ランを保持し、その書式だけを変更する。同じ位置で変更を繰り返す場合は空ランを再利用する。
- リボンの書式状態もカーソル位置の入力用ランから取得する。ランの途中では既存書式を引き継いで分割し、境界では従来の入力処理と同じく直前のランの所属を保持する。リンク内・リンク境界で書式を変更しても、次の入力のリンク所属を保持する。
- ヘッダー／フッターの全6種類と先頭・途中・ラン境界・末尾の5位置の30通りで、既存文字と書式の保持、書式状態の照会、空ランの再利用、不正なUTF-8境界での無変更、次の入力とDOCX保存・再読込を検証する。ハイパーリンクと編集識別子の保持も確認する。
- 検証: ヘッダー／フッター関連63件、Web版WASMライブラリのビルド検査（約25秒）、両リポジトリの差分の空白検査が成功。
- 限界: 表・フィールドなど、表示文字と段落の文字位置を対応付けられない行は従来の経路を使用する。ブラウザ実操作、Word実機との境界での入力書式の網羅的比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターで次に入力する文字のサイズを拡大・縮小する（2026-11-08）

- 文字を選択せずにサイズを拡大・縮小すると、段落全体の既存文字のサイズが変更されていた。文字位置を対応付けられる通常の段落では、カーソル位置の入力用ランだけを変更する。一般の文字書式変更と同じランの作成・再利用処理を使い、サイズの計算には本文と共通の段階変更／1pt変更を使用する。
- ヘッダー／フッターの全6種類、先頭・途中・ラン境界・末尾の5位置、拡大／縮小、段階変更／1pt変更の120通りで、連続操作後も既存文字のサイズを保持することと、次の入力のサイズをDOCX保存・再読込後まで検証する。複合文字用サイズの指定、空段落の段落プロパティからの継承、不正なUTF-8境界での無変更、空ランの再利用、リンク・編集識別子の保持も確認する。
- 検証: ヘッダー／フッター関連64件、Web版WASMライブラリのビルド検査（約28秒）、両リポジトリの差分の空白検査が成功。
- 限界: 表・フィールドなど、表示文字と段落の文字位置を対応付けられない行は従来の経路を使用する。ブラウザ実操作とWord実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターのフォント設定を入力位置の書式に合わせる（2026-11-08）

- 未選択のカーソルが異なる書式のラン境界にある場合、フォント設定ダイアログは後ろのラン、文字入力は直前のランを参照していた。カーソル用の書式照会を追加し、入力やリボンの書式変更と同じランを参照する。入力用空ランがある場合は、その書式を優先する。選択範囲の照会は従来どおり範囲内の文字を対象にする。
- 文字ランがない段落では、段落プロパティ内の文字書式もダイアログへ反映する。中央／右揃えとタブ配置のカーソル位置の変換は、入力処理と共通の位置対応を使用する。
- ヘッダー／フッターの全6種類と先頭・途中・ラン境界・末尾の5位置の30通りで、表示する書式と実際の入力の書式が一致することを保存・再読込後まで検証する。空段落の太字・サイズの継承、照会でXMLを変更しないこと、選択用の照会が後ろの文字を引き続き参照することも確認する。
- 検証: ヘッダー／フッター関連65件、Web版WASMライブラリのビルド検査（約30秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールドを含む段落の照会は従来の代替経路を使用する。ブラウザ上のダイアログ操作とWord実機との比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの改段落で入力位置の文字書式を引き継ぐ（2026-11-08）

- 次の入力用の書式を設定してから改段落した場合、中間の空段落が直前の表示文字の書式に戻ることがあった。通常の文字段落は、分割前に入力用空ランを優先してカーソル位置の書式を取得し、新しい空段落へ引き継ぐ。
- 分割後の段落が異なる書式の既存文字で始まる場合も、先頭に入力用書式を保持する。次の入力に分割位置の書式を使い、残っている文字の書式は変更しない。
- ヘッダー／フッターの全6種類、先頭・途中・ラン境界・末尾の5位置、入力用書式の設定あり／なしの60通りで、2回の改段落後の中間段落と末尾段落への入力をDOCX保存・再読込後まで検証する。太字・斜体・下線・サイズ、段落間隔、既存リンク・編集識別子の保持を確認する。
- 検証: ヘッダー／フッター関連66件、Web版WASMライブラリのビルド検査（約31秒）、両リポジトリの差分の空白検査が成功。
- 限界: フィールドなどを含む段落は従来の分割処理を使用する。段落スタイルの次スタイル、リンク内での改段落のWordとの網羅的比較、ブラウザ実操作は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの改段落で次の段落スタイルを適用する（2026-11-08）

- 段落末尾の改段落では、本文と共通の処理でスタイルの `w:next` を解決する。新しい段落の文字書式を次スタイルに合わせ、直接指定された段落間隔などは保持する。途中で分割した場合は元のスタイルと文字書式を引き継ぐ。
- 連続改段落では直前の段落から順に次スタイルを解決する。次スタイルの指定なし・自己参照・存在しない参照で連鎖が終わっても、以降の段落が最初のスタイルに戻らず、最後の有効なスタイルを保つ。
- ヘッダー／フッター全6種類、末尾／途中、終端の3条件の36通りで3回の改段落を検証。各新段落への入力をDOCX保存・再読込し、段落スタイル、文字サイズ、太字の解除、直接指定の段落間隔を確認する。
- 検証: ヘッダー／フッター関連67件、本文の改段落15件、Web版WASMライブラリのビルド検査（約34秒）、両リポジトリの差分の空白検査が成功。
- 限界: Wordとの網羅的な実機比較、ブラウザ実操作、リンク内での改段落の互換性は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの空のリスト項目でEnterを押すと番号を解除する（2026-11-08）

- 本文と同様に、空の番号付き段落の先頭でEnterを1回押した場合、段落を増やさず番号を解除する。既存のヘッダー／フッター編集履歴を通じて変更する。貼り付けや複数段落の挿入は従来の改段落処理を使う。
- 本文の番号解除コマンドを共用し、段落スタイル・既定スタイルから番号を継承する場合は明示的な `w:numId=0` で再表示を防ぐ。直接設定された番号の解除でも、段落間隔、文字書式、編集識別子、隣接段落を保持する。
- ヘッダー／フッター全6種類、直接指定・スタイル・既定スタイル・変更履歴付き・履歴のみ・解除済み・通常段落の7条件、空・空白・文字・フィールド・改行・描画の6条件の252通りを検証。空の有効なリスト項目だけを変更し、その他はXMLを変更しない。DOCX保存・再読込後の番号解除と書式、変更履歴内の過去の番号の保持を確認する。
- 検証: ヘッダー／フッター関連68件、Web版WASMライブラリのビルド検査（約28秒）。履歴保持の追加アサーションを含む対象テストの再実行、両リポジトリの差分の空白検査も成功。
- 限界: Word・ブラウザでの実操作、ヘッダー／フッター内の表のリストキー操作、Tab／Shift+Tabによる階層変更は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターのリスト先頭でBackspaceを押すと番号を解除する（2026-11-08）

- 選択範囲のないリスト段落の先頭でBackspaceを押すと、隣接段落との結合より先に番号を解除する。文字・リンク・フィールド・描画を含む段落にも適用し、既存の編集履歴に記録する。タブで区切られた表示領域の途中は段落先頭として扱わない。
- 空のリストでのEnterと番号解除処理を共用する。スタイルから継承した番号も解除し、変更履歴内の過去の番号を保持する。解除後のBackspaceは従来の削除・段落結合処理に進む。
- ヘッダー／フッター全6種類、番号設定7条件、空・空白・文字・フィールド・改行・描画・リンクの7条件の294通りで番号解除の可否を検証。直接の番号設定以外のXML全体を比較し、隣接段落やリンク、フィールド、書式、変更履歴がDOCX保存・再読込後も保たれることを確認する。
- 検証: ヘッダー／フッター関連68件が成功（約5秒）、Web版WASMライブラリのビルド検査が成功（約31秒）。
- 限界: キー操作と取り消しのブラウザ実操作、Wordとの実機比較、表内のリストキー操作、Tab／Shift+Tabによる階層変更は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターのリストにTab／Shift+Tabの階層変更を接続する（2026-11-08）

- `listTab` がヘッダー／フッターの編集モードでも動作し、リスト項目の先頭ではTabで1階層下げ、Shift+Tabで1階層上げる。単一段落の先頭からの選択、および複数のリスト段落をまたぐ選択にも適用する。タブ区切りの表示領域の途中を段落先頭として扱わない。
- 本文のインデントコマンドを共用し、各項目を元の階層から個別に変更する。階層は0〜8の範囲に収め、端の階層でもキーを消費してタブ文字の挿入を防ぐ。通常段落が混じる選択は変更しない。選択とカーソルを保持し、変更時だけ既存の編集履歴へ記録する。
- 番号解除と共通のストーリーXML編集処理を使用する。全6種類のヘッダー／フッター、番号設定7条件、内容7条件の294通りで階層変更と復帰を検証し、番号設定以外のXMLを比較する。DOCX保存・再読込後の番号IDと階層、異なる階層・番号IDの範囲変更、上下限、逆順指定、通常段落を含む範囲の変更拒否も検証する。
- 検証: ヘッダー／フッター関連69件が成功（約6秒）、Web版WASMライブラリのビルド検査が成功（約51秒）、両リポジトリの差分の空白検査が成功。
- 限界: ブラウザでのキー操作・選択・取り消し、Wordとの実機比較、表内のリスト操作は未検証。フィールドや描画を含む段落は先頭キャレットを解決できる場合に対応し、その途中を含む選択範囲の判定は未対応。Wordとの完全互換は未達。

### タブ区切りのヘッダー／フッターでEnter時の文字書式を保持する（2026-11-08）

- 左・中央・右の表示領域にタブで分けた段落でEnterを押すとき、表示領域のカーソル位置を元の段落の位置に変換し、段落XMLを直接分割する。文字列の置換で書式を失い得る経路を避け、既存の文字書式を保持する。改行を含む貼り付けも同じ分割処理を使用する。
- 分割後は、新しい段落の先頭に対応する表示領域へカーソルを移す。表など段落との対応を確定できない場合は従来の処理を使用する。
- 全6種類のヘッダー／フッター、段落の配置3条件、タブ区切りの7位置、改段落数1・2の252通りを検証。分割前後の文字列、DOCX保存・再読込後の太字・斜体・下線・文字サイズ、続けて入力した文字の挿入位置を確認する。次に入力する文字用の空の書式ランと、既存の文字範囲の書式を区別して比較する。
- 検証: ヘッダー／フッター関連70件が成功（約13秒）、Web版WASMライブラリのビルド検査が成功（約32秒）。
- 限界: ブラウザでのEnter・貼り付け・取り消し操作、Wordとの実機比較、リンク内の分割、表内のタブ区切り段落は今回の検証に含まない。Wordとの完全互換は未達。

### ヘッダー／フッターの空のタブ領域を保持して段落を分割する（2026-11-08）

- 表示行では省略される末尾の空の領域を、元の段落のタブ数から判定する。末尾にタブがある場合やタブだけの段落でも、元の段落内の位置へ変換し、Enterで文字書式を保持したまま分割できる。元の段落に存在しない右領域は位置変換しない。
- タブ区切りの左領域しか文字がない場合、段落配置によって文字を中央・右領域へ移動させる重複処理を削除した。タブのない通常段落は、既存の配置別の処理を使用する。
- ヘッダー／フッター全6種類、配置3条件、末尾・中央・全領域が空になる5文字列、各領域の先頭・末尾の468通りを検証。表示位置と元の位置、DOCX保存・再読込後の各段落の文字・太字・文字サイズ、分割後に入力する位置を確認する。
- 検証: ヘッダー／フッター関連71件が成功（約18秒）、Web版WASMライブラリのビルド検査が成功（約32秒）。表示行抽出の既存テストも成功。
- 限界: ブラウザでのキー操作・取り消し、Wordとの実機比較、リンク内の分割は今回の検証に含まない。3個以上のタブを含む段落や表など、表示行との対応が曖昧な場合は従来の処理を使用する。Wordとの完全互換は未達。

### 3個以上のタブを含むヘッダー／フッターの文字と編集位置を保持する（2026-11-08）

- 表示行の右領域へまとめる際、3個目以降のタブを空白に変換せず、空の領域や末尾のタブも含めてそのまま保持する。文字ランの連結文字列も表示文字列と一致させる。タブだけの段落でも右領域に残るタブを省略しない。
- 元の段落を最初の2個のタブで左・中央・右に分けて表示行と照合し、右領域内のタブの前後を元の段落内の位置に変換する。Enterおよび同じ経路を使う改行貼り付けでは、文字列の置換へ迂回せず段落XMLを分割する。
- 空の領域を含む回帰テストを8文字列・各領域内の全ての文字境界へ拡張した。全6種類のヘッダー／フッターと配置3条件の702通りで、元の位置への変換、分割後の文字列、DOCX保存・再読込後の太字と文字サイズ、分割後の入力位置を検証する。
- 検証: ヘッダー／フッター関連71件が成功（約28秒）、表示行抽出の既存12件が成功、Web版WASMライブラリのビルド検査が成功（約64秒）。両リポジトリの差分の空白検査も成功。
- 限界: 任意のタブ位置・リーダーを含む描画のWord実機比較、ブラウザでのキー操作と取り消し、リンク内での分割、表の表示行の位置変換は未検証。文字と編集位置の保持を確認したもので、タブ配置の完全互換を示すものではない。Wordとの完全互換は未達。

### ヘッダー／フッターの表示領域をまたぐ選択削除を接続する（2026-11-08）

- 同じ段落の左・中央・右領域をまたぐ選択を、元の段落内の位置に変換する。Backspace／Deleteや選択を置き換える入力では、間のタブを含む選択全体を削除する。既存の段落XML編集を使い、選択外の文字書式を保持する。
- タブの削除によって表示領域の構成が変わった場合も、編集後の段落からカーソルの領域と位置を解決する。位置を解決できない場合は変更を復元する。選択削除は既存の取り消し履歴へ記録する。コピーは選択内のタブを含む文字列を取得し、選択表示は各領域内の文字範囲を使用する。
- 全6種類のヘッダー／フッター、配置3条件、6種類の範囲、順方向・逆方向の216通りで位置変換と削除を検証する。タブだけの削除、全体削除、右領域に残る追加のタブ、UTF-8文字の途中の位置の拒否、DOCX保存・再読込後の太字・斜体・下線・文字サイズ、続けて入力する位置を確認する。
- 検証: ヘッダー／フッター関連72件が成功（約31秒）、Web版WASMライブラリのビルド検査が成功（約49秒）。両リポジトリの差分の空白検査も成功。
- 限界: タブ区切りの複数段落にまたがる選択、表・フィールドを含む段落の位置変換、ブラウザでのドラッグ・コピー・削除・取り消しと選択表示、Wordとの実機比較は今回の検証に含まない。Wordとの完全互換は未達。

### ヘッダー／フッターの領域をまたぐ文字書式と大文字・小文字変換（2026-11-08）

- 左・中央・右領域をまたぐ同一段落の選択を、文字書式の適用範囲とリボンの書式状態判定へ接続した。太字などの文字装飾、文字サイズの指定・拡大縮小は、間のタブを含む選択範囲を使用する。対応できない領域間の選択をカーソル位置への指定として処理せず、範囲指定の編集に失敗した場合も段落全体へ適用しない。
- 大文字・小文字変換の選択方向を、各領域内の位置ではなく元の段落内の位置で決定する。変換後のカーソルと選択開始位置は更新後の段落から解決する。左・中央領域のUnicode変換でバイト数が変わり、後続領域の開始位置がずれる場合も、元の領域の接頭辞を引き続き差し引かない。位置を解決できない場合は変更を復元する。
- 削除後のカーソル位置解決と変換後の選択位置解決を共通化した。全6種類のヘッダー／フッター、配置3条件、順方向・逆方向、大文字・小文字・大小反転の108通りで、選択範囲への太字・文字サイズ、範囲外の書式保持、`İ`の結合文字への展開、DOCX保存・再読込後の文字と書式、変換後の選択への再適用を検証する。タブだけの範囲でも選択方向を別途検証する。
- 検証: ヘッダー／フッター関連74件が成功（約29秒）、Web版WASMライブラリのビルド検査が成功（約32秒）。両リポジトリの差分の空白検査も成功。
- 限界: タブ区切りの複数段落選択、表・フィールドを含む選択、ブラウザでのリボン・ダイアログ・キー操作・取り消しとWord実機比較は今回の検証に含まない。Wordとの完全互換は未達。

### タブ区切りのヘッダー／フッターで複数段落をまたぐ選択を編集する（2026-11-08）

- 左・中央・右の表示領域から元の段落内の位置へ変換する処理を、複数段落の選択へ広げた。コピーでは領域間のタブと段落間の改行を保持し、削除・文字書式・文字サイズ・大文字小文字変換は同じ範囲を使用する。選択表示は各段落の表示領域ごとに文字範囲を求める。
- 選択削除では段落XMLを結合し、選択外の書式を保持する。削除によってタブの数や表示領域が変わった場合も、結合後の段落からカーソルを解決する。変換後の選択方向とUnicode展開後の位置も保持する。段落と表示領域の対応を確認できない範囲は対象にしない。
- 全6種類のヘッダー／フッター、配置3条件、6種類の範囲、順方向・逆方向の216通りを追加した。途中の空領域・追加のタブ・末尾のタブ、選択文字列、表示領域ごとの範囲、書式状態、Unicode小文字変換、DOCX保存・再読込後の文字・書式・段落数、削除後の入力位置を検証する。
- 検証: ヘッダー／フッター関連75件が成功（約29秒）、Web版WASMライブラリのビルド検査が成功（約35秒）。両リポジトリの差分の空白検査も成功。
- 限界: ブラウザでのドラッグ・クリップボード・削除・取り消し・選択表示、Wordとの実機比較、表やフィールドを含む選択は未検証。書式付きパッケージとしての複数段落コピーの対象範囲は今回広げていない。Wordとの完全互換は未達。

### タブ区切りのヘッダー／フッターをパッケージ編集の対象へ接続する（2026-11-08）

- 書式付きクリップボード用のパッケージ対象を判定する際、タブ区切りの表示領域を元の段落内の位置へ変換する。中央・右領域内のカーソル、表示領域間や複数段落の選択を対象として公開する。選択のない貼り付け先も同じ変換を使用し、元のストーリーのパーツパスと選択方向を保持する。
- パッケージ貼り付け後のカーソルは、更新後の段落内の位置から表示領域と領域内の位置へ変換する。タブを含む文字列の貼り付けや、置換によって領域構成が変わる場合にも適用する。対応できない位置はドキュメントや履歴を更新する前に拒否する。既存の非同期競合検査と取り消し履歴は共用する。
- 全6種類のヘッダー／フッター、配置3条件、空段落・空領域・追加のタブ・末尾のタブを含む5文字列の90条件で、DOCX再読込後の全ての文字境界を検証する。貼り付け後に使う位置変換とパッケージ対象への逆変換、その位置からの次の入力、元の太字と文字サイズ、無効なUTF-8位置の拒否を確認する。前回の複数段落216条件にもパッケージ対象と選択範囲の一致検査を追加する。
- 検証: ヘッダー／フッター関連76件が成功（約36秒）、Web版WASMライブラリのビルド検査が成功（約29秒）。両リポジトリの差分の空白検査も成功。
- 限界: ブラウザのシステムクリップボードを介した書式付きコピー・貼り付け、非同期競合時の実操作、取り消し、Wordとの実機比較は今回未検証。表やフィールドを含む段落のパッケージ対象判定は今回広げていない。Wordとの完全互換は未達。

### ヘッダー／フッターの選択した全段落へ段落書式を適用する（2026-11-08）

- 配置・段落スタイル・インデント・段落間隔・タブ・背景色・段落書式のリセットは、複数段落の選択がある場合に選択内の全段落へ適用する。従来はカーソルのある段落だけを変更していた。通常段落とタブ区切り段落の位置変換を共用し、表示行から段落の範囲を確定できない選択は変更しない。
- 文書の複製に範囲全体を適用してから反映する。途中のエラーや変更後の位置解決失敗で一部の段落だけを変更しない。配置やスタイルによって表示領域が移動する場合も、元の段落内の位置から選択の両端を解決する。既存の履歴へ範囲全体を1件として記録する。
- 全6種類のヘッダー／フッター、配置3条件、通常・タブ区切りの2条件、順方向・逆方向、配置・スタイル・インデント・段落間隔・背景色の5操作の360通りを検証する。DOCX保存・再読込後の選択内の書式と文字列、隣接する範囲外段落の保持、選択方向と両端の位置、ページの異なる無効な選択の変更拒否を確認する。
- ページ番号フィールドを含む単一段落の選択は従来どおり書式変更できる。ヘッダー／フッターと配置3条件の6通りで、保存後の配置とライブのPAGEフィールドの保持を検証する。
- 検証: ヘッダー／フッター関連78件が成功（約31秒）、Web版WASMライブラリのビルド検査が成功。テスト専用の補助関数を本番ビルドから除き、今回の未使用関数警告も解消した。両リポジトリの差分の空白検査も成功。
- 限界: ブラウザでのリボン・ダイアログ・取り消し操作とWordとの実機比較は今回未検証。表やフィールドを含む複数段落選択の位置変換、未対応の段落コマンドは今回の範囲に含まない。Wordとの完全互換は未達。

### ヘッダー／フッターの段落方向・罫線・改ページ設定を編集する（2026-11-08）

- 本文用の段落コマンドへ接続する対象を、段落方向、段落罫線、改ページと改行の4設定、段落ダイアログの追加設定へ広げる。従来はヘッダー／フッターでこれらを操作しても変更されなかった。複数段落の選択にも一括適用する。
- ヘッダー／フッターの元のXMLと対象段落のIDを持つ作業用文書で、本文と同じ書式変更・XML更新処理を使う。段落のスタイル定義も引き継ぎ、継承したタブの削除、背景色や罫線の解除、継承したオン設定への明示的なオフを保存する。対象外の段落・フィールド・文字書式・独自XMLを保持する。無効なコマンドを含む一括変更は反映しない。
- 共通の段落スタイル継承処理で抜けていた段落方向（`bidi`）も補う。右書きの継承を認識して左書きへ変更でき、明示的なオフはスタイルのオンに上書きされない。
- 全6種類のヘッダー／フッター、配置3条件、通常・タブ区切り、選択2方向、書式9操作の648条件で、選択した全段落への適用と選択外段落・文字列・選択位置の保持をDOCX保存・再読込で検証する。別のテストでは全6種類でスタイル由来の設定をオフ→オン→オフに変更し、PAGEフィールドと周辺XML、本文、スタイルパーツの保持、同じ設定の再適用、無効な一括変更の拒否も確認する。
- 検証: ヘッダー／フッター関連79件が成功（約54秒）、段落方向の既存3件も成功。Web版WASMライブラリのビルド検査と、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザの各ダイアログ操作、RTLの実描画、Wordでの開き直しは今回未検証。ヘッダー／フッターの改ページ設定は文書への保存を検証し、本文と異なるページ分割規則の再現は今回検証していない。Wordとの完全互換は未達。

### ヘッダー／フッターのインデント操作・段落間隔切り替え・文字体裁を編集する（2026-11-08）

- インデントの増減、ぶら下げインデントの増減、段落前12ptの切り替え、段落の日本語文字体裁6設定、グリッドに合わせる設定を本文と同じコマンド処理へ接続する。従来はヘッダー／フッターで操作しても反映されなかった。
- 選択段落をまとめた作業用文書を使い、段落前の間隔が混在する場合も選択全体を12ptへそろえ、全段落が12ptならまとめて解除する。複数コマンドも同じ選択範囲へ順番に適用し、元のXMLの対象段落だけを更新する。選択の両端、方向、対象外の段落、PAGEフィールドと独自XMLを保持する。
- 作業用文書には段落スタイル・スタイルの番号設定・文書の既定タブ幅を引き継ぐ。文字単位、ポイント単位、右書きの論理開始端、リスト階層の上限に応じて、各段落の現在値からインデントを増減する。
- 範囲選択の検証を全6種類・配置3条件・通常とタブ区切り・選択2方向・14操作の1,008条件へ拡張する。別途、混在する段落前間隔の3回の切り替え、既定タブ24ptでの増減、右書き、文字単位、リスト階層8での変更なしと階層7への変更、範囲外指定の拒否を保存・再読込で確認する。
- 検証: ヘッダー／フッター関連80件が成功（約82秒、1,008条件の選択範囲テストを含む）。既存のインデント17件、段落前間隔3件、文字体裁関連2件も成功。Web版WASMライブラリのビルド検査と両リポジトリの差分の空白検査も成功。
- 限界: ブラウザのリボン・ショートカットの実操作、文字体裁の画面描画、Wordでの実機比較は今回未検証。表を含む選択、未接続の他のコマンドへの対応も残る。Wordとの完全互換は未達。

### ヘッダー／フッターの箇条書き・段落番号・アウトライン番号（2026-11-08）

- 箇条書き・段落番号の切り替え、3種類のアウトライン番号、独自の番号書式をヘッダー／フッターの段落コマンドへ接続する。選択範囲をまとめて変更し、解除には本文と同じ番号解除処理を使う。
- 編集対象のヘッダー／フッターから番号の種類・階層を取得する。従来は本文の選択位置を参照していた。番号の種類も現在の階層の定義で判定し、階層を指定しないアウトライン番号の操作では現在値を保持する。
- 作業用文書へ既存の番号定義とOPCパッケージ情報を渡し、番号定義・本文パーツの関連付け・コンテンツタイプを編集したストーリーと一緒に反映する。本文の段落と既存の番号定義を維持する。ヘッダー／フッターの履歴には文書全体のスナップショットが入るため、追加した番号定義も同じ操作の履歴に含まれる。
- 選択範囲の検証を18操作の1,296条件へ拡張し、保存・再読込後の番号ID・階層・書式と、選択方向・タブ区切り・配置・対象外の段落を確認する。別途、全6種類で番号解除、無効な階層の拒否、PAGEフィールド・独自XML・段落間隔・本文の保持を確認する。
- 検証: ヘッダー／フッター関連81件が成功（約98秒、1,296条件の選択範囲テストを含む）。既存番号定義と保存後の本文XMLを比較する追加検証1件、既存のアウトライン番号7件も成功。Word編集エンジン単体のWASMビルド検査、両リポジトリの差分の空白検査も成功。Web版全体のWASM検査は別途変更中のPowerPointコード（`jade-pptx-writer/src/xml/objects.rs` と `shape_insertion.rs` の `format_text_range` 呼び出しで第3引数が不足）で失敗し、今回の最終状態では完了していない。
- 限界: ブラウザのリボンの実操作、Wordでの実機比較、表を含む選択範囲は今回未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの選択範囲へ文字スタイルを適用する（2026-11-08）

- `setCharacterStyle` をヘッダー／フッターの選択範囲へ接続する。従来は本文の選択を参照しており、ストーリー内の文字スタイルを変更できなかった。既存スタイルの適用・変更と「標準の文字」への解除を、本文と同じ範囲処理で実行する。
- スタイルの表示名から実際のIDを解決し、選択された文字と、複数段落をまたぐ場合の段落記号へ適用する。直接指定された色・太字のオフなどは保持し、スタイルを変更・解除した後の継承サイズも再計算する。範囲外の文字、リンク、PAGEフィールド、段落設定、独自XML、本文を保持する。
- 作業用文書で検証・編集し、成功した変更だけをストーリーへ反映する。未定義のスタイルや段落スタイルの誤指定、UTF-8途中の位置、範囲外の位置、未対応のフィールドを含む範囲では文書を変更しない。選択の両端と方向を保持し、既存のヘッダー／フッター履歴へ変更全体を1件として記録する。
- 検証: 全6種類、単一段落・複数段落・次段落先頭までの範囲、選択2方向、適用・変更・解除の108通りで、DOCX保存・再読込後の文字列・スタイル・継承サイズ・段落記号・直接書式・周辺XMLを確認。同じスタイルの再適用と無効な指定での文書不変も確認した。ヘッダー／フッター関連83件、本文の文字スタイル既存5件が成功。Web版全体のWASMライブラリのビルド検査と両リポジトリの差分の空白検査も成功。
- 限界: 表を含むストーリー、フィールドを含む選択範囲、キャレットだけへの適用は今回の対象外。ブラウザでのギャラリー・取り消し操作、Wordでの実機比較は今回未検証。リンク／フィールド挿入の未対応経路も引き続き残る。Wordとの完全互換は未達。

### ヘッダー／フッターのリンク挿入・解除（2026-11-08）

- 既存のリンク挿入・解除APIをヘッダー／フッターの編集へ接続する。選択文字列へのリンク設定、表示文字列の挿入・置換、内部ブックマークへのリンク、ヒント文字列、選択範囲またはキャレット位置での解除を本文と同じ処理で行う。
- 外部リンクの関連付けを各ヘッダー／フッターのリレーション部品に保存する。本文の関連付けや本文XMLを変更せず、文書全体と選択位置を既存の履歴へ1件として記録する。範囲・リンク先の検証と編集は作業用文書で行い、失敗時は実文書を変更しない。
- キャレットがリンクより前にある場合に、後方のリンクを解除していた共通処理の判定も修正する。
- 全6種類・選択2方向で、段落をまたぐ外部リンク、URLのXMLエスケープ、日本語のヒント、内部リンクの文字挿入、キャレットと範囲からの解除、本文・段落属性・対象外のPAGEフィールドの保持を確認する。外部リンクはDOCX保存・再読込後のリンク先と選択範囲を検証する。無効なURL・UTF-8途中の位置・フィールド内の位置・リンクより前のキャレットでは文書を変更しないことも確認する。
- 検証: リンク関連27件が成功（性能測定用1件は既定で無視）。Web版全体のWASMライブラリのビルド検査と両リポジトリの差分の空白検査も成功。ヘッダー／フッター関連84件も成功（約97秒）。
- 限界: 表を含むストーリーとフィールドを含む編集範囲は今回の対象外。ブラウザのリンクダイアログ・取り消し操作とWordでの実機比較は今回未検証。ブックマーク作成、相互参照、インラインフィールド挿入などの未接続経路も残る。Wordとの完全互換は未達。

### ヘッダー／フッターのインラインフィールド挿入（2026-11-08）

- 日付・時刻・ページ番号・総ページ数・ファイル名の既存挿入APIをヘッダー／フッターへ接続する。キャレット位置への挿入、選択文字列の置換、複数段落をまたぐ置換に対応し、通常文字列へ変換せずDOCXのフィールドとして保存する。
- 文字書式を引き継ぎ、本文、対象外のフィールド、段落設定、独自XMLを保持する。作業用文書で編集し、変更後の表示位置も解決できた場合だけ反映する。文書と選択位置を既存のヘッダー／フッター履歴へ1件として記録する。
- ページ番号と総ページ数は内部の表示用プレースホルダーの長さで挿入後のキャレットを計算する。配置とタブ区切りも考慮し、フィールドのキャッシュ文字数との違いで位置がずれることを防ぐ。
- 全6種類・5種類のフィールド・3種類の範囲・選択2方向の180条件で、保存・再読込後のフィールド種別・キャッシュ・文字書式と本文・周辺XMLの保持を検証する。別途、配置3条件、通常・タブ区切り・空のタブ領域、3種類のフィールドの216条件で、保存前後のキャレット表示位置を検証する。無効なキャッシュ文字列やUTF-8途中の位置では文書を変更しないことも確認する。
- 検証: 追加テスト2件（計396条件）を含むヘッダー／フッター関連86件が成功（約112秒）。本文の既存フィールドテスト3件、Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 表を含むストーリーと、既存のフィールドを含む編集範囲は今回の対象外。ブラウザでの挿入・取り消し操作、Wordでの実機比較は今回未検証。Wordとの完全互換は未達。

### ヘッダー／フッターから本文ブックマークを相互参照する（2026-11-08）

- 名前付きブックマークへの相互参照APIをヘッダー／フッターへ接続する。本文のブックマークを参照する文字列（REF）とページ（PAGEREF）を、リンク指定を持つフィールドとして挿入する。本文の参照先や関連付けを変更せず、参照先文字列をキャッシュとして保持する。
- 既存のフィールド挿入と、表示位置の解決・文書の反映・履歴記録を共用する。空の参照先と1,024文字を超える参照先も保存し、通常フィールド挿入の入力制限は維持する。存在しないブックマーク、未対応の番号参照、非空の選択、UTF-8途中の位置では文書を変更しない。
- 全6種類、参照先3条件（日本語・空・1,100文字）、文字列／ページの2種類、挿入位置4条件の144条件で、保存・再読込後のフィールド種別・リンク指定・キャッシュ・前後の文字列・本文ブックマークと周辺XMLの保持を検証する。
- 検証: 追加テスト1件（144条件）を含むヘッダー／フッター関連87件が成功（約114秒）。本文の既存ブックマーク・相互参照テスト13件、Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 今回接続したのは名前を指定する相互参照API。見出し・図表・脚注などのターゲット選択API、ヘッダー／フッター内のブックマーク作成、既存のフィールドを含む段落への挿入、表を含むストーリー、制御文字を含む参照先は未対応。ブラウザでの挿入・取り消し操作、ページ参照の実描画、Wordでの実機比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターで見出し・図表・脚注を相互参照する（2026-11-08）

- ターゲット選択式の相互参照APIをヘッダー／フッターへ接続する。本文の見出し・図表・ブックマークへの文字列／ページ参照、図表の番号参照、脚注・文末脚注の番号／ページ参照を挿入する。番号は既存の書式を引き継ぎ、脚注番号はNOTEREF、図表番号は番号部分へのREFとして保存する。
- 参照先の検証・隠しブックマーク作成を本文と共用する。同じ参照範囲のブックマークを再利用し、挿入に失敗した場合は本文の自動ブックマークも残さない。本文の参照先作成とヘッダー／フッターの挿入は既存の履歴へまとめて記録する。
- 全6種類、見出し・図表・脚注・文末脚注の4対象、文字列／ページ／番号の3指定（計72条件）で、対応する指定の保存・再読込とフィールド種別・リンク指定・表示値・ブックマーク再利用を検証する。古い参照先、不適合な参照種類、UTF-8途中の位置で文書が変わらないことも確認する。
- 検証: 追加テスト1件（72条件）を含むヘッダー／フッター関連88件が成功（約95秒）。本文の既存ブックマーク・相互参照テスト13件、Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ヘッダー／フッター内のブックマーク作成、既存のフィールドを含む段落への挿入、表を含むストーリー、制御文字を含む参照先は未対応。ブラウザでの挿入・取り消し操作、ページ参照の実描画、Wordでの実機比較は未検証。Wordとの完全互換は未達。


### 既存フィールドのあるヘッダー／フッターへ続けて挿入する（2026-11-08）

- フィールドを含む段落でも、既存フィールドの前後や通常文字列の途中へフィールドを挿入できるようにする。ヘッダー／フッターの表示位置と編集用モデルの位置を合わせ、PAGE・NUMPAGESの表示用プレースホルダーも考慮する。
- 既存のフィールド種別・保存済みの表示値、本文、段落設定、独自XMLを保持する。日付などの通常フィールドと相互参照は同じ挿入経路を使い、表示位置の解決・履歴記録も既存の処理と共用する。
- 全6種類、既存PAGE・DATEと日本語を含む段落の6挿入位置、3種類の挿入フィールド（108条件）で保存・再読込を検証する。配置3種類とタブ区切りを含む既存の216条件にも、挿入後のキャレットからさらにフィールドを挿入する検証を追加する。
- 検証: 追加・拡張したテストを含むヘッダー／フッター関連89件が成功（約134秒）。本文の既存フィールド挿入テスト3件、Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: フィールドの表示内部への挿入、既存フィールドを選択する置換、表・画像・テキストボックス・数式を含む編集対象は未対応。ブラウザでの操作とWordでの実機比較は未検証。Wordとの完全互換は未達。


### ヘッダー／フッターの既存フィールドを選択して置換する（2026-11-08）

- フィールド全体を含む選択範囲への挿入で、選択された既存フィールドを削除して新しいフィールドへ置換する。単純形式のフィールドはラッパーXMLを削除し、複合形式は既存のフィールドコード削除処理と連携する。通常の本文編集のフィールド保持規則は変更しない。
- フィールド表示内部の部分選択は適用せず、文書を変更しない。選択範囲外のフィールド、本文、段落設定と独自XMLを保持する。
- 全6種類、単純／複合形式、PAGEのみ・DATEのみ・両方・複数段落の4範囲、選択2方向の96条件を追加する。保存・再読込後の表示文字列、フィールド命令数、複合フィールドの開始コード数、本文と段落設定を検証する。
- 検証: 追加した96条件を含むヘッダー／フッター関連90件が成功（約176秒）。本文の既存フィールド挿入テスト3件、Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: フィールド表示内部だけの編集、表・画像・テキストボックス・数式を含む編集対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。


### ヘッダー／フッターのフィールドを選択して削除する（2026-11-08）

- 単純形式・複合形式のフィールドを含む選択範囲を、通常の削除操作で削除する。フィールド置換と同じ処理で、選択されたフィールドの表示値とコードを削除し、複数段落の結合にも対応する。
- 選択位置はフィールド表示とタブ区切りを考慮して編集位置へ変換する。削除後の表示位置を解決してから反映し、既存の削除操作の取り消し・やり直し履歴へ接続する。部分的なフィールド選択では通常文字列への変換を行わず、文書を保持する。
- 全6種類、単純／複合形式、4範囲、選択2方向の96条件を削除にも拡張する。別途、配置3種類・タブ区切りを含む216条件で選択2方向の削除と保存・再読込後の文字列を検証する。
- 検証: 削除を追加したヘッダー／フッター関連90件と本文フィールド挿入3件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 今回は選択範囲の削除。フィールドに隣接するキャレットからのBackspace／Delete、フィールド表示内部だけの編集、表・画像・テキストボックス・数式を含む編集対象は対象外。ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### ヘッダー／フッターの隣接フィールドをBackspace／Deleteで削除する（2026-11-08）

- フィールド直前のDeleteと直後のBackspaceで、表示値とフィールドコードをまとめて削除する。PAGE／NUMPAGESの表示用プレースホルダーと保存された値の長さの違いを考慮して範囲を決める。
- 表示位置を段落内の位置へ変換し、既存の選択範囲削除へ接続する。削除後のカーソル位置を解決してから、既存のヘッダー／フッター編集の取り消し・やり直し履歴へ記録する。
- 単純／複合形式、全6種類のヘッダー／フッターで、PAGEとDATEの両端と通常文字・フィールド内部の位置を検証する。配置3種類とタブ区切りを含む216条件では、両キー方向の削除、カーソル位置、保存・再読込後の元の文字列を検証する。
- 検証: 追加した両キー方向の削除条件を含むヘッダー／フッター関連90件と、本文のフィールド挿入3件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: フィールド内部からの編集、表・画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### ヘッダー／フッターのフィールド前後で通常文字を編集する（2026-11-08）

- フィールドを含む段落への通常文字入力・削除を、表示用プレースホルダーと保存値の長さの違いを考慮した編集へ接続する。配置とタブ区切りのある行でも表示位置から段落内の範囲を求める。
- 通常文字を独立したテキストとして挿入し、フィールドの結果に混ざることを防ぐ。PAGE直後の入力が保存後に失われる再現テストを追加し、単純／複合形式の全6種類でフィールド命令と元の結果、本文を保持する。
- 9種類の入力・削除を計108条件で検証する。別途、配置3種類・タブ区切りを含む216条件で、フィールドの両側への日本語入力、追加した文字の削除、保存・再読込後の内容を検証する。
- 検証: 入力・削除条件を追加したヘッダー／フッター関連90件と本文フィールド挿入3件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: この経路の挿入は通常文字を対象とし、改行・タブ等の制御文字、フィールド表示内部の部分編集、表・画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### フィールドを含むヘッダー／フッターへのタブ入力（2026-11-08）

- フィールド前後へのタブ入力を許可し、通常文字と同じくフィールドの外側に保存する。中央・右揃えのタブ位置を設定し、既存のフィールド命令・保存値を保持する。
- タブで表示領域が変わった後は、段落内の表示位置からカーソルを再計算する。続けて入力した文字を新しい領域に挿入する。画面が扱う3領域を超えてカーソルを解決できない場合は、タブ入力前の状態へ戻す。
- 単純／複合形式、全6種類のヘッダー／フッターにタブ入力・置換の72条件を追加。画面側では配置3種類とPAGE／NUMPAGES／DATEの前後へのタブ入力・日本語の連続入力を216条件で検証し、保存・再読込後の文字列とカーソル、3つ目のタブ入力の取り消しを確認する。
- 検証: タブ入力とカーソル移動の条件を追加したヘッダー／フッター関連90件、本文フィールド挿入3件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: タブを3つ以上含む段落の画面編集、フィールド表示内部の部分編集、表・画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### フィールドを含むヘッダー／フッターの段落境界を削除する（2026-11-08）

- 段落先頭のBackspace／前段落末尾のDeleteで、フィールドを含む隣接段落を結合できるようにする。画面側の結合対象判定にフィールドの表示文字列を使い、既存のXML結合処理と取り消し・やり直し履歴へ接続する。
- 結合後のカーソルは、存続する前段落の配置と表示上の末尾を引き継ぐ。後段落に別の配置が設定されていても、保存されたフィールド値の長さに左右されず結合境界を保持する。
- 全6種類、配置3組、PAGE／NUMPAGES／DATE、前後いずれか・両方にフィールドがある条件、削除2方向の324条件で、保存・再読込後のフィールド数、段落数、文字列、カーソル位置を検証する。単純／複合形式の12条件では、フィールド命令・元の保存値・本文の保持も確認する。
- 検証: 段落結合の条件を追加したヘッダー／フッター関連91件、本文フィールド挿入3件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: タブ区切りや表・画像・テキストボックス・数式を含む段落の画面からの結合、フィールド内部の編集、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### フィールドを含むヘッダー／フッターのEnter（2026-11-08）

- フィールドの前後でEnterを押したとき、表示上の位置に合わせて元のXMLを分割する。PAGE／NUMPAGESの表示用文字列とDATEの保存値の長さを区別し、フィールド命令・保存値を保持する。
- タブ区切りと段落の配置から、分割後の段落先頭のカーソルを計算する。解決できない場合は分割前へ戻し、フィールド内部を分割する要求は変更前に拒否する。
- 全6種類、単純／複合形式、PAGEとDATEの前後・段落先頭末尾、改行数1／2の144条件で保存・再読込後の文字列、命令・保存値、本文の保持を検証する。画面側では配置3種類、PAGE／NUMPAGES／DATEとタブの組合せ432条件で、前後の分割結果とカーソルを検証する。
- 検証: ヘッダー／フッター関連91件、段落分割関連78件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: フィールド内部の部分編集、3つ以上のタブや画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### タブとフィールドを含むヘッダー／フッターの段落結合（2026-11-08）

- タブで区切られた段落でも、段落先頭のBackspace／末尾のDeleteから隣接段落を結合する。表示領域内の先頭・末尾と段落全体の境界を区別し、通常文字とフィールドを保持する。
- カーソルは結合後の表示領域から再計算する。後段落のタブによって前段落の表示領域が変わる場合も、元の段落境界に続けて入力する。カーソルを解決できない場合は変更前へ戻す。
- 全6種類、配置3種類、前段落のみ・後段落のみ・両方・空のタブ領域の4組、Backspace／Deleteの144条件で、PAGEとDATEを保持した結合、保存・再読込後の文字列と段落数、カーソル位置、日本語の連続入力を検証する。
- 検証: ヘッダー／フッター関連92件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 結合後にタブが3つ以上となる段落、画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### タブとフィールドを含むヘッダー／フッターの箇条書き操作（2026-11-08）

- タブ区切りの段落にフィールドがある場合も、段落全体の先頭を判定してBackspaceから箇条書きの番号を解除する。中央・右の表示領域の先頭は、段落先頭として扱わない。
- フィールドを含む複数段落の選択で、表示位置を段落内の位置へ変換し、Tab／Shift+Tabによるレベル変更につなげる。選択方向が逆でも同じ段落範囲へ適用する。
- 全6種類、配置3種類、通常／空のタブ領域、PAGE／DATEの72組で、先頭判定、選択2方向・レベル変更2方向、保存・再読込後の文字列・フィールド・レベル、番号解除と隣接段落の保持を検証する。
- 検証: ヘッダー／フッター関連93件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 3つ以上のタブや画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### 後方に表があるヘッダー／フッターのフィールド編集（2026-11-08）

- 編集対象より後方に表があるヘッダー／フッターでも、表より前の通常段落にあるフィールドの前後へ入力できるようにする。同じ範囲のフィールド置換・削除、複数段落の選択削除、フィールド新規挿入にも対応する。
- 対象範囲までの段落構造を検証し、後方の表だけを理由に編集を拒否しない。表をまたぐ選択や表の後方への操作は、引き続き表を考慮した位置変換が必要となる。
- 全6種類、単純／複合形式の12組・編集4種類の48条件で、保存・再読込後の文字列、フィールドコード・保存値、後方の表XML・さらに後ろの段落・本文の保持を検証する。表をまたぐ選択の拒否と変更前のXML保持も確認する。
- 検証: ヘッダー／フッター関連94件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 表内・表をまたぐフィールド編集、表より後方の画面上の位置変換、画像・テキストボックス・数式を含む対象、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### 後方に表があるヘッダー／フッターのリンク・文字スタイル（2026-11-08）

- 後方に表があるヘッダー／フッターでも、表より前の通常段落へハイパーリンクの挿入・解除と文字スタイルの適用・解除を行えるようにする。選択範囲までの段落構造を検証し、後方の表だけを理由に操作を拒否しない。
- 既存の範囲選択テストを表あり／なしの両方で実行する。全6種類と選択2方向に加え、文字スタイルでは段落内・段落境界・複数段落の3種類の終端を使う。表ありの48組で、保存・再読込後の表XML・後続段落の保持、リンクの参照先・関係ファイル、文字スタイル・段落マーク・直接書式・本文の保持を検証する。
- 表をまたぐ選択は変更前のXMLを保持したまま拒否することも確認する。
- 検証: ヘッダー／フッター関連94件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: 表内・表をまたぐ編集、表より後方の画面上の位置変換、ブラウザ操作とWordでの実機比較は未対応または未検証。Wordとの完全互換は未達。

### タブ位置設定を持つヘッダー／フッターのEnterと次段落スタイル（2026-11-08）

- タブ位置を定義する段落プロパティを表示文字として数えないようにする。実際のタブ文字は従来どおり1文字として扱い、段落末尾でEnterを押したときに次段落スタイルを適用する。
- 全6種類、通常文字・タブ文字・単純／複合PAGEフィールドの4種類、途中／末尾の2位置の48条件で、保存・再読込後のスタイル連鎖、継続入力の文字サイズ・太字、タブ位置定義、文字列・フィールドコード・保存値の保持を検証する。
- 検証: 修正前は次段落スタイルがAからBへ切り替わらない失敗を再現。修正後はヘッダー／フッター関連95件と本文・脚注等を含む分割関連78件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### 3つ以上のタブを含むヘッダー／フッターのフィールド編集（2026-11-08）

- 表示側が保持している3つ目以降のタブを含む右側領域も、フィールドの表示位置から段落内の位置へ変換できるようにする。タブ数による編集・段落結合の拒否を取り除く。
- 通常／空のタブ領域にあるPAGE・NUMPAGES・DATEの前後で、入力、追加タブの入力、Enter、選択方向2通りの削除、隣接フィールド削除、連続挿入と保存・再読込を検証する。既存テストの組を4種類から7種類へ拡張し、全6種類・配置3種類を含む378組を扱う。
- 段落結合も7種類へ拡張する。合計3つ以上のタブになる場合を含め、両方向の結合、保存・再読込後の文字列とフィールド、結合後の位置変換と継続入力を検証する。
- 検証: ヘッダー／フッター関連95件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。WASM再検査の途中では並行変更中のPowerPoint側の未定義関数で失敗したが、関数の存在を確認した後の再実行は成功。
- 限界: 右側領域にまとめた追加タブのWordと同じ位置への描画、ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### 入れ子の複合フィールドを含む段落の分割（2026-11-08）

- 段落分割時に複合フィールドの入れ子の深さを追跡し、内側の終了記号で外側のフィールドを途中切断しないようにする。命令領域の入れ子は外側の表示文字数に含めず、結果領域の入れ子はそれぞれの表示文字数で数える。
- ヘッダー／フッター全6種類とフィールド前後の2位置で、命令領域にPAGEを持つDATEフィールドを分割し、保存・再読込後の表示文字列と全開始・区切り・終了記号が同じ段落に残ることを検証する。
- 結果領域のPAGE、3段の入れ子、外側がPAGEの場合、終了記号の不足についても、表示文字数・消費ノード数・分割先を直接検証する。
- 修正前は外側の全11ノードのうち内側の終了に当たる7ノードで処理を終える失敗を再現。修正後はヘッダー／フッター関連96件と段落分割関連80件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザ操作とWordでの実機比較、同じrun内に複数のフィールド境界を持つXMLは未検証。Wordとの完全互換は未達。

### 同じrunに境界・命令文・結果がある複合フィールドの段落分割（2026-11-08）

- 複合フィールドの境界と命令文・結果、複数の境界が同じrunにあっても、XML内の順番でフィールドを認識するようにする。段落分割時に対象runの内容を区切り、元のrun属性と文字プロパティを各断片へ引き継ぐ。
- 全内容が1つのrun、開始と命令文、区切りと結果、結果と終了が同じrunの4形式を扱う。PAGE／DATE、ヘッダー／フッター全6種類、フィールド前後の2位置の96条件で、表示文字数、保存・再読込後の段落別文字列・境界・保存値・太字・文字サイズを検証する。
- 検証: ヘッダー／フッター関連97件、段落分割関連81件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: この変更は表示位置による段落分割が対象。通常文字とフィールド全体を同じrunに持つ場合のモデル上の分離、ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### 同じrun内にあるページ番号の削除（2026-11-08）

- 「ページ番号の削除」で、PAGEの境界・命令文・保存値が同じrunにある場合も認識する。境界を区切る際にrun属性と文字書式を引き継ぎ、フィールド前後の通常文字を残す。
- 全6種類のヘッダー／フッターについて、1つのrunにあるPAGE、境界と結果が複数runへ分散する形式、連続する2個のPAGE、DATEを内側に持つPAGEの4形式を検証する。各ストーリーの表内にも同じrunに収まるPAGEを配置する。
- 保存・再読込後の通常文字・NUMPAGES・DATE・保存値・太字・文字サイズ、本文、表幅・表内文字、未知要素を検証する。削除件数と、再実行で削除数が0になりXMLが変わらないことも確認する。
- 検証: ページ番号削除の3件とUnicodeを生成する既存の64ケースが成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### 複数セクションにあるページ番号の削除（2026-11-08）

- 「ページ番号の削除」で、最終セクションだけでなく全セクションが参照するヘッダー／フッターを走査する。編集中のセクションに依存せず、共有されるパーツは一度だけ削除対象にする。
- 2セクションの独立した通常ヘッダー2個、前セクションから継承する先頭ページ用ヘッダーとフッターを対象に、編集位置なし／第1／第2セクションの3条件で検証する。
- 削除件数、保存・再読込後の全ストーリー、NUMPAGES・未知要素・セクション参照・本文の保持、未参照パーツの保持、編集位置の保持、再実行時の削除数0を確認する。
- 検証: ページ番号削除の4件、既存の複数セクション編集・参照作成の2件が成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### 共有ヘッダー／フッター編集後の表示同期（2026-11-08）

- 通常・先頭・偶数ページ用の複数種類が同じヘッダー／フッターパーツを参照するとき、編集後に一致する表示キャッシュをすべて更新する。以前は編集した種類だけを更新していた。
- 参照が存在しない種類は一致とみなさない。以前のセクションの先頭ページ用ヘッダーを編集しても、最終セクションに存在しない種類の表示キャッシュへ流し込まない。
- ページ番号削除でも同じ更新処理を使い、パーツごとの再解析を1回にする。前項の複数セクション削除テストに、存在しない種類が存在しないまま保たれることを追加した。
- ヘッダー／フッター×3種類の6条件で、共有パーツ編集直後と保存・再読込後の各表示、他セクションの独立したパーツ、本文と参照XMLの保持を検証する。前セクションの先頭ページ用ヘッダーの表示・継承も検証する。
- 検証: ヘッダー／フッター関連148件が成功（227.74秒）。ページ番号削除の4件も成功。Web版全体のWASMライブラリのビルド検査、両リポジトリの差分の空白検査も成功。
- 限界: ブラウザ操作とWordでの実機比較は未検証。Wordとの完全互換は未達。

### セクションのヘッダー／フッターを前と同じに戻す（2026-11-08）

- 最終セクションを「前と同じ」に戻す際、削除した参照の表示キャッシュを破棄する。前セクションに対応する内容がない場合に、削除前のヘッダー／フッターが画面に残る問題を修正した。
- 先頭ページ用・偶数ページ用も同様に更新し、継承元の内容はセクションごとの参照から取得する。独立編集後の再連結と保存・再読込を回帰テストに含める。
- 存在しないセクションへの連結状態の照会・変更はエラーにし、文書と編集対象を変更しない。
- 検証：`jade-docx-editor` のヘッダー／フッター関連100テスト成功（追加3件を含む）。`jade-web` のWASMビルド確認と差分の空白チェックも成功。

### サブフォルダー内のヘッダー／フッターの参照維持（2026-11-08）

- ヘッダー／フッターを独立化する際、複製先ディレクトリに合わせて内部リレーションシップの参照先を調整する。サブフォルダー内のパーツから画像や埋め込みオブジェクトへの参照がずれる問題を修正した。
- 専用エディター用DOCXへの変換と保存時にも同じ調整を行い、外部リンクと絶対パッケージ参照は維持する。
- 通常・先頭ページ・偶数ページのヘッダー／フッター6種類について、参照先・リソースの存在・独立編集後の元セクション・保存と再読込を検証する。
- 検証：新規回帰テスト1件（6種類のストーリーを走査）、既存の連結関連2件、セクション・ストーリー結合テスト7件が成功。WASMビルドと差分の空白チェックも成功。結合テストで旧仕様を期待していた段落グリッドのアサーションは、現実装の明示オン保存と再読込後の有効状態を確認する形に更新した。

### ヘッダー／フッター専用エディターの画像保存（2026-11-08）

- 専用エディターから戻した画像と既存メディアのパスが同じでも、内容が異なる場合は別パーツとして保存する。本文や別のストーリーが共有する元画像を変更せず、新しい画像を表示する。
- XMLとリレーションシップが同一でも画像のバイト列だけが変わった場合は変更として保存する。同じ参照先を持つ複数の画像リレーションシップは、同じ複製先を使用する。
- 通常・先頭ページ・偶数ページのヘッダー／フッター6種類について、衝突・画像だけの更新・元画像の保持・保存と再読込・変更なし保存を回帰テストに追加した。
- 検証：画像保存の新規回帰テスト1件（6種類を走査）、サブフォルダー参照の既存回帰テスト1件、セクション・ストーリー結合テスト7件が成功。`jade-web` のWASMビルドと差分の空白チェックも成功。

### ヘッダー／フッター編集の取り消し履歴（2026-11-08）

- 専用エディターの変更なし保存と、「前と同じ」の変更なし再設定ではUndo履歴を追加せず、既存のRedoを保持する。変更がなかった場合はパッケージの準備処理もチェックポイントから戻す。
- スナップショットコマンドに変更有無を返す経路を追加し、既存の変更コマンドは従来どおり履歴へ記録する。
- 通常・先頭ページ・偶数ページのヘッダー／フッター6種類について、画像を含む編集を2回Undo／Redoして保存内容が復元されること、変更なし保存で履歴が増えないことを検証する。連結操作のUndo後に変更なし操作を行ってもRedoを保持する回帰テストを追加した。
- 検証：スナップショットによるUndo／Redo関連11テスト（追加2件を含む）、セクション・ストーリー結合テスト7件が成功。`jade-web` のWASMビルドと差分の空白チェックも成功。

### ヘッダー／フッター専用エディターの対象セクション検証（2026-11-08）

- 専用エディター用DOCXの読み出しと保存で、対象セクションの存在を先に検証する。存在しない番号が最終セクションに解釈され、ヘッダー／フッターや画像パーツが変更される経路を防いだ。
- 連結状態の照会と同じ検証を使用し、文書がない場合も明示的なエラーを返す。1セクションの文書ではセクション0を有効とする。
- 1セクション・複数セクションの文書と6種類のヘッダー／フッターを対象に、範囲外の番号・最大整数・文書なしのエラーを検証する。画像を含む保存要求を拒否した後も、保存データ全体と編集中のセクションが変わらないことを確認する。
- 検証：新規の対象セクション回帰テストに加え、ヘッダー／フッター関連テスト一式が成功。`jade-web` のWASMビルドと差分の空白チェックも成功。

### ヘッダー／フッター専用エディターの依存パーツ保存（2026-11-08）

- 専用エディターからの保存で、画像に加えて埋め込みオブジェクト・チャートなどの内部参照先を取り込む。参照先のリレーションシップを再帰的にたどり、依存パーツ・コンテンツタイプ・外部リンクを保持する。
- 共有パーツと内容が異なる場合は、変更を含む依存グラフを別パーツへ複製する。参照元XMLが同じで埋め込みデータだけが更新された場合も検出し、元の本文や別ストーリーのデータは保持する。複数の参照先の共有と循環参照を維持する。
- 6種類のヘッダー／フッターについて、新規取り込み・既存共有パーツとの衝突・依存データのみの更新・循環参照・外部リンク・コンテンツタイプ・再読込・変更なし保存を回帰テストに追加した。今回の検証は保存パッケージの維持が対象で、チャートの画面表示とWord実機比較は未検証。
- 検証：保存関連4テスト（新規1件は6種類×新規／衝突の12通り）、サブフォルダー参照の既存回帰テスト1件、Undo／Redo関連11テストが成功。`jade-web` のWASMビルドと差分の空白チェックも成功。

### ヘッダー／フッター専用エディターのスタイル保存（2026-11-08）

- 専用エディターで追加・変更した段落／文字／表スタイルのうち、ストーリーから参照する定義を保存する。継承元・次のスタイル・リンク先を再帰的にたどり、継承元だけが変わったスタイルと循環参照も扱う。
- 既存の定義が同じ場合は共有し、変更された定義とその参照元は別IDへ複製する。本文と未使用スタイルの既存定義を保持し、スタイル定義だけの変更でもストーリーを再読込する。保存後の専用エディターで変更せず閉じた場合は、追加の複製や履歴を作らない。
- この修正は名前付きスタイルの保存が対象。番号定義とテーマ色・フォントの取り込みは下記の追加修正で対応。文書既定値の変更の取り込み、およびWord実機との表示比較は引き続き未検証。
- 検証：保存関連5テスト（新規1件で6種類それぞれのスタイル定義のみの変更・追加・継承／循環参照・本文保持・再読込・変更なし保存を検証）、Undo／Redo関連11テスト、既存のスタイル適用1テスト、サブフォルダー参照1テストが成功。`jade-web` のWASMビルドと差分の空白チェックも成功。

### ヘッダー／フッター専用エディターの段落番号保存（2026-11-08）

- 専用エディターから段落番号のインスタンス・抽象定義・画像付き箇条書きを取り込み、本文と共有する定義を変更した場合は別IDへ複製する。開始番号とレベルごとの開始番号指定も維持する。
- 段落から直接指定する番号に加え、スタイルが指定する番号と番号定義が参照するスタイルをまとめて解決する。スタイルと番号の相互参照を更新し、本文の定義は保持する。
- 画像付き箇条書きのリレーションシップと画像を保存し、番号XMLを変えずに画像だけを変更した場合も取り込む。変更のない保存では定義を複製しない。
- 検証範囲：6種類×直接指定／スタイル指定の12通りで、開始番号・本文定義の保持・画像のみの更新・再読込・変更なし保存を確認する。テーマと文書既定値の取り込み、Word実機との表示比較は未検証。
- 検証結果：保存関連6テスト、Undo／Redo関連11テスト、既存スタイル適用1テスト、サブフォルダー参照1テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。

### ヘッダー／フッター専用エディターのテーマ書式保存（2026-11-08）

- 編集元と本文のテーマまたは色の割り当てが異なる場合、ストーリー・参照スタイル・番号定義のテーマ色とテーマフォントを編集元の値へ解決して保存する。本文のテーマと設定、既存のスタイル・番号定義は保持する。
- 同じテーマでの保存はテーマ参照を維持する。異なるテーマで保存した後の再読込と変更なし保存でも、定義の追加複製を防ぐ。
- 6種類のヘッダー／フッターについて、テーマだけの変更と色の割り当てだけの変更を追加テストで扱う。直接文字書式・名前付きスタイル・番号ラベルの色とフォント、本文保持、再読込、変更なし保存を確認する。
- 今回の対象はテーマ色・フォントの解決。文書既定値の変更、テーマの図形効果・書式スキームの取り込み、Word実機との表示比較は未検証。
- 検証結果：保存関連7テスト、Undo／Redo関連11テスト、既存のテーマ編集1テスト、サブフォルダー参照1テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。


### ヘッダー／フッター専用エディターの既定スタイル保存（2026-11-08）

- スタイル未指定の段落・表で、編集元の既定スタイルやその継承元が変更された場合、対象ストーリーにスタイル参照を補って保存する。表セル内の段落も対象とし、本文の既定スタイルと既存定義は保持する。
- 編集元に定義のないスタイル参照は編集元の既定スタイルへ解決する。保存先だけに同じIDのスタイルがある場合に、その書式が誤って適用される経路を防ぐ。有効な明示指定は維持する。
- 6種類×既定スタイルの既存ID／新規IDの12通りで、継承元の変更、段落・表・セル内段落、本文保持、再読込、変更なし保存を検証する。
- 今回の対象は段落・表の既定スタイル。`docDefaults` の変更・削除の取り込みとWord実機との表示比較は未対応・未検証。
- 検証結果：保存関連8テスト、Undo／Redo関連11テスト、既存スタイル適用・テーマ編集・サブフォルダー参照の各1テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。


### ヘッダー／フッター専用エディターの文書既定値の分離（2026-11-08）

- 編集元と保存先で `docDefaults` が異なる場合、編集元の段落・文字既定値を専用の段落基底スタイルへ取り込む。参照する段落スタイルの継承元を補い、スタイル未指定の段落にも参照を設定する。本文の文書既定値は変更しない。
- 太字・斜体など12種類の文字トグルは、保存先の既定値との差を基底スタイルへ記録する。既定値と段落／文字スタイルの反転、直接書式による上書きを保つ。
- 編集元から削除した文字色を自動色に戻し、段落の前後間隔・自動間隔・行間・行間種別の不足を明示する。保存先の既定値が再適用されることを防ぐ。
- 6種類×段落スタイルの有無×既定値の変更／削除の24通りで、書式の継承、本文保持、再読込、変更なし保存を検証する。
- 文書既定値の完全対応は未達。フォント・サイズなどの指定削除に対する取り消し、循環する段落スタイル継承での既定値の分離、表スタイルとの優先順位、Word実機との表示比較は引き続き必要。
- 検証結果：保存関連9テスト、Undo／Redo関連11テスト、既存スタイル適用・テーマ編集・サブフォルダー参照の各1テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。


### ヘッダー／フッターの文字既定書式を削除した場合の保存（2026-11-08）

- 編集元の文書既定値から下線・蛍光ペン・上付き／下付き文字を削除した場合、専用の基底スタイルにそれぞれ `w:u=none`、`w:highlight=none`、`w:vertAlign=baseline` を記録する。保存先の文書既定値から削除前の書式が再適用されることを防ぐ。編集元に値がある場合はその値を保持する。
- 文書既定値の保存テスト24通りに、上記3種類の変更／削除、段落スタイルと直接書式による上書き、本文の既定値保持、再読込後の変更なし保存を追加した。
- 継承と未指定時の意味は Microsoft の [Underline](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.underline?view=openxml-3.0.1)、[Highlight](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.highlight?view=openxml-3.0.1)、[VerticalTextAlignment](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.verticaltextalignment?view=openxml-3.0.1) の仕様説明に基づく。
- フォント・サイズなど他の既定値削除、循環する段落スタイル、表スタイルとの優先順位、Word実機での表示一致は未検証であり、Wordとの完全互換は未達。
- 検証結果：保存関連9テスト（24通りの文書既定値テストを含む）、Undo／Redo関連13テスト、既存スタイル・入れ子のパート参照・テーマの各1テストの計25テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。


### ヘッダー／フッターの循環する段落スタイルと文書既定値（2026-11-08）

- 編集元と保存先で文書既定値が異なり、段落スタイルの `basedOn` が循環している場合も、編集元の既定値を保存できるようにした。
- 各循環メンバーから見て、継承先を最初に訪問する順序を保持する有限の補助スタイル列を作り、その末尾を専用の文書既定値スタイルへ接続する。単一の共有辺を切る方式で起こる、継承書式の欠落や優先順位の変更を避ける。
- 補助スタイルには重複しないIDと専用の名前を付け、文書の既定スタイル指定を引き継がせない。`Normal` の名前を複製すると読込時に本文の標準スタイルへ統合されるため、名前も分離する。本文の既定値とスタイルは維持する。
- 既定値の保存テストを60通りへ拡張し、自己参照・2／3スタイルの循環（3スタイルでは各メンバーを直接使用）、既定値の変更／削除、文字書式の優先順位とトグル、再読込後の変更なし保存を検証する。
- 循環メンバー数を n とすると、各開始点からの優先順位を保つため最大 n(n−1) 個の補助スタイルを準備する。循環のない通常のスタイルでは補助スタイルは生成しない。Word実機での循環スタイルの扱いとの一致は未検証。
- フォント・文字サイズなどの既定値削除、表スタイルとの優先順位、Word実機での表示一致は引き続き未達。
- 検証結果：保存関連9テスト（文書既定値の60通りを含む）、Undo／Redo関連13テスト、既存スタイル・入れ子のパート参照・テーマの各1テストの計25テストが成功。`jade-web` のWASMチェックと差分の空白チェックも成功。

### 表の条件付きスタイルの自動文字色とテーマ色（2026-11-08）

- 表スタイルの `tblStylePr/rPr/color` が `val="auto"` の場合、未指定と混同して継承元の固定色を残していた。通常の文字スタイルと色解決処理を共通化し、明示的な自動色を保持する。
- 条件付き文字色の `themeColor`、`themeTint`、`themeShade` を現在のテーマから解決する。テーマが解決できない場合は保存された `val` を使用する。
- Microsoft の [Color の仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.color?view=openxml-3.0.1) に従い、色の省略による継承と、背景に応じて色を選ぶ自動指定を区別する。
- 回帰検証は親子・孫のスタイル継承、先頭行・最終列・帯・隅の条件、テーマの明暗変換と未解決時の色を対象にする。描画は通常行と繰り返し見出し行で、自動色の背景コントラストと直接指定色の優先を確認する。
- Word 実機との表示比較、および文書既定値・表スタイル・段落スタイル間の優先順位全体は引き続き検証対象。
- 検証結果：reader の `styles::tests` 66件と editor の `canvas_renderer::table::tests` 全件を通過。`jade-web` の WASM 向け `cargo check` も成功。

### 表スタイル全体の太字・斜体・文字色（2026-11-08）

- `w:style[@w:type="table"]/w:rPr` の `b`・`i`・`color` が読み込み時に欠落していたため、表全体へ指定した文字書式が描画されなかった。表示モデルへ保持し、`basedOn` の親子継承と各セルへの適用を接続した。
- 表全体の値を土台に、`tblLook` に応じた条件付き領域が上書きする。条件付き書式のない表でも全体の指定を適用する。色は自動指定・テーマ色・明暗指定にも対応する。
- 表スタイルの太字・斜体は、[Microsoft の表スタイルに関する補足](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/fc004387-4bf8-471c-9937-2ebc2cb7724d) に従って指定値へ設定し、明示的な `val="0"` を保持する。条件付き `type="wholeTable"` は引き続き Word の挙動に合わせて無視し、スタイル直下の `rPr` と区別する。
- 親・子・孫の継承、全体の明示的な解除、先頭行での上書き、無効な `wholeTable` による汚染がないことを回帰テストで確認する。通常行・繰り返し見出し行の色描画テストを、条件付き指定と全体指定の両方へ拡張した。
- フォント・サイズなど、表スタイルが持つ他の文字書式、文書既定値と段落スタイルを含めた優先順位全体、Word 実機との表示比較は引き続き検証対象。
- 検証結果：reader のスタイル67件と表モデル6件、editor の表描画219件、計292件が成功。`jade-web` の WASM 向け `cargo check` と両リポジトリの `git diff --check` も成功。

### 表スタイルの文字サイズ（2026-11-08）

- 表スタイル直下と条件付き領域の `w:sz`・`w:szCs` をポイント単位で保持し、親子継承とセルごとの優先順位を反映する。通常文字と複雑な用字系のサイズは独立して扱う。
- 読み込み時に補われた文字サイズを直接指定と混同せず、直接指定・文字スタイル・段落スタイル・表スタイルの順で解決する。描画用の変更で直接指定の保存情報を書き換えない。
- [MS-DOCX 2.3.1](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/2690d8ad-f5b8-4cd1-863e-6f67117898c3) の規則に合わせ、既定段落スタイルの11pt／12ptは通常、表スタイルのサイズを上書きしない。`overrideTableStyleFontSizeAndJustification` が有効な文書では通常の継承順を使う。
- 列幅の自動調整でも同じ表・段落・文字書式を使い、通常行と繰り返し見出し行でサイズが一致するようにする。幅計測キャッシュにはスタイルと互換設定、直接指定サイズの情報を含める。
- フォント名・テーマフォント、他の文字書式、段落スタイルの継承元が既定スタイルの場合の例外範囲、Word実機との表示比較は引き続き検証対象。
- 検証結果：reader のスタイル68件と表モデル6件、editor の表描画220件、計294件が成功。`jade-web` の WASM 向け `cargo check` と両リポジトリの `git diff --check` も成功。

### 表スタイルのフォント名とテーマフォント（2026-11-08）

- 表スタイル直下・条件付き領域の `w:rFonts` を保持し、欧文ASCII・高位ANSI・東アジア・複雑な用字系の各スロットと `hint` を個別に継承する。
- [Microsoft の RunFonts の仕様説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.runfonts?view=openxml-3.0.1) に合わせ、同じスロットのテーマ指定は同一要素の名前指定より優先し、後の継承層が名前を指定すると前のテーマ指定を解除する。条件付き `wholeTable` は従来どおり無視する。
- 文書既定値・表スタイル・段落スタイル・文字スタイル・直接指定の順でスロットを重ね、最後に文書テーマを解決する。読み込み時に補われたフォントを直接指定と混同せず、描画用の処理で保存用の直接指定情報を変更しない。
- 通常行・繰り返し見出し行と自動列幅で同じフォントを使用する。幅計測キャッシュには文書テーマと直接指定フォントの情報も含める。
- Word実機との表示比較、東アジアのフォント選択に用いる authored フラグ、その他の表文字書式と優先順位全体は引き続き検証対象。
- 検証結果：reader のスタイル69件と表モデル6件、editor の表描画221件、計296件が成功。`jade-web` の WASM 向け `cargo check` と両リポジトリの `git diff --check` も成功。

### 表スタイルの東アジアフォントヒントと直接指定（2026-11-08）

- 表スタイルのフォントを再計算する際、直接指定した `w:rFonts` に `hint` と東アジアフォント名がない場合は、本文と同じく `default` ヒントを使用する。表から継承した `eastAsia` ヒントが復活し、`§` などの記号が東アジアフォントへ切り替わる経路を修正した。
- 読み込み時に補われたフォントではなく直接指定のスナップショットから判断し、保存用の情報を変更しない。明示した `default`・`eastAsia`・`cs` ヒント、直接指定の東アジアフォント名、直接指定なしの継承も維持する。通常行・繰り返し見出し・自動列幅が共有する処理に適用した。
- [Microsoft の文字範囲別フォント選択の説明](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/aef3c9a6-5d6c-434b-90b7-85e761fd8e62) にあるヒント依存の記号を対象に、ヒント・東アジアフォント名・直接指定の有無・読み込み時の補完の64通りで実際に選択されるフォントを検証した。
- 検証結果：editor の表描画222テスト、`jade-web` の WASM 向け `cargo check`、両リポジトリの `git diff --check` が成功。Word実機との表示比較、言語や文字範囲に応じたフォント選択全体の一致は引き続き未検証。

### 表スタイルと既定段落スタイルの配置優先順位（2026-11-08）

- [MS-DOCX 2.3.1](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-docx/2690d8ad-f5b8-4cd1-863e-6f67117898c3) に従い、通常は既定段落スタイルの左揃えで表スタイルの配置を上書きしない。`overrideTableStyleFontSizeAndJustification` が有効な場合は通常の継承順を使用する。
- 直接指定の左揃え、別の段落スタイルによる配置、表外の段落は維持する。本文・ヘッダー／フッター・注・テキストボックスの読み込みと再構築、および段落スタイルの変更時に同じ例外を適用する。
- 既定段落スタイルの配置3種類、互換設定2種類、段落スタイル3種類、直接指定3種類、既定／継承する表スタイル2種類の108通りで、表内外の配置を検証した。
- 条件付き表スタイルの段落配置、既定スタイルを継承する別スタイルへの例外範囲、Word実機との比較は引き続き未検証。
- 検証結果：108通りの新しい配置テストを含む文書テスト383件と表描画222件が成功（ベンチマーク2件は除外）。文書テストで見つけた空のフィールドのXML重複2件も、以下の修正後に通過した。editor単体と `jade-web` のWASMチェックは成功。

### 結果が空のページ番号フィールドの保存（2026-11-08）

- `PAGE`／`NUMPAGES` の画面用プレースホルダーを保存済み結果の文字数として数え、空の `w:fldSimple` を再利用できず二重生成していた。空かどうかは結果の子要素から判断する。
- 元のフィールドの識別子を優先して再利用し、識別子がない場合もロック状態を含む命令の一致を確認する。ロックされた空のフィールドも、元のXMLと結果なしの状態を保持する。
- 既存の失敗テストで、隣への入力・直前の削除・書式変更、空要素と空の結果run、ロックされた単純／複合フィールドと再読み込みを検証する。
- 検証結果：文書383件、表描画222件、フィールド関連186件が成功（各分類には重複するテストがある）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの段落配置（2026-11-08）

- `w:tblStylePr/w:pPr/w:jc` を条件付き書式として保持し、`basedOn` による継承と、表の表示設定・縞模様サイズに応じた領域の優先順位を適用する。段落指定のない条件付き書式を重ねても親の配置を消さず、`rPr` など別の場所にある `jc` は拾わない。
- [Microsoft の条件付き書式の適用順](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/2ac331d4-cf1e-4fa0-8bca-6da74411e284) に従う既存の領域判定を使用し、表全体の配置の上に条件付き配置を重ねる。直接指定・段落スタイルの優先順位と既定スタイルの互換例外も維持する。
- 読み込み時の補完と段落スタイル変更時の継承計算で共通の処理を使用する。入れ子の表では最内側の表の条件を使い、保存用XMLへ補完された配置を直接指定として書き込まない。
- 非表示行や変更履歴の表示切替に伴う領域の再判定、行列構造を変更した後の再評価、条件付き段落書式の配置以外の項目、Word実機との比較は引き続き検証・改善対象。
- テストで見つけた、段落スタイルを「標準」へ戻す際に配置を再び左揃えにしてしまう経路も修正した。表ごとの継承済み配置を変更処理でも使用し、直接指定は維持する。
- 検証結果：既定／継承／不明な表スタイル、互換設定、段落スタイル、直接指定を組み合わせた216セルの配置と、入れ子の表・スタイル変更／解除・保存再読み込みを検証。reader のスタイル70件、editor の文書385件・表描画222件・段落スタイル変更4件、計681テストが成功（ベンチマーク2件を除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表の編集後の継承配置の更新（2026-11-08）

- 名前付き表スタイルの変更・解除、ギャラリープリセットの適用、表スタイルのオプション切替で、対象の表の段落配置を再計算する。行・列・セルの挿入／削除とセル分割でも、更新後の位置から条件付き配置を求める。
- 段落自身の `w:jc` を直接指定の根拠とし、段落スタイル・互換設定・既定の表スタイルの優先順位を維持する。補完した配置は保存XMLへ直接指定として書き込まない。別の表や入れ子の表の配置も変更しない。
- 通常の段落一覧の再構築には再計算を追加しない。初回の全体テストで検出した、通常編集のUndo時に空のセルへ段落プロパティが補完されて内部状態が変わる副作用を防ぐ。
- 新しい回帰テストでは、表スタイル変更・解除、先頭行設定、行列の挿入・削除の各段階について、編集中と保存・再読込後の配置を照合する。直接指定と段落スタイルの優先順位、直接指定のXML数も確認する。
- 表全体の分割、非表示行・変更履歴の表示切替に伴う領域の再判定、配置以外の条件付き段落書式、Word実機との比較は引き続き改善・検証対象。
- 検証結果：editor の全体テスト2,576件が成功（ベンチマーク等21件は除外）。通常編集のUndo関連8件も個別に再確認した。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表分割後の条件付き段落配置（2026-11-08）

- 通常の「表の分割」と、表内からのセクション区切り挿入で使う共通処理で、分割後の両方の表の継承配置を再計算する。先頭行・最終行・行の縞模様の条件は、それぞれの表内での新しい位置から求める。
- 表の識別子を使用して対象を限定する。前半に入れ子の表があって表の通し番号が変わる場合も、後半の表を正しく更新し、入れ子の表と別の表は変更しない。先頭行からの操作では表自体を分割しないため、継承配置を変更しない。
- 新しい回帰テストは4種類の分割位置と2種類の操作の8通りについて、直接配置、段落スタイル、入れ子・別の表、保存再読込、UndoのXML／モデル完全復元、RedoのXML一致を確認する。補完した配置を直接書式として保存しないことも検証する。
- 行の縞幅はテストで1を明示する。[Microsoft の Word 実装注記](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/77519f5b-08b3-4547-9f5d-0c4d87854220)では、省略時の値は0であり、行の縞模様を適用しない。既存の既定値0を維持した。
- 非表示行・変更履歴の表示切替に伴う領域の再判定、配置以外の条件付き段落書式、Word実機との比較は引き続き改善・検証対象。
- 検証結果：editor の全体テスト2,577件が成功（ベンチマーク等21件は除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの段落間隔とインデント（2026-11-08）

- 条件付き表スタイルの `w:pPr/w:spacing` と `w:pPr/w:ind` を読み込み、段落間隔・行間・インデントをセルごとに適用する。[Microsoft の要素定義](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.tablestyleproperties.styleparagraphproperties)にある条件付き段落プロパティの経路を拡張した。
- `basedOn` の継承と条件付き領域の重ね合わせでは属性単位で合成する。0による上書き、自動段落間隔の有効／無効、文字単位のインデントも既存の段落スタイルと同じ処理を使う。runの文字間隔やセル内の別要素を段落間隔として拾わない。
- 表スタイル・表示設定・行列構造の変更と表分割後には、配置とともに間隔・インデントも再計算する。段落自身が指定した属性は維持し、未指定の属性だけが新しい継承値へ更新される。補完した値は保存XMLへ直接指定として書き込まない。
- 新しい回帰テストは、表スタイルの親子継承、0指定、段落スタイルと直接指定の優先順位、表スタイル変更、段落スタイル変更、分割、Undo／Redo、保存再読込を検証する。Word実機との比較、これら以外の条件付き段落書式は引き続き改善・検証対象。
- 検証結果：reader のスタイル71件と条件付き領域6件、editor の全体2,578件が成功（ベンチマーク等21件は除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの改ページ制御と段落方向（2026-11-08）

- 条件付き段落書式の `keepNext`、`keepLines`、`pageBreakBefore`、`widowControl`、`bidi` を読み込み、親スタイルの継承と条件付き領域の重ね合わせを通じてセル内の段落へ反映する。明示的な無効指定も保持する。
- 表スタイル変更や行列構造の変更・表分割で5項目を再計算する。段落スタイルと直接指定の優先順位を維持し、継承した値を保存XMLへ直接書式として書き込まない。
- 回帰テストで親子継承、無効指定、別コンテナからの混入防止、表／段落スタイル変更、表分割、Undo／Redo、保存再読み込みを確認する。Word実機とのページ配置比較や、残りの条件付き段落書式は引き続き検証・改善対象。
- 検証結果：reader のスタイル72件と条件付き領域6件、editor の全体2,579件が成功（ベンチマーク等21件は除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの段落背景と段落罫線（2026-11-08）

- 条件付き段落書式の `w:shd` と `w:pBdr` を読み込み、表の各領域に適用する。セル背景・セル罫線やrunの背景とは別に保持する。[Microsoft の段落書式定義](https://learn.microsoft.com/ja-jp/dotnet/api/documentformat.openxml.wordprocessing.styleparagraphproperties?view=openxml-3.0.1)にある段落背景・罫線の経路を拡張した。
- 罫線は6辺それぞれを継承し、明示的な `nil`／`none` で継承を遮断する。背景も明示的な無効指定を保持する。
- 表スタイルの変更や表分割の後に背景と罫線を再計算する。直接罫線は辺単位で保持し、未指定の辺は更新後の継承値に追従する。段落スタイル変更時も同じ原則で再計算する。
- 回帰テストで親子継承、背景の無効化、6辺の読込、別コンテナからの混入防止、直接指定・段落スタイルの優先順位、表分割、Undo／Redo、保存再読込と継承値を直接書式として保存しないことを確認する。Word実機との描画比較や、残りの条件付き段落書式は引き続き検証・改善対象。
- 検証結果：reader のスタイル73件と条件付き領域6件、editor の全体2,580件が成功（ベンチマーク等21件は除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの段落タブ（2026-11-08）

- 条件付き領域の `w:pPr/w:tabs/w:tab` を読み込み、表スタイルの継承元から派生先へ、適用される条件付き領域の順に追加する。段落スタイル、直接指定も含めたタブ一覧を既存のレイアウト処理へ渡し、同じ位置の置換・`clear`・配置・リーダーを反映する。[Microsoft の Tabs 定義](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.tabs?view=openxml-3.0.1)に従い、タブの継承は追加方式とする。
- 表スタイルや段落スタイルの変更、表分割などの条件再評価では、直接指定を元の段落XMLから読み直し、新しい継承値に重ねる。古いスタイル由来のタブを直接指定として保持しない。条件付きタブを `styles.xml` の通常の段落書式やセル書式と混同しない。
- 回帰テストでは継承・解除・同位置の置換・リーダー、表／段落スタイル変更、表分割、保存再読み込み、Undo／Redoを照合する。保存XMLには直接指定のタブだけを保持し、元の `styles.xml` は変更しない。
- Word実機との比較、および未対応の条件付き段落書式は引き続き改善・検証対象。
- 検証結果：editor の全体テスト2,581件（21件除外）、reader のスタイル74件・条件付き領域6件、計2,661テストが成功。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの段落間隔・グリッド関連設定（2026-11-08）

- 条件付き領域の `contextualSpacing`、`snapToGrid`、`adjustRightInd`、`mirrorIndents`、`suppressLineNumbers`、`suppressAutoHyphens` を読み込み、継承・領域の適用順・段落スタイル・直接指定を重ねた値を既存処理へ渡す。明示的な無効値も保持する。[Microsoft の段落プロパティ定義](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.paragraphpropertiesbasestyle?view=openxml-3.0.1)に含まれる項目。
- 表スタイル変更や表分割などの条件再評価後にも更新し、継承値を段落自身のXMLへ書き込まない。保存再読み込みとUndo／Redoの一致を検証する。
- 通常の段落書式補完でも漏れていた `mirrorIndents` を補完対象に追加する。文書既定の `mirrorIndents` も読み込み、直接指定による無効化を優先する。
- Word実機での描画・改ページの比較、および残る条件付き段落書式は引き続き改善・検証対象。
- 検証結果：editor の全体テスト2,582件（21件除外）、reader のスタイル76件・条件付き領域6件、計2,664テストが成功。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 条件付き表スタイルの行内文字揃え（2026-11-08）

- `w:tblStylePr/w:pPr/w:textAlignment` を読み込み、継承元から派生先、条件付き領域、段落スタイル、直接指定の優先順位で解決する。表／段落スタイル変更や表分割時にも再評価し、既存の行内文字配置処理へ渡す。`auto`／`baseline` の明示指定も継承値を上書きする。[Microsoft の TextAlignment 定義](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.textalignment?view=openxml-3.0.1)を参照。
- 文書既定値 `w:docDefaults/w:pPrDefault/w:pPr/w:textAlignment` の読み落としも修正する。条件付きの指定が通常の段落書式に漏れず、異なる階層の同名要素を誤認しないことを回帰テストで確認する。
- 保存再読み込み、スタイル変更、表分割、Undo／Redoで解決値を照合し、保存XMLには直接指定のみを保持する。Word実機との文字位置比較と残る条件付き書式の対応は継続課題。
- 検証結果：editor の全体テスト2,583件（21件除外）、reader のスタイル77件・条件付き領域6件、計2,666テストが成功。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表セル内の行内文字揃えの描画（2026-11-08）

- 解決済みの `w:textAlignment` の `top`／`center`／`bottom` を、横書きセルの行ボックス内の文字基準線へ反映する。フォントの上昇・下降量と行の余白から位置を決め、行高を超える文字には負の余白を加えない。
- 行内画像も同じ基準線に配置し、画像が必要とする最小行高を保持する。自動幅計算時の既定配置キャッシュが明示指定を上書きしないようにする。
- 基準線だけでなく記録された文字描画座標、自動幅テーブルを通した描画、画像と既存のグリッド配置を回帰テストで検証する。
- 後続の改善で、横書き行の異なるサイズの文字・画像も個別に上下位置を補正する。Word実機との位置比較、縦書きの同指定は引き続き検証・改善対象。
- 検証結果：editor の全体テスト2,585件が成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。


### 表セル内の混在サイズの行内文字揃え（2026-11-08）

- 横書きセルの `w:textAlignment` の `top`／`center`／`bottom` で、共有基準線の移動に加え、各文字のフォント上昇・下降量から個別の上下位置を補正する。異なるフォントサイズの文字も、行内の共通の上端・中心・下端へ揃える。
- 描画、選択範囲、行内画像、自動ハイフンに共通の補正を使う。同じ補正量の隣接文字はまとめ、文字のつながりを保つ。`auto`／`baseline` の指定は従来の基準線配置を保つ。
- 参照仕様：[TextAlignment の説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.textalignment?view=openxml-3.0.1)。Word実機との座標比較と縦書きは未検証であり、Wordとの完全な一致を示すものではない。
- 検証結果：関連4テストとeditor全体2,587テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 本文の混在サイズの行内文字揃え（2026-11-08）

- 横書き本文の `w:textAlignment` の `top`／`center`／`bottom` にも、各文字のフォントボックスを共通の上端・中心・下端へ揃える補正を適用する。個別の文字上げ・文字下げは補正後も保持し、共有する書式キャッシュは変更しない。
- 文字描画、選択範囲、文字装飾、タブリーダー、編集記号に補正を反映する。行内画像・テキストボックス・数式も各オブジェクトの上下端から位置を補正する。`auto`／`baseline` は従来の配置を保つ。
- 異なるサイズのArial文字、文字上げ、選択範囲、描画キャッシュの再利用、高さの異なる行内画像の位置を回帰テストで検証する。Word実機との座標比較、縦書き、複雑な混在書体・ルビとの組み合わせは引き続き検証対象。
- 検証結果：関連59テストとeditor全体2,589テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 本文の同一書式内での混在書体の行内文字揃え（2026-11-08）

- 横書き本文の `w:textAlignment` の `top`／`center`／`bottom` で、一つの文字書式内でも欧文・東アジア文字などのフォントボックスが変わる箇所ごとに上下位置を補正する。同じボックスの隣接文字はまとめ、文字のつながりを保つ。
- 元の文字位置、表示順、先頭の追加送り、選択範囲、描画キャッシュを保持する。不完全なクラスタ情報では従来の処理へ戻し、通常の配置では分割しない。ルビの分割片は継続文字として扱い、注音を重複させない。
- ArialとMS Minchoを一つの書式に指定した本文で、上端・中心・下端、選択範囲、再描画の一致を検証する。分割単位のテストでは逆順の文字位置とルビ情報の保持も確認する。Word実機との座標比較、縦書き、複雑なルビの配置は引き続き検証対象。
- 検証結果：関連33テストとeditor全体2,594テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### テキストボックス段落の行内文字揃え（2026-11-08）

- テキストボックスの段落書式から `w:textAlignment` を描画設定へ渡し、横書きの `top`／`center`／`bottom` を行の余白と各文字のフォントボックスへ反映する。同じ書式内の欧文・東アジア文字や異なる文字サイズも共通の上端・中心・下端へ揃える。
- 選択範囲と文字描画で同じ補正を使い、蛍光ペン・下線・取り消し線・スペルチェックの位置も補正後の基準線へ追従させる。`auto`／`baseline` は既存の配置を保持する。
- 行に余白のあるテキストボックスで、混在書体・文字サイズ、選択位置、再描画の一致を回帰テストで検証する。Word実機との座標比較、縦書き、ルビなどテキストボックス内の残る機能は引き続き改善対象。
- 検証結果：テキストボックス関連61テストとeditor全体2,595テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### テキストボックス内のルビ描画（2026-11-08）

- テキストボックス内の `w:ruby` で、確保していた親文字の幅に読み仮名も描画する。本文と共通のルビ描画処理を使い、中央配置などの配置指定、文字色、`w:hpsRaise` の上げ量を反映する。
- 親文字の途中で太字などの書式が変わっても、継続する文字全体へ一度だけルビを配置する。明示した行内上下揃えでは、親文字の補正後の上端も使う。ルビのある行は共有レイアウトのフォントボックスと行送りを使う。
- ルビは編集可能な親文字の位置へ混ぜず、補助表示として実際の描画座標を記録する。回帰テストで通常・上端・中央・下端揃え、上げ量指定の有無、書式の異なる親文字、再描画時の位置一致を検証する。
- Word実機との座標比較、縦書き、上げ量を省略したルビの複雑な行送り、改行をまたぐ親文字は引き続き検証・改善対象。
- 検証結果：追加したルビ回帰テストとeditor全体2,596テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 上げ量を省略したルビの高さと行送り（2026-11-08）

- `w:hpsRaise` のないルビで、読み仮名のフォント上端と親文字からの間隔を共有レイアウトの高さへ加算する。本文とテキストボックスの描画は、拡張前の親文字の上端を使い、高さの確保によってルビが二重に上へ移動するのを防ぐ。
- テキストボックスの通常の行送りには、ルビを含む文字ボックスの高さを下限として反映する。固定行間 (`exact`／`exactly`) はこの下限の対象に含めない。
- 親文字より大きい読み仮名を使い、通常・上端・中央・下端揃えで、本文とテキストボックスの連続する行のルビが割り当てた高さに収まることを検証する。テキストボックスは固定行間の維持も確認する。
- 本文の固定行間で内容が高い場合に行送りが広がる既存動作、Word実機との座標比較、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：ルビ関連14テスト、共有レイアウト全体725テスト、editor全体2,597テストが成功（editor失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 本文の固定行間とクリップ（2026-11-08）

- 本文の固定行間 (`exact`／`exactly`) は、文字・ルビ・行内オブジェクトが高くても指定された行送りを使う。段落末尾の編集記号や番号の高さも、固定行間の行高計画を広げない。 段落と浮動オブジェクトの位置関係を計算する共有の高さ見積もりも、同じ行送りを使う。
- 通常の行で内容全体へ広げていた描画クリップを、固定行間では行の上下端に制限する。新規描画と描画キャッシュに同じクリップを記録する。Macの既存の基準線位置は両表記で適用する。
- 参照仕様：[SpacingBetweenLines.Line の説明](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.spacingbetweenlines.line?view=openxml-3.0.1)。高さ不足時の切り取りと、行に余裕がある場合の中央配置を本文へ反映する。
- Word実機との座標比較、縦書き、ドロップキャップと固定行間の組み合わせは引き続き検証対象。
- 検証結果：固定行間関連5テスト、共有レイアウト全体725テスト、editor全体2,598テストが成功（失敗0件、共有レイアウト12件・editor21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表セルの固定行間とクリップ（2026-11-08）

- 横書きの表セルで、固定行間 (`exact`／`exactly`) の行送りと段落高を、行内画像の自然な高さで広げない。ページ途中で段落を分割しても、この指定と行高を引き継ぐ。
- 固定幅・自動調整の両方で文字の基準線を計算し、文字・装飾・行内画像を指定した行高へクリップする。共通の描画経路を使う繰り返し見出し行にも適用する。通常行間では画像の高さを確保する。
- 回帰テストで、大きい画像を含む行の高さ、分割後の高さ、Windows／Macの基準線、および固定幅・自動調整での選択範囲と描画クリップを検証する。
- Word実機との座標比較、縦書き、回転文字、固定行間と特殊な配置指定の組み合わせは引き続き検証対象。
- 検証結果：追加した固定行間の2テストとeditor全体2,600テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表セル内のルビ描画と行高（2026-11-08）

- 横書きの表セルで `w:ruby` の読み仮名を描画する。本文・テキストボックスと共通の描画処理を使い、配置、文字色、`w:hpsRaise` の上げ量を反映する。
- 親文字の途中で太字や文字サイズが変わっても、一続きの親文字の幅へ読み仮名を一度だけ配置する。明示的な上下揃えと文字の上げ下げを反映した親文字の上端も参照する。
- 通常行間では共有レイアウトが計算したルビを含む高さを確保し、固定行間は指定された高さと行クリップを保つ。固定幅・自動調整、上下揃え、上げ量の有無、再描画を組み合わせて検証する。
- Word実機との座標比較、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：追加したルビの回帰テストで32通りの条件を確認し、editor全体2,601テストが成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 表セル内ルビの座標取得（2026-11-08）

- 横書きの通常セルと繰り返し見出し行で、描画した読み仮名を補助ストーリー `ruby` として記録する。文字ごとの実際の描画位置と行の基準線を再利用し、親文字の編集範囲へ読み仮名を追加しない。
- 座標取得の無効・補助・全体モードを切り替えながら描画位置を比較し、再描画で記録が重複しないことを確認する。繰り返し見出し行ではページごとの絶対座標と、編集用段落を複製しないことも確認する。
- 縦書き・回転文字、改行をまたぐ親文字、Word実機との座標比較は引き続き改善・検証対象。
- 検証結果：通常セルの32条件を取得3モードで確認し、繰り返し見出し行のページ別座標テストも成功。editor全体2,602テスト成功（失敗0件、21件除外）、`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### ルビ専用フォントの継承とテーマ解決（2026-11-08）

- 読み仮名の `w:rFonts` にある文字種別の名前・テーマ参照・`w:hint` を保持する。同じスロットにテーマ指定がある場合、読み込み時にキャッシュされた名前を除き、文書テーマから解決する。
- 親文字にテーマ指定がない場合も、読み仮名のテーマ参照を解決する。共有レイアウトでは通常文字と同じスロット別の継承規則を使い、読み仮名の名前指定によって親文字のテーマ指定も置き換える。
- 改行・段落描画・適用書式のキャッシュにルビの全フォントスロットとヒントを含め、フォント指定だけの編集で古い結果が再利用されないようにする。
- Word実機とのフォント別座標比較、読み仮名の複数書式、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：ルビ関連35テスト、reader全体675テスト、layout全体726テスト、editor全体2,603テストが成功（失敗0件、全体で33件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 読み仮名内の複数書式（2026-11-08）

- `w:rt` の各文字列と文字書式を個別に保持する。読み仮名の途中で文字サイズ・フォント・色・太字・斜体が変わっても、最後の書式で全体を上書きしない。読み仮名は編集対象の本文へ混入しない。
- 共有レイアウトは各区間の幅を合計し、最大の上端を行高へ反映する。明示サイズのない区間は `w:hps`、未指定なら親文字の半分を使う。テーマフォントも区間ごとに解決する。
- 描画は区間ごとの書式を使い、キャッシュへ同じ書式と座標を記録する。読み仮名の書式だけを変えた場合も改行・段落描画・適用書式のキャッシュを更新する。
- 本対応は通常の横書き描画と共有レイアウトを対象とする。Word実機との複数書式の座標比較、読み仮名の文字スタイル参照・下線等の装飾、縦書き、親文字の改行分割は引き続き改善・検証対象。
- 検証結果：新規回帰テスト6件を含め、reader全体677件、layout全体728件、editor全体2,605件、計4,010件が成功（失敗0件、33件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 読み仮名の文字スタイル参照（2026-11-08）

- 読み仮名の各 `w:rStyle` を文字スタイル定義から解決し、`basedOn` の継承と太字・斜体等のトグル規則を適用した後、区間自身の直接書式を優先する。元の読み仮名データは変更しない。
- スタイルでサイズを指定した区間はそのサイズを使い、未指定の区間はルビの `w:hps` または親文字の半分を引き続き使う。存在しないスタイル参照はそのまま保持する。
- 解決済みのルビ書式をヘッドレスの段落入力にも渡す。画面側はスタイル解決後にテーマフォントを解決し、スタイル定義の更新後も描画キャッシュを再利用しない。
- 文字スタイルより直接書式を優先する順序は、[MicrosoftのRunProperties仕様](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.wordprocessing.runproperties?view=openxml-3.0.1)を参照。Word実機との読み仮名座標比較、装飾、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：新規回帰テスト3件を含め、reader全体678件、layout全体729件、editor全体2,606件、計4,013件が成功（失敗0件、33件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 読み仮名の下線・取り消し線（2026-11-08）

- 読み仮名の各書式区間に指定された下線、一重・二重取り消し線を描画する。区間のフォントサイズと文字色、下線の種類・独自色を反映し、親文字の装飾は未指定の読み仮名へ引き継がない。
- 線の範囲はルビ配置後の文字座標から求め、均等配置の字間を含める。単語のみの下線は空白を除いた範囲へ描画する。
- キャッシュ再描画にも同じ装飾命令を記録する。回帰テストは複数書式・均等配置・単語下線について、実描画と命令再生の線の座標・色・太さを比較する。
- Word実機との座標比較、その他の装飾、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：新規回帰テスト1件（通常下線・単語下線の2条件）を含むルビ関連21件と、editor全体2,607件が成功（失敗0件、21件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。

### 読み仮名の文字位置と上下範囲（2026-11-08）

- 読み仮名の各区間の上付き・下付きと `w:position` を、ルビ自身の持ち上げ量へ加算して描画する。下線・取り消し線とキャッシュ命令にも同じ位置を渡す。
- 共有レイアウトは、位置指定後の読み仮名の上端・下端を行のフォント範囲へ含める。`w:hpsRaise` の指定時と省略時の両方で、上げた区間・大きく下げた区間を計算する。
- 読み仮名の縦方向の寸法は各文字に選択されたフォントで測定する。レイアウト記録も、ベースラインが異なる区間を別フラグメントとして記録する。
- 前項の装飾テストの均等配置指定を内部の小文字表記へ修正し、字間が実際に拡張された状態で線とキャッシュ再描画を比較する。
- Word実機との新しい位置指定の座標比較、その他の装飾、縦書き、改行をまたぐ親文字は引き続き改善・検証対象。
- 検証結果：新規回帰テスト2件を含むlayout全体730件、editor全体2,608件、計3,338件が成功（失敗0件、33件除外）。`jade-web` のWASMチェックと両リポジトリの差分空白チェックも成功。


### Word実機に合わせた読み仮名の位置指定の修正（2026-11-08）

- Word for Mac 16.114.3から、通常・2pt上げ・3pt下げ・上付き・下付き・上付きと2pt上げ併用・30pt下げの7条件をPDFへ出力し、表示画像と文字ごとのベースラインを確認した。
- 読み仮名内の `w:position` はWordで位置に影響しなかった。30pt下げても親文字より約15pt上に描画され、次行との間隔も通常と同じだった。上付きと2pt上げの併用も、上付きだけの相対ベースラインと一致した。
- この実測に従い、前項で追加した `w:position` の反映を修正した。解析済みの元の指定値は保存用に保持し、読み仮名の描画・装飾・行高計算用の書式からだけ除外する。上付き・下付きは引き続き反映する。
- 再現用DOCX、Word出力PDF、各文字の座標JSON、生成・出力・抽出スクリプトを `jade-core/tests/word-oracle/mac-probes-20261108/ruby-position/` に保存した。JSONには入力とPDFのSHA256を記録する。
- 今回の実測範囲は横書き・明示的な持ち上げ量のあるルビ。Wordとnativeの全座標一致、持ち上げ量省略、縦書き、改行をまたぐ親文字は未検証。
- 検証結果：元データ保持、位置指定によらない行高、上付き・下付きの描画・装飾・座標記録を含むルビ関連36件（editor 22件、layout 14件）が成功。`jade-web` ライブラリのWASMチェックと両リポジトリの差分空白チェックも成功。WASMの全ターゲット指定では既存のネイティブ用 `word_glyph_sweep` 補助CLIが依存関係エラーになるため、Web本体は `--lib` で検証した。

### Word実機PDFとのルビ相対ベースライン比較（2026-11-08）

- 保存済みWord PDFとOffice Arialを使うネイティブ統合テスト `ruby_word_positions` を追加。元DOCX・PDFのハッシュと実フォントを検証し、原点を補正せず親文字とルビの相対ベースラインを7条件で照合する。
- **未解消の差**：下付きルビの間隔はWordが14.40pt、現実装が14.00ptで、許容差0.375ptを超える。他6条件は許容差内。フォント依存かルビ固有かを追加計測する必要があり、描画値・許容差は変更していない。このテストは既知の差とOfficeフォント要件を明記して既定では除外し、明示実行では失敗する。
- 計測値・再現手順・両描画画像は `jade-core/tests/word-oracle/mac-probes-20261108/ruby-position/` に保存。PNGの目視確認も実施した。絶対座標・行間・文字幅まで一致したという結果ではない。
- 下線・取り消し線7条件の別プローブも準備したが、Macがロック中でWord PDFの出力待ち。こちらの互換性は未検証。

- 追加計測用にArialの通常文字／ルビ×上付き／下付き×6〜30pt（0.5pt刻み）の196条件を `mac-probes-20261108/arial-script-sweep/` に準備し、DOCXのXMLと条件数を確認した。Wordでの出力・計測は未実施。装飾線プローブの出力プロセスはAppleEventタイムアウト（-1712）で終了しており、現在バックグラウンドで出力中ではない。
