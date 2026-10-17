# Validation

## Initial candidate — October 16, 2026

Linux x86_64, Rust/Cargo 1.99.0, offline cached dependencies, two workers:

- 15 unit tests passed.
- All-targets Clippy with warnings denied passed.
- The mechanical rustfmt candidate passed formatting, all 15 tests and Clippy.
- CLI catalog/policy validation accepted 11 messages, 22 bindings and zero approvals.
- WordCraft and PdfCraft exports emitted zero approved rows and reported 11 pending candidates, as intended.
- Invalid app names, unknown catalog `approved` fields and malformed JSON were rejected.

Formatted source SHA-256:
- lib.rs: 0a8bcfafdd2343037201a3e36a28849f873ccdafd250e2cd662f0c3b537f6960
- main.rs: 3f9ed6c6f73882d1ab36a52f1ce9dedb67ffa7bf34539bff8e7d05bb4deb5390
- tests.rs: 9941676f9311cf4936380b37d2956485ef3115b30c95c697c1b3e2046daceeaa
- Cargo.lock: 53eb7a22aff398901cb32d40acdf717141d91e323d15061d27a9adecae9b42f2

## Recheck — October 17, 2026

The unchanged formatted candidate was rechecked on Linux: all 15 tests, formatting and all-targets Clippy passed again (exit 0). All 14 input/manifest hashes matched before and after. Publication documentation is separate from those runtime checks.

## Not validated or implemented

Native WordCraft/PdfCraft parser integration, baseline catalog merging, app UI workflows, full plural support and a translation editor/service are not implemented or qualified. No translation has been human-approved. No existing app catalog has been changed.

JSON schemas describe structural shape; the Rust validator additionally enforces cross-record and compatibility rules. Future app adapters must validate their merged catalogs through the real app parsers before migration.
