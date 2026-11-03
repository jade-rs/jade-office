# Windows Word coordinate verification

The installed reference application is **Word 16.0.19127.20860**. Captures open
DOCX files read-only and preserve source/application/PDF/EMF hashes. Different
Word builds and repeated runs get distinct version/capture directories.

The indexed corpus covers **15 unique documents, 93 pages and 17,139 visible
non-whitespace characters**. Both absolute axes must match within 0.375pt;
text, order and page count must also match. No origin alignment, fitted scale
or relaxed tolerance is used. See
[the engine evidence inventory](../jade-core/tests/word-oracle/windows/README.md)
for the original data and capture commands.

Windows corrections include requested-font line boxes, Yu Mincho/Yu Gothic
OS/2 metrics, grid pitch, fixed CJK widths/punctuation, numbering labels and
separators, and Times New Roman/Japanese quarter-em boundary spacing. Headless
layout also respects omitted sizes, leading blank paragraphs and keepNext.
Each platform-specific change retains the existing Mac policy and goldens.

Word drawing origins are captured directly through `Page.EnhMetaFileBits`.
All 10,908 characters of the two Yokohama documents, contract, numbering-restart
and Yu Gothic probe pass against these origins. PDF exports and failed PDF
comparisons are retained too: PDF glyph rounding introduces subpoint differences
in some long lines, so matching Word drawing origins does not imply those
PDF-origin comparisons passed.

Local verification after the boundary-spacing correction:

- Native layout: 760 passed, 13 existing ignored.
- Native renderer library: 11 passed.
- Mac coordinates: 9 passed, 5 existing licensed Office-font-dependent ignored.
- Windows coordinates: 2 passed, including every installed-font X/Y reference.
- PDF fixtures: 45 passed.
- Capture integrity: 10 passed; EMF extraction/rejection: 8 passed.

Core Actions runs an independent Mac coordinate gate on `macos-latest` for
every PR. Deviations fail the job. A separate `windows-latest` gate reconstructs
EMF references and tests portable Windows geometry with bundled stand-ins;
it includes 8,438 complete X/Y characters plus numbering boundaries and
controlled baselines. Full installed-font comparisons run on this Word machine.
Parent Actions runs its own Mac coordinate gate because submodule workflows
are not triggered by parent PRs. Neither gate allows failure or refreshes Mac
snapshots. Final-head remote CI/performance results must be checked separately.

Only the installed Word build has been measured; other full product versions
will be retained when captured rather than inferred from this version.
