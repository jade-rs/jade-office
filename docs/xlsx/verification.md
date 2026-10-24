# XLSX parity verification

Current replacement results are in [pivot creation/deletion](pivot-lifecycle.md)
and its [machine-readable proof](pivot-lifecycle-verification.json). Earlier sections
retain historical scopes and counts; they are not additive.


Record started 2026-10-23; later milestones retain their own timestamps.

The first table covers the initial metadata/name-management changes described in
[ibis-parity.md](ibis-parity.md). They do not certify the remaining feature
gaps, or establish pixel/Office parity.

## Verification scope

The core milestone totals in this record cover six selected implementation
crates: `jade-xlsx-reader`, `jade-xlsx-writer`, `jade-xlsx-calc`,
`jade-xlsx-headless-editor`, `jade-xlsx-layout` and `jade-xlsx-render`.
They include all library/integration/doc targets for that selected set.
The workspace also has `jade-xlsx-headless-api`, `jade-xlsx-headless-cli`,
`jade-xlsx-headless-mcp`, `jade-xlsx-diff` and `jade-xlsx-native-render`.
Their own test targets are outside that six-package driver unless separately
recorded. Compilation as a dependency does not replace those test suites.

The workbook-protection milestone below separately runs the API/CLI/MCP/diff
suites and the native renderer's own manifest, in addition to the selected six.
The respective counts remain separate; this does not imply all workspace
packages, feature combinations, examples or benchmarks were executed.

Earlier machine-readable checkpoints used the phrase "all six XLSX crates"
for this selected set; their explicit target arrays show the actual scope.
That phrase did not establish complete workspace coverage. The historical
counts are retained, and the human-readable labels now state the selection.
The broader parity goal remains open.

The protection-authoring follow-up uses the unchanged package versions with
explicit test/dev opt-level=1 to share actual core/model build artifacts.
The only dependency-list addition is `encoding_rs` in `jade-opc`, for legacy
password code-page conversion. Runtime gates retain their actual manifests,
lockfiles and assertions; profile and intermediate follow-up scopes are
recorded separately in its machine-readable proof.

| Check | Result |
|---|---|
| XLSX reader, writer, calc, headless editor, layout and render | 780 passed, 0 failed, 129 test targets including doc-test targets |
| Vendored calculation engine library | 2,238 passed, 0 failed, 8 pre-existing ignored tests |
| Shared model: names, links, protection, typing, tables | 10 passed, 0 failed |
| Print/PDF behavior on both tiny-skia and Skia | 12 passed, 0 failed |
| Native view: name-box creation/navigation and tab hide/unhide commands | 1 passed, 0 failed |
| Native shell compile | Passed |
| Browser shell, wasm32 | Passed with Rust 1.99.0 |
| Vendored source patch replay | The canonical `jade.patch` applied to the original ironcalc_base 0.8.3 archive; all 486 source/build files match the vendored tree |
| Whitespace checks in both repositories | Passed |

Rust 1.93.1 was used except for the browser shell, which requires 1.99.0.
The build used a separate target directory and temporary Cargo cache on a
volume with available space; no dependencies or toolchains in the repository
were changed to bypass a check. An early attempt with an inconsistent compiler
selection was discarded. The final engine checks ran against the real
`jade-core/Cargo.toml` and its lockfile, not the temporary diagnostic workspace.

The original upstream tests treated unqualified name formulas as invalid and
expected `#NAME?` after deleting the sheet behind a name. Those expectations
were updated to the supported expression syntax and `#REF!` propagation;
malformed formulas remain rejected. Jade's corresponding integration test
now verifies the displayed error and current cache state.

## Initial commands

From the jade-office root, with the repository's toolchains selected:

```sh
cargo test --locked --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-reader -p jade-xlsx-writer -p jade-xlsx-calc \
  -p jade-xlsx-headless-editor -p jade-xlsx-layout -p jade-xlsx-render \
  --no-fail-fast
cargo test --locked --manifest-path jade-core/third_party/ironcalc-base/Cargo.toml --lib
cargo test --locked --manifest-path crates/jade-office-model/Cargo.toml \
  --features tiny-skia --test names_like_excel --test links_like_excel \
  --test protected_sheets_like_excel --test typing_like_excel --test tables_like_excel
cargo test --locked --manifest-path crates/jade-office-model/Cargo.toml \
  --features tiny-skia,skia --test print_like_excel
cargo test --manifest-path crates/jade-gpui-app/Cargo.toml \
  names_and_tab_commands_work_through_the_view
cargo check --manifest-path crates/jade-gpui-app/Cargo.toml
cargo +stable check --manifest-path crates/jade-gpui-web/Cargo.toml \
  --target wasm32-unknown-unknown
```

Run the browser command from its crate directory to use its pinned toolchain,
or explicitly select a Rust 1.99.0 installation. The `+stable` spelling above
was 1.99.0 in this environment and is not a permanent version pin.

No Excel GUI repair check, newly captured Excel calculation oracle or visual
pixel comparison has been performed for these changes. Page-setting writers
and tab-colour model APIs still need their full editing UI; see the gap table.

## Input-rule and hyperlink milestone

The final 2026-10-23 checks for [validation](validation.md) and
[hyperlinks](hyperlinks.md) cover the same current tree as the initial changes
above, with these additions:

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, real core manifest and lockfile | 800 passed, 0 failed, 132 targets including doc tests |
| Entire shared model with tiny-skia (including sheet, Word and deck regressions) | 333 passed, 0 failed, 33 targets including doc tests |
| Native worksheet scenarios, including validation dialog/alerts/IME and hyperlink edit/remove | 35 passed, 0 failed |
| Vendored calculation engine library | 2,238 passed, 0 failed, 8 pre-existing ignored tests |
| Native production compile | Passed |
| Browser production compile, wasm32 | Passed with Rust 1.99.0 |
| Canonical vendored patch replay | 486 source/build files match the original archive plus `jade.patch` |
| Rust formatting of changed Rust files and whitespace in both repositories | Passed |

Core additions include six data-validation persistence tests, three calculation
validation tests and eleven hyperlink tests. Shared-model scenarios include
seven validation and three link tests; two of the latter are new authoring
cases. The native worksheet suite now exercises validation choices, multiline
messages, IME composition and link commands as well as the existing grid,
clipboard, format, chart and file operations.

The first broad run exposed a regression in a pre-existing fixture whose
`r:id` lacked an `xmlns:r` declaration. The reader retains that legacy
convention; explicitly foreign namespace bindings remain foreign. Editing the
legacy link now replaces its unbound attribute with a correctly bound ID.
The original regression and a dedicated repair/foreign-attribute case pass.
The spill tests also cover formatting-only links and explicit labels that
block an output; format edits no longer count as writes into array results.

Commands from the jade-office root (offline was used with a populated cache):

```sh
cargo test --locked --offline --manifest-path jade-core/Cargo.toml \
  -p jade-xlsx-reader -p jade-xlsx-writer -p jade-xlsx-calc \
  -p jade-xlsx-headless-editor -p jade-xlsx-layout -p jade-xlsx-render \
  --no-fail-fast
cargo test --locked --offline --manifest-path crates/jade-office-model/Cargo.toml \
  --features tiny-skia --no-fail-fast
cargo test --offline --manifest-path crates/jade-gpui-app/Cargo.toml workbook::tests::
cargo test --locked --offline --manifest-path jade-core/third_party/ironcalc-base/Cargo.toml --lib
cargo check --offline --manifest-path crates/jade-gpui-app/Cargo.toml
cargo +stable check --offline --manifest-path crates/jade-gpui-web/Cargo.toml \
  --target wasm32-unknown-unknown
```

The browser check emitted a non-fatal `rust-objcopy` debug-stripping warning
about the installed toolchain's `libLLVM.dylib`; compilation succeeded without
a repository dependency change or a skipped check. This milestone has no Excel
application save/repair or newly captured calculation/pixel oracle. Those are
still separate acceptance gates, along with the functional gaps in the audit.

## Table-authoring milestone

The table work adds creation, resizing, style/header/filter options, calculated
columns, all eight built-in totals, custom totals, renaming and conversion to a
normal range. Both shells expose the shared table dialog and Command/Ctrl+T.
See [tables.md](tables.md) for the implementation boundaries.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, real core manifest/lockfile, final snapshot-paste changes included | 815 passed, 0 failed, 0 ignored; all 134 library/integration/doc targets |
| Table-authoring integration target (included above) | 14 passed, including save/reopen, exact history, namespace/metadata preservation, entry/snapshot expansion, calculated columns, totals and conversion |
| Existing internal clipboard and table/shape targets (included above) | 11 + 9 passed |
| Shared model and all four table scenarios, tiny-skia | Type checking passed; runtime not executed |
| Native production and all native test targets | Type checking passed, including final snapshot-paste follow-up; runtime not executed for the new table UI scenario |
| Browser production, wasm32, Rust 1.99.0 | Passed for the table UI/model before the final platform-independent snapshot-paste writer follow-up |
| Formatting and whitespace in both repositories | 74 changed/new Rust files and `git diff --check` passed |

The full core results and formatted source hashes are recorded in
[table-verification.json](table-verification.json). The earlier 812-test run
also passed before the three snapshot-paste scenarios were added. The final
815-test run covers their writer changes too; these numbers are successive
checkpoints, not additive totals. The previously verified engine patch was
not changed during table authoring.

The external SSD disappeared during this work. Core targets were run one at a
time, removing only task-private test executables and completed-package cache
variants to stay within the remaining disk space. Two interrupted targets did
not execute because linking/debug stripping exhausted storage. Both were
rebuilt and passed, and the remaining targets then completed. Debug stripping
was disabled for test executables after the first interruption; the actual
compiler, dependency graph, source files and test assertions were retained.
Native checks relocated metadata output from the full shared target and
symlinked it back; no dependency or source was bypassed. A cached non-fatal
jobserver warning was emitted by one dependency.

The commands for the final core run use the initial six-crate command above,
split into `-p PACKAGE --lib`, `-p PACKAGE --test TARGET` and `-p PACKAGE --doc`
invocations with `--locked --offline`, one job, incremental compilation off and
dev/test debug information off. Native verification used `cargo check --tests`;
shared table verification used `cargo check --features tiny-skia --test
tables_like_excel`. These compile checks are not recorded as executed tests.

