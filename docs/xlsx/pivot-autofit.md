# Pivot refresh column fitting

The later [PivotChart checkpoint](pivot-charts.md) adds atomic bound-chart
updates and worksheet/source name identity. Its totals and source hashes
supersede this historical fitting-only checkpoint.


Selected and workbook-wide pivot refresh now save fitted column widths in the
same history edit as rebuilt caches, reports and dependent formula results.
Both application shells supply their backend font measurement through the shared
model. A headless caller can supply `LayoutOptions` to
`refresh_pivot_with_layout` or `refresh_all_pivots_with_layout`; the existing
methods use deterministic fallback metrics when no font backend is available.
Those fallback widths are not exact Office font measurements.

## Fitting inputs

The fitter measures complete number-format text and a cell's own font, before
width-dependent scientific/hash substitutions. Automatic pivot font emphasis
is not used for fitting. Indentation and cell padding contribute to the width.
Report-filter values and label headers reserve dropdown space. A corner caption,
unrelated worksheet content, merged cells and hidden/zero-width columns do not
drive widths. Noncompact reports retain their first label column's width.

`applyWidthHeightFormats="0"` disables automatic width changes. An absent flag
uses the pinned reference's enabled policy; this is not a claim that the XSD
supplies that default. `useAutoFormatting`, `preserveFormatting` and legacy
`autoFormatId` have separate roles and are not reinterpreted as this width switch.
The existing custom/manual-style preservation behavior is retained.

The reader now retains location `rowPageCount`/`colPageCount`. Checked page-field
coordinates use `pageWrap` and `pageOverThenDown`; conflicting dimensions or
coordinates outside the worksheet cannot silently select unrelated cells.
All 22 reports in the unmodified normative workbook resolve their actual saved
B2/C2 filter cells. Wrapped synthetic cases also test both directions. They do
not establish all producer/filter-layout oracles.

Multiple rebuilt reports sharing a column contribute their widest requirement.
Refresh-all fits the set rebuilt so far after each staged cache update, before
its next calculation/dependency check. A later short report cannot shrink a
column below an earlier refreshed report's requirement. Shared-cache reports
on different worksheets are included. The writer preserves unrelated metadata
and skips widths already equal to the target.

Protection permissions remain independent: an authorized pivot refresh can
proceed while forbidden column formatting keeps saved widths intact. A pivot
permission refusal still rejects the complete refresh. Invalid metrics,
conflicting geometry, later cache failures or exhausted fitting budgets restore
the whole command, including prior formula caches, widths and the redo branch.
The shared model also wraps refresh plus relayout in its atomic edit checkpoint.

## Evidence and limits

The first regression reproduced a long refreshed label leaving the report
without a fitted width. Cases now cover saved width/history, explicit opt-out,
protected widths, invalid metrics, failure after an earlier cache's width update,
and two distinct caches sharing physical columns. Layout tests use an independent
fixed measurement to check literal expected pixels for captions, filters,
numeric formats and fonts. The shared TinySkia integration uses a narrow-glyph
label to distinguish actual backend measurement from the generic half-em
fallback, ignores unrelated wide text, reopens the widths and verifies one-step
Undo/Redo. Existing pivot style raster/history cases remain in the regression.

Reports are bounded to one million cells. Sparse fitting scans are bounded to
20 million inspected cells per call and conservatively charged against a
20 million inspection budget for a refresh batch. Stored widths use the existing
pixel/MDW conversion and 255-character limit. These are implementation resource
bounds, not full Excel size-domain equivalence.

Automatic refresh on file open, complete legacy AutoFormat behavior, all complex
rotation/rich-text/group-caption/filter layouts, every font/producer exact-width
comparison and Office save/repair/manual-interaction oracles remain open.
PivotChart binding synchronization and pivot editing also remain in the
[full audit](ibis-parity.md). This follow-up does not establish full ibisOffice
parity.

Primary semantics: [Excel PivotTable options](https://support.microsoft.com/en-gb/excel/pivottable-options)
and [standard location/filter dimensions](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.location?view=openxml-3.0.1).
The independent implementation reuses Jade's unit conversion, font measurement,
number formatting and preserving writer; no reference implementation code was
incorporated.

## Verified checkpoint

| Check | Result |
|---|---|
| Reader/writer/calc/headless/layout/render | 1,190 passed /174 targets |
| API/CLI/MCP/diff | 182 passed /15 targets |
| Standalone native renderer | 13 passed /4 targets |
| Combined eleven XLSX packages | 1,385 passed /193 targets |
| Shared OPC suite | 22 passed |
| Shared model, named tiny-skia targets | 153 passed /28 targets |
| Actual native production/tests and wasm production compilation | Passed |

The final snapshot covers 333 scoped source/build/data/provenance inputs and
28 model targets. All 209 scoped dirty/new Rust files pass formatting. The
engine is unchanged; its historical runtime evidence is carried, while patch
replay was rerun. Detailed source hashes, final target results, metric
regressions and exclusions are in
[pivot-autofit-verification.json](pivot-autofit-verification.json). Totals replace
earlier checkpoints; the full parity goal remains active.
