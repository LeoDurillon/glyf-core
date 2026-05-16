//! Low-level utility functions used throughout the parser.
//! These functions have no dependencies on the parser's own types.
use std::sync::LazyLock;

use regex::Regex;

static MULTIPLIER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(\d+)").unwrap());

/// Returns the byte index of the first occurrence of `target` at nesting depth 0,
/// skipping characters inside `()` or `{}`.
///
/// When `target` is `>`, consecutive `>>` sequences are also skipped — `>>` is
/// the text content operator and must not be confused with the child operator `>`.
///
/// This is the core of the depth-aware operator parsing that prevents `>` or `+`
/// inside attribute values like `:onClick={a>b}` from being mistaken for operators.
pub(super) fn find_at_depth_zero(input: &str, target: char) -> Option<usize> {
    let mut depth = 0usize;
    let bytes = input.as_bytes();

    for (i, c) in input.char_indices() {
        match c {
            c if c == target && depth == 0 => {
                if target == '>'
                    && (bytes.get(i.saturating_add(1)) == Some(&b'>')
                        || i > 0 && bytes.get(i - 1) == Some(&b'>'))
                {
                    continue;
                }
                return Some(i);
            }
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
pub(super) fn get_multiplier(element: &str) -> Option<usize> {
    let index = find_at_depth_zero(element, '*')?;
    let slice = element.split_at(index + 1).1;
    let capture = MULTIPLIER_REGEX.captures(slice)?;

    let multiplier = capture.get(0)?;

    Some(multiplier.as_str().parse::<usize>().unwrap_or(1))
}

/// Check string for node operator (> or +)
/// For ">" check that it is not preceed or followed by ">"
///
/// Returns `true` at first found match to avoid unnecessary lookup
/// Returns `false` if it does not found any match in string
pub(super) fn has_node_operator(element: &str) -> bool {
    let bytes = element.as_bytes();
    for (i, &b) in bytes.iter().enumerate() {
        match b {
            b'>' => {
                let prev_gt = i > 0 && bytes[i - 1] == b'>';
                let next_gt = bytes.get(i + 1) == Some(&b'>');
                if !prev_gt && !next_gt {
                    return true; // single >, not part of >>
                }
            }
            b'+' => return true,
            _ => {}
        }
    }
    false
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

        #[test]
        fn should_skip_inner_text_call() {
            assert_eq!(find_at_depth_zero("div>>Hello", '>'), None);
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

    mod has_node_operator_tests {
        use super::*;

        #[test]
        fn detects_child_operator() {
            assert!(has_node_operator("div>p"));
        }

        #[test]
        fn detects_sibling_operator() {
            assert!(has_node_operator("h1+p"));
        }

        #[test]
        fn skips_text_content_operator() {
            assert!(!has_node_operator("div>>Hello"));
        }

        #[test]
        fn detects_single_gt_before_text_operator() {
            assert!(has_node_operator("div>p>>text"));
        }

        #[test]
        fn group_child_is_detected() {
            assert!(has_node_operator("(div)>p"));
        }

        #[test]
        fn plain_identifier_is_false() {
            assert!(!has_node_operator("a:href"));
        }
    }
}
