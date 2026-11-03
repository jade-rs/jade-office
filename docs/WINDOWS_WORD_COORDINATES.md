# Windows Word coordinate verification (2026-11-01)

Reference application: installed Windows Word **16.0.19127.20860**. Rust verification:
stable 1.99.1, native Skia renderer, installed Windows fonts. Captures open
fixtures read-only and preserve source/PDF hashes; they do not resave DOCX.

The current versioned corpus is in `jade-core/tests/word-oracle/windows/`:
ten passing documents, 58 pages, 6,231 visible non-whitespace glyphs. Both raw origin axes match Word's
PDF within 0.375pt; page count, glyph text/count/order are also checked.
There is no origin alignment or fitted scale. A separate portable test checks
Windows-policy baselines using bundled stand-ins, including on Mac CI. The
original unversioned capture is retained and tested too. New captures use the
complete product version and unique UTC/UUID capture directories; they never
overwrite old versions. Executable, DOCX and PDF hashes preserve provenance.

Corrections cover known font line boxes without useFELayout, Windows Yu
Mincho vertical metrics, document-grid centring/pitch, fixed CJK punctuation
and advance heuristics, and numbering separator spaces. Windows font path
lists, generic family lookup and the embedded-font cache also use Windows
conventions. Mac policies retain their original results and golden bytes.

Verification:

- Layout library after merging current main: 742 passed, 13 existing ignored tests.
- Native renderer unit tests: 11 passed.
- Mac coordinate suite: 7 passed, 5 existing Office-for-Mac-font probes ignored.
  Includes 0.01pt coordinate snapshots and comparator rejection tests.
- Windows coordinate suite: 2 passed (full installed-font origins and portable
  baselines), covering every registered version/capture and the legacy data.
- Capture integrity tests: 7 passed (version separation, no overwrite,
  invalid/incomplete data, stale PDF comparisons and failed promotion).
- Formatting and Git whitespace checks passed.
- Targeted Clippy passed with the existing shared test-helper
  `chunks_exact_to_as_chunks` lint allowed locally. An unrestricted all-target
  Clippy run on Rust 1.99.1 reports this newer lint in unchanged test helpers
  and other existing tests. No repository or CI lint allowance was added.

`scripts/test-windows-word.ps1` repeats the coordinate and layout checks with
locked dependencies. It sets the official Skia 0.87 Windows binary URL for
the available Vulkan feature superset; the CPU rendering path is unchanged.
`scripts/check-word-windows.ps1` captures new versioned Word PDFs, extracts
independent origins and compares native output. `archive-word-windows-capture.py`
retains all results, while registering only explicitly selected fully passing
fixtures in the regression index. Word versions other than the installed
16.0.19127.20860 have not yet been captured.

The core CI runs Mac coordinates in an independent macos-latest job, and
registered Windows baselines in a separate windows-latest job. The latter
uses bundled font stand-ins and checks vertical policy; full X/Y installed-font
tests run on this Windows Word machine.
The parent jade-office CI now explicitly runs the Mac coordinate gate too,
because submodule workflows do not execute for parent-repository PRs. Neither
gate permits failure or updates Mac snapshots. No existing Mac golden data,
snapshot tolerance or core CI gate was changed. Real macOS/GPUI execution and
remote GitHub Actions execution have not been performed from this Windows
session. Push was attempted for both repositories but GitHub credentials are
not configured on this computer, so remote publication/Actions are pending.

Remaining evidence is explicit: `pending/contract.json` and its Word PDF are
not counted as passing. All three pages and 1,907 glyphs correspond, and the
median X/Y differences are 0.144/0.157pt, but 111 glyphs still exceed 0.375pt
in at least one axis. The tolerance has not been relaxed to accept them.

New captures also exposed a 1.83pt first-line offset with larger numbering
labels. Windows now includes the label's ascent, while Mac retains its
preceding behavior. A new capture passes all 105 glyphs; the earlier failing
capture/report remains in the archive. Both Ruby probes pass all 1,020 glyphs.
