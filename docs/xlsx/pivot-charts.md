# PivotChart synchronization

The later [saved-pivot authoring checkpoint](pivot-authoring.md) adds explicit
measure identity mapping and a common editor. It also corrects the original
fixture generator's missing required table-to-cache relationship and adds legacy
repair plus independent XSD/SDK checks. This page's counts/hashes describe the
earlier checkpoint.


The standard `chartSpace/pivotSource` binding is now read explicitly. Refreshing
a locally resolved report reconciles its chart categories and series in the
same staged transaction as caches, report cells, column widths and calculation.
This closes a gap in the [pinned ibisOffice comparison](ibis-parity.md), where
the reference updates bound charts after rebuilding pivot data. Jade's
implementation is independent; no reference implementation code is copied.

## Behavior

- Categories come from visible report leaves. Subtotals and grand totals are
  excluded when a dimension is present. Multiple column members/measures,
  sparse intersections and values on rows have projection regression cases.
- Stable cache member identities retain point colors and labels when a member
  moves. Existing series retain their styles, IDs and combination-chart lane;
  added measures produce actual series. Ambiguous or filtered saved series
  are refused atomically rather than assigned an arbitrary identity.
- Contiguous report references retain their formulas and source wrapper
  extensions while caches and ranges change. Noncontiguous output can become
  literal chart data only when the source metadata is understood. Opaque
  metadata requiring conversion, custom error bars and unsupported plot
  topology refuse the complete refresh.
- The XML path handles classic categorical bar, line, area, pie, doughnut and
  radar plots, including combinations. Concrete refresh and pixel cases cover
  column charts and a column/line combination; this is not a complete producer
  oracle for every declared family or option.
- Chart titles, plot/series styles, axes and unrelated extension XML remain.
  Selected/all refresh retains one final history entry; a later chart failure
  restores source bytes and the existing redo branch.
- Renaming a supported report sheet updates its locally corroborated chart
  binding. Sheet entries and pivot worksheet-source names now decode/encode
  `ST_Xstring`; chart source names and formulas remain plain XML strings.
  Literal `_xHHHH_`, apostrophes and XML entities survive refresh and reopen.
  Imported workbook defined names also use their logical decoded identity:
  formulas calculate, update/delete targets the existing name, new names are
  encoded and named pivot sources remain refreshable through save/history.

## Binding and limits

The source follows Office's `[filename]sheet!pivottable` spelling. Jade requires
a unique declared local sheet/table plus report-local category/value references
or matching saved literal data. Explicit external data, path/URI-bearing source
tokens, unresolved sources and inconsistent local references are retained
without fetching or rebinding them. The byte-only API does not know the actual
workbook filename: it cannot independently verify that component or rule out
every same-name external alias. No complete external-link identity guarantee
is claimed.

Projection is bounded to one million report cells, the existing 100,000 chart
point limit, and one million series/category intersections. The planned matrix
is charged before value-vector allocation, with at least one slot per empty
series. Physical rows are indexed once by measure to avoid repeatedly scanning
the complete report for values-on-rows charts. These are Jade resource
policies, not Excel equivalence claims. Series measure identity uses
the data-field index during refresh; future field insertion/reordering APIs
must reconcile that identity explicitly.

Pivot field buttons/controls, chart field filtering, full multilevel category
reconciliation, complete date axes, custom error sources, filtered series,
extended/3-D/stock/scatter/bubble PivotChart conversion, automatic load refresh,
OLAP, slicers/timelines and broader pivot authoring remain open. The general
chart parser's complete namespace/MCE/schema/resource coverage is also outside
this change.

## Evidence and provenance

The original, deterministic
[fixture generator](../../scripts/xlsx/generate_pivot_chart_fixture.py) produces
`jade-core/public/fixtures/pivot-chart-compat.xlsx`: East 15, West 27, an
explicitly sorted Region field, declared drawing/chart relationships, one blue
series, an orange West point and preserved extension metadata. Regeneration is
byte-identical. It removes the historical base fixture's nonstandard worksheet
pivot marker and uses its real internal relationship. This is synthetic test
data, not a workbook authored or repaired by Office.

The shared model test renders actual TinySkia pixels: two bars become three
after refresh, survive save/reopen, and return through Undo/Redo. The native
renderer independently samples both saved bar colors using Skia. Writer tests
also cover compact ancestor labels, values-on-rows and sparse multi-measure
projections. Headless tests cover repeated edits, growth/removal, combinations,
series reordering, extension preservation, external-data exclusion, naming,
save/reopen and complete failure rollback.

The official schema confirms `CT_Sheet/@name`, `CT_DefinedName/@name` and
`CT_WorksheetSource/@sheet` and `@name` use `ST_Xstring`. The chart binding uses
plain string text. Relevant references are the
[Office pivot-source note](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oi29500/3a82b89e-7264-40c1-a09f-d569374fe6a9),
[PivotSource definition](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.drawing.charts.pivotsource?view=openxml-3.0.1)
and [ECMA-376](https://ecma-international.org/publications-and-standards/standards/ecma-376/).
No new third-party asset is shipped. The inspected Apache POI
`ExcelPivotTableSample.xlsx` has no charts and supplies no PivotChart oracle.
Three original fixtures from the pinned Open XML SDK repository
(`RelationalPivotA2`, `RelationalPivotB2`, `OlapPivotA3`) also pass a temporary
reader probe asserting their complete filename/sheet/table bindings. The
fixtures contain multilevel/external/OLAP features outside this refresh scope;
this check establishes binding reading only. They remain outside the repository
and package test totals. Revision, URLs, asset hashes and probe evidence are
recorded in the verification file.

The complete Office save/repair/visual and native/browser pointer/keyboard
interaction oracles remain unverified. Full ibisOffice parity remains open.

## Verified checkpoint

| Check | Result |
|---|---|
| Reader/writer/calc/headless/layout/render | 1,213 passed /176 targets |
| API/CLI/MCP/diff | 182 passed /15 targets |
| Standalone native renderer | 14 passed /4 targets |
| Combined eleven XLSX packages | 1,409 passed /195 targets |
| Shared OPC suite | 38 passed /8 targets |
| Shared model, named tiny-skia targets | 154 passed /28 targets |
| Actual native production/tests and wasm production compilation | Passed |

The final snapshot covers 342 scoped source/build/data inputs and 28 named
model target sources. All 218 scoped dirty/new Rust files pass formatting.
The engine and registry are unchanged: 2,246 historical passes/eight existing
ignored cases and 507 registered functions are carried runtime evidence;
the unchanged 489-file patch was replayed against the pinned release again.
These totals replace earlier checkpoints and are not additive. Detailed hashes,
target results, original failing regressions and exclusions are in
[pivot-charts-verification.json](pivot-charts-verification.json). Full parity
remains open.
