# XLSX data validation

The validation work extends Jade's standard SpreadsheetML and x14
`dataValidation` rules. It is part of the ongoing [ibis comparison](ibis-parity.md), not a
claim of complete Excel parity.

## Implemented paths

Both native and browser worksheets offer Data Validation and Clear Data
Validation in the context menu. The dialog edits Any value, Whole number,
Decimal, List, Date, Time, Text length and Custom rules, comparison operators,
Ignore blank, the in-cell dropdown, input messages, error messages/styles and
stored IME mode. It accepts multiple target ranges and can apply changes to
cells with matching settings. The Input Message and Error Alert tabs support
multiline messages. All changes and multi-range clearing use one history step.

List sources accept literal comma-separated entries or `=Name` / `=formula`.
Simple cell ranges retain their displayed number formats in dropdowns. Named
ranges, hidden source sheets, INDIRECT and OFFSET are evaluated. Relative
references follow the active cell and are rebased when editing part of a rule
or clearing its first range. Numeric/date bounds can use formulas. Typed dates
are converted using the workbook's 1900 or 1904 epoch.

The candidate value is installed in a scratch calculation model before the
predicate is evaluated. Custom rules such as `COUNTIF($A$1:$A$10,A1)=1` therefore
see the proposed entry. Formula entries are checked using their calculated
result. Source workbook cells are not changed by a rejected entry. Stop
cannot be bypassed through the warning-acceptance API. Warning and Information
require an explicit choice before saving invalid input. Disabled error alerts
allow the entry. When error alerts are enabled, unsupported evaluation is surfaced explicitly.

