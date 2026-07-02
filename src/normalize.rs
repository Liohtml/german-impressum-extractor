//! Turn raw text (or, via `html.rs`, flattened HTML) into one canonical string:
//! `\n` separates lines/blocks, `\t` separates a label from its value, and
//! invisible/duplicate whitespace and well-formed HTML entities are cleaned up.

use std::sync::LazyLock;

use regex::Regex;
use unicode_normalization::UnicodeNormalization as _;

// Collapses 3+ consecutive newlines down to exactly two (one blank line = one
// block boundary).
static BLANK_LINES_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\n{3,}").unwrap());

/// Normalize raw text into the canonical intermediate form.
pub(crate) fn normalize_text(input: &str) -> String {
    // 1. Unicode NFC once.
    let nfc: String = input.nfc().collect();
    // 2. Unify line endings before per-char work.
    let unified = nfc.replace("\r\n", "\n").replace('\r', "\n");
    // 3. Decode well-formed HTML entities.
    let decoded = decode_entities(&unified);
    // 4. Char-level cleanup: drop invisibles, fold whitespace (keep \n and \t).
    let mut cleaned = String::with_capacity(decoded.len());
    for ch in decoded.chars() {
        match ch {
            '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' | '\u{00AD}' | '\u{2060}' => {}
            '\n' => cleaned.push('\n'),
            '\t' => cleaned.push('\t'),
            c if c.is_whitespace() => cleaned.push(' '),
            c => cleaned.push(c),
        }
    }
    // 5. Per-line: collapse runs of spaces and runs of tabs, trim line ends.
    let mut lines: Vec<String> = Vec::new();
    for line in cleaned.split('\n') {
        lines.push(collapse_line_ws(line));
    }
    let joined = lines.join("\n");
    // 6. Collapse blank-line runs to a single blank line.
    let collapsed = BLANK_LINES_RE.replace_all(&joined, "\n\n").into_owned();
    // 7. Trim leading/trailing blank lines and whitespace so callers (e.g. the
    // HTML flattener, which opens with a block-start `\n`) get a canonical
    // string with no stray edge newlines.
    collapsed
        .trim_matches(|c: char| c == '\n' || c == ' ' || c == '\t')
        .to_string()
}

/// Collapse consecutive spaces (and consecutive tabs) to one, and trim leading
/// and trailing spaces/tabs. Interior single tabs (label/value separators) and
/// other content are preserved.
fn collapse_line_ws(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut prev_space = false;
    let mut prev_tab = false;
    for ch in line.chars() {
        match ch {
            ' ' => {
                if !prev_space {
                    out.push(' ');
                }
                prev_space = true;
                prev_tab = false;
            }
            '\t' => {
                if !prev_tab {
                    out.push('\t');
                }
                prev_tab = true;
                prev_space = false;
            }
            c => {
                out.push(c);
                prev_space = false;
                prev_tab = false;
            }
        }
    }
    out.trim_matches(|c| c == ' ' || c == '\t').to_string()
}

/// Decode only well-formed HTML entities: named (from a small table), decimal
/// `&#NNN;`, and hex `&#xHH;`. Anything not recognized is emitted verbatim.
///
/// This is a SINGLE decoding pass: each well-formed entity is decoded exactly
/// once, per HTML semantics. Doubly-escaped input like `&amp;#64;` therefore
/// decodes only to `&#64;` (the literal escaped ampersand followed by
/// `#64;`), not all the way to `@`. Iterating to a fixed point would be
/// incorrect and could over-decode legitimate text (e.g. `&amp;copy;` must
/// stay `&copy;`, not become `©`).
fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        // Find a terminating ';' within a short window.
        let end = (i + 1..chars.len().min(i + 12)).find(|&j| chars[j] == ';');
        match end {
            Some(j) => {
                let entity: String = chars[i + 1..j].iter().collect();
                if let Some(decoded) = decode_one_entity(&entity) {
                    out.push_str(&decoded);
                    i = j + 1;
                } else {
                    out.push('&');
                    i += 1;
                }
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

/// Decode the inner text of one entity (without `&`/`;`). Returns None if unknown.
fn decode_one_entity(body: &str) -> Option<String> {
    if let Some(num) = body.strip_prefix('#') {
        let code = if let Some(hex) = num.strip_prefix(['x', 'X']) {
            u32::from_str_radix(hex, 16).ok()?
        } else {
            num.parse::<u32>().ok()?
        };
        return char::from_u32(code).map(|c| c.to_string());
    }
    let c = match body {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{00A0}',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "eacute" => 'é',
        "egrave" => 'è',
        "agrave" => 'à',
        "ccedil" => 'ç',
        "euro" => '€',
        "copy" => '©',
        "reg" => '®',
        "ndash" => '–',
        "mdash" => '—',
        _ => return None,
    };
    Some(c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_zero_width_and_soft_hyphen() {
        assert_eq!(normalize_text("Mül\u{00AD}ler\u{200B}"), "Müller");
    }

    #[test]
    fn normalizes_nbsp_and_crlf_and_collapses_spaces() {
        assert_eq!(
            normalize_text("Hans\u{00A0}Müller\r\nSitz   Berlin"),
            "Hans Müller\nSitz Berlin"
        );
    }

    #[test]
    fn decodes_well_formed_entities_only() {
        assert_eq!(
            normalize_text("M\u{00FC}ller &amp; S\u{00F6}hne"),
            "Müller & Söhne"
        );
        assert_eq!(
            normalize_text("&uuml;ber &#252;ber &#xFC;ber"),
            "über über über"
        );
        // Malformed / no semicolon: left untouched.
        assert_eq!(normalize_text("R&D Abteilung"), "R&D Abteilung");
    }

    #[test]
    fn decodes_entities_exactly_once() {
        // A doubly-escaped ampersand must decode to the literal escaped
        // text `&#64;`, NOT be over-decoded to `@`.
        assert_eq!(decode_entities("&amp;#64;"), "&#64;");
        assert_eq!(decode_entities("&amp;"), "&");
        assert_eq!(decode_entities("&#64;"), "@");
        assert_eq!(
            decode_entities("M&uuml;ller &amp; S&ouml;hne"),
            "Müller & Söhne"
        );
    }

    #[test]
    fn preserves_tabs_and_collapses_blank_lines() {
        assert_eq!(
            normalize_text("Telefon\t030\n\n\n\nSitz"),
            "Telefon\t030\n\nSitz"
        );
    }

    #[test]
    fn never_panics_on_multibyte_garbage() {
        for s in [
            "\u{200B}\u{00AD}",
            "€\t\r\n",
            "ä".repeat(50).as_str(),
            "&#;&#x;&amp",
        ] {
            let _ = normalize_text(s);
        }
    }
}