The new shared/native scenarios and both table dialogs still need runtime
exercise. No real Excel save/repair, calculation or visual oracle was captured.
Table-only cell shifts, imported edge cases, custom style authoring and the
other gaps in the audit remain open.

## Cell-shift and structural-header milestone

Cell insertion/deletion now moves only the selection's column/row strip,
including its cells, formulas and standard metadata, and is available in both
context menus. Creating table headings uses this structural writer instead of
copying values inside the selection. Occupied cells below the new table move
with their notes, links and absolute/dependent references. Coverage and limits
are described in [cell-shifts.md](cell-shifts.md).

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, real core manifest/lockfile | 833 passed, 0 failed, 0 ignored; all 136 library/integration/doc targets |
| New cell geometry/reference cases (included above) | 6 passed |
| New headless cell-shift/header-insertion cases (included above) | 12 passed |
| Shared table/model scenarios with tiny-skia, including cell shifts | 5 passed, 0 failed, 0 ignored |
| Native production and all native test code | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed on the same source |
| Formatting and whitespace | 79 changed/new Rust files and both `git diff --check` checks passed |

[cell-shift-verification.json](cell-shift-verification.json) records the 136
target results, source hashes and individual runtime/compile scopes. This
833-test checkpoint replaces the earlier 815-test checkpoint for the current
core source; the totals are not additive. The shared-model test used its actual
manifest/lockfile, Rust 1.93.1, tiny-skia, opt-level 1 and debug information off.
Its optimized build reduced temporary storage needs; no dependency or source
was removed to obtain the runtime result.

The core runner exhausted storage while recording its progress JSON after
target 111. The first 110 successful exits were recovered from the append-only
log. After deleting only completed-package variants in the task-private cache,
the unrecorded target was rerun and the remaining targets passed. The recovered
report was written atomically. No failed/unexecuted target was counted as a
passing test. Native compile output was relocated as in the table milestone;
the browser and model runtime builds used task-private targets. The canonical
vendored engine patch was not changed in this milestone.

Native screen tests were attempted twice with the real native manifest and
lockfile at opt-level 1. Dependency compilation exhausted storage first in
`objc2-foundation` metadata and then in the `naga` archive, even after removing
222.1 MiB of task-private variants outside the native dependency graph. The
new native scenarios did not execute; successful native type checking is not
counted as runtime proof. The completed/failed private build cache was removed
after both cargo processes exited, retaining logs and evidence. Manual
context-menu interaction also remains unverified. x14/extension reconciliation,
table calculated-column fill after band insertion, and the other imported edge cases listed in the cell
shift notes remain. No Excel save/repair, calculation or pixel oracle was
captured. The full parity goal is not complete.

## Calculated-column origin and preservation milestone

Structural insertion now fills recorded table formulas only into new data
rows. Retained literal, blank and formula exceptions are left in place. The
saved template is rebased when the first data row or header changes, using
retained column IDs and the original formula before reference rewriting. The
same implementation serves whole-axis and partial-band edits. Templates in
other sheets also follow their changed dependencies. Formula copy/transpose
now skips complete structured selectors, including column names containing
cell-like text such as `Cost A1 USD`. No-op child edits preserve the original
XML bytes of an untouched ordinary sheet.

Four added cases reproduced failures in new-row fill, first-row origins and
unaffected-sheet byte preservation before the fix. A further structured-name
case failed before bracket-aware translation was added. The corrected cases
pass alongside the existing suite.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 842 passed, 0 failed, 0 ignored; all 136 library/integration/doc targets |
| Headless cell-shift target (included above) | 20 passed, covering new-row fill, retained exceptions, first-row/header/all-data deletion, column IDs, absolute/structured/cross-sheet templates, future extension, save/reopen and history |
| Reader table-reference target (included above) | 2 passed, including copy/transpose over a cell-like column name |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed |
| Shared-model table/cell-shift target, tiny-skia | 6 passed, 0 failed, 0 ignored, including new-row fill/cache/history |
| Rust formatting and whitespace | 81 changed/new Rust files and both repository whitespace checks passed |

The current target results and source hashes are in
[table-template-verification.json](table-template-verification.json). The 842
tests supersede the earlier 833-test core checkpoint; counts are not additive.
The compiler, actual manifests/lockfiles, source and assertions were retained;
targets ran sequentially with task-private executable cleanup as before. No
vendored calculation-engine source or patch was changed.

The new shared-model scenario exercises calculated fill, cache refresh and
Undo/Redo through the model used by both shells. Its runtime result is recorded
separately in the machine-readable evidence. Native screen runtime was not
retried: the preceding two dependency-build attempts exhausted storage. The
x14/paired-extension, old table-geometry namespace, imported/grid-edge formula
and real Excel oracle gaps remain; see [cell-shifts.md](cell-shifts.md).

## Extended-rule coordinate and aggregate-population milestone

Partial cell shifts now keep each aggregate conditional-format population
intact over its mapped union. Splitting a data bar or colour scale into
independent rule populations previously changed its min/max scaling. Relative
predicates still split at translation boundaries. Standard formula-valued
CFVO attributes also follow the structural edit.

The preserving writer reconciles paired x14 data bars with their legacy
coverage/IDs, keeps standalone extended priorities in the same order as
standard rules, and renews cloned revision UIDs and standalone IDs without
colliding with package IDs. An explicit x14 priority takes precedence over its
otherwise matching legacy link ID. Extended validation records follow
coverage, both formula bounds, relative anchors and counts. Sparkline display
cells move/delete without filling inserted blanks; source/date-axis formulas
follow their worksheet context. Strict/default namespace aliases and foreign
lookalikes are covered. These changes do not expose x14 input rules in the
reader or enforce them during editing.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 851 passed, 0 failed, 0 ignored; all 137 library/integration/doc targets |
| New writer extension target (included above) | 8 passed, including both axes, paired/standalone rules, mixed priorities, aggregate union, CFVO formulas, validation identities/counts, sparklines, cross-sheet source references and namespace isolation |
| Headless x14 data-bar target (included above) | 3 passed; the added partial-shift case verifies preserved signed geometry/properties, save/reopen and exact Undo/Redo |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 84 changed/new Rust files and both repository whitespace checks passed |

[x14-verification.json](x14-verification.json) records target results,
source hashes and manifest/lockfile hashes. The 851-test checkpoint supersedes
the 842-test core checkpoint; counts are not additive. Source remained frozen
during the full run and both compilation checks. Actual dependency graphs,
source, assertions and toolchain selection were retained. No vendored engine
source or patch changed in this milestone.

Shared-model runtime was not repeated; its six table scenarios last passed
at the calculated-template checkpoint above. New native screen scenarios and
manual context-menu use remain unverified after the earlier storage-limited
runtime build attempts. No Excel save/repair, calculation or pixel oracle was
captured. x14 input interpretation/enforcement, other imported extension
families and the broader parity audit remain open. The parity goal is active.

## Extended input-rule reading and enforcement milestone

The reader now recognizes x14 validation coverage and wrapped formulas in
worksheet extension containers. A shared record-location list gives the
preserving writer the same indices as the reader, including standard/x14
mixes, multiple containers and missing-range siblings. Indexed replacement
keeps the original storage form and opaque metadata. Clear/apply operates
across both forms; retained relative bounds are rebased. Adding missing formula
wrappers respects schema order and empty recognized containers are removed.
Appending a standard rule now returns its actual reader index before extended
records. Existing dialog apply uses the standard authoring form over the
selected coverage; imported x14 rules use the same input model and validator.

Five added scenarios failed before x14 interpretation/edit routing was added.
Two more reproduced missing-bound schema ordering and misreading an irrelevant
x14 `sqref` attribute. All seven pass in the final full regression.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 858 passed, 0 failed, 0 ignored; all 138 library/integration/doc targets |
| New headless extended-validation target (included above) | 7 passed: mixed reading/evaluation, preserving edit/delete, append index, overlap clearing/replacement, Strict/default namespaces, multiple containers/unmodeled siblings, missing-bound order and namespace/attribute exclusion |
| Shared-model validation target, tiny-skia | 8 passed, 0 failed, 0 ignored; imported x14 lists/prompts/IME metadata, Stop/Warning flow, dialog initial values, clearing, history and save/reopen covered |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 85 changed/new Rust files and both repository whitespace checks passed |

[extended-validation-verification.json](extended-validation-verification.json)
records target results and source/manifest/lockfile hashes. The 858-test count
supersedes the 851-test core checkpoint; counts are not additive. Actual
manifests, dependency graphs, compiler selection, source and assertions were
retained. Production and core test sources remained frozen during the final
core/native/browser checks. No vendored engine source or patch changed.

The first model runtime attempt retained the original table relationship while
replacing its test worksheet. C1 therefore remained a table heading and stored
`10` as text. The test fixture now removes that stale relationship; the final
eight model scenarios pass. Only this model fixture changed after the full
core/compile checks, and it was compiled and executed afterward. This setup
correction is not reported as a production bug fix.

Native screen runtime, manual context-menu use and real Excel save/repair,
calculation or pixel oracles remain unverified. Quoted-item `x12ac:list`,
AlternateContent and container-level prompt presentation remain open; see
[validation.md](validation.md). The broader parity goal is active.

## Table ownership and namespace milestone

The reader now discovers tables through direct worksheet `tableParts` entries
and recognized internal table relationships, preserving document order and
deduplicating targets. Unlisted/external/wrong-type/foreign links do not
activate local definitions. Unlisted definitions and their formulas remain
preserved through ordinary cell edits and structural shifts. Thus a stale
relationship no longer turns a numeric edit into a table-header string.

Structural geometry now uses the namespace-aware table model and direct
column/filter/sort/formula children. Opaque namesakes and nested extension
formula lookalikes remain untouched. Inserted columns use their container's
namespace scope, and imported maximum unsigned column IDs no longer cause
arithmetic overflow. Duplicate relationship targets move once, with aliases
removed when their table is deleted. Formula-part discovery also excludes
unlisted table definitions and caches ownership per source worksheet.

