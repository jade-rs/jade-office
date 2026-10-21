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
(`/canvas.html`) only views Word documents and is not covered. Nothing was
run.

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
| Recent files | 12 paths kept in Application Support | 8 entries in memory, lost on reload | no path to reopen; persistent storage not implemented |
| Rendering | Skia at the window's scale factor; re-rasterised when it changes; all pages resident | tiny-skia on a WebGPU/WebGL2 canvas; 128-row tiles re-uploaded only when changed; at most 8 pages kept as pixels, others as 1/8-size thumbnails; pixel ratio read once | wasm memory (a 174-page file once ran out); Skia unavailable on wasm |
| Text effects | shadows, blur, glow, SVG | **canvas shadows ignored** (shadow and emboss text effects missing); SVG via resvg | tiny-skia painter has no shadow pass |
| Layout platform | the machine's Word (Mac on macOS) | the visitor's OS, or `?word_platform=mac\|windows` | intentional: lay out like the Word the user has — see [compatibility-modes.md](compatibility-modes.md) |
| Document fonts | installed fonts: Office's faces and system faces found by any name (游明朝, ＭＳ 明朝, Calibri…), indexed on a background thread; Apple Symbols for marks; Arial Unicode/DejaVu for Hebrew and Arabic. `JADE_FONTS=bundled` switches to bundled fonts | **bundled only**: Noto Sans JP Regular/Bold and Noto Sans Thai; 21 Office family names alias to Noto Sans JP | no font enumeration on the web (Local Font Access is Chromium-only and permissioned); Office fonts cannot be redistributed; download size. Result: different metrics, so line breaks can differ from native; Korean, Arabic, Hebrew, Devanagari show as boxes |
| UI fonts and font menu | system fonts; menu lists installed families | Noto Sans JP; menu is the built-in Japanese/Latin list; picking a font writes the name but still draws Noto | same limit |
| Typing, IME, selection | full | full; Ctrl works as ⌘; shift-click extends; triple-click selects a paragraph | — |
| Ribbon tabs | Home, Insert, Design, Layout, References, Review, View; context: Table Design, Table Layout, Header & Footer | Home, Insert, Layout, References, Review; context: Table Design, Table Layout | Design and View not ported; Page Background sits under Layout |
| Font, Paragraph, Asian Typography, Ruby, Enclose, Symbol, Date and Time dialogs | yes, with shortcuts | yes; Symbol and Date and Time have no shortcut | not ported |
| Tables | both tabs, Table Properties and table dialogs; ⇧⌘↩ / ⇧⌘⌫ for rows | same tabs and dialogs; no row shortcuts (Tab past the last cell adds a row) | not ported |
| Page setup | ribbon, dialogs, Design tab | same dialogs (文字数と行数, margins, paper, line numbers, columns, page numbers, breaks, cover pages, page colour and borders) | — |
| Headers and footers | edited in place as a full story: several paragraphs, tables, pictures, fields; first-page, odd/even and linked variants; Header & Footer tab | **one-line bar**: left/centre/right zones of the first line only | not ported (needs a second story drawn over the page) |
| Comments and tracked changes | full: new, reply, edit, resolve, delete, pane; ⌘M, ⇧⌘T | same features; no ⌘M or ⇧⌘T | ⇧⌘T reopens a browser tab |
| Fields, TOC, notes, links | TOC insert/update, notes (⌥⌘F/⌥⌘D), ⌘K; DATE/TIME show the current time on open | same, but DATE/TIME show the saved result until changed | field clock not ported |
| Find and replace | ⌘F, ⌘E, ⌘G, ⌃H, ⌥⇧⌘F; bar without buttons | ⌘F, ⇧⌘H; bar with buttons, previous (⇧⏎, ⌘⇧G) | ⌘G/⌘E collide with the browser's find |
| Format painter | ribbon (Clipboard group), ⇧⌘C / ⇧⌘V | ribbon (Editing group), ⇧⌘C / ⇧⌘V | — |
| Pictures | system dialog, wrap, move, resize | picker or drop while a document is active (recognised by content), wrap, move, resize | one shared picker |
| Save | ⌘S writes in place atomically; ⇧⌘S Save As | ⌘S / download button: **always a download**; no Save As; the dirty mark clears even if the download is cancelled | sandbox; File System Access API not used (Chromium-only, needs permission) |
| Unsaved work | dot on the tab; no prompt on close; no autosave | dot on the tab; **no warning on reload**; no autosave | persistence not implemented |
| Export PDF | ⌥⌘P: **vector PDF** with selectable text (Skia) | button: **PDF of page images** (text not selectable), painted page by page | tiny-skia cannot write PDF; bounded memory |
| Print | ⌘P: PDF via `jade-docx-pdf-renderer` (a separate, simpler renderer) then the macOS print panel; other OSes report an error | **none for Word** | no print dialog API; `window.print` would print the canvas |
| Clipboard | writes plain text plus an in-process rich copy; Paste, Cut, Copy buttons, Paste Options, Paste Special (⌃⌘V), Paste and Match Formatting (⌥⇧⌘V) | writes plain text plus the in-process rich copy; ⌘V through the browser's paste event; **no Paste button, Paste Options or Paste Special** | clipboard reads only inside a paste event; neither shell writes HTML/RTF to the system clipboard |
| Undo / redo | ⌘Z, ⇧⌘Z, menu, ribbon | ⌘Z, ⇧⌘Z, ⌘Y, title-bar buttons | no menu bar |
| Hyperlinks | ⌘-click opens the default browser | ⌘/Ctrl-click opens a tab (pop-up blockers may apply) | — |
| Dark mode | System / Light / Dark, saved; separate dark-pages toggle | follows the OS; toggle not saved; pages always turn dark with the UI | not ported |
| Zoom | ⌘= ⌘- ⌘0 ⌘9, View tab, status bar | status bar only; pixel ratio not re-read after browser zoom | browser owns ⌘= / ⌘- |
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
| Align, indent | ⇧⌘L/E/R, ⌘] / ⌘[ | ribbon | not ported |
| Insert/delete table row | ⇧⌘↩, ⇧⌘⌫ | ribbon | not ported |
| Comment, Track Changes | ⌘M, ⇧⌘T | Review tab | key clash / not ported |
| Symbol, Date and Time, Asian Typography | ⌥⇧⌘S, ⌃⇧D, ⌥⇧⌘K | ribbon | not ported |
| Select Paragraph | ⌥⌘A | triple-click | not ported |
| Next/previous page | PageDown/PageUp, View tab | — | not ported |
| Formatting marks | View › Marks | — | not ported |
| Blank Page, Page Break buttons | Insert, Layout | ⌘↩, Breaks menu | not ported |
| Design tab | Page Color, Page Borders | under Layout | layout choice |
| Header & Footer tab, in-place editing | yes | one-line bar | not ported |
| Theme and dark-pages commands | ⌃⌥⌘S/L/D/P, saved | toggle, not saved | not ported |
| Details panel | ⌥⌘I | — | not ported |

The browser has a few of its own: Ctrl as ⌘ everywhere, ⌘Y, picture drop
onto the page, `?open=` and `?word_platform=`, tile residency.

## To port

Rows of the reference list that are full in the native app but whose UI the
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