The alert choices and editing-only trigger follow [Microsoft's validation
behavior](https://support.microsoft.com/en-us/excel/more-on-data-validation).
Paste and fill continue through their existing editing paths; they do not
invoke the typed-entry alert flow. A pending alert prevents a subsequent paste
from silently overwriting the unresolved entry. Protected sheets reject rule
authoring, including rules on unlocked cells.

## Package preservation

The reader recognizes transitional and strict namespace URIs, namespace
aliases, entity/CDATA formula text and ST_Xstring prompt strings. Unrelated
foreign elements are not interpreted as rules. The writer preserves
foreign children/attributes, container settings and rule metadata while
updating counts, formulas and ranges. Removing part of a rule rebases its
formula anchor. Empty worksheet roots and schema ordering are covered by
regressions. New formulas receive the required saved-file function prefixes.

## Remaining gaps and evidence limits

- General Markup Compatibility processing around entire worksheets, validation
  containers/records and other feature families remains outside this parser.
  The supported alternatives are sources within an interpreted input rule,
  including x14 formula-leaf choices, plus x14 coverage reading/whole-axis
  movement. Indexed authoring and partial shifts of coverage alternatives
  remain open. Custom-type `x12ac:list` variants and
  additional noncanonical quoting/coercion cases still need application oracles.
- Container-wide `disablePrompts` and saved prompt-window coordinates remain
  preserved settings; they are not yet reflected in prompt presentation.
- Circle Invalid Data, validation-only Paste Special and automatic revalidation
  after dependency changes are not implemented.
- Input prompts currently use a fixed panel. Excel's near-cell placement and
  Escape dismissal remain to be implemented.
- Accepting an invalid entry currently keeps the original active cell; the
  navigation request that triggered the alert is not replayed.
- IME mode is stored and editable; it does not force an operating-system input
  mode. IME composition routing is tested in the native view harness.
- Dynamic dropdown values currently use General formatting and sources larger
  than 10,000 items return unsupported.
- Evaluation projects current workbook values. Predicates that need other
  dependent formulas recalculated against the candidate need further work and
  Office comparison. Matching-rule expansion currently compares normalized
  saved settings, not equivalence of different relative formula spellings.
- Excel's title/message length constraints, locale-specific list separators,
  and advanced list coercion need explicit format/oracle cases.
- No Excel application save/repair, new calculation oracle or pixel comparison
  has been run. Tests and compilation evidence are in
  [verification.md](verification.md).

## Extended input rules

The reader recognizes x14 rules in worksheet extension containers, reading
`xm:sqref` and the `xm:f` inside each formula wrapper. Standard rules precede
extended records in the public list. Shared record locations keep reader and
writer indices aligned even with unmodeled/missing-range siblings and multiple
extension containers. Namespace aliases and default namespaces work in both
Transitional and Strict worksheets. Foreign wrappers/formula lookalikes are
not interpreted.

Existing x14 rules can be edited/deleted by index in their original storage
form, retaining revision IDs, unknown attributes/children and container
settings. Adding missing formula wrappers respects schema order; new formula
leaves get a local namespace declaration when necessary. A newly appended
standard rule returns its actual reader index before the x14 records.
Clearing/replacing overlapping ranges operates across both forms and rebases
retained relative bounds. Empty recognized containers are removed.

The shared candidate validator and input model use the imported rules for
list choices, formula bounds, Stop/Warning/Information alerts and prompts.
Dialog apply replaces the selected coverage using the standard authoring form;
low-level indexed edits retain the original x14 form. Partial structural shifts
continue to follow ranges, wrapped formulas and cloned revision identities;
see [cell-shifts.md](cell-shifts.md).

The format mapping follows Microsoft's [CT_DataValidation](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/89029dfc-1ca8-4ff9-afe0-46f9454d09c6),
[formula wrapper](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/20ed0abd-113f-4b8a-8de3-c68e733a300a)
and [container](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/4ec60c5e-de69-4d16-944d-7fab3e45fdff)
definitions. In particular, this x14 definition gives `showDropDown` display
semantics, mapped to the model's `suppress_dropdown` flag; standard rules retain
the existing suppression mapping. Excel application captures remain necessary
to verify imported-file behavior independently of the specification.

The preceding extended-rule checkpoint passed 858 core tests across all 138
targets and eight shared-model input-rule scenarios; see
[extended-validation-verification.json](extended-validation-verification.json)
for that historical snapshot.

## Quoted literal choices and source alternatives

`DataValidation.literal_list` retains decoded `x12ac:list` CSV text separately
from `formula1`. Literal entries are never parsed as formulas. The list codec
keeps quoted commas, doubled quotes, embedded line breaks, empty entries and
spaces. XML entities, CDATA and ST_Xstring escapes are decoded at their proper
layers. Malformed CSV is retained on import but evaluates as unsupported;
invalid authored CSV is refused atomically.

Within a recognized rule, the reader chooses the first `mc:Choice` whose
`Requires` prefixes all resolve to understood namespaces. Otherwise it reads
`mc:Fallback`. Prefix aliases and Strict namespaces work; foreign lookalikes,
unbound requirements and unknown choices cannot override the selected source.
This is source-level MCE handling, not a general worksheet MCE processor.

Indexed source edits keep branch/container metadata and opaque siblings,
update understood choices and fallbacks together, and remove stale source
representations when switching between formulas and literal choices. New rich
lists use a namespace-bound AlternateContent choice and a standard formula
fallback. Older processors using that fallback cannot distinguish a literal
comma from an item separator; exact rich candidates require the supported
choice. Coverage clearing and partial cell shifts retain literal values;
relative formulas in both source alternatives rebase at each split anchor.
Nested x14 formula-leaf alternatives update in place rather than receiving a
second direct `xm:f` beside their old choice.

The shared dialog displays/edits CSV choices, keeps formula sources prefixed
with `=`, and reports malformed quoting in both UI languages. Candidate
selection in both shells uses the model's validation-choice method so a value
such as `=SUM(A1)` is entered as text rather than executed as a formula.
Existing ordinary numeric coercion is retained; valid textual boolean/date
choices retry as text when normal typed-entry validation would reject their
coerced type. Disabled-alert and broader locale/coercion behavior still needs
application comparisons.

This format handling follows Microsoft's [list element](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/de43f3b8-5c1b-4e5c-96a7-7f18e7bcdb6c)
and [Open XML data validation model](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.datavalidation?view=openxml-3.0.1).
The codec uses the quoted-field rules of [RFC 4180](https://www.rfc-editor.org/rfc/rfc4180#section-2).
LibreOffice's [source-level import handling](https://github.com/LibreOffice/core/blob/master/sc/source/filter/oox/worksheetfragment.cxx)
was inspected only to corroborate the CSV and fallback representation; no code
was copied. Real Excel repair/save/calculation/pixel oracles remain outstanding.

The preceding quoted-list checkpoint passed 882 core tests across all 141 targets and 18
shared-model scenarios (eleven validation, seven tables). Native production/test
and browser wasm production compilation pass; these are compilation checks,
not manual UI runtime or Excel application evidence. Detailed results and
source hashes are in
[quoted-validation-verification.json](quoted-validation-verification.json).


## Whole-axis insertion and deletion

Whole-row/column edits now use the same interpreted rule locations as the
reader. Each standard/x14 container is consumed once, with only its recognized
records edited. Foreign containers, nested lookalikes, missing-range records,
unknown source choices and foreign formula wrappers retain their contents.
The generic worksheet formula pass no longer enters input-rule containers.
Counts include retained unmodeled siblings. When the last interpreted rule is
removed, a truly empty container goes away; a container with opaque children
or comments stays, with its remaining record count. Real Excel repair behavior
for such opaque-only imported containers still needs an application oracle.

A formula is stored for the first cell of the first range. If deletion removes
that source cell or an entire first union member, the writer first translates
the formula to the first surviving original cell, then applies the grid edit.
Thus deleting rows 1–2 from a rule over B1:B5 with a relative bound `$C1` keeps
`$C1` over the surviving B1:B3. A bound `$D$1` still becomes `#REF!` when that
absolute source is deleted. A reference into another sheet retains the
surviving cell's original relative operand. These are per-cell preservation
checks, not a captured Excel UI comparison.

Supported standard/x14 formula alternatives and fallbacks are updated once;
CDATA is handled as formula text rather than XML markup. Interpreted x14
coverage alternatives also move, including when only fallback coverage is
affected. Unknown choices remain opaque. Insertion clips retained rule coverage
at the last worksheet row/column instead of dropping a range whose far end is
pushed off-grid. Completely displaced members are removed. Partial cell shifts on the edited worksheet
also now leave unknown source choices untouched. The package-wide partial-shift pass now routes other worksheets through
reader-owned validation records and supported sources; see
[formula-ownership.md](formula-ownership.md). Other formula passes remain open.

The pinned reference's [structural writer](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-pptx/src/save/xlsx_structure.rs)
was re-inspected for ordinary validation coverage/formula and count updates.
Its implementation is comparison evidence, not an oracle establishing the
expected result of every imported alternative or deleted-anchor case. No code
was copied. Microsoft's [extended sqref definition](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-xlsx/4d7cc415-6c51-4c71-8dbd-a2e28bdd9193)
permits range adjustment during worksheet layout changes. General MCE around
records/containers, indexed edits or partial shifts of noncanonical coverage
alternatives, conditional formatting and other formula/extension contexts
still need their own cases.


The preceding whole-axis checkpoint passed 895 core tests across all 142 targets and 19 shared-model
scenarios (twelve validation, seven tables), with native production/test and
browser wasm compilation. See
[validation-structure-verification.json](validation-structure-verification.json)
for hashes, target results and evidence limits. No manual native UI or Excel
application oracle was run.


## Package-wide table and partial-cell formula edits

The common package pass now uses the worksheet ownership visitor for changes
on other sheets as well as the edited worksheet. Standard/x14 rule source
choices and fallbacks move once, while foreign and missing-coverage records
remain opaque. x14 coverage supplies the source's table/current-row context for
header renames and conversion to ordinary A1 references. The shared model
checks the resulting relative bound after header rename, including input
rejection, save/reopen and history. See
[formula-ownership.md](formula-ownership.md) for supported paths and remaining
whole-axis/sheet-operation, reader and application-oracle gaps.


The current formula-ownership checkpoint passes 905 core tests across all 143
targets and 20 shared-model scenarios (thirteen validation, seven tables), with
native production/test and browser wasm compilation. See
[formula-ownership-verification.json](formula-ownership-verification.json)
for target results, frozen hashes and evidence limits. Manual UI and Excel
application oracles remain outstanding.