Seven initial ownership/geometry tests failed before the fix. Additional
cases reproduced opaque formula rewriting and reuse of a sibling's locally
declared prefix. Eleven new core scenarios pass in the final full regression.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 869 passed, 0 failed, 0 ignored; all 140 library/integration/doc targets |
| New headless ownership target (included above) | 6 passed: unlisted numeric/header behavior and byte preservation, external/foreign/wrong-type links, Strict aliases/order/deduplication, template hierarchy, duplicate shifts/deletion, save/reopen and history |
| New writer namespace target (included above) | 5 passed: owned columns/filter/sort metadata, foreign/nested preservation, link removal, local declaration scope and maximum column IDs |
| Shared-model table/cell-shift and validation targets, tiny-skia | 15 passed, 0 failed, 0 ignored: seven table scenarios including the new stale-relationship numeric edit, plus eight validation scenarios |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 88 changed/new Rust files and both repository whitespace checks passed |

[table-ownership-verification.json](table-ownership-verification.json) records
all targets and source/manifest/lockfile hashes. The 869-test core checkpoint
supersedes the earlier 858-test checkpoint; counts are not additive. Production
and test sources stayed frozen through the final full core run, the final model
retest, and native/browser checks. The actual compiler, manifests, lockfiles,
dependency graphs and assertions were retained. No vendored calculation engine
source or patch changed. Completed task-private core/model/browser build caches
were archived and gzip-verified after successful runs to fit available storage;
source, test results and logs were retained.

The native screen scenarios and manual interaction remain unverified. No
Excel/LibreOffice/Numbers bundle was found at the common application locations
inspected; this does not establish absence at every possible location. No
Excel save/repair, calculation or pixel oracle was captured. Other worksheet,
drawing and formula namespace/ownership contexts, broader imported table cases
and the other audit areas remain open. The full parity goal is active.


## Quoted literal validation and source alternatives milestone

The reader, calculator and preserving writer now distinguish decoded
`x12ac:list` CSV values from formula sources. Quoted commas, doubled quotes,
empty entries, spaces, line breaks and ST_Xstring values retain their meaning.
Malformed imported CSV remains preserved but unsupported; malformed authored
sources are refused atomically. Supported Choice/Fallback source alternatives
are selected through resolved namespace requirements. This handling is scoped
to recognized rules and their formula wrappers, not general worksheet MCE.

Source edits update understood branches and fallback together while keeping
opaque branches and metadata. Nested x14 formula-leaf alternatives are edited
in place, without leaving an extra direct formula. Partial structural shifts
rebase formulas in both branches at the new split anchor and leave literal
candidates unchanged. A new rich list has a standard fallback; processors
using only that fallback cannot distinguish an embedded literal comma from
an item separator. Exact rich candidates require the supported choice.

Both shell selectors use the shared model's list-choice method. Formula-like
candidates are entered as text; textual boolean/date candidates can retry as
text when ordinary typed-input validation would reject their coerced type.
The dialog uses CSV and separates a quoted literal beginning with `=` from
a formula source. Disabled-alert/locale/coercion details still need application
comparisons.

The initial five scenarios had four failures before source interpretation was
implemented. A later preserving-edit check exposed a stale selected source.
Two additional tests reproduced duplicate x14 formula leaves and unchanged
relative formulas inside split MCE sources. All eleven integration scenarios
and both new reader codec cases pass in the final full regression.

| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 882 passed, 0 failed, 0 ignored; all 141 library/integration/doc targets |
| New headless quoted-validation target (included above) | 11 passed: literal CSV decoding/evaluation, source selection, preserving edits, Strict/x14 namespaces, authoring/fallback, malformed/atomic rejection, foreign sources, x14 leaf cardinality, structural relative anchors and history |
| Reader codec additions (included above) | 2 passed: exact quoted values and malformed records |
| Shared-model validation and table targets, tiny-skia | 18 passed, 0 failed, 0 ignored: eleven validation scenarios and seven table/cell-shift scenarios |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser production, wasm32, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 90 changed/new Rust files with their Rust 2021 edition and both repository whitespace checks passed |
| English/Japanese translation data | 3,448 unique matching keys per language; TSV structure and placeholder sets checked in Python |

[quoted-validation-verification.json](quoted-validation-verification.json)
records targets, source/build-file hashes and checks. The 882-test checkpoint
supersedes the previous 869-test core checkpoint; counts are not additive.
Production, test and translation sources stayed frozen across the final full
core/model/native/browser runs. Actual manifests, lockfiles, dependency graphs,
compiler selection and assertions were retained. No vendored calculation-engine
source or patch changed in this milestone. Completed task-private build caches
were archived and gzip-verified to bound storage.

The translation data check found identical duplicate `link.address` keys and
a prior goal-introduced `table.formula` collision. The duplicate was removed
and the calculated-column label now uses `table.calculated_formula`, preserving
the generic Word formula label. The whole translation data check passed; the
native Rust i18n test itself was not executed. The first authoring fixture
asserted one lexical namespace shape; it now verifies fallback behavior by
making the newer list namespace unsupported. That fixture correction is not
reported as a production bug fix.

Native screen runtime, manual selector/dialog interaction and actual Excel
repair/save/calculation/pixel oracles remain unverified. General MCE around
worksheets/containers/records, non-list `ac:list`, broader coercion and the
other audit areas remain open. The full parity goal is active.


## Whole-axis validation structure milestone

The original seven new structural scenarios all failed before this fix. They
reproduced deleted relative anchors becoming `#REF!`, unchanged external
relative operands after the first range disappeared, x14 containers being
dropped by the old count pass, foreign/unmodeled formula rewriting, CDATA
sources failing to move, and lost opaque container contents. The replacement
uses reader-owned standard/x14 locations, rebases from the first surviving
original cell before grid adjustment, and consumes input-rule containers as
whole spans so their descendants cannot be shifted again.

The extended checks cover row and column deletion, first-union removal,
qualified references when another sheet changes, exact unknown-choice
preservation in whole-axis edits and partial shifts on the edited worksheet,
empty/opaque containers,
coverage clipping at both grid edges, independent x14 coverage alternatives,
CDATA/no-op byte stability, save/reopen, history and actual candidate results.
Supported coverage/fallback alternatives are moved even when the selected
coverage itself is unchanged. Absolute deleted sources remain `#REF!`.

Source-level comparisons used the pinned ibis structural writer and the
LibreOffice condition/validation update paths as implementation evidence.
No implementation code was copied. The new candidate checks establish the
surviving external bound's behavior; neither source comparison establishes
Excel's application behavior for all imported edge cases. General MCE around
records/containers, noncanonical coverage authoring/partial shifts, broader
conditional-formatting anchors/namespace handling, the package-wide partial-
shift formula pass on other sheets, other extensions and the
full parity goal remain open.


| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 895 passed, 0 failed, 0 ignored; all 142 library/integration/doc targets |
| New headless structural-validation target (included above) | 13 passed, including candidate evaluation, save/reopen and exact history |
| Shared-model validation/table targets, tiny-skia | 19 passed, 0 failed, 0 ignored: twelve validation cases (including standard/x14 row deletion) and seven table/cell-shift cases |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser wasm production, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 92 changed/new Rust files with Rust 2021 edition and both repository whitespace checks passed |

[validation-structure-verification.json](validation-structure-verification.json)
records targets, source/build-file hashes and evidence limits. The 895-test
checkpoint supersedes the previous 882-test core checkpoint; counts are not
additive. The 98 tracked source/build/translation hashes stayed unchanged
through the final full core/model/native/browser checks. Actual manifests,
lockfiles, assertions, compiler selection and dependency graphs were retained.
No calculation-engine source or patch changed during this milestone. The
browser compiler replayed its existing build-script debug-stripping warning
about `rust-objcopy`/`libLLVM.dylib`; `cargo check` completed successfully.
Completed task-private build caches were archived and gzip-verified. Native
screen runtime, manual interaction and Excel application oracles remain
unverified. The full parity goal is active.


## Worksheet formula ownership milestone

The initial six integration scenarios all failed before the fix. They exposed
extra rewrites in foreign/unknown standard and x14 validation sources, stale
standard conditional-threshold attributes, missing x14 rule table context and
missing implicit-cell context during column rename. The common package pass
now delegates worksheets to a namespace/path-aware visitor and passes actual
source anchors to table-reference conversion. Other package-part paths are
unchanged. Source scopes are captured while streaming instead of rescanning
the full worksheet for each cell.

A seventh integration case checks x14 current-row A1 conversion. Three writer
unit cases cover known/unknown cell formula alternatives, invalid replacement
XML text and nontext formula/CDATA/comment preservation. The nontext case
failed before the leaf mapper was corrected: an opaque child was discarded.
Source comments now survive changed text. The cell MCE unit verifies writer
serialization only; it does not certify the cell reader's MCE behavior.

The new model scenario checks x14 table-relative bounds through header rename,
input validation, save/reopen and Undo/Redo. A Rust borrow error in the new
conversion test setup was corrected by snapshotting its expected rules before
calling mutable save; that setup correction is not a production bug fix.

See [formula-ownership.md](formula-ownership.md) for supported paths and limits.
The whole-axis and sheet rename/delete passes, other package-part ownership,
cell/sparkline interpretation, general MCE and Office application oracles
remain separate open work. The full parity goal is active.


| Check | Result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 905 passed, 0 failed, 0 ignored; all 143 library/integration/doc targets |
| New headless ownership target (included above) | 7 passed, including table conversion, save/reopen and history |
| New worksheet writer units (included above) | 3 passed, covering scoped source alternatives, leaf preservation and invalid output |
| Shared-model validation/table targets, tiny-skia | 20 passed, 0 failed, 0 ignored: thirteen validation cases and seven table/cell-shift cases |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser wasm production, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 94 changed/new Rust files with Rust 2021 edition and both repository whitespace checks passed |

[formula-ownership-verification.json](formula-ownership-verification.json)
records all targets and source/build-file hashes. This 905-test checkpoint
supersedes the previous 895-test core checkpoint; counts are not additive.
The 100 source/build/translation hashes stayed frozen across the final full
core/model/native/browser checks. Actual manifests, lockfiles, dependency
graphs, compiler selection, assertions and engine source were retained.
Completed task-private caches were archived and gzip-verified after success.
The browser compiler replayed the existing debug-stripping warning about
`rust-objcopy`/`libLLVM.dylib`; the check completed successfully. Native screen
runtime, manual dialogs and Excel application oracles remain unverified.


## Compatibility calls and implicit intersection milestone

The engine now registers 506 English names. The generated inventory against
pinned ibisOffice has six remaining names, down from nine; this measures
registration, not full semantic parity. ECMA.CEILING has an actual arithmetic
path; SINGLE/ANCHORARRAY have lookup, case/arity/prefix metadata and operator
lowering. Imported display-prefix normalization no longer loses those calls.

