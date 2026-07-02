# TP4 Multi-Address Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an additive `extract_addresses(&str) -> Vec<Address>` returning every address-bearing block's address (document order, deduped), without changing `extract_address`/`extract_all`/`Extracted`.

**Architecture:** A new public `Address` struct + `extract_addresses` that parses each TP1 `Document` block with the existing `parse_postcode_city`/`parse_street` helpers, emitting one `Address` per block that has a component. The single-result `extract_address`/`address_from_document` are untouched.

**Tech Stack:** Rust (edition 2024), existing deps only.

## Global Constraints
- MSRV 1.85; no APIs newer than 1.85. `#![forbid(unsafe_code)]`; no unsafe. No new dependency.
- Non-breaking: `extract_address`, `extract_all`, `Extracted`, `Person`, all existing signatures + tests unchanged. `Extracted` gains NO field.
- Crate is `#![warn(missing_docs)]` + clippy `-D warnings`: every public item (`Address`, all its fields, `extract_addresses`) MUST have a `///` doc comment.
- `Address` derives `Debug, Clone, Default, PartialEq, Eq` + serde via `#[cfg_attr(feature = "serde", derive(...))]`.
- Never mix address components across blocks: each `Address` is built from a single block.

---

## File Structure
- Modify `src/lib.rs` — add `Address` struct (near `Person`), `extract_addresses` (near `extract_address`), private `addresses_from_document`. Reuse existing `parse_postcode_city`/`parse_street`.
- Create `tests/multi_address.rs`.
- Modify `README.md`, `CHANGELOG.md`.

---

## Task 1: `Address` + `extract_addresses`

**Files:**
- Modify: `src/lib.rs`
- Test: `tests/multi_address.rs` (create)

**Interfaces:**
- Produces: `pub struct Address { pub postcode: Option<String>, pub city: Option<String>, pub street: Option<String> }`; `pub fn extract_addresses(text: &str) -> Vec<Address>`.
- Consumes: existing `segment::Document`, `normalize::normalize_text`, `parse_postcode_city`, `parse_street`.

- [ ] **Step 1: Write the failing tests**

Create `tests/multi_address.rs`:

```rust
use german_impressum_extractor::{extract_address, extract_addresses, Address};

#[test]
fn returns_one_address_per_block_in_order() {
    let text = "\
Standort Nord
Alpenstraße 1
80331 München

Standort Süd
Seeweg 7
79098 Freiburg";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 2, "got {addrs:?}");
    assert_eq!(addrs[0], Address {
        postcode: Some("80331".into()),
        city: Some("München".into()),
        street: Some("Alpenstraße 1".into()),
    });
    assert_eq!(addrs[1], Address {
        postcode: Some("79098".into()),
        city: Some("Freiburg".into()),
        street: Some("Seeweg 7".into()),
    });
}

#[test]
fn single_address_matches_extract_address() {
    let text = "Hauptstraße 12, 10115 Berlin";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 1);
    let (pc, city, street) = extract_address(text);
    assert_eq!(addrs[0].postcode, pc);
    assert_eq!(addrs[0].city, city);
    assert_eq!(addrs[0].street, street);
}

#[test]
fn partial_block_yields_partial_address() {
    // Only a street, no postcode/city.
    let addrs = extract_addresses("Nur Musterweg 5");
    assert_eq!(addrs, vec![Address {
        postcode: None,
        city: None,
        street: Some("Musterweg 5".into()),
    }]);
}

#[test]
fn identical_address_blocks_are_deduped() {
    let text = "Hauptstraße 1\n10115 Berlin\n\nHauptstraße 1\n10115 Berlin";
    let addrs = extract_addresses(text);
    assert_eq!(addrs.len(), 1, "duplicates not deduped: {addrs:?}");
}

#[test]
fn no_address_yields_empty() {
    assert!(extract_addresses("Kein Adressinhalt hier.").is_empty());
}
```

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --test multi_address 2>&1 | head -15`
Expected: FAIL — `Address` / `extract_addresses` not found.

- [ ] **Step 3: Add the `Address` struct**

In `src/lib.rs`, after the `pub struct Person { ... }` definition, add:

```rust
/// A single postal address (one per address-bearing text block).
///
/// Each field is independent: a block containing only a street yields an
/// `Address` with `street: Some(..)` and the rest `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Address {
    /// German postcode (5 digits), if present in the block.
    pub postcode: Option<String>,
    /// City following the postcode, if present.
    pub city: Option<String>,
    /// Street + house number, if present in the block.
    pub street: Option<String>,
}
```

- [ ] **Step 4: Add `extract_addresses` + `addresses_from_document`**

In `src/lib.rs`, near `extract_address`, add:

```rust
/// Extract every postal address on the page — one [`Address`] per text block
/// that contains a postcode/city and/or a street, in document order, with exact
/// duplicates removed.
///
/// Unlike [`extract_address`] (which returns only the first address), this is
/// intended for pages listing multiple locations/branches. Address components
/// are only ever combined within a single block, so parts from different
/// entities are never mixed.
pub fn extract_addresses(text: &str) -> Vec<Address> {
    let doc = segment::Document::parse(normalize::normalize_text(text));
    addresses_from_document(&doc)
}

