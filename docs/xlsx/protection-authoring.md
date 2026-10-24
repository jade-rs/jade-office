# Workbook protection authoring and authentication

The headless editor, shared model and both worksheet shells can now protect
workbook structure with an optional password and authenticate its removal.
The sheet-tab context menu opens **Workbook Protection…**. The shared dialog
masks both inputs, confirms a new password, reports wrong/unsupported
credentials, handles field focus and IME composition, and retains the dialog
when authentication fails. Cancel does not write. Native/browser runtime
interaction captures are still required; compilation and shared-model tests
are separate evidence.

## File behavior

Password authoring writes SHA-512, a fresh platform-random 16-byte salt and
100,000 iterations. The shared model returns an error if platform randomness
is unavailable; it does not use a time-based or zero-filled fallback. The
low-level writer accepts an explicitly supplied salt for non-UI callers.
No plaintext password is written to the package. Passwordless protection
writes the structure flag without inventing verifier attributes.

The preserving writer follows the workbook's actual package relationship,
including relocated workbook parts, and inserts protection before views and
sheets. Strict and Transitional namespace aliases are supported. Foreign
siblings, revision credentials, unknown attributes and extension children
survive. Unprotect clears structure/windows flags and their shared workbook
credentials after authentication, preserving revision protection. Existing
credentials must be removed with authentication before a replacement can be
created. No-op removal writes no bytes or history.

Wrong passwords, malformed/incomplete modern metadata, unsupported hash
algorithms and duplicate owned protection records are refused. A modern
record never falls back to a matching legacy verifier after failed or
unsupported verification. Source, cell values and undo/redo remain unchanged
on these refusals. Successful metadata changes are undoable and survive
save/reopen; the existing structural guards follow the restored state.

## Algorithms and evidence

ISO verification supports SHA-1, SHA-224, SHA-256, SHA-384 and SHA-512 over
UTF-16LE. A leading BOM is removed; ordinary password whitespace is retained.
After the initial salt/password hash, each iteration appends its little-endian
32-bit counter to the previous hash. Spin counts above 10,000,000 and salts
above 65,536 bytes are rejected. Base64 XML whitespace is folded without
counting it toward the payload allocation bound. New passwords are limited
to 255 UTF-16 units.

The algorithm and resource bounds follow Microsoft's
[ISO write-protection description](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-offcrypto/1357ea58-646e-4483-92ef-95d718079d6f).
Legacy verifier operations follow the
[Method 1 description](https://learn.microsoft.com/en-us/openspecs/office_file_formats/ms-offcrypto/fb2d125c-1012-4999-b5ef-15a2bd4bec36),
with the SpreadsheetML empty-password zero convention. The XML attribute
names and independent structure/windows/revision roles are described by
[WorkbookProtection](https://learn.microsoft.com/en-us/dotnet/api/documentformat.openxml.spreadsheet.workbookprotection?view=openxml-3.0.1).

Legacy XML does not identify its originating Windows ANSI code page. The
verifier tries the listed Western/Central/Eastern European, Thai, Japanese,
Chinese and Korean Windows encodings, plus a low/high-byte convention used
by existing OOXML consumers. This is an explicit cross-locale compatibility
policy. It does not prove every Office locale's legacy conversion or resolve
the inherent collisions of a 16-bit verifier. An `encoding_rs` dependency
was added to the shared OPC protection module for these conversions; other
locked package versions were retained.

Generic ISO attribute names referenced by the [Office implementation notes](https://learn.microsoft.com/en-us/openspecs/office_standards/ms-oe376/ae53f189-04e6-45e6-acb2-7f61fecabee4)
are recognized as an import compatibility form. Modern verification takes
precedence over legacy attributes across both naming forms. If multiple
modern verifiers are present, each must authenticate; generic ISO attributes
with revision protection or revision credentials are refused because their
scope is ambiguous. This policy does not establish an Office application oracle.

Five cryptographic test cases include independent Python hashlib vectors for
all five ISO algorithms, non-BMP text, BOM/whitespace, fresh supplied salts,
partial/unsupported-modern rejection, metadata bounds, and independent legacy
ASCII/CP932/CP1251/CP1252 vectors. Eleven writer/headless cases exercise strong
and passwordless authoring, wrong/correct passwords, independent imported
SHA-256, windows-only protection, revision/foreign metadata, strict aliases,
relocated parts, saved guards, historical ISO attributes and history. Six additional shared-model cases
exercise confirmation/masking, cancellation, CSPRNG salt freshness, rejected
entries, historical ISO authentication and redo. The extended preflight
reproduced two generic-attribute authentication failures (seven controls
passed); the refreshed final model driver checks the repaired parser. Exact regression/build scopes are in
[protection-authoring-verification.json](protection-authoring-verification.json).

## Remaining work

A [worksheet follow-up](sheet-protection-authoring.md) adds sheet authentication,
cell locks/hidden contents and shared permission controls. Editable ranges and
session unlocks, revision/window operation guards, encryption, additional digest algorithms,
broader legacy/application oracles and general MCE around protection records
remain open. The existing structural writers' broader ownership and complete
menu availability also require coverage. Neither package round-trip tests
nor independent digest vectors replace Excel save/repair/calculation/UI
oracles. The [ibisOffice comparison](ibis-parity.md) remains active. No
reference implementation code was incorporated.