The initial eight integration cases all failed before registration/metadata
work. With names recognized, three cases exposed array scalarization (including
a debug assertion), cross-sheet intersection rejection and reference-taking
context behavior. The scalar implementation now follows the documented
single-value result. An initial ISREF assertion incorrectly expected a
reference result; it was corrected after checking primary descriptions and the
LibreOffice implementation, not relaxed to claim extra reference support.
The engine regression then exposed a legacy FORMULATEXT unimplemented
expectation. Ordinary ranges now use the top-left formula; invalid scalar or
explicitly scalarized inputs return VALUE. Prior stub assertions were replaced
with positive formula-text results and typed-error checks.

Further cases reproduced missing CEILING array arithmetic and accepting an
ordinary scalar formula as a spill anchor. CEILING/ECMA.CEILING now broadcast
numeric operands and retain per-element errors, using the existing engine
matrix cap. The size product is saturating on 32-bit targets. Spill references
evaluate their anchor before reading its extent, reject invalid/non-array
sources with REF and propagate errors/cycles. A direct engine test covers a
later anchor; integration also covers one-cell arrays, blocked/unblocked
spills, lambda binding shadowing, cache replacement, arities, prefixes,
FORMULATEXT length boundaries, save/reopen and history.

The pinned ibis math source groups ECMA.CEILING with the PRECISE variant.
Jade follows the signed two-argument ECMA description and documented CEILING
examples instead; this semantic difference is explicitly recorded in
[compatibility-functions.md](compatibility-functions.md). No reference code
was copied. Further application captures remain required.

The first standalone-engine attempt used the core workspace package selector,
which cannot run dev-dependency tests for its excluded engine package. Missing
locked dev artifacts were fetched without changing dependencies or lockfiles;
final tests use the engine's actual standalone manifest/lockfile. A test fixture
used the nonexistent `CellInput::Empty` name and was corrected to `Blank`;
that fixture compilation correction is not a production bug fix. The earlier
917-test core checkpoint predates the final spill-operand correction and is
not used as the final proof below.

| Check | Final result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 918 passed, 0 failed, 0 ignored; all 144 library/integration/doc targets |
| Compatibility-call integration target, included above | 13 passed, 0 failed |
| Standalone calculation engine, actual engine manifest/lockfile | 2,239 passed, 0 failed, 8 pre-existing ignored cases |
| Shared-model typing/validation/table targets, tiny-skia | 25 passed, 0 failed, 0 ignored: five typing, thirteen validation and seven table cases |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser wasm production, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 106 changed/new Rust files with Rust 2021 edition and both repository whitespace checks passed |
| Canonical engine patch replay | 487 source/build files matched byte for byte; original 480-file upstream hash inventory unchanged |
| Generated function inventory | Actual final engine inventory matches the saved 506 names; pinned reference has 512 |

[compatibility-functions-verification.json](compatibility-functions-verification.json)
records the final targets, manifests/lockfiles, patch replay and 118 frozen
source/build/translation/provenance/script hashes. This 918-test core checkpoint
supersedes the earlier 905-test checkpoint; counts from different checkpoints
are not additive. Sources, assertions, dependency graphs and compiler selection
were retained across the checks. Completed task-private caches were archived
in gzip-verified, hash-recorded parts to bound storage. The browser check
replayed the existing `rust-objcopy`/`libLLVM.dylib` debug-stripping warning and
completed successfully. Native screen/dialog runtime, Excel application oracles,
the six remaining names and the broader audit areas remain open; the parity
goal is active.

## Scoped sparkline reading milestone

The initial six reader cases reproduced five failures: foreign/misplaced
groups, wrapped colours/data, nested group replacement, CDATA loss and invalid
destinations interpreted as A1. The namespace/hierarchy/text fix passed those
six cases. A seventh case then reproduced dropping valid one-cell ranges;
the final decoder accepts ranges whose endpoints identify the same cell,
including absolute references, while excluding multi-cell/out-of-grid targets.
The headless target adds a Transitional/Strict package scenario with CDATA,
cross-sheet edits, exact foreign XML, save/reopen and package/model history.
See [sparkline-reading.md](sparkline-reading.md) for the boundary and limits.

| Check | Final result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 926 passed, 0 failed, 0 ignored; all 145 library/integration/doc targets |
| New reader ownership target, included above | 7 passed |
| Headless formula-ownership target, included above | 8 passed, including one new save/history case |
| Shared-model typing/validation/table targets, tiny-skia | 25 passed, 0 failed, 0 ignored |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser wasm production, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 108 changed/new Rust files with Rust 2021 edition and both repository whitespace checks passed |

[sparkline-reading-verification.json](sparkline-reading-verification.json)
records the targets and 120 frozen source/build/translation/provenance/script
hashes. The initial 925-test run predates the one-cell-range correction; final
evidence uses 926. Sources stayed frozen across the final core/model/native/
browser checks. The engine source/build/patch hashes and dependency manifests/
lockfiles are unchanged from the preceding compatibility-call checkpoint.
Its 2,239-pass/eight-ignored engine regression, 487-file patch replay and
506-name inventory are explicitly carried forward, not reported as newly run.
Counts from different checkpoints are not additive.

The first native attempt selected an uncached task-private target and exhausted
the build volume compiling `syn`. Only those task-private artifacts were removed;
the actual existing shared target then passed, and native compilation was
repeated after the final reader correction. The final core run also encountered
a transient filesystem-full error while saving its atomic checkpoint after
43 recorded passing targets. The source hashes were verified unchanged and
the run resumed at its first unrecorded target. All 145 final targets passed.
No manifest, compiler, dependency graph or assertion was changed for storage.
Task-private caches were preserved in gzip-verified, hash-recorded parts;
partial model-cache archiving was recovered by verifying the saved parts plus
the remaining raw-file inventory before continuing. The browser check replayed
the existing debug-stripping warning and completed successfully.

Sparkline authoring, date-axis rendering, general MCE, cell-reader ownership,
other audit areas, manual dialogs and Excel application oracles remain open.
The parity goal remains active.

## Cell content and compatible source updates milestone — 2026-10-24

Standard grid/shared-string values, rich runs and phonetic readings now use
one namespace/hierarchy and Choice/Fallback interpretation. Empty string items
keep their indices, and implicit coordinates agree across readers. Original
package XML stays separate from that temporary view. Cell edits, shared-formula
detachment, safe ordering and cache updates follow source ownership; existing
understood alternate cells also receive value/style edits. Unknown choices
and unrelated foreign XML remain opaque. See [cell-content.md](cell-content.md)
for supported paths and limits.

The initial reader target had 0 passes and 9 failures; the first writer target
had 0 passes and 4 failures. Further cases
reproduced stale real-cell caches, attribute-quote damage, inactive formulas
treated as stored-cell caches, and stale known Fallback values. A cache-hoisting
attempt exposed duplicate values when an unknown Choice became understood;
the final implementation updates value leaves in place and places missing
caches inside their preferred formula/value context. Empty/nested choices,
namespace rebindings and local declarations are covered. A previous assertion
expecting one raw value was replaced by checks for the two understood branch
caches and one effective value under each tested selection. This is a stronger
representation check, not removal of the stale-value assertions.

| Check | Final result |
|---|---|
| Six selected XLSX implementation crates, actual core manifest/lockfile | 953 passed, 0 failed, 0 ignored; all 148 library/integration/doc targets in that set |
| New cell-content targets, included above | 12 reader, 12 writer and 3 headless cases passed |
| Shared-model typing/validation/table targets, tiny-skia | 25 passed, 0 failed, 0 ignored |
| Native production/test code, Rust 1.93.1 | Compilation passed |
| Browser wasm production, Rust 1.99.0 | Compilation passed |
| Rust formatting and whitespace | 117 changed/new Rust files with Rust 2021 edition and both repository whitespace checks passed |

[cell-content-verification.json](cell-content-verification.json) records the
selected package list, all target results, build manifests/lockfiles and 129
frozen source/build/translation/provenance/script hashes. The API, CLI, MCP,
diff and standalone native-render packages' separate test suites are outside
this milestone's six-package driver. Those checks remain part of the broader
goal. The unchanged engine's earlier 2,239-pass/eight-ignored regression,
487-file patch replay and 506-name inventory are explicitly carried forward,
not reported as newly run. Counts from different checkpoints are not additive.

An intermediate full run had 943 passes and one failure: its prefix fixture
used `urn:sheet`, rather than a SpreadsheetML namespace. The positive fixture
now uses the standard namespace with its prefix assertions retained, and a
new negative case rejects a foreign worksheet root. A new headless fixture
also corrected field access on the model's phonetic String. Subsequent
950-test runs predate the final cache-cardinality and namespace follow-ups;
final proof uses 953. During the final run, a filesystem-full error interrupted
the atomic checkpoint write after 67 recorded passing targets. The same
frozen hashes were rechecked, and the run resumed at the first unrecorded
target. All 148 selected targets passed. No source, compiler, dependency graph
or assertion was bypassed for storage. Task-private caches were preserved in
gzip-verified, hash-recorded parts. The browser replayed the existing
debug-stripping warning and completed successfully.

The auxiliary 100,000-cell memory probe and its limits are recorded separately
in [cell-content.md](cell-content.md) and the JSON. It does not substitute for
the actual Cargo regression and build checks. General MCE, differing alternate
grid/shared-formula topology, the other package suites, production corpus and
Office application oracles, manual dialogs and the wider feature gaps remain
open. The parity goal remains active.

## Workbook structure protection and wider package suites — 2026-10-24

Imported `workbookProtection/@lockStructure` was not enforced by the existing
sheet writers. The first writer/headless target reproduced eight failures
and two passing controls. The reader now exposes the owned direct flag, the
preserving writer rejects actual structure changes, and both shell command
handlers refuse before changing selection/scroll state or starting rename.
No-op writes remain byte-identical. Password metadata alone and windows or
revision locks do not lock structure; ordinary cell edits still follow the
separate worksheet protection. Details and limits are in
[workbook-protection.md](workbook-protection.md).

