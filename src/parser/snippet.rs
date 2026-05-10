//! Snippet lookup and expansion.
//!
//! Snippets map short aliases to fuller Glyf strings. The expansion
//! happens *before* attribute parsing, so `"img"` becomes `"img:src:alt"`
//! which is then split into identifier `img` and attributes `:src :alt`.

use crate::config::Config;

const SNIPPET_BOUNDARIES: &[char] = &[':', '.', '#', '>', '+', '*', '<', '/'];

/// Expands a snippet alias to its fuller Glyf form.
///
/// Looks up the longest key in [`Snippet::get`] that is a prefix of `value`
/// and is either an exact match or immediately followed by `:`.
/// If no key matches, `value` is returned unchanged.
///
/// The returned string is still an Glyf string, not HTML — it is fed back
/// into the parser for identifier and attribute extraction.
///
/// # Examples
/// ```
/// // "br" → "br/"  (self-closing flag preserved)
/// // "a"  → "a:href" (attribute shorthand appended)
/// // "a:blank" wins over "a" because it is the longer matching prefix
/// // "div" → "div"  (no snippet, returned as-is)
/// // "a:id=main" → "a:href:id=main" (extra attrs appended after expansion)
/// ```
pub(super) fn parse_snippet(value: &str) -> String {
    let mut transformed_value = String::from(value);
    let config = &Config::get();
    let snippets = &config.snippets;

    let matching_key = snippets
        .keys()
        .filter(|key| {
            value.starts_with(*key) && {
                let rest = &value[key.len()..];
                rest.is_empty() || rest.starts_with(SNIPPET_BOUNDARIES)
            }
        })
        .max_by_key(|key| key.len());

    if let Some(key) = matching_key
        && let Some(expanded) = snippets.get(key)
    {
        transformed_value = format!("{}{}", expanded, value.split_at(key.len()).1);
    }

    transformed_value
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // parse_snippet
    // -------------------------------------------------------------------------
    mod parse_snippet_tests {
        use std::collections::HashMap;

        use crate::config::ParserMode;

        use super::*;

        fn init_config() {
            Config::init(
                ParserMode::HTML,
                HashMap::from([
                    ("a".to_string(), "a:href".to_string()),
                    ("br".to_string(), "br/".to_string()),
                    ("hr".to_string(), "hr/".to_string()),
                    ("img".to_string(), "img:src:alt".to_string()),
                    ("btn".to_string(), "button".to_string()),
                    ("bq".to_string(), "blockquote".to_string()),
                    (
                        "a:blank".to_string(),
                        "a:href=${0}:target=_blank:rel=noopener noreferrer".to_string(),
                    ),
                    ("input".to_string(), "input/".to_string()),
                ]),
            );
        }
        #[test]
        fn unknown_tag_is_returned_unchanged() {
            init_config();
            assert_eq!(parse_snippet("div"), "div");
            assert_eq!(parse_snippet("section"), "section");
        }

        #[test]
        fn expands_anchor_shorthand() {
            init_config();
            assert_eq!(parse_snippet("a"), "a:href");
        }

        #[test]
        fn expands_self_closing_tag() {
            init_config();
            assert_eq!(parse_snippet("br"), "br/");
            assert_eq!(parse_snippet("hr"), "hr/");
        }

        #[test]
        fn expands_img_shorthand() {
            init_config();
            assert_eq!(parse_snippet("img"), "img:src:alt");
        }

        #[test]
        fn expands_btn_alias() {
            init_config();
            assert_eq!(parse_snippet("btn"), "button");
        }

        #[test]
        fn expands_bq_alias() {
            init_config();
            assert_eq!(parse_snippet("bq"), "blockquote");
        }

        #[test]
        fn longer_key_wins_over_shorter_prefix() {
            init_config();
            // both "a" and "a:blank" match — "a:blank" is longer
            assert_eq!(
                parse_snippet("a:blank"),
                "a:href=${0}:target=_blank:rel=noopener noreferrer"
            );
        }

        #[test]
        fn appends_extra_attributes_after_snippet_expansion() {
            init_config();
            // "a" expands to "a:href"; extra ":id=main" is appended
            assert_eq!(parse_snippet("a:id=main"), "a:href:id=main");
        }

        #[test]
        fn no_expansion_when_rest_is_not_colon_prefixed() {
            init_config();
            // "input" snippet only matches when followed by nothing or ':'
            // "inputxyz" is NOT a valid snippet call (rest = "xyz", no ':')
            assert_eq!(parse_snippet("inputxyz"), "inputxyz");
        }
    }
}
