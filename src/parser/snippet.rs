//! Snippet lookup and expansion.
//!
//! Snippets map short aliases to fuller Glyf strings. The expansion
//! happens *before* attribute parsing, so `"img"` becomes `"img:src:alt"`
//! which is then split into identifier `img` and attributes `:src :alt`.

use super::consts::BASE_SNIPPET;
use std::{collections::HashMap, sync::OnceLock};

static SNIPPETS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
const SNIPPET_BOUNDARIES: &[char] = &[':', '.', '#', '>', '+', '*', '<', '/'];

/// Provides access to the built-in snippet table.
///
/// The table is built once on first call and reused for the lifetime of
/// the process (`OnceLock`), so repeated calls are allocation-free.
pub struct Snippet {}

impl Snippet {
    /// Returns a reference to the global built-in snippet map.
    ///
    /// Keys are Glyf aliases; values are their expanded Glyf strings.
    /// See [`super::consts::BASE_SNIPPET`] for the full list.
    pub fn get() -> &'static HashMap<&'static str, &'static str> {
        SNIPPETS.get_or_init(|| HashMap::from(BASE_SNIPPET))
    }
}

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
pub(super) fn parse_snippet(value: &str, custom: Option<&HashMap<String, String>>) -> String {
    let mut transformed_value = String::from(value);
    let built_ins = Snippet::get();
    let snippets_keys = built_ins
        .keys()
        .map(|k| *k)
        .chain(custom.iter().flat_map(|m| m.keys().map(String::as_str)));

    let matching_key = snippets_keys
        .filter(|key| {
            value.starts_with(*key) && {
                let rest = &value[key.len()..];
                rest.is_empty() || rest.starts_with(SNIPPET_BOUNDARIES)
            }
        })
        .max_by_key(|key| key.len());

    if let Some(key) = matching_key {
        let matcher = match (custom, built_ins.get(key)) {
            (None, built_in) => built_in,
            (Some(custom_snippet), built_in) => {
                if let Some(value) = custom_snippet.get(&key.to_string()) {
                    Some(&value.as_str())
                } else {
                    built_in
                }
            }
        };
        if let Some(expanded) = matcher {
            transformed_value = format!("{}{}", expanded, value.split_at(key.len()).1);
        }
    }

    return transformed_value;
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // parse_snippet
    // -------------------------------------------------------------------------
    mod parse_snippet_tests {
        use super::*;

        #[test]
        fn unknown_tag_is_returned_unchanged() {
            assert_eq!(parse_snippet("div", None), "div");
            assert_eq!(parse_snippet("section", None), "section");
        }

        #[test]
        fn expands_anchor_shorthand() {
            assert_eq!(parse_snippet("a", None), "a:href");
        }

        #[test]
        fn expands_self_closing_tag() {
            assert_eq!(parse_snippet("br", None), "br/");
            assert_eq!(parse_snippet("hr", None), "hr/");
        }

        #[test]
        fn expands_img_shorthand() {
            assert_eq!(parse_snippet("img", None), "img:src:alt");
        }

        #[test]
        fn expands_btn_alias() {
            assert_eq!(parse_snippet("btn", None), "button");
        }

        #[test]
        fn expands_bq_alias() {
            assert_eq!(parse_snippet("bq", None), "blockquote");
        }

        #[test]
        fn longer_key_wins_over_shorter_prefix() {
            // both "a" and "a:blank" match — "a:blank" is longer
            assert_eq!(
                parse_snippet("a:blank", None),
                "a:href=${0}:target=_blank:rel=noopener noreferrer"
            );
        }

        #[test]
        fn appends_extra_attributes_after_snippet_expansion() {
            // "a" expands to "a:href"; extra ":id=main" is appended
            assert_eq!(parse_snippet("a:id=main", None), "a:href:id=main");
        }

        #[test]
        fn no_expansion_when_rest_is_not_colon_prefixed() {
            // "input" snippet only matches when followed by nothing or ':'
            // "inputxyz" is NOT a valid snippet call (rest = "xyz", no ':')
            assert_eq!(parse_snippet("inputxyz", None), "inputxyz");
        }
    }

    // -------------------------------------------------------------------------
    // parse_snippet — custom snippet map
    // -------------------------------------------------------------------------
    mod custom_snippet_tests {
        use super::*;
        use std::collections::HashMap;

        /// Convenience: build a `HashMap<String, String>` from literal pairs.
        fn snippets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
            pairs
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        }

        #[test]
        fn brand_new_alias_expands() {
            // "mc" is not a built-in key — it must be resolved via the custom map
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("mc", Some(&c)), "MyComponent");
        }

        #[test]
        fn alias_with_boundary_char_triggers_expansion() {
            // key "mc" matches "mc.foo" (rest ".foo" starts with '.') and
            // "mc:id=1" (rest ":id=1" starts with ':')
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("mc.foo", Some(&c)), "MyComponent.foo");
            assert_eq!(parse_snippet("mc:id=1", Some(&c)), "MyComponent:id=1");
        }

        #[test]
        fn alias_without_boundary_is_not_expanded() {
            // "mchero" — rest "hero" is not a boundary char, so no expansion
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("mchero", Some(&c)), "mchero");
        }

        #[test]
        fn custom_overrides_builtin_with_same_key() {
            // built-in "btn" → "button"; custom entry shadows it with "MyButton"
            let c = snippets(&[("btn", "MyButton")]);
            assert_eq!(parse_snippet("btn", Some(&c)), "MyButton");
        }

        #[test]
        fn builtin_used_when_key_absent_from_custom_map() {
            // custom map is provided but does not contain "btn" → built-in fires
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("btn", Some(&c)), "button");
        }

        #[test]
        fn builtin_still_works_alongside_custom_map() {
            // an unrelated custom entry must not prevent built-in "br" → "br/"
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("br", Some(&c)), "br/");
        }

        #[test]
        fn custom_longer_key_wins_over_builtin_shorter_key() {
            // input "a:extra":
            //   built-in "a"       matches (rest ":extra" starts with ':') → "a:href"
            //   custom   "a:extra" also matches (rest "" is empty)          → custom
            // max_by_key(len) picks "a:extra" (7 chars) over "a" (1 char)
            let c = snippets(&[("a:extra", "a:href:custom-attr")]);
            assert_eq!(parse_snippet("a:extra", Some(&c)), "a:href:custom-attr");
        }

        #[test]
        fn custom_rest_is_appended_after_expansion() {
            // key "mc" matches, the tail ":foo=bar" is appended verbatim
            let c = snippets(&[("mc", "MyComponent")]);
            assert_eq!(parse_snippet("mc:foo=bar", Some(&c)), "MyComponent:foo=bar");
        }

        #[test]
        fn custom_self_closing_expansion() {
            // an expansion ending with "/" signals a self-closing element
            let c = snippets(&[("myimg", "MyImage:src:alt/")]);
            assert_eq!(parse_snippet("myimg", Some(&c)), "MyImage:src:alt/");
        }
    }
}