| Final check | Result |
|---|---|
| Six selected implementation crates, actual core manifest/lockfile | 968 passed, 0 failed, 0 ignored; 150 library/integration/doc targets |
| Additional API/CLI/MCP/diff suites, actual core manifest/lockfile | 182 passed, 0 failed, 0 ignored; 15 library/bin/integration/doc targets |
| Standalone native renderer, its own actual manifest/lockfile | 12 passed, 0 failed, 0 ignored; four library/bin/integration/doc targets |
| Shared-model `*_like_excel.rs` targets, actual root manifest/lockfile, tiny-skia | 85 passed, 0 failed, 0 ignored across 19 selected targets; feature-gated cases described below |
| Native production/test compilation, actual app graph, Rust 1.93.1 | Passed |
| Browser wasm production compilation, actual web graph, Rust 1.99.0 | Passed from a fresh task-private target |
| Changed/new Rust source formatting | 122 files passed rustfmt; root/core diff checks passed |

The core and broader package targets total 1,162 passing tests across the
11 XLSX-prefixed packages. The API currently supplies zero runtime tests;
its library/doc gates compile successfully. Targets were enumerated from
Cargo metadata and run individually. Examples, benchmarks and the full
feature matrix were not executed. The final standalone renderer checks real
Skia pixels for viewports, charts, pictures, conditional formatting and
filter buttons. The diff suite checks seven attributed public workbooks,
provenance/hashes/profiles, exact mutation events, stable reports and stress
cases. Those are separate from an Office application oracle.

The model driver enumerates the 19 `*_like_excel.rs` files. With tiny-skia,
`print_like_excel` runs zero cases because it requires both tiny-skia and
skia; a Skia-only Thai-text case is also gated out. These are not counted as
passed or ignored tests. Other model integration/unit targets are outside
that naming-based selection. The three new protection cases verify selected
sheet/package/history atomicity, preserved redo, normal cell input and
unlocked controls. The reader/writer protection targets add fifteen cases.

The 242 primary source/build/test/corpus/provenance hashes were frozen
before the final core/broader/shell checks and remained unchanged. The 19
model test-file hashes were separately frozen before that driver. Actual
compilers, lockfiles, dependency graphs and source assertions were retained;
the graph was not replaced to fit storage. The unchanged engine's earlier
2,239-pass/eight-ignored suite, 487-file patch replay and 506-name inventory
are carried forward explicitly, rather than added to the new test count.
Machine-readable results and exact target scopes are in
[workbook-protection-verification.json](workbook-protection-verification.json).

An initial standalone native-render build terminated on ENOSPC while
compiling `nom`. After releasing redundant task-private archives/binaries,
the same graph passed, and the final metadata-driven driver reran all four
native-render targets on the final sources. An initial manual binary
selector used `xlsx-to-png`; Cargo rejected that name. The final driver
selects `xlsx_to_png` from metadata. The wasm build emitted the previously
observed debug-stripping warning: its `rust-objcopy` could not load
`libLLVM.dylib`. Cargo check completed successfully with the original
compiler and graph.

At this guard-only checkpoint, password authoring/authentication, editable
ranges, formula hiding, window/
revision guards, encryption, general MCE around workbook protection, broader
structural-writer ownership, manual UI captures and complete disabled-menu
availability remain open. Pivots, slicers, queries, advanced charts, the six
remaining function names and the other audit gaps still prevent claiming
full ibisOffice parity.

## Workbook protection authoring and authentication

The [authoring follow-up](protection-authoring.md) adds optional-password
workbook structure protection, authenticated removal and a masked shared
dialog in both worksheet shells. Its preserving writer supports specific
workbook SHA attributes, historical generic ISO attributes and legacy
verifiers. Modern failures never fall back to a matching legacy verifier.
Ambiguous historical revision credentials are refused without mutation;
separate revision/foreign metadata survives successful workbook changes.

| Final check | Result |
|---|---|
| Selected six implementation packages, all library/integration/doc targets | 979 passed, 0 failed, 151 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,173 passed, 0 failed, 170 targets |
| Shared OPC complete unit suite, including existing encryption/signature cases | 19 passed, 0 failed; five new protection cases |
| Shared model, 19 tiny-skia `*_like_excel` targets | 91 passed, 0 failed; six additional protection cases |
| Native shell production and test compilation | Passed with Rust 1.93.1 |
| Browser shell production compilation, wasm32 | Passed with Rust 1.99.0 |
| Changed/new Rust formatting and both repository whitespace checks | Passed; 131 Rust files |

These are replacement checkpoint counts, not additions to previous totals.
The model selection retains the print and Skia-only Thai exclusions above;
other model/unit/feature targets were not run. Eleven writer/headless cases
cover authoring, authentication, metadata preservation and history. The
cryptographic cases use independent Python digest/legacy vectors, including
non-BMP passwords and Windows code-page inputs. They are algorithm evidence,
not an Excel application oracle. The cold model preflight exposed two
historical-ISO failures while seven controls passed; the refreshed final
driver rebuilt the repaired writer and passed all nine protection cases.

Final core/broader/OPC/model runtime gates use actual manifests and lockfiles,
test/dev opt-level=1, debug information/incremental off and one build job.
The added `encoding_rs` dependency in `jade-opc` is the only lockfile
dependency-list change; reconstructing the prior lockfile bytes confirms
all existing package versions were retained. The 253 primary source records
and 19 model test-source records were checked after the final gates, allowing
only that intended browser lockfile resolution. The unchanged engine's
2,239-pass/eight-ignored suite, 487-file patch replay and 506-name inventory
are carried forward, without claiming another engine test run.

Storage exhaustion interrupted native dependency compilation and the first
optimized OPC attempt. Successful targets were retained; the native targets
and OPC suite resumed in a separate private target with the same graph and
profile after verified core/model cache archival. No assertions or production
implementations were bypassed. The browser check again reports the existing
`rust-objcopy` missing-`libLLVM.dylib` debug-stripping warning and completes
successfully. Exact scopes, hashes, corrections and limitations are in
[protection-authoring-verification.json](protection-authoring-verification.json).

Manual native/browser interaction, IME/clipboard and Office save/repair
oracles remain unverified. At this workbook-only checkpoint,
sheet-protection authoring/authentication,
editable ranges, formula hiding, revision/window operation guards, encryption,
broader algorithms/locales, general MCE and the remaining audit gaps are
still open. This milestone does not establish full ibisOffice parity.

## Worksheet protection and hidden contents

The [worksheet follow-up](sheet-protection-authoring.md) adds protect/unprotect,
SHA/legacy password authentication, fifteen permission toggles and shared
native/browser controls. Cell-lock and hidden-content formatting persists
through named-style/cell/row/column resolution. Protected hidden contents are
blank in the formula bar while their stored formulas and calculated grid
values remain. Selection checks include sparse row/column bands and absent-row
navigation. Unsupported root-level MCE protection authoring is refused while
opaque XML survives normal edits.

| Final check | Result |
|---|---|
| Selected six implementation packages, all library/integration/doc targets | 990 passed, 0 failed, 153 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,184 passed, 0 failed, 172 targets |
| Shared OPC complete unit suite | 19 passed, 0 failed; existing protection/crypto cases rerun |
| Shared model, twenty tiny-skia `*_like_excel` targets | 97 passed, 0 failed; six new worksheet cases |
| Native production and test compilation | Passed with Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed with Rust 1.99.0 |
| Changed/new Rust formatting and both repository whitespace checks | Passed; 139 Rust files |

These are replacement checkpoint counts. Four new reader cases and seven
writer/headless cases are included above. The model selection retains its
print and Skia-only Thai exclusions; no other model/unit/feature targets are
implied. The initial reader probe reproduced accepting an invalid boolean
as unprotected while its namespace/nesting control passed. All final reader
cases pass. The initial full regression found an exhaustive format-view test
literal missing the two new fields. The repaired test retains every existing
full-equality/history assertion and additionally checks unlocked/hidden axis
formats. Its five cases passed on retry; the other 152 targets passed unchanged.
A private retry helper's initial three-case expectation was corrected to the
actual five, and the target was executed again with its zero exit code recorded.

The final runtime gates use unchanged actual manifests/lockfiles, Rust 1.93.1,
test/dev opt-level=1, debug information/incremental off and one job. No dependency
or locked package version changed. The 259 primary source records include the
single test adaptation, and twenty model target-source hashes were independently
recorded. All hashes were checked after the gates. The engine's earlier
2,239-pass/eight-ignored run, 487-file patch replay and 506-name inventory remain
explicitly carried forward; no new engine runtime run is claimed.

Verified private cache restoration/archive and metadata relocation provided
build space without replacing graphs or bypassing assertions. All three final
caches passed part-size/SHA256/gzip/member checks. Browser compilation retains
the nonfatal engine debug-stripping warning about `rust-objcopy` being unable to
load `libLLVM.dylib`. Exact results and limits are in
[sheet-protection-verification.json](sheet-protection-verification.json).

Editable ranges/session unlocks, independent objects/scenarios-only operation
guards, full merged/compound selection and platform interaction oracles,
revision/window guards, encryption and the other audit gaps remain open.
Specification-generated fixtures and compilation do not replace Office or
native/browser runtime evidence. The full ibisOffice parity goal remains active.


## Editable ranges and session authentication

The [editable-range follow-up](editable-ranges.md) adds standard/x14 reading,
preserving create/update/rename/delete, strong/legacy range authentication,
worksheet-local session grants and shared native/browser management. Both
representations follow whole-axis and partial structural movement. Malformed,
ambiguous and unsupported MCE metadata cannot grant edits. Passwordless ranges
work without removing sheet protection; session unlock/relock changes no source
bytes, history or dirty state. Credential replacement is explicit. Grants
survive ordinary cell history and structural movement, but deletion or protection/
credential changes revoke them; Undo does not resurrect revoked authentication.

| Final check | Result |
|---|---|
| Selected six implementation packages, all library/integration/doc targets | 1,012 passed, 0 failed, 156 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,206 passed, 0 failed, 175 targets |
| Shared OPC complete unit suite | 21 passed, 0 failed; two new range ISO-over-legacy cases |
| Shared model, twenty-one tiny-skia `*_like_excel` targets | 104 passed, 0 failed; seven new editable-range cases |
| Native production and test compilation | Passed with Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed with Rust 1.99.0 |
| Changed/new Rust formatting and both repository whitespace checks | Passed; 147 Rust files |

