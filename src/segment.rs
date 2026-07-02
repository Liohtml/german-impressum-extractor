//! Split the canonical normalized string into labeled blocks and segments.
//! A blank line separates blocks; a `\t` or a leading `Label:` marks a
//! label→value segment.

use std::ops::Range;
use std::sync::LazyLock;

use regex::Regex;

/// Kind of label a segment carries, if any. Broader than TP1 uses; later
/// sub-projects (scoring, precision) reuse it as a signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LabelKind {
    Phone,
    Fax,
    Email,
    Postal,
    Managers,
    VatId,
    TaxNumber,
    Register,
    Court,
    Bank,
    LegalName,
    Founded,
    Web,
    Other,
}

// Matches a leading "Label: value" shape. Group 1 = label, group 2 = value.
static LABEL_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*([\p{L}.\-/ ]{1,30}?)\s*:\s*(.*)$").unwrap());

pub(crate) struct Segment {
    pub(crate) span: Range<usize>,
    pub(crate) label: Option<LabelKind>,
    pub(crate) value_span: Range<usize>,
}

struct Block {
    span: Range<usize>,
}

pub(crate) struct Document {
    text: String,
    blocks: Vec<Block>,
    segments: Vec<Segment>,
}

impl Document {
    pub(crate) fn text(&self) -> &str {
        &self.text
    }

    pub(crate) fn block_texts(&self) -> impl Iterator<Item = &str> {
        self.blocks.iter().map(move |b| &self.text[b.span.clone()])
    }

    /// True if any segment carries the given label.
    pub(crate) fn has_label(&self, kind: LabelKind) -> bool {
        self.segments.iter().any(|s| s.label == Some(kind))
    }

    #[cfg(test)]
    fn first_segment(&self) -> &Segment {
        &self.segments[0]
    }

    /// Segment already-normalized text into blocks and labeled segments.
    pub(crate) fn parse(text: String) -> Document {
        let mut blocks = Vec::new();
        let mut segments = Vec::new();
        let mut block_start: Option<usize> = None;
        let mut block_end = 0usize;
        let mut pos = 0usize;

        for line in text.split_inclusive('\n') {
            let content_len = line.trim_end_matches('\n').len();
            let start = pos;
            let end = start + content_len;
            pos += line.len();

            let content = &text[start..end];
            if content.trim().is_empty() {
                // Blank line closes the current block.
                if let Some(bs) = block_start.take() {
                    blocks.push(Block {
                        span: bs..block_end,
                    });
                }
                continue;
            }
            if block_start.is_none() {
                block_start = Some(start);
            }
            block_end = end;
            segments.push(build_segment(&text, start..end));
        }
        if let Some(bs) = block_start.take() {
            blocks.push(Block {
                span: bs..block_end,
            });
        }

        Document {
            text,
            blocks,
            segments,
        }
    }
}

fn build_segment(text: &str, span: Range<usize>) -> Segment {
    let line = &text[span.clone()];
    // 1. Tab-separated label/value (from HTML dt/dd or table cells).
    if let Some(tab) = line.find('\t') {
        let label_text = line[..tab].trim();
        let value_rel_start = span.start + tab + 1;
        let value_span = value_rel_start..span.end;
        return Segment {
            label: Some(classify_label(label_text)),
            span,
            value_span,
        };
    }
    // 2. "Label: value" shape.
    if let Some(caps) = LABEL_RE.captures(line) {
        let label_text = caps.get(1).map(|m| m.as_str()).unwrap_or("");
        if let Some(value) = caps.get(2) {
            let value_span = (span.start + value.start())..(span.start + value.end());
            return Segment {
                label: Some(classify_label(label_text)),
                span,
                value_span,
            };
        }
    }
    // 3. Plain line.
    Segment {
        label: None,
        value_span: span.clone(),
        span,
    }
}

/// Map a label string to a `LabelKind` (case-insensitive substring match).
fn classify_label(label: &str) -> LabelKind {
    let l = label.to_lowercase();
    let has = |k: &str| l.contains(k);
    if has("telefax") || has("fax") {
        LabelKind::Fax
    } else if has("telefon") || has("tel.") || has("tel ") || has("phone") || l == "tel" {
        LabelKind::Phone
    } else if has("mail") {
        LabelKind::Email
    } else if has("geschäftsführ")
        || has("geschaeftsführ")
        || has("inhaber")
        || has("vorstand")
        || has("vertretungsberechtigt")
        || has("verantwortlich")
    {
        LabelKind::Managers
    } else if has("ust") || has("umsatzsteuer") || has("vat") {
        LabelKind::VatId
    } else if has("steuernummer") || has("steuer-nr") || has("st.-nr") || has("stnr") {
        LabelKind::TaxNumber
    } else if has("registergericht") || has("amtsgericht") || (has("gericht") && !has("register")) {
        LabelKind::Court
    } else if has("handelsregister") || has("registernummer") || l == "hrb" || l == "hra" {
        LabelKind::Register
    } else if has("iban") || has("bic") || has("bank") {
        LabelKind::Bank
    } else if has("sitz") || has("adresse") || has("anschrift") {
        LabelKind::Postal
    } else if has("gegründet") || has("gegruendet") || has("seit") || has("gründungsjahr") {
        LabelKind::Founded
    } else if has("web") || has("internet") || has("homepage") {
        LabelKind::Web
    } else {
        LabelKind::Other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_blocks_on_blank_line() {
        let doc = Document::parse("A GmbH\nHauptstr. 1\n\nDatenschutz\nText".to_string());
        let blocks: Vec<&str> = doc.block_texts().collect();
        assert_eq!(blocks, vec!["A GmbH\nHauptstr. 1", "Datenschutz\nText"]);
    }

    #[test]
    fn detects_tab_label_value() {
        let doc = Document::parse("Telefon\t030 123".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, Some(LabelKind::Phone));
        assert_eq!(&doc.text()[seg.value_span.clone()], "030 123");
    }

    #[test]
    fn detects_colon_label_value() {
        let doc = Document::parse("USt-IdNr.: DE123456789".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, Some(LabelKind::VatId));
        assert_eq!(&doc.text()[seg.value_span.clone()], "DE123456789");
    }

    #[test]
    fn plain_line_has_no_label_and_full_value_span() {
        let doc = Document::parse("Musterreinigung GmbH".to_string());
        let seg = doc.first_segment();
        assert_eq!(seg.label, None);
        assert_eq!(&doc.text()[seg.value_span.clone()], "Musterreinigung GmbH");
    }

    #[test]
    fn has_label_detects_present_labels() {
        let doc = Document::parse("Telefon: 030 123\nMuster GmbH".to_string());
        assert!(doc.has_label(LabelKind::Phone));
        assert!(!doc.has_label(LabelKind::Bank));
    }
}
