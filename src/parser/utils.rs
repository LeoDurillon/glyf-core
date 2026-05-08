//! Low-level utility functions used throughout the parser.
//! These functions have no dependencies on the parser's own types.
use std::sync::LazyLock;

use regex::Regex;

static MULTIPLIER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)").unwrap());

/// Returns the byte index of the first occurrence of `target` at nesting depth 0,
/// skipping characters inside `()` or `{}`.
///
/// This is the core of the depth-aware operator parsing that prevents `>` or `+`
/// inside attribute values like `:onClick={a>b}` from being mistaken for operators.
///
/// # Examples
/// ```
/// use glyf_core::parser::find_at_depth_zero;
/// assert_eq!(find_at_depth_zero("div>p",         '>'), Some(3));
/// assert_eq!(find_at_depth_zero("(div>p)+span",  '+'), Some(7)); // skips inner '>'
/// assert_eq!(find_at_depth_zero("a:fn={x>y}+b", '+'), Some(10)); // skips '>' in {}
/// ```
pub(super) fn find_at_depth_zero(input: &str, target: char) -> Option<usize> {
    let mut depth = 0usize;
    for (i, c) in input.char_indices() {
        match c {
            c if c == target && depth == 0 => return Some(i),
            '(' | '{' => depth += 1,
            ')' | '}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    None
}

/// Extracts the `*N` repetition count from an element string.
///
/// Returns `None` when no `*` is present at depth 0, or when `*` is not
/// followed by a digit. The `N` value is everything up to the next
/// non-digit character, so `li*3+span` returns `Some(3)`.
///
/// # Examples
/// ```
/// use glyf_core::parser::get_multiplier;
/// assert_eq!(get_multiplier("div*3"),     Some(3));
/// assert_eq!(get_multiplier("li*12"),     Some(12));
/// assert_eq!(get_multiplier("*3+span"),   Some(3)); // star-prefixed (after a group)
/// assert_eq!(get_multiplier("div"),       None);    // no multiplier
/// assert_eq!(get_multiplier("div*"),      None);    // star with no digits
/// assert_eq!(get_multiplier("a:x={v*2}"),None);    // star is inside {}, depth > 0
/// ```
pub(super) fn get_multiplier(element: &str) -> Option<usize> {
    match element.contains("*") {
        false => return None,
        true => {
            let Some(index) = find_at_depth_zero(element, '*') else {
                return None;
            };
            let slice = element.split_at(index + 1).1;
            let Some(capture) = MULTIPLIER_REGEX.captures(slice) else {
                return None;
            };

            let Some(multiplier) = capture.get(0) else {
                return None;
            };

            return Some(multiplier.as_str().parse::<usize>().unwrap_or(1));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // find_at_depth_zero
    // -------------------------------------------------------------------------
    mod find_at_depth_zero_tests {
        use super::*;

        #[test]
        fn finds_target_in_flat_string() {
            assert_eq!(find_at_depth_zero("div>p", '>'), Some(3));
        }

        #[test]
        fn returns_none_when_target_absent() {
            assert_eq!(find_at_depth_zero("div", '>'), None);
        }

        #[test]
        fn returns_none_for_empty_string() {
            assert_eq!(find_at_depth_zero("", '>'), None);
        }

        #[test]
        fn finds_target_at_position_zero() {
            assert_eq!(find_at_depth_zero(">div", '>'), Some(0));
        }

        #[test]
        fn skips_target_inside_parentheses() {
            // '>' at index 4 is inside (div>p) and must be skipped
            // '+' at index 7 is at depth 0 and must be returned
            assert_eq!(find_at_depth_zero("(div>p)+span", '+'), Some(7));
        }

        #[test]
        fn skips_target_inside_braces() {
            // '>' at index 14 is inside {val>2} and must be skipped
            // '+' at index 17 is at depth 0 and must be returned
            assert_eq!(find_at_depth_zero("a:onClick={val>2}+b", '+'), Some(17));
        }

        #[test]
        fn handles_nested_groups() {
            // everything before index 10 is inside one or two '('
            assert_eq!(find_at_depth_zero("(p>(span))+div", '+'), Some(10));
        }

        #[test]
        fn returns_first_occurrence_at_depth_zero() {
            assert_eq!(find_at_depth_zero("div+p+span", '+'), Some(3));
        }

        #[test]
        fn does_not_find_target_only_inside_parens() {
            assert_eq!(find_at_depth_zero("(div>p)", '>'), None);
        }
    }

    // -------------------------------------------------------------------------
    // get_multiplier
    // -------------------------------------------------------------------------
    mod get_multiplier_tests {
        use super::*;

        #[test]
        fn returns_none_without_star() {
            assert_eq!(get_multiplier("div"), None);
        }

        #[test]
        fn extracts_multiplier_from_element_with_prefix() {
            assert_eq!(get_multiplier("div*3"), Some(3));
        }

        #[test]
        fn extracts_multiplier_from_star_only_prefix() {
            assert_eq!(get_multiplier("*3"), Some(3));
        }

        #[test]
        fn extracts_multi_digit_multiplier() {
            assert_eq!(get_multiplier("li*12"), Some(12));
        }

        #[test]
        fn extracts_from_star_prefix_with_suffix() {
            // parse_group calls get_multiplier("*3+span") on the text after ')'
            assert_eq!(get_multiplier("*3+span"), Some(3));
        }

        #[test]
        fn returns_none_when_star_not_followed_by_digit() {
            assert_eq!(get_multiplier("div*"), None);
        }

        #[test]
        fn returns_none_when_star_inside_braces() {
            // '*' is at depth 1 inside {}, find_at_depth_zero skips it
            assert_eq!(get_multiplier("div:x={val*3}"), None);
        }

        #[test]
        fn multiplier_stops_at_non_digit_char() {
            assert_eq!(get_multiplier("div*3+p"), Some(3));
        }
    }
}