These replace preceding checkpoint counts. Six reader, eight writer and eight
headless cases are included in the package suites. The first valid runtime
control reproduced refusal of a passwordless editable range; it passes after
the fix. Preflight test-import and one-based row-trigger corrections retained
the intended behavior/history assertions. No source/test changes were needed
during final gates. The model selection retains the print and Skia-only Thai
exclusions; other model/unit/feature targets are not implied. EN/JA protection
translations each contain the same 58 distinct keys.

All 267 primary source hashes and twenty-one model target source hashes were
frozen before the final gates and rechecked afterwards. Actual manifests,
lockfiles, compiler/dependency graphs and test assertions were retained; no
dependency or locked version changed. Runtime gates use Rust 1.93.1, test/dev
opt-level=1, debug information/incremental off and one job. The engine's earlier
2,239-pass/eight-ignored run, 487-file patch replay and 506-name inventory are
carried forward with unchanged hashes, not counted as a new engine runtime run.

The three final private caches passed part-size/SHA256/gzip/member checks.
Temporary dependency file aliases in the shared target were normalized to
ordinary self-contained archive entries before their external copies were
removed. Native metadata directory links/destinations remain available. Browser
compilation retains the nonfatal engine `rust-objcopy`/`libLLVM.dylib` debug-strip
warning. Exact gate scope, source hashes, vectors and limitations are in
[editable-ranges-verification.json](editable-ranges-verification.json).

Automatic edit-triggered prompts, account/ACL authentication, general MCE,
independent objects/scenarios-only guards, complete merged/compound selection,
revision/window guards, encryption, broader locale/performance coverage and
Office/native/browser interaction oracles remain open. Pivots, slicers, queries,
advanced charts and the six missing function registrations remain in the
[active audit](ibis-parity.md). Full ibisOffice parity is not established.


## Independent worksheet object protection

The [object-protection follow-up](object-protection.md) fixes supported object
commands bypassing imported `sheet="0" objects="1"` protection. Reader and
cached headless queries retain independent content/object/scenario flags with
the same worksheet ownership and selected MCE policy as cell reading. Notes,
shape/picture/chart movement and deletion, shape text and chart insertion/design
now refuse before writing when the independent object flag forbids them.
Cell-only protection retains its prior meaning; scenario-only protection and
`objects="0"` do not lock object operations. Undo/redo and authenticated removal
refresh the cached state, and other sheets remain unaffected. Existing shared
native/browser dialog and command paths use those guards.

| Final check | Result |
|---|---|
| Selected six implementation packages, all library/integration/doc targets | 1,020 passed, 0 failed, 157 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,214 passed, 0 failed, 176 targets |
| Shared OPC complete unit suite | 21 passed, 0 failed; no new primitive cases |
| Shared model, twenty-two tiny-skia `*_like_excel` targets | 106 passed, 0 failed; two new object-protection cases |
| Native production and test compilation | Passed with Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed with Rust 1.99.0 |
| Changed/new Rust formatting and both repository whitespace checks | Passed; 151 Rust files |

These replace preceding checkpoint counts. Two new reader and six new headless
cases are included above. The actual initial runtime probe wrote a forbidden
note successfully. A subsequent numeric-cell control used a table-header cell,
which existing table rules store as text; it was corrected to ordinary D20,
retaining the numeric equality and permission/history assertions. The first
model draft accessed a private method; its public undo/redo and protection-query
replacement exercises the same history invariants. No source/test adaptation
was needed during final gates. The model selection retains the print and
Skia-only Thai exclusions and does not imply other model/unit/feature targets.

All 271 primary source hashes and twenty-two model target-source hashes were
frozen before the final gates and checked afterwards. Actual manifests,
lockfiles, compiler/dependency graphs and assertions were retained; no dependency
or locked version changed. Runtime gates use Rust 1.93.1, test/dev opt-level=1,
debug information/incremental off and one job. The earlier engine
2,239-pass/eight-ignored run, 487-file patch replay and 506-name inventory are
carried forward with unchanged hashes, not counted as new engine runtime runs.

The three final private caches passed byte-count/SHA256/gzip/member checks.
Streaming consuming restoration relocated 103 immutable non-Jade dependency
outputs with fresh SHA checks; normalized archiving restored ordinary regular
entries before removing the external copies. Native metadata directory links
and destinations remain available. Browser compilation retains the nonfatal
engine `rust-objcopy`/`libLLVM.dylib` debug-strip warning. Exact scope and evidence
are in [object-protection-verification.json](object-protection-verification.json).

The current policy remains sheet-wide: individually unlocked DrawingML/VML
objects can be overrestricted. Their client-data/text-lock handling, scenario
editing, general DrawingML ownership/MCE and object selection/gesture/runtime
oracles remain open. Automatic range-authentication prompts, encryption and
the wider [ibisOffice audit](ibis-parity.md) also remain active. These tests and
compilation do not establish Excel application behavior or full ibisOffice parity.


## Individual drawing and note protection

The [individual-object follow-up](individual-object-protection.md) replaces the
preceding sheet-wide overrestriction with imported DrawingML/VML position and
text flags. The shared owned-source parser, selected worksheet relationships,
preserving anchor/text/comment writes and existing shared-model commands are
covered by the final gates below. Note text updates retain the original VML
bytes, geometry and locks. Supported MCE note-text alternatives retain their
own fonts; unknown alternatives remain opaque. Read-only comments also select
MCE authors/records. Root/list/author MCE authoring remains refused.

| Final check | Result |
|---|---|
| Six implementation packages, all library/integration/doc targets | 1,043 passed, 0 failed, 160 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,237 passed, 0 failed, 179 targets |
| Shared OPC complete unit suite | 21 passed, 0 failed |
| Shared model, twenty-three tiny-skia `*_like_excel` targets | 109 passed, 0 failed |
| Native production and test compilation | Passed, Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed, Rust 1.99.0 |

These replace the preceding checkpoint counts. New cases are reader 8, writer 7,
headless 8 and shared model 3. The initial headless probe reproduced refusal of
an unlocked object; the final cases exercise all four position/text flag
combinations, identity/namespace/ref ownership, opaque payloads, malformed
metadata, atomic refusals, save/reopen and history. The initial complete core run found four failures in two targets: three
lexical drawing/note expectations and one real chartsheet-import regression.
The projection now supports owned chartsheet/dialogsheet references; the
unchanged 32-workbook corpus passes. Existing drawing/note checks now verify
values/fonts/identity and preservation, including the previously discarded
shapeId, rather than exact emitted QNames. These corrections preceded the
second source freeze and complete final rerun. No source/test adaptation
was made during that final rerun. The model selection retains the print and
Skia-only Thai exclusions and does not imply the whole model/feature matrix.

All primary source/model hashes were frozen before final gates and rechecked
afterwards. Actual manifests, locked dependencies and compiler graphs were
retained. The engine's earlier 2,239-pass/eight-ignored run, 487-file patch
replay and 506-name inventory are carried with unchanged hashes, not rerun or
counted as new engine results. Native/browser checks retain the existing
nonfatal wasm debug-strip `libLLVM.dylib` warning.

Private cache restoration/normalization retained 375 SHA-checked immutable
non-Jade outputs without substituting dependency graphs. All final archives
passed byte-count/SHA256/gzip/member checks and contain self-contained regular
entries; raw targets and temporary aliases were removed after verification.
Native metadata directory links/destinations remain available. Exact formatting,
source counts, scope and evidence are in
[individual-object-protection-verification.json](individual-object-protection-verification.json).

Individual flag-authoring UI, complete locked-object selection/gesture feedback,
group/member editing, general DrawingML/VML ownership/MCE, scenario editing,
automatic range prompts and the wider [ibisOffice audit](ibis-parity.md) remain
open. Specification tests and compilation do not establish actual Excel or
native/browser interaction behavior, and full ibisOffice parity is incomplete.


## Automatic range-password prompts for cell entries

The [range-prompt follow-up](range-unlock-prompts.md) connects protected typed
single-cell refusals to a shared range-password dialog in native and browser
editors. A cached headless query lists only valid covering password alternatives;
a metadata validator checks supported verifiers without spin iterations. Pending
state binds workbook-session identity, worksheet/revision/range definitions and
the exact cell/text/candidate. Wrong/cancelled/stale requests change no source,
history, dirty or layout state. Successful authentication is session-only, then
the existing commit path retains validation/formula checks and history behavior.
Manual range managers also gain workbook-session identity checks.

| Final check | Result |
|---|---|
| Six implementation packages, all library/integration/doc targets | 1,045 passed, 0 failed, 161 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,239 passed, 0 failed, 180 targets |
| Shared OPC complete unit suite | 22 passed, 0 failed; one new metadata-validation case |
| Shared model, twenty-four tiny-skia `*_like_excel` targets | 116 passed, 0 failed; seven new prompt cases |
| Native production and test compilation | Passed, Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed, Rust 1.99.0 |

These replace preceding counts. Two new headless cases cover candidate
permissions, overlap, malformed/ambiguous credentials, unlocked styles and
coordinate bounds. The shared-model cases exercise password masking, failure/
cancellation, session-only grants, original-entry matching, candidate-name
retargeting, replacement-workbook identity, sheet/protection/history staleness,
save/reopen and ordinary formula/data-validation refusal after authentication.
The model selection retains print/Skia-only Thai exclusions and does not imply
the full model/unit/feature matrix.

Primary/model sources were frozen before runtime gates. The first actual native
check found E0277 in the shared UI hint: its formatter expected a borrowed
Display value. One interpolation argument was corrected; native/browser checks
were rerun. All other primary/model/test hashes and runtime assertions remained
unchanged. Final source hashes, the old/new UI hash and formatting are in
[range-unlock-prompts-verification.json](range-unlock-prompts-verification.json).
Actual manifests, locks, dependency versions and compiler graphs were retained.
The earlier engine 2,239-pass/eight-ignored result, 487-file patch replay and
506-name inventory are carried forward with unchanged hashes, not new runtime
runs. Browser compilation retains the existing nonfatal debug-strip
`rust-objcopy`/`libLLVM.dylib` warning.

All final private caches passed byte-count/SHA256/gzip/member checks. The shared
restore retained 375 SHA-checked immutable non-Jade outputs with original mtimes;
native restoration also relocated 82 completed immutable outputs after transient
root exhaustion. Pending-part members were excluded and the same consuming
restore resumed from its journal. Normalized archiving produces self-contained
regular entries before external copies are removed. Raw targets are absent and native metadata directory links
and destinations remain available.

