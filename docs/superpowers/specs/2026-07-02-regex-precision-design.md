# Follow-up: sharpen label regexes (authority / chamber)

**Status:** Approved design (2026-07-02) — autonomous follow-up (Spec→PR).
**Context:** The TP5 whole-branch review flagged that `CHAMBER_RE` (and, similarly, `SUPERVISORY_AUTHORITY_RE`) can over-capture prose: bare "Berufskammer"/"Aufsichtsbehörde" appearing mid-sentence (e.g. "…Mitglied der **Berufskammer** der Ärzte …") is captured as the value. These are label fields; the label almost always begins a line in a real Impressum.

## Change
Anchor both regexes to **line start** using `(?im)^\s*` (multiline + case-insensitive). A label only matches when it begins a line (after optional leading whitespace, which normalization already trims). This preserves every legitimate form — "Label: value", "Label value", and label-on-its-own-line with the value on the next line (`\s*` still spans the newline) — while rejecting mid-sentence occurrences.

- `SUPERVISORY_AUTHORITY_RE`: `(?im)^\s*Aufsichtsbeh(?:ö|oe)rde\s*[:\-]?\s*([^\n]{2,100})`
- `CHAMBER_RE`: `(?im)^\s*(?:zust(?:ä|ae)ndige\s+Kammer|Berufskammer)\s*[:\-]?\s*([^\n]{2,100})`

## Non-breaking for real labels
All existing tests pass unchanged (their labels start their lines). Only mid-sentence prose captures — which were noise — now return `None`. This is a precision improvement consistent with the crate's noise-reduction goal.

## Constraints
- MSRV 1.85; `#![forbid(unsafe_code)]`; no new dep; clippy `-D warnings` clean; no API/signature change.

## Testing
- Existing supervisory/chamber tests + the `extract_all` integration/scored tests pass unchanged.
- New adversarial negatives: a mid-sentence "…Mitglied der Berufskammer der Ärzte…" and "…unterliegt der Aufsichtsbehörde des Landes…" return `None`.

## Success criteria
1. Both regexes line-start-anchored; all existing tests pass unchanged.
2. Mid-sentence prose no longer captured (adversarial tests).
3. No new dep; Rust 1.85; clippy clean.
