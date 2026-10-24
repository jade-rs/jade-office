# Password prompts for protected range entries

Native and browser cell editors now offer range-password authentication after
an ordinary typed entry is refused by cell protection. The dialog lists only
valid password-bearing ranges covering that locked cell. Overlapping ranges
are alternatives: authenticating any one covering range permits the entry.
Unprotected cells, unlocked styles, passwordless coverage and already-granted
coverage do not show this prompt. Formatting/object permissions are unchanged.

The [standard protected-range definition](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.protectedrange?view=openxml-3.0.1)
describes prompting users for a password-bearing range and permits everyone to
edit a passwordless range, irrespective of security descriptors. The
[range-unprotection API](https://learn.microsoft.com/en-us/office/vba/api/excel.alloweditrange.unprotect)
also documents the case-sensitive range password. Existing standard/x14,
legacy/ISO verification and unsaved session-grant policies remain in effect;
this does not add account/domain identity authentication.

The pinned ibisOffice [protection](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/protection.rs)
and [cell-edit](https://github.com/ibisjp/ibis-office/blob/6e004f71ed43ea73c1a6cb0f0496483c107a0485/crates/ibis-office-app/src/xlsx/cell_edit.rs)
files were reviewed for their explicit range-authentication and edit-guard
behavior. No implementation code was incorporated. This automatic dialog follows
the OOXML requirement above; a complete reference UI/call-graph comparison was
not performed.

## Candidate and pending-entry state

`HeadlessWorkbook::range_unlock_candidates` reuses the cached cell permission
check, including effective row/column/cell styles and current overlapping grants.
It returns only owned, unambiguous covering definitions whose authoritative
verifiers have supported, well-formed metadata. Invalid coordinates are refused.
`PasswordVerifier::validate` checks algorithms, base64 payloads, digest sizes,
salt/spin bounds and legacy spelling without running the spin iterations.
Modern metadata remains authoritative over a legacy fallback attribute.
The query changes no source, history or authentication grants.

The shared `ProtectionDialog` and `RangeEditor` bind an automatic request to
its workbook session identity, worksheet part, layout revision, range definitions,
cell and exact entered text. A weak reference to an `Arc` identity survives moves
and prevents applying a prompt to a replacement workbook with matching names,
bytes or revision. The same identity check also strengthens manually opened
range managers. Sheet switches, source-changing edits/history and changed
protection/range definitions invalidate an outstanding request. The target range
must remain one of the original candidates, even if a caller alters the public
selected-name field.

Both shells retain the refused cell editor. Cancellation or an incorrect
password leaves its text, source, history, layout revision and dirty state intact.
Successful authentication adds only a live session grant, closes the prompt and
retries the still-matching original entry through the ordinary commit path.
Formula syntax checks, data-validation alerts, recalculation, clipboard cleanup
and undo grouping therefore retain their existing behavior. The prompt itself
never removes worksheet protection or creates a document history step.

The prompt uses the existing localized range dialog, masked password input and
range list. It displays the target cell, omits range-management/relock controls
and closes after successful authentication. The manual range manager retains
its existing editing, authentication and relocking behavior.

## Verification and limits

Focused runtime cases exercise candidate ownership/coverage, overlapping grants,
malformed credential metadata, atomic query/refusal behavior, wrong password,
cancellation, exact-entry matching, different workbook identities, sheet round
trips, unprotect/undo staleness, save/reopen, history and ordinary formula and
data-validation refusal after authentication. Final gates passed 1,239 XLSX cases across 180 targets, 22 OPC unit cases and
116 shared-model cases across twenty-four named tiny-skia targets. Native
production/test and browser production checks passed. Final scope and source hashes are
recorded in [range-unlock-prompts-verification.json](range-unlock-prompts-verification.json).

This checkpoint covered ordinary typed single-cell commits, including
the formula-bar entry path used by those editors. Paste, clear, fill, compound
selections and other bulk operations retain their existing guarded refusal and
manual range-manager authentication; their automatic prompting was still open at this checkpoint; the later
[bulk-authentication follow-up](bulk-range-authentication.md) adds it for the
supported paste/clear/fill/move commands.
An interrupted navigation gesture is not replayed: successful authentication
commits the entry at its original cell. General merged/compound selection,
account/ACL authentication, full MCE authoring and real Excel/native/browser
interaction oracles remain open. Runtime model tests and shell compilation do
not establish pixel, keyboard or Office application equivalence. The wider
[ibisOffice parity audit](ibis-parity.md) remains active.

The later [compound-edit milestone](bulk-edit-atomicity.md) fixes data/history
leakage on returned errors from tiled paste and active-entry range fill.
The later [bulk-authentication follow-up](bulk-range-authentication.md) also adds
deferred password prompts and atomic command/grant commit for supported bulk edits.