Automatic paste/clear/fill/compound prompts and replay of interrupted navigation
remain open. Such operations retain existing guarded refusals and manual range
authentication. Account/ACL identity authentication, broader selection, full MCE,
actual native/browser/Excel interaction evidence and the wider
[ibisOffice audit](ibis-parity.md) remain incomplete.


## Compound paste and range-fill atomicity

The [compound-edit follow-up](bulk-edit-atomicity.md) fixes tiled text/internal-copy
paste and active-entry range fill returning refusal after changing their first
cell. Two actual red probes observed A1=42 instead of1 despite errors. A third
probe showed a protected-source cut was incorrectly accepted. Scoped shared-model
rollback now restores complete source/model/permissions/grants/history/redo,
clipboard and layout/dirty/change-notification state. Cut-paste guards both source
and destination, including cross-sheet sources.

An opaque headless checkpoint shares immutable history entries through Arc;
frozen clipboard bytes also share Arc payloads. Checkpoint restoration retains
the original workbook identity and does not reopen/recalculate source. Current
source/model and optional row-height base can be copied; this is not a claim of
constant memory or large-production performance. The guarantee concerns returned
errors in these compound paths, excluding panic/process/allocation-failure recovery.

| Final check | Result |
|---|---|
| Six implementation packages, all library/integration/doc targets | 1,053 passed, 0 failed, 162 targets |
| API/CLI/MCP/diff and standalone native-render targets | 194 passed, 0 failed, 19 targets |
| Combined eleven XLSX packages above | 1,247 passed, 0 failed, 181 targets |
| Shared OPC complete unit suite | 22 passed, 0 failed; no new primitive cases |
| Shared model, twenty-five tiny-skia `*_like_excel` targets | 126 passed, 0 failed; ten new compound-edit cases |
| Native production and test compilation | Passed, Rust 1.93.1 |
| Browser production compilation, wasm32 | Passed, Rust 1.99.0 |

These replace preceding counts. Eight new headless cases exercise checkpoint
identity/history/redo/grants, cut-source permission, huge dense limits and sparse
whole-sheet/column permissions. An independent bounded comparison checks batch
permission decisions against actual individual cell edits across row/column/cell
style precedence and authenticated/passwordless ranges. Model tests cover failed
text/internal-copy paste, a retryable cut, existing redo, successful percentage
fill/paste grouping, save/reopen, late size refusal, invalid selections and paint
notifications. Print/Skia-only Thai exclusions remain; this is not the complete
model/unit/feature matrix.

The first full core run completed 162 targets but found four failures in two
existing clipboard/dimension targets: a blanket dense-size cap incorrectly
blocked their sparse whole-axis operations. Those regression assertions remain
unchanged. The corrected cap applies only to dense rectangles, and sparse
style/default/authenticated-range bands check blanks and explicit exceptions
without a 17-billion-cell walk. New cases retain whole-axis behavior with a far
locked-cell exception and a one-cell password gap. Test-draft field references,
a partial-merge fixture and missing required style-table definitions were
corrected before the second source freeze; permission assertions were retained.
No source/test adaptation occurred during the final rerun.

Exact formatting, source hashes and gate evidence are in
[bulk-edit-atomicity-verification.json](bulk-edit-atomicity-verification.json).
Actual manifests, locks, dependency versions and compiler graphs were retained.
Earlier engine 2,239-pass/eight-ignored results, 487-file patch replay and 506-name
inventory are carried with unchanged hashes, not new engine runtime runs. Browser
check retains the existing nonfatal rust-objcopy/libLLVM debug-strip warning.

Shared-cache restoration resumed from the same verified journal after transient
root exhaustion. Thirteen remaining compressed parts (330,691,449 bytes) were
moved with SHA checks to the verification volume and consumed only after complete
extraction. Browser restoration also resumed after twelve verified parts, following
a SHA-checked early move of the completed native archive. Shared 375 and native 82 immutable non-Jade output candidates retain
fresh hashes/mtimes; normalized archiving restores self-contained regular entries
before removing external copies. All final archives pass byte-count/SHA/gzip/
member checks; raw targets absent and native metadata directory links retained.

Bulk-operation automatic authentication, pending-navigation replay, broader
clipboard/filter/spill/table/schema cases, account identity, full MCE and real
Excel/reference/native/browser interaction oracles remain open. Full ibisOffice
parity is not established; the wider [audit](ibis-parity.md) remains active.


## Deferred bulk range authentication

The [bulk-authentication follow-up](bulk-range-authentication.md) adds automatic
native/browser prompts for supported paste/clear/fill/move commands. Passwords
remain staged until all actual write cells are covered, then grants and the
original command commit atomically. Cancellation/wrong passwords preserve live
grants and document/history/clipboard state. Sixteen new model cases use the
same shared attempt/dialog/retry path as both shells; four headless cases verify
pure credential proofs, sparse coverage, invalidation and checkpoint behavior.

Final replacement counts: 1,251 XLSX cases /182 targets (1,057 /163 in the six
implementation packages; 194 /19 in API/CLI/MCP/diff and native renderer), 22 OPC
unit cases and 142 model cases /26 named tiny-skia targets. Both actual locked,
offline application compiler graphs passed. Model Skia-only print/Thai exclusions,
other model feature/unit combinations and actual application interaction/Office
oracles remain. Engine runtime/patch/function inventory results are carried with
unchanged source hashes rather than rerun. The browser retains its recorded
nonfatal rust-objcopy/libLLVM warning. Exact source/gate scope is in
[bulk-range-authentication-verification.json](bulk-range-authentication-verification.json).

A draft validation regression was corrected using Microsoft's explicit copied/
filled-data behavior; existing implementation checks and old assertions were not
relaxed. A separate actual formula-XML writer rejection verifies full rollback,
including newly authenticated grants. Current caches contain regular files and
pass SHA256/gzip/member verification. Host storage became available, so prior
immutable dependency aliases were materialized; native metadata directories were
restored exactly and their established directory links retained. No graph,
dependency-version, manifest, compiler or assertion substitution was used.

Pending-navigation replay, other protection/clipboard/schema cases and all
remaining areas in the [full audit](ibis-parity.md) remain active work.


## Declared pivot/cache reader foundation

The [pivot-reader follow-up](pivot-reader.md) adds a shared catalog for owned
workbook/worksheet pivot declarations, typed/shared records, location/axis/page/
data/filter/style metadata, stable unused/unknown item positions and bounded
metadata diagnostics. It preserves original pivot/cache bytes across ordinary
edits/save/history and the same shared metadata payload across checkpoints.
Source edits do not falsely refresh the saved report. GETPIVOTDATA evaluation,
pivot refresh/authoring and complete structural reconciliation remain open.

Final replacement counts: 1,272 XLSX cases /184 targets (1,078 /165 in the six
implementation packages; 194 /19 in API/CLI/MCP/diff and native renderer), 22 OPC
unit cases and 144 model cases /27 named tiny-skia targets. New cases: reader17,
headless4, shared-model2. Both actual locked/offline application compiler graphs
passed. The Skia-only print/Thai and other model feature/unit exclusions remain.
Engine runtime/patch/inventory evidence is carried with unchanged engine hashes;
506 registrations remain, including the six recorded missing names.

The new fixture is original deterministic Jade test data with byte-identical
regeneration, not an Office/schema/repair/render oracle. A duplicate worksheet-
relationship test draft was corrected to retain the existing stricter whole-input
ownership rejection; no production guard or old assertion was relaxed. Exact
source, target, resource bounds and provenance are in
[pivot-reader-verification.json](pivot-reader-verification.json). Full pivot
calculation/refresh/layout/authoring and the wider [audit](ibis-parity.md) remain
active work.


## Saved pivot queries and structural consistency

The [GETPIVOTDATA follow-up](getpivotdata.md) supplies actual model-owned visible
pivot context and queries saved result cells, including captions/source aliases,
row/column/compressed items, selected page items, dates, multiple measures and
array broadcasting. It does not pretend to refresh source records. Owned MCE
declarations are selected, ambiguous owners retain uncertainty, and uninterpreted
inner alternatives/cube identities preserve imported formula caches. A red test
reproduced a false REF overwrite before this last fix; all final graphs were
rerun after the source snapshot was refreshed.

Report/source row/column and covered-strip geometry follows edits, and source
rename preserves foreign lookalikes. Report resizing/removal, nonrectangular or
fully removed sources, report/source cut moves and direct report values/paste
are refused atomically until their corresponding authoring/rebuilding is
available. Formatting and copy-out remain available. Shared/headless tests
exercise typed entry, saving/reopening, relocation, refusals and Undo/Redo.

Final replacement results are 1,300 XLSX cases /186 targets (1,106 /167 in the
six implementation packages; 194 /19 in API/CLI/MCP/diff and native renderer),
22 OPC cases, 146 model cases /27 named tiny-skia targets and 2,242 standalone
engine cases with eight existing ignored. Both actual locked/offline application
compiler graphs passed. The actual registry has 507 names; the regenerated
engine patch replays all 488 source/build files against the hash-pinned original
480-file inventory. Exact hashes, target results, constraints and evidence are
in [getpivotdata-verification.json](getpivotdata-verification.json). No producer/
Excel repair/save/render or manual shell interaction oracle is claimed; the
Skia-only and other model feature/unit exclusions remain. That checkpoint preceded explicit refresh. The larger parity goal and five
missing function names remain active; current refresh scope follows below.


## Explicit pivot refresh

The [refresh follow-up](pivot-refresh.md) rebuilds current source records and
supported reports sharing a cache, with stable typed item indices, eleven
standard aggregates, supported row/column/compact layouts, page/member filters,
totals, date identity and inferred measure captions. Both shells use one shared
model command. Source formula verification, collision checks and returned-error
rollback precede a single undoable commit. Unknown replaced metadata is refused.

Final replacement results: 1,334 XLSX cases /188 targets (1,140 /169 in the six
implementation packages and 194 /19 in API/CLI/MCP/diff/native renderer), 22 OPC
cases, 148 shared-model cases /28 named tiny-skia targets and 2,242 actual engine
cases with eight existing ignored. Actual native production/test and browser
production compiler graphs passed locked/offline. Registry507 and the 488-file
engine patch remain unchanged and were re-executed/replayed. Final hashes cover
312 scoped files and 28 model target sources; 193 scoped changed/new Rust files
pass formatting. Details are in
[pivot-refresh-verification.json](pivot-refresh-verification.json).