fn addresses_from_document(doc: &segment::Document) -> Vec<Address> {
    let mut out: Vec<Address> = Vec::new();
    for block in doc.block_texts() {
        let pc = parse_postcode_city(block);
        let street = parse_street(block);
        if pc.is_none() && street.is_none() {
            continue;
        }
        let (postcode, city) = match pc {
            Some((code, city)) => (Some(code), Some(city)),
            None => (None, None),
        };
        let addr = Address { postcode, city, street };
        if !out.contains(&addr) {
            out.push(addr);
        }
    }
    out
}
```

- [ ] **Step 5: Re-export `Address`**

In `src/lib.rs`, `Address` is defined at crate root so it is already public as `german_impressum_extractor::Address`. If the crate uses an explicit re-export list (e.g. a `pub use` block), add `Address` there; otherwise no action. Verify `use german_impressum_extractor::Address;` resolves (the test uses it).

- [ ] **Step 6: Run tests**

Run: `cargo test --test multi_address 2>&1 | tail -12` → 5 tests pass.
Run: `cargo test --all-targets 2>&1 | tail -6` → all existing tests still pass.
Run: `cargo test --all-targets --features html 2>&1 | tail -4` → pass.

- [ ] **Step 7: serde round-trip check (add to tests/multi_address.rs)**

```rust
#[cfg(feature = "serde")]
#[test]
fn addresses_serde_roundtrip() {
    let a = extract_addresses("Hauptstraße 12, 10115 Berlin");
    let json = serde_json::to_string(&a).unwrap();
    let back: Vec<Address> = serde_json::from_str(&json).unwrap();
    assert_eq!(a, back);
}
```

Run: `cargo test --test multi_address --features serde 2>&1 | tail -6` → passes.

- [ ] **Step 8: fmt + clippy**

Run: `cargo fmt --all && cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 9: Commit**

```bash
git add src/lib.rs tests/multi_address.rs
git commit -m "feat(address): add extract_addresses + Address for multi-location pages"
```

---

## Task 2: Docs + CHANGELOG

**Files:**
- Modify: `README.md`, `CHANGELOG.md`

- [ ] **Step 1: README**

Read `README.md`. In the granular-extractors area, add:

```markdown
### Multiple addresses

`extract_address` returns the first address; for pages listing several
locations use `extract_addresses`, which returns one `Address` per address
block (components are never mixed across blocks):

```rust
use german_impressum_extractor::extract_addresses;

for a in extract_addresses(impressum_text) {
    println!("{:?} {:?} {:?}", a.street, a.postcode, a.city);
}
```
```

If the "Robustness & limits" section has a bullet saying only the first address is returned, update it to point to `extract_addresses`.

- [ ] **Step 2: CHANGELOG**

Under `## [Unreleased]` → `### Added`, add:

```markdown
- `extract_addresses` + `Address`: return every postal address on the page
  (one per block, document order, deduped) for multi-location Impressum pages.
  `extract_address` (first address) and `extract_all` are unchanged.
```

- [ ] **Step 3: Verify**

Run: `cargo build --all-features 2>&1 | tail -2` → builds.
Run: `cargo clippy --all-targets --all-features -- -D warnings 2>&1 | tail -3` → clean.

- [ ] **Step 4: Commit**

```bash
git add README.md CHANGELOG.md
git commit -m "docs: document extract_addresses multi-address API"
```

---

## Self-Review

**1. Spec coverage:** `Address` + `extract_addresses` + `addresses_from_document` (per-block, deduped, doc order, no cross-block mixing) → Task 1; docs → Task 2. `extract_address`/`extract_all`/`Extracted` untouched. ✓
**2. Placeholder scan:** complete code in every step. ✓
**3. Type consistency:** `Address { postcode, city, street }`, `extract_addresses(&str)->Vec<Address>`, `addresses_from_document(&Document)->Vec<Address>`, reuses `parse_postcode_city`/`parse_street`. ✓

**Executor notes:**
- Do NOT modify `extract_address`, `address_from_document`, `extract_all`, or `Extracted`.
- If any existing test changes output, stop and report (should be impossible — this only adds new items).
- Keep `#[allow(dead_code)]` on `mod segment;`/`mod candidate;`.
