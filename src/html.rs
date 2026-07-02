//! HTML → canonical structured text, behind the `html` feature. Uses the
//! `html5gum` tokenizer (which also decodes character references). Block-level
//! elements and `<br>` become `\n`; `<dt>`/`<dd>` and table cells become
//! `label\tvalue`; `<script>`/`<style>`/`<head>`/`<noscript>` content is dropped.

use html5gum::{HtmlString, Token, Tokenizer};

use crate::normalize::normalize_text;

/// Flattens raw HTML into the canonical `\n`/`\t` string.
pub(crate) trait HtmlFlattener {
    fn flatten(&self, html: &str) -> String;
}

pub(crate) struct DefaultFlattener;

fn name_is(name: &HtmlString, want: &[u8]) -> bool {
    name.eq_ignore_ascii_case(want)
}

fn is_block(name: &HtmlString) -> bool {
    const BLOCK: &[&[u8]] = &[
        b"p",
        b"div",
        b"section",
        b"article",
        b"header",
        b"footer",
        b"main",
        b"aside",
        b"nav",
        b"ul",
        b"ol",
        b"li",
        b"table",
        b"tr",
        b"dl",
        b"blockquote",
        b"address",
        b"h1",
        b"h2",
        b"h3",
        b"h4",
        b"h5",
        b"h6",
        b"br",
    ];
    BLOCK.iter().any(|b| name_is(name, b))
}

fn is_raw(name: &HtmlString) -> bool {
    name_is(name, b"script")
        || name_is(name, b"style")
        || name_is(name, b"head")
        || name_is(name, b"noscript")
        || name_is(name, b"template")
}

impl HtmlFlattener for DefaultFlattener {
    fn flatten(&self, html: &str) -> String {
        let mut out = String::with_capacity(html.len());
        let mut raw_depth: u32 = 0;

        for token in Tokenizer::new(html).flatten() {
            match token {
                Token::StartTag(tag) => {
                    if is_raw(&tag.name) {
                        raw_depth += 1;
                        continue;
                    }
                    if is_block(&tag.name) {
                        out.push('\n');
                    }
                }
                Token::EndTag(tag) => {
                    if is_raw(&tag.name) {
                        raw_depth = raw_depth.saturating_sub(1);
                        continue;
                    }
                    // Label/value separators.
                    if name_is(&tag.name, b"dt")
                        || name_is(&tag.name, b"th")
                        || name_is(&tag.name, b"td")
                    {
                        out.push('\t');
                    } else if is_block(&tag.name) {
                        out.push('\n');
                    }
                }
                Token::String(s) if raw_depth == 0 => {
                    out.push_str(&String::from_utf8_lossy(&s));
                }
                _ => {}
            }
        }
        out
    }
}

/// Convert an HTML document into the canonical Impressum text (flatten + normalize).
pub fn html_to_impressum_text(html: &str) -> String {
    normalize_text(&DefaultFlattener.flatten(html))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(html: &str) -> String {
        normalize_text(&DefaultFlattener.flatten(html))
    }

    #[test]
    fn dl_becomes_label_tab_value() {
        assert_eq!(
            flat("<dl><dt>Telefon</dt><dd>030 123</dd></dl>"),
            "Telefon\t030 123"
        );
    }

    #[test]
    fn table_row_becomes_label_tab_value() {
        assert_eq!(
            flat("<table><tr><th>PLZ</th><td>10115</td></tr></table>"),
            "PLZ\t10115"
        );
    }

    #[test]
    fn script_and_style_content_dropped() {
        assert_eq!(
            flat("<p>Hallo</p><script>var x=1;</script><style>a{}</style>"),
            "Hallo"
        );
    }

    #[test]
    fn br_and_blocks_become_newlines_and_entities_decode() {
        assert_eq!(
            flat("Meyer&amp;Co<br>10115 Berlin"),
            "Meyer&Co\n10115 Berlin"
        );
    }

    #[test]
    fn broken_markup_does_not_panic() {
        let _ = flat("<div><span>unclosed <b> &notanentity <<< ");
    }
}