This does not establish full style/autofit/PivotChart synchronization, refresh-all
or automatic load refresh, advanced pivot authoring or full ibisOffice parity.
Existing model feature exclusions and producer/Excel/manual interaction oracle
limits remain. Historical counts above are not additive.


## Atomic refresh of all pivot tables

The [refresh-all follow-up](pivot-refresh-all.md) covers every declared supported
worksheet pivot, independent of selected cell/sheet. Shared cache IDs are
deduplicated. Each source is calculated before consumption, intermediate
packages stay inside an edit checkpoint, and the final update is one history
step. A later collision or unverified formula restores bytes, calculated state
and both history branches. No reports is a true no-op; unknown catalogs refuse.

Final replacement results: 1,338 XLSX cases /188 targets (1,144 /169 in the six
implementation packages, 194 /19 in API/CLI/MCP/diff/native renderer), 22 OPC
cases, 150 model cases /28 named tiny-skia targets, and 2,242 actual engine cases
with eight existing ignored. Actual native production/test and browser production
compiler graphs passed locked/offline. All graphs ran against 312 final source
hashes and 28 model target hashes; 193 scoped changed/new Rust files pass
formatting. Registry507 and the 488-file engine patch were re-executed/replayed.
See [pivot-refresh-all-verification.json](pivot-refresh-all-verification.json).

Processing order follows saved declarations, with calculation between caches;
this does not establish general dependency scheduling or backward/cyclic
freshness. Automatic load refresh, styles/autofit/PivotCharts, broader authoring
and full parity remain open. Existing model feature and Office/manual oracle
exclusions remain.


## Observed dependency order and dynamic source freshness

The [ordering follow-up](pivot-refresh-order.md) reproduces and fixes reverse
cache ordering (old27 versus expected117) and undetected source cycles. Optional
model-owned read tracing records actual scalar/reference/range/spill paths from
the calculation supplying source values. Range compaction retains empty-cell
dependencies. Source bounds reuse the exact writer resolver. Sparse transitive
reads produce predecessor sets; graphs are rechecked after each staged update
and completion timestamps invalidate consumers of later-refreshed predecessors.
Dynamic cycles or exhausted trace/work/retry budgets restore the whole batch.

Final replacement results: 1,350 XLSX cases /189 targets (1,156 /170 in the six
implementation packages, 194 /19 in API/CLI/MCP/diff/native renderer), 22 OPC
cases, 151 model cases /28 named tiny-skia targets, and 2,246 actual engine cases
with eight existing ignored. Actual native production/test and browser production
compiler graphs passed locked/offline against 315 scoped source hashes and 28
model target hashes. 196 scoped changed/new Rust files pass formatting. The
engine patch was regenerated/replayed across 489 files against pinned upstream480;
registry507 was re-executed and unchanged. Details are in
[pivot-refresh-order-verification.json](pivot-refresh-order-verification.json).

Observed references conservatively overapproximate dependencies and are not a
static all-branches/Excel calculation-chain oracle. Volatile sources produce
samples, not a permanent source/cache equality guarantee. Full style/autofit/
PivotChart, authoring, producer/Office/manual interaction and model feature/unit
coverage remain open; historical counts above are not additive.

## Named pivot styles and corrected worksheet ownership

The [style follow-up](pivot-styles.md) adds custom and84 normative named
PivotTable overlays, actual workbook-theme resolution, independent pivot region
precedence, explicit/conditional priority and unstored report surfaces. The
unmodified normative22-table workbook reproduced the old reader exposing zero
pivots: it expected worksheet pivotTableParts markers absent from CT_Worksheet.
The reader now follows actual worksheet relationships and retains unresolved
cache-source uncertainty. Workbook cache declarations remain real standard XML.

Final replacement results: 1,371 XLSX cases /191 targets, 22 OPC cases, 152 shared
model cases /28 named tiny-skia targets, both actual app compiler graphs and206
scoped Rust formatting checks. Native raster assertions check header/manual/
blank/total colors. The model raster case checks total-row growth, saved
styles, reopen and Undo/Redo. Snapshots cover330 primary inputs and28 model
targets. Engine source/patch and registry are unchanged: prior2,246 passed/eight
ignored and507 registered functions are carried, with the489-file pinned patch
replayed in this milestone. See [the proof](pivot-styles-verification.json).

Page-field styling, complete nested/tabular group borders and differential
properties, autofit/PivotChart bindings, authoring and full Office/application
oracles remain open. The normative sample verifies ownership/legacy defaults;
all84 style names resolving is not84 visual oracle comparisons.

## Pivot column fitting on refresh

The [fitting follow-up](pivot-autofit.md) saves widths with refreshed caches,
reports and formula results in one history edit. Both app shells pass backend
metrics through the shared model; headless callers can supply metrics or use
the documented deterministic fallback. Complete formatted values/own fonts,
indentation, report/filter ownership, hidden/merged/opt-out/protected widths and
shared-column maxima are covered. Typed page geometry matches all 22 normative
workbook filter locations. Invalid/overflowed metrics, conflicting geometry or
later failures preserve bytes, caches, widths and the redo branch. Disabling
fitting does not require a working measurer.

Final replacement totals: 1,385 XLSX cases /193 targets, 22 OPC cases and 153
model cases /28 named tiny-skia targets, plus both actual app compiler graphs.
Snapshots cover 333 primary inputs and 28 model targets; 209 scoped Rust files
pass formatting. The unchanged engine retains prior 2,246 passed/eight ignored
and registry 507 evidence; the 489-file patch was replayed again. See the
[fitting proof](pivot-autofit-verification.json). Complex/legacy/all-font width
and full Office/save/manual interaction oracles, PivotChart bindings, authoring
and the other full-audit gaps remain open.

## PivotChart reconciliation and worksheet name identity

The [PivotChart follow-up](pivot-charts.md) reads standard chart-space
bindings and refreshes chart topology with reports in one atomic edit. It
preserves member colors through sorted insertion/removal, existing series
styles/IDs and combination lanes, contiguous source references and opaque
wrapper extensions. Unsupported conversions or malformed identities refuse
without damaging history. Supported report-tab renames retain local bindings;
worksheet and pivot-source sheet names now honor ST_Xstring while chart source
names/formulas remain plain XML strings.

Final frozen-source replacement totals: 1,409 XLSX cases /195 targets, 38 OPC
cases and 154 model cases /28 named tiny-skia targets; both actual app compiler
graphs pass. Actual TinySkia refresh/save/history and native Skia import pixels
are included. Source coverage is 342 primary inputs and 28 model target
sources; 218 own dirty/new Rust files pass formatting. The engine is unchanged,
so 2,246 engine passes/eight ignored and registry 507 remain carried evidence;
the unchanged 489-file patch was replayed again. See
[pivot-charts-verification.json](pivot-charts-verification.json).

Filename identity in byte-only APIs, all chart producers/families/options,
external aliases, multilevel/filtered/custom-error reconciliation, controls,
Office save/repair/visual and app interaction oracles remain unproven. Pivot
authoring and the rest of the full audit remain open.

## Existing pivot authoring, official schemas and actual SDK validation

The [authoring follow-up](pivot-authoring.md) adds preserving source/layout/
measure/member/page/field/style editing and a common native/browser dialog.
Actual saved values, style-removal pixels, member refresh, stale configuration,
page geometry, collision protection and one-step history pass.

Final frozen replacement totals:1,426 XLSX cases /196 targets,160 model cases
/28 named tiny-skia targets and38 OPC cases; both actual app compiler graphs
pass. Source355/model28 hashes and228 own dirty/new Rust formatting checks are
recorded in [pivot-authoring-verification.json](pivot-authoring-verification.json).
Production dependencies/locks and engine/registry are unchanged. The optional
standalone .NET10 QA project adds a pinned Open XML SDK3.5.1 verification
reference/lock; it does not enter production Cargo graphs.

Twenty original synthetic outputs (ten per format) assert real chart values.
Official unmodified schemas check180 components:160 raw XSD passes,20 documented
Office xml:space deviations with raw errors retained, zero unexpected failures.
Actual SDK Office2021 validation passes ten Transitional packages with zero
errors. Its full Strict probe retains ten UInt16 errors on valid150% gap widths;
no output/schema is adapted to hide them and no Strict SDK pass is claimed.
These extra oracle counts are separate from the package test totals.

Historical engine2,246 passes/eight ignored, registry507 and489-file canonical
patch replay remain carried evidence. Full source field remapping/create/delete,
advanced pivots, Office equivalence, widget interactions and all other audit
gaps remain open.

## Pivot creation/deletion and current displayed chart values

The [creation/deletion follow-up](pivot-lifecycle.md) adds complete supported
report/cache relationship graphs, existing/new sheet destinations, common native/
browser source entry, owned value removal, shared cache retention and static
chart snapshots. Static data follows currently displayed saved worksheet points,
including stale/empty/missing chart caches and cached formula display values.
Uniform numeric category formats, title string identity and chart readback are
checked. Hidden visible-only point indices, mixed numeric category formats,
pivot format deltas and unsupported bindings remain explicit refusals.

Final replacement totals:1,448 XLSX cases /197 targets,164 model cases /28 named
tiny-skia targets,38 OPC cases and both actual app compiler graphs. Production
inputs stayed unchanged during the final gates. A model test precondition was
corrected to use the public reader; all28 model targets reran after that test-
only correction. The final source record covers364 inputs/model28 target
sources, with237 own dirty/new Rust files formatted.

Thirty-four original synthetic outputs supply actual value/graph assertions.
Official unmodified schemas check308 components:282 raw passes,26 documented
Office xml:space deviations with raw errors retained, zero unexpected failures.
The real SDK3.5.1 Office2021 validator gives17 Transitional packages zero errors;
its Strict probe retains17 UInt16 errors on valid150% gap widths. These counts
are separate from package tests and do not establish full Strict SDK or Office
application equivalence. Schemas and outputs are not adapted to suppress errors.

Production and QA dependencies/locks are unchanged. Engine2246 passes/eight
ignored, registry507 and489-file replay remain historical carried evidence.
See [pivot-lifecycle-verification.json](pivot-lifecycle-verification.json).
Blank/no-value pivots, source field remapping, reused caches, advanced features,
complete chart/Office/widget oracles and other audit gaps remain open.
