# XLSX PR integration

The compatibility implementation and application controls are submitted as
[jade-core #190](https://github.com/jade-rs/jade-core/pull/190) and
[jade-office #121](https://github.com/jade-rs/jade-office/pull/121).
Both integrate the current main branch. The application checkout used for this
integration is isolated from the user's untracked collaboration-demo work.
Only its already-tracked renderer lockfile is refreshed for the core dependency
update; the demo's functionality is not changed by these PRs.

The milestone records through [pivot-lifecycle-verification.json](pivot-lifecycle-verification.json)
are historical frozen checkpoints. Their source/build hashes, 1,448 XLSX cases,
164 named model cases, 38 OPC cases and XSD/SDK results are retained as originally
verified. Rebasing adds current upstream package/signature dependencies and
requires downstream lock updates. Clippy fixes subsequently simplify checked
option access, iterator use, type signatures and dialog state allocation; a
Word table unit-test fixture supplies the upstream `text_bounds` field.
The model also recognizes upstream `runFormat`/`paragraphFormat` revisions in
formatting classification and pane labels, retaining the earlier `formatted`
name. This resolves the native reject-from-pane and web document-comparison
regressions observed during full CI.
Those changes mean the historical hashes are not all the final PR hashes.

Integration checks pass with warnings denied for the XLSX/OPC packages and
all their targets, the calculation engine and all its targets, the native app
and all its targets, the browser app on wasm32, and the shared model with both
Skia and tiny-skia across all targets. The regenerated canonical engine patch
replays all 489 source/build files against the original hash-pinned 0.8.3 crate.
The native production/test and browser production compilations also pass.

[pr-integration-verification.json](pr-integration-verification.json) records the
current scoped hashes, changed historical inputs and local check evidence.
The PR checks contain the final integrated runtime suites, complete CI build
results and any platform limitations. The integration record does not turn
historical oracle results into new final-source or full Office-equivalence claims.
The core PR is merged. Its workspace tests, lint, XLSX native checks, Word
oracle and benchmark checks pass. The macOS Word timing case exceeded its
unchanged 25ms limit once (29.5ms); a rerun was requested without weakening it.
Final job results remain linked from the PR.
All remaining compatibility gaps in [ibis-parity.md](ibis-parity.md) still apply.
