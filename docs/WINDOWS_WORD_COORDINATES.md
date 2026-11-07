# Windows Word coordinate verification

The installed reference application is **Word 16.0.19127.20860**. Captures open
DOCX files read-only and preserve source/application/PDF/EMF hashes. Different
Word builds and repeated runs get distinct version/capture directories.

The indexed corpus covers **26 unique documents, 158 pages and 28,995 visible
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
All 11,868 characters of the two Yokohama documents, contract, numbering-restart,
the Yu Gothic probe and both Times New Roman sweeps pass against these origins.
PDF exports and failed PDF comparisons are retained too: PDF glyph rounding introduces subpoint differences
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
it includes 9,398 complete X/Y characters plus numbering boundaries and
controlled baselines. Full installed-font comparisons run on this Word machine.
Parent Actions runs its own Mac coordinate gate because submodule workflows
are not triggered by parent PRs. Neither gate allows failure or refreshes Mac
snapshots. Final-head remote CI/performance results must be checked separately.

Only the installed Word build has been measured; other full product versions
will be retained when captured rather than inferred from this version.

November 7 adds nine deterministic edge fixtures and 10,896 full X/Y drawing
origins: paragraph indents/alignment, decimal tabs, contextual spacing,
run position/width, and all four Arial faces' super/subscripts at 49 sizes.
Windows-only fixes exclude trailing spaces from alignment, anchor decimal
tabs on the decimal point (or text end), preserve contextual-after collapse,
and use Arial's measured script sizes/moves without inflating line pitch.
The Windows Actions gate checks these complete origins using the runner's
installed Arial; the separate Mac gate remains mandatory and unchanged.

All captures are under the full product-version directory
`jade-core/tests/word-oracle/windows/word-16.0.19127.20860/`, each with its
own immutable timestamp/ID. `capture.json` records Word's product/application
version and build, executable hash, OS and original DOCX/PDF/EMF hashes;
`verification.json` records the engine commit and matching/pending fixtures.
Failed captures are kept separately from the registered passing references.
