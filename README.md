# CraftLingo: first implementation slice

Shared translation data, app-specific meaning, and community review without giving a translation service access to application code.

## Status and limits

This is an initial implementation candidate, not a deployed service or an application release. No translation service, credentials or app integration has been configured. Existing WordCraft/PdfCraft catalogs are untouched. The first Spanish pilot contains 11 proposed shared actions with pinned source/usage links, not 11 newly human-approved translations.

- `../craft-translations/catalogs/`: data-only payload in the separate translation repository.
- `tooling/`: trusted Rust validator/exporter and schemas, kept outside service write access.
- `trusted/`: explicit legacy-key bindings, source provenance and the approval policy, also outside service write access.

Tooling and translation data use separate repositories. Separate repositories alone do not enforce review: production must maintain distinct write permissions and a trusted CI job. Never grant a translation service write access to tooling, bindings or approvals. Do not execute workflows, build scripts or tooling supplied by the catalog repository. Consume a pinned catalog revision as data only.

## First commands

Check out `FredApps/craftlingo` and `FredApps/craft-translations` as sibling directories. During draft review, use `feat/approved-translation-overlays` for tooling and `feat/spanish-shared-pilot` for data; main contains only the bootstrap README. From `craftlingo/tooling/`, run:

```
cargo run -- check ../../craft-translations/catalogs/es/common.json ../trusted/pilot-policy.json
cargo run -- export ../../craft-translations/catalogs/es/common.json ../trusted/pilot-policy.json wordcraft
cargo run -- export ../../craft-translations/catalogs/es/common.json ../trusted/pilot-policy.json pdfcraft
```

The checked-in policy has ZERO approvals. Initial exports intentionally contain no translation rows and report all 11 candidates pending. This avoids pretending imported or AI-authored strings have received a native-speaker review.

Exports are deterministic, ordinary three-column TSV OVERLAYS, not complete catalogs. Never replace an app's complete catalog with an overlay or concatenate it to create duplicate keys. A subsequent reviewed adapter must merge by exact `(context, source)` into a pinned baseline, preserving unrelated rows/comments and validating through each app's actual parser. That merge and app integration are not implemented in this slice.

## Approval boundary

Catalog provenance (`imported`, `human`, `ai`) grants no permission. Export requires a trusted approval binding the exact locale, semantic ID, source English, app/shared scope and translation via a domain-separated SHA-256 of a fixed JSON tuple. A text/source/locale/scope/ID change invalidates the approval. Unknown catalog fields such as `approved: true` are rejected.

The trusted approval policy is the security boundary. A reviewer name inside a file is not authentication; protect the policy with maintainer review in the code repository. The tool does not create approvals or attest that a reviewer is human. Maintainers must verify that outside the tool. No catalog-side reviewed flag is accepted.

An approved app override wins. Otherwise an explicitly permitted and approved shared translation is inherited. An unapproved override never overwrites an approved value. Without either approval, no row is emitted, preserving the application's existing catalog/fallback. Exact English equality alone does not authorize a shared semantic binding.

## Compatibility contract

- Version 1 handles ordinary text only. It rejects reserved `@id`/`@plural` adapter contexts rather than flattening them incorrectly. Plural schemas and locale-specific plural rules are deferred.
- WordCraft contexts are limited to empty, `preview`, `template`. PdfCraft ordinary non-reserved contexts remain distinct.
- Placeholder comparison matches existing parsers exactly: sorted MULTISETS of text between each opening `{` and next `}`, including repeated/empty names. It is intentionally not a new format-string grammar.
- Ordinary source/translation trailing Unicode `…` must match exactly, without trimming. ASCII `...` is different.
- TSV escapes only backslash, tab and newline as `\\`, `\t`, `\n`; unsupported control characters are rejected.
- Duplicate semantic IDs, scopes, approvals and legacy output keys fail validation. Namespace IDs remain separate even when English labels happen to match.
- WordCraft's neutral Spanish and PdfCraft's European-Spanish style policies remain app-owned. Identical pilot values are proposals, not a decision to unify regional style.
- Inputs are bounded to 8 MiB per CLI file, 10,000 messages and limited policy sizes. No network access, dynamic code loading or catalog-controlled file writes occur.

## Pilot

Save, Undo, Redo, Copy, Cut, Cancel, Close, Apply, Print, OK, Browse…. Pinned baseline/usage links are in `trusted/pilot-provenance.json`. Open is intentionally excluded because WordCraft also uses it for paragraph spacing. Select All is excluded because the observed usage scopes differ.

## Next acceptance gates

1. Preserve the passing unit/CLI, formatting and Clippy baseline documented in VALIDATION.md and re-run it for changes.
2. Human Spanish/semantic review of the 11 proposed bindings; only then add trusted approvals.
3. Implement explicit non-destructive baseline merge and validate exported catalogs with the actual app parsers.
4. Exercise one complete Spanish workflow in each app, including missing translation and app override cases, and check bundle-size/offline behavior.
5. Establish separate repository permissions before connecting any translation service. No service is selected or connected yet.

No project-wide migration, full plural support, translation-editor UI or app release is claimed by this first slice.
