# XLSX hyperlink authoring

Jade now creates, edits and removes worksheet hyperlinks. Both shells expose
Hyperlink and Remove Hyperlinks from the cell context menu; Command/Ctrl+K
opens the editor. It accepts web/mail/file destinations or a workbook place
such as `#'Sheet 2'!B3` / `#Destination`, with optional display text and
ScreenTip. Visible worksheets and range names are offered as destinations.
The saved ScreenTip is shown on hover; a cell note takes priority when present.

The operation applies to the selected rectangle, retaining links outside it.
Partial overlap subtracts rectangles rather than discarding a larger link.
The top-left empty cell gets the destination as its visible text; an explicit
label changes only a non-formula cell. An unchanged numeric label retains the
numeric cell type. Formula cells keep their formula. Text and hyperlink
formatting are saved with the link in one undo step. Removal keeps the text
and formatting. This matches the basic [Excel link editing
flow](https://support.microsoft.com/en-us/excel/work-with-links-in-excel).

External destinations use worksheet relationships with `TargetMode=External`;
internal destinations use `location`. External workbook subaddresses can keep
both target and location. Relationship IDs are allocated without replacing
existing records, identical external targets can share a relationship, and
removal prunes only affected hyperlink relationships that no element in the
worksheet still references. Exact-range edits retain unknown link metadata;
other worksheet parts and unrelated relationships remain intact.

The reader and writer cover strict/transitional namespaces, alternate prefixes,
foreign lookalike elements, escaped text and ST_Xstring display/tooltip/location
attributes. The reader also retains compatibility with legacy `r:id` attributes
whose conventional namespace declaration is missing; editing such a link
replaces that attribute with a correctly bound relationship ID. A prefix bound
to a foreign URI is not treated as a relationship. The writer inserts hyperlinks in worksheet schema order, including
when the original worksheet is self-closing. Protected-sheet edits honor
hyperlink permission and cell locking. Styling a spill range does not count as
writing into its outputs; an explicit label that overwrites an output still
blocks the spill and clears its other outputs.

Remaining limits:

- Relative file paths and hyperlink base properties are saved but opening them
  relative to the workbook's filesystem location/base address is not yet
  implemented in both shells.
- The link style currently uses the default Office blue rather than looking up
  a customized theme hyperlink style; visited-link styling remains absent.
- Existing linked formulas retain their formula; this dialog does not rewrite
  the arguments of a `HYPERLINK()` formula.
- File pickers and a dedicated email form are absent; their addresses can be
  entered directly.
- No Excel save/repair or visual oracle has been captured. The authoring tests
  use Jade save/reopen, preserving XML assertions and shared/native scenarios;
  see [verification.md](verification.md).
