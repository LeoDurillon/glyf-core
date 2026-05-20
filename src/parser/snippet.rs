//! Snippet lookup and expansion.
//!
//! Snippets map short aliases to fuller Glyf strings. The expansion
//! happens *before* attribute parsing, so `"img"` becomes `"img:src:alt"`
//! which is then split into identifier `img` and attributes `:src :alt`.

use std::collections::HashMap;

const SNIPPET_BOUNDARIES: &[char] = &[':', '.', '#', '>', '+', '*', '/'];

/// Expands a snippet alias to its fuller Glyf form.
///
/// Looks up the longest key in the provided `snippets` map that is a prefix
/// of `value` and is either an exact match or immediately followed by a boundary
/// character.
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
pub(super) fn parse_snippet(value: &str, snippets: &HashMap<String, String>) -> String {
    let longest_match = snippets
        .iter()
        .filter(|(key, _)| {
            value.starts_with(key.as_str()) && {
                let rest = &value[key.len()..];
                rest.is_empty() || rest.starts_with(SNIPPET_BOUNDARIES)
            }
        })
        .max_by_key(|(key, _)| key.len());

    match longest_match {
        Some((key, expanded)) => {
            let mut cleaned_expand = expanded.as_str();
            let mut cleaned_value = &value[key.len()..];
            let mut is_self_closing = false;
            if expanded.ends_with("/") {
                cleaned_expand = expanded.strip_suffix('/').unwrap_or(expanded);
                cleaned_value = cleaned_value.strip_suffix('/').unwrap_or(cleaned_value);
                is_self_closing = true;
            }
            format!(
                "{}{}{}",
                cleaned_expand,
                cleaned_value,
                if is_self_closing { "/" } else { "" }
            )
        }
        None => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn s(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // -------------------------------------------------------------------------
    // parse_snippet
    // -------------------------------------------------------------------------
    mod parse_snippet_tests {
        use super::*;

        #[test]
        fn unknown_tag_is_returned_unchanged() {
            let snips = s(&[]);
            assert_eq!(parse_snippet("div", &snips), "div");
            assert_eq!(parse_snippet("section", &snips), "section");
        }

        #[test]
        fn expands_anchor_shorthand() {
            let snips = s(&[("a", "a:href")]);
            assert_eq!(parse_snippet("a", &snips), "a:href");
        }

        #[test]
        fn expands_self_closing_tag() {
            let snips = s(&[("br", "br/"), ("hr", "hr/")]);
            assert_eq!(parse_snippet("br", &snips), "br/");
            assert_eq!(parse_snippet("hr", &snips), "hr/");
        }

        #[test]
        fn expands_img_shorthand() {
            let snips = s(&[("img", "img:src:alt")]);
            assert_eq!(parse_snippet("img", &snips), "img:src:alt");
        }

        #[test]
        fn expands_btn_alias() {
            let snips = s(&[("btn", "button")]);
            assert_eq!(parse_snippet("btn", &snips), "button");
        }

        #[test]
        fn expands_bq_alias() {
            let snips = s(&[("bq", "blockquote")]);
            assert_eq!(parse_snippet("bq", &snips), "blockquote");
        }

        #[test]
        fn longer_key_wins_over_shorter_prefix() {
            // both "a" and "a:blank" match — "a:blank" is longer
            let snips = s(&[
                ("a", "a:href"),
                (
                    "a:blank",
                    "a:href=${0}:target=_blank:rel=noopener noreferrer",
                ),
            ]);
            assert_eq!(
                parse_snippet("a:blank", &snips),
                "a:href=${0}:target=_blank:rel=noopener noreferrer"
            );
        }

        #[test]
        fn appends_extra_attributes_after_snippet_expansion() {
            // "a" expands to "a:href"; extra ":id=main" is appended
            let snips = s(&[("a", "a:href")]);
            assert_eq!(parse_snippet("a:id=main", &snips), "a:href:id=main");
        }

        #[test]
        fn no_expansion_when_rest_is_not_colon_prefixed() {
            // "input" snippet only matches when followed by nothing or ':'
            // "inputxyz" is NOT a valid snippet call (rest = "xyz", no ':')
            let snips = s(&[("input", "input/")]);
            assert_eq!(parse_snippet("inputxyz", &snips), "inputxyz");
        }

        #[test]
        fn user_typed_self_closing_matches_self_closing_snippet() {
            // input/ should not expand to input// via the "input → input/" snippet
            let snips = s(&[("input", "input/")]);
            assert_eq!(parse_snippet("input/", &snips), "input/");
        }
    }
}
