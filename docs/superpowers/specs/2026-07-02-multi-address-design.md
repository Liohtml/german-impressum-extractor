# TP4: Multi-address extraction

**Status:** Approved design (2026-07-02) — autonomous continuation (Spec→PR without per-gate acceptance).
**Sub-project:** TP4 of 5. Builds on TP1's block segmentation (merged).

## Rationale / scope choice
The roadmap's "multi-entity" theme meant "return multiple addresses / phones / persons cleanly separated." But `phones`, `persons`, and `emails` already return `Vec` (multiple). The one real gap — and a documented README limitation — is **address**: `extract_address`/`extract_all` return only the *first* address, so pages listing multiple branch/location addresses lose all but one. TP4 closes exactly that gap.

Full **entity grouping** (associating a specific person + address + phone into one legal-entity record) is genuinely fuzzy and error-prone; it is explicitly out of scope for TP4 (future work). TP4 delivers the concrete, bounded, low-risk win: *all* addresses, in document order.

## Scope
**In scope**
- A public `Address { postcode, city, street }` struct (each `Option<String>`), serde-gated derives, `Debug/Clone/Default/PartialEq/Eq`.
- `extract_addresses(&str) -> Vec<Address>` — one `Address` per text block that contains a postcode/city and/or a street, in document order, exact-duplicate-deduplicated. Built on TP1's block model (a block = lines between blank lines; a real address's street + "PLZ City" lines sit in the same block).

**Out of scope**
- Entity grouping (person↔address↔phone association). Multi-value for phones/persons/emails (already `Vec`). New fields (TP5). Changing `extract_address`, `extract_all`, or `Extracted`.

## Design
`Address` and `extract_addresses` are purely additive:

```rust
pub fn extract_addresses(text: &str) -> Vec<Address> {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    addresses_from_document(&doc)
}
```

`addresses_from_document(&Document)`: iterate `doc.block_texts()`; for each block run the existing `parse_postcode_city` + `parse_street` helpers; if the block has at least one component, push an `Address { postcode, city, street }` built from *that block* (never mixing components across blocks); dedupe exact duplicates; preserve document order.

`extract_address` (the singular tuple API) and `address_from_document` are left **unchanged** (byte-identical) — TP4 adds a parallel path rather than refactoring the single-result logic, guaranteeing non-breaking behavior and identical existing-test results. The two paths share the `parse_postcode_city`/`parse_street` helpers (no logic duplication).

`Extracted` is **not** modified (adding a public field would be a breaking change to struct-literal construction); callers who want all addresses call `extract_addresses`.

## Error handling / constraints
- Infallible/panic-free; `#![forbid(unsafe_code)]`; MSRV 1.85; no new dependency.
- `Address` derives `Eq` (all fields are `Option<String>`), unlike the score types.
- Every public item (`Address`, its fields, `extract_addresses`) documented (crate is `#![warn(missing_docs)]` + clippy `-D warnings`).

## Testing
- Non-breaking: `extract_address` and all existing tests unchanged.
- Multi-block: a page with two distinct address blocks yields two `Address` values, each with the correct same-block components, in document order — no cross-block mixing.
- Single address: `extract_addresses` yields one `Address` equal to the `extract_address` tuple's components.
- Partial blocks: a block with only a street (or only a postcode/city) yields an `Address` with the present component(s) and `None` for the rest.
- Dedup: an address block repeated verbatim yields a single `Address`.
- serde round-trip of `Vec<Address>` (under `serde`).

## Success criteria
1. `extract_address`, `extract_all`, `Extracted`, and all existing tests unchanged (non-breaking).
2. `extract_addresses` returns every address-bearing block's `Address`, same-block components only, document order, deduped.
3. For single-address input, `extract_addresses(t)[0]` components equal `extract_address(t)`.
4. No new dep; builds + tests on Rust 1.85; clippy `-D warnings` clean.
