//! Glyf abbreviation parser.
//!
//! Converts compact Glyf syntax into an [`Element`] AST which renders
//! to indented HTML/JSX via its [`std::fmt::Display`] implementation.
//!
//! # Syntax reference
//!
//! | Syntax | Meaning | Example | Output |
//! |--------|---------|---------|--------|
//! | `tag` | Element | `div` | `<div></div>` |
//! | `tag/` | Self-closing | `br/` | `<br />` |
//! | `a>b` | Child | `ul>li` | `<ul>\n\t<li></li>\n</ul>` |
//! | `a+b` | Sibling | `div+p` | `<div></div>\n<p></p>` |
//! | `(a+b)*3` | Group × N | `(li)*3` | three `<li>` elements |
//! | `tag*N` | Repeat | `li*3` | three `<li>` elements |
//! | `tag.cls` | Class | `div.foo` | `<div class="foo">` |
//! | `tag#id` | Id | `div#app` | `<div id="app">` |
//! | `tag:key=val` | Prop | `a:href=url` | `<a href="url">` |
//! | `tag:key={expr}` | JSX prop | `div:onClick={fn}` | `<div onClick={fn}>` |
//! | `tag>>text` | Text content | `p>>Hello` | `<p>Hello</p>` |
//! | `tag#{expr}` | JSX dynamic id | `div#{myId}` | `<div id={myId}>` |
//! | `.cls` / `#id` / `:prop` / `>child` | Implicit div | `.foo` | `<div class="foo">` |
//! | `e` | JSX fragment (JSX mode only) | `e>p` | `<>\n\t<p></p>\n</>` |
//!
//! # Quick start
//!
//! ```
//! use glyf_core::expand_to_tree;
//!
//! assert_eq!(
//!     expand_to_tree("ul>li.item*2", None, None).unwrap().to_string(),
//!     "<ul>\n\t<li class=\"item\"></li>\n\t<li class=\"item\"></li>\n</ul>"
//! );
//! ```

pub mod attribute;
mod error;
pub(super) mod html;
mod snippet;
mod types;
mod utils;
pub(super) mod validate;

pub use error::GlyfError;
pub use types::{Element, Node};
use utils::{find_at_depth_zero, get_multiplier};

use crate::config::Config;

const IMPLICIT_DIV_PREFIXES: [char; 4] = [':', '.', '#', '>'];

/// Parses a grouped Glyf expression `(...)` with an optional `*N` multiplier
/// and an optional sibling following the closing `)`.
///
/// This is called automatically by [`parse_input`] when the input starts with `(`;
/// you rarely need to call it directly.
///
/// # Errors
/// Propagates any [`GlyfError`] from parsing the inner content or the sibling.
/// ```
pub(super) fn parse_group(
    input: &str,
    level: Option<usize>,
    config: &Config,
) -> Result<Element, GlyfError> {
    let closing = find_at_depth_zero(&input[1..], ')').unwrap_or(input.len() - 1);
    let inner = &input[1..];
    let element = &inner[..closing];
    let mut rest = &inner[(closing + 1).min(input.len())..];
    let parsed_element = parse_input(element, level, config)?;

    let mut multiplier = 1;

    if !rest.is_empty() && rest.starts_with("*") {
        multiplier = get_multiplier(rest).unwrap_or(1);
        let multiplier_len = multiplier.ilog10() as usize + 3;
        rest = &rest[multiplier_len.min(rest.len())..]
    } else if !rest.is_empty() {
        rest = &rest[1..]
    }

    let mut sibling = None;

    if !rest.is_empty() {
        let scoped_sibling = parse_input(rest, level, config)?;
        sibling = Some(scoped_sibling);
    }

    Ok(Element::from_group(
        Box::new(parsed_element),
        multiplier,
        sibling.map(|sibling| Box::new(Node::Sibling(sibling))),
        level,
        config.mode.clone(),
    ))
}

/// Parses an Glyf abbreviation string into an [`Element`] tree.
///
/// `level` is the indentation depth of the root element in the output.
/// Pass `None` (or `Some(0)`) for top-level output; pass `Some(n)` when
/// the expansion will be embedded inside an already-indented context.
///
/// Inputs starting with `.`, `#`, `:`, or `>` trigger **implicit div**:
/// the abbreviation is treated as if it were prefixed with `div`.
///
/// # Errors
/// - [`GlyfError::NoIdentifier`] — input is empty or has no tag name.
/// ```
pub(super) fn parse_input(
    input: &str,
    level: Option<usize>,
    config: &Config,
) -> Result<Element, GlyfError> {
    if input.starts_with("(") {
        return parse_group(input, level, config);
    }

    let formatted = if input.starts_with(IMPLICIT_DIV_PREFIXES.as_slice()) {
        &format!("div{input}")
    } else {
        input
    };

    let first_down =
        formatted.split_at(find_at_depth_zero(formatted, '>').unwrap_or(formatted.len()));
    let first_sibling =
        formatted.split_at(find_at_depth_zero(formatted, '+').unwrap_or(formatted.len()));
    let element = if first_down.0.len() < first_sibling.0.len() {
        first_down.0
    } else {
        first_sibling.0
    };

    if element.is_empty() {
        return Err(GlyfError::NoIdentifier);
    }

    let multiplier = get_multiplier(element).unwrap_or(1);
    let element_value = match find_at_depth_zero(element, '*') {
        Some(pos) => &element[..pos],
        None => element,
    };

    if element.len() == formatted.len() {
        return Element::from_abbr(element_value, multiplier, None, level, config);
    }

    let next_element_level = if first_down.0.len() < first_sibling.0.len() {
        level.map_or(Some(1), |l| Some(l + 1))
    } else {
        level
    };

    let next_element = parse_input(&formatted[element.len() + 1..], next_element_level, config)?;

    let node_type = if first_down.0.len() < first_sibling.0.len() {
        Node::Children(next_element)
    } else {
        Node::Sibling(next_element)
    };

    Element::from_abbr(
        element_value,
        multiplier,
        Some(Box::new(node_type)),
        level,
        config,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use crate::config::{Config, ParserMode};

    use super::*;

    fn jsx_config() -> Config {
        Config::new(ParserMode::JSX(None), HashMap::new())
    }

    fn html_config(snippets_list: &[(&str, &str)]) -> Config {
        Config::new(ParserMode::HTML, snippets(snippets_list))
    }
    /// Convenience: build a `HashMap<String, String>` from literal pairs.
    fn snippets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // convenience: unwrap a Result and panic with the input on Err
    fn ok(r: Result<Element, GlyfError>) -> Element {
        r.expect("parse returned Err unexpectedly")
    }

    // -------------------------------------------------------------------------
    // parse_input
    // -------------------------------------------------------------------------
    #[cfg(test)]
    mod parse_input_tests {
        use crate::parser::attribute::AttributeType;

        use super::*;

        #[test]
        fn single_element() {
            let r = ok(parse_input("div", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            assert_eq!(r.multiplier, 1);
            assert!(r.node.is_none());
            assert!(r.group.is_none());
        }

        #[test]
        fn element_with_multiplier_strips_star_n() {
            let r = ok(parse_input("div*3", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div")); // NOT "div*3"
            assert_eq!(r.multiplier, 3);
            assert!(r.node.is_none());
        }

        #[test]
        fn element_with_child_operator() {
            let r = ok(parse_input("div>p", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let node = r.node.expect("should have a node");
            assert!(matches!(*node, Node::Children(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
            assert!(node.element().node.is_none());
        }

        #[test]
        fn element_with_sibling_operator() {
            let r = ok(parse_input("div+p", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let node = r.node.expect("should have a node");
            assert!(matches!(*node, Node::Sibling(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
        }

        #[test]
        fn child_beats_sibling_when_appearing_first() {
            // '>' at 3, '+' at 5 -> Children wins at the top level
            let r = ok(parse_input("div>p+span", None, &Config::default()));
            let node = r.node.expect("div should have a child node");
            assert!(matches!(*node, Node::Children(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
            let span = node
                .element()
                .node
                .as_ref()
                .expect("p should have sibling span");
            assert!(matches!(**span, Node::Sibling(_)));
            assert_eq!(span.element().identifier.as_deref(), Some("span"));
        }

        #[test]
        fn sibling_beats_child_when_appearing_first() {
            // '+' at 3, '>' at 5 -> Sibling wins at the top level
            let r = ok(parse_input("div+p>span", None, &Config::default()));
            let node = r.node.expect("div should have sibling node");
            assert!(matches!(*node, Node::Sibling(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
        }

        #[test]
        fn chained_children_build_nested_tree() {
            let r = ok(parse_input("ul>li>a", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("ul"));
            let li = r.node.expect("ul -> li");
            assert!(matches!(*li, Node::Children(_)));
            assert_eq!(li.element().identifier.as_deref(), Some("li"));
            let a = li.element().node.as_ref().expect("li -> a");
            assert!(matches!(**a, Node::Children(_)));
            assert_eq!(a.element().identifier.as_deref(), Some("a"));
        }

        #[test]
        fn multiplier_with_child() {
            let r = ok(parse_input("li*3>a", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("li"));
            assert_eq!(r.multiplier, 3);
            let node = r.node.expect("li -> a");
            assert!(matches!(*node, Node::Children(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("a"));
        }

        #[test]
        fn group_input_is_delegated_to_parse_group() {
            let r = ok(parse_input("(div)+span", None, &Config::default()));
            assert!(r.identifier.is_none());
            assert!(r.group.is_some());
        }

        // ── level propagation ─────────────────────────────────────────────

        #[test]
        fn top_level_element_gets_given_level() {
            let r = ok(parse_input("div", Some(2), &Config::default()));
            assert_eq!(r.level, Some(2));
        }

        #[test]
        fn child_gets_level_plus_one() {
            let r = ok(parse_input("div>p", Some(0), &Config::default()));
            assert_eq!(r.level, Some(0));
            let child = r.node.expect("div -> p");
            assert_eq!(child.element().level, Some(1));
        }

        #[test]
        fn sibling_keeps_same_level() {
            let r = ok(parse_input("div+p", Some(3), &Config::default()));
            assert_eq!(r.level, Some(3));
            let sibling = r.node.expect("div + p");
            assert_eq!(sibling.element().level, Some(3));
        }

        #[test]
        fn level_accumulates_through_nested_children() {
            // div(0) > ul(1) > li(2)
            let r = ok(parse_input("div>ul>li", Some(0), &Config::default()));
            let ul = r.node.expect("div -> ul");
            assert_eq!(ul.element().level, Some(1));
            let li = ul.element().node.as_ref().expect("ul -> li");
            assert_eq!(li.element().level, Some(2));
        }

        #[test]
        fn element_with_snippet_expansion() {
            // "a" expands to "a:href" -> identifier="a", href prop present
            let config = html_config(&[("a", "a:href")]);
            let r = ok(parse_input("a", None, &config));
            assert_eq!(r.identifier.as_deref(), Some("a"));
            let attrs = r.attributes.expect("a should have href attribute");
            assert_eq!(attrs[0], AttributeType::Props("href".into(), None));
        }

        #[test]
        fn element_with_class_in_abbreviation() {
            let r = ok(parse_input("div.container", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let attrs = r.attributes.expect("should have class attr");
            assert_eq!(attrs[0], AttributeType::Class("container".into()));
        }

        // ── implicit div ──────────────────────────────────────────────

        #[test]
        fn implicit_div_class_yields_div_identifier() {
            let r = ok(parse_input(".foo", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let attrs = r.attributes.expect("should have class attr");
            assert_eq!(attrs[0], AttributeType::Class("foo".into()));
        }

        #[test]
        fn implicit_div_id_yields_div_identifier() {
            let r = ok(parse_input("#main", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let attrs = r.attributes.expect("should have id attr");
            assert_eq!(attrs[0], AttributeType::Id("main".into()));
        }

        #[test]
        fn implicit_div_prop_yields_div_identifier() {
            let r = ok(parse_input(":disabled", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let attrs = r.attributes.expect("should have prop attr");
            assert_eq!(attrs[0], AttributeType::Props("disabled".into(), None));
        }

        #[test]
        fn implicit_div_child_operator_yields_div_with_child() {
            let r = ok(parse_input(">p", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let node = r.node.expect(">p should produce div -> p");
            assert!(matches!(*node, Node::Children(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
        }

        #[test]
        fn empty_input_returns_err() {
            assert!(parse_input("", None, &Config::default()).is_err());
        }

        #[test]
        fn id_from_variable_render_correctly() {
            let r = ok(parse_input("#{myId}", None, &Config::default()));
            assert_eq!(r.identifier.as_deref(), Some("div"));
            let attributes = r.attributes.expect("should have id attribute");
            assert_eq!(attributes.len(), 1);
            assert_eq!(attributes[0], AttributeType::Id("{myId}".into()));
        }

        #[test]
        fn bare_child_operator_returns_err() {
            // ">" prepends div, but then the child is empty string -> NoIdentifier
            assert!(parse_input(">", None, &Config::default()).is_err());
        }

        // ── fragment (e snippet) ────────────────────────────────────

        #[test]
        fn fragment_has_empty_identifier() {
            let r = ok(parse_input("e", None, &jsx_config()));
            assert_eq!(r.identifier.as_deref(), Some(""));
            assert!(!r.self_closing);
            assert!(r.attributes.is_none());
            assert!(r.node.is_none());
        }

        #[test]
        fn fragment_with_child_has_children_node() {
            let r = ok(parse_input("e>div", None, &jsx_config()));
            assert_eq!(r.identifier.as_deref(), Some(""));
            let node = r.node.expect("fragment should have child");
            assert!(matches!(*node, Node::Children(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("div"));
        }

        #[test]
        fn fragment_with_sibling_has_sibling_node() {
            let r = ok(parse_input("e+p", None, &jsx_config()));
            assert_eq!(r.identifier.as_deref(), Some(""));
            let node = r.node.expect("fragment should have sibling");
            assert!(matches!(*node, Node::Sibling(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("p"));
        }
    }

    // -------------------------------------------------------------------------
    // parse_group
    // -------------------------------------------------------------------------
    #[cfg(test)]
    mod parse_group_tests {
        use super::*;

        #[test]
        fn simple_group_no_suffix() {
            let r = ok(parse_group("(div)", None, &Config::default()));
            assert!(r.identifier.is_none());
            assert_eq!(r.multiplier, 1);
            assert!(r.node.is_none());
            let inner = r.group.expect("group content should exist");
            assert_eq!(inner.identifier.as_deref(), Some("div"));
        }

        #[test]
        fn group_with_multiplier_only() {
            let r = ok(parse_group("(div)*3", None, &Config::default()));
            assert_eq!(r.multiplier, 3);
            assert!(r.node.is_none());
        }

        #[test]
        fn multi_digit_multiplier() {
            let r = ok(parse_group("(li)*10", None, &Config::default()));
            assert_eq!(r.multiplier, 10);
            assert!(r.node.is_none());
        }

        #[test]
        fn group_with_sibling_no_multiplier() {
            let r = ok(parse_group("(div)+span", None, &Config::default()));
            assert_eq!(r.multiplier, 1);
            let node = r.node.expect("should have a sibling node");
            assert!(matches!(*node, Node::Sibling(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("span"));
        }

        #[test]
        fn group_with_multiplier_and_sibling() {
            let r = ok(parse_group("(div)*3+span", None, &Config::default()));
            assert_eq!(r.multiplier, 3);
            let node = r.node.expect("should have a sibling node");
            assert!(matches!(*node, Node::Sibling(_)));
            assert_eq!(node.element().identifier.as_deref(), Some("span"));
        }

        #[test]
        fn group_inner_content_recursively_parsed() {
            let r = ok(parse_group("(ul>li)", None, &Config::default()));
            let inner = r.group.expect("group content should exist");
            assert_eq!(inner.identifier.as_deref(), Some("ul"));
            let child = inner.node.expect("ul should have child li");
            assert!(matches!(*child, Node::Children(_)));
            assert_eq!(child.element().identifier.as_deref(), Some("li"));
        }

        #[test]
        fn sibling_chain_after_group() {
            let r = ok(parse_group("(div)+p+span", None, &Config::default()));
            let first = r.node.expect("should have first sibling");
            assert!(matches!(*first, Node::Sibling(_)));
            assert_eq!(first.element().identifier.as_deref(), Some("p"));
            let second = first
                .element()
                .node
                .as_ref()
                .expect("p should have sibling span");
            assert!(matches!(**second, Node::Sibling(_)));
            assert_eq!(second.element().identifier.as_deref(), Some("span"));
        }

        #[test]
        fn nested_group_in_inner_content() {
            let r = ok(parse_group("((div>p)+span)", None, &Config::default()));
            let inner = r.group.expect("outer group content should exist");
            assert!(inner.group.is_some(), "inner content should be a group");
        }

        #[test]
        fn group_level_is_passed_to_inner_content() {
            // level given to parse_group is given to the inner Element tree
            let r = ok(parse_group("(div>p)", Some(1), &Config::default()));
            let inner = r.group.expect("group should exist");
            assert_eq!(inner.level, Some(1));
            let child = inner.node.expect("div -> p");
            assert_eq!(child.element().level, Some(2));
        }

        #[test]
        fn group_with_snippet_inside() {
            let config = html_config(&[("a", "a:href")]);
            // "a" snippet should expand inside the group too
            let r = ok(parse_group("(a)+div", None, &config));
            let inner = r.group.expect("group should exist");
            assert_eq!(inner.identifier.as_deref(), Some("a"));
            // "a" expands to "a:href" -> should have href attribute
            assert!(inner.attributes.is_some());
        }
    }

    // -------------------------------------------------------------------------
    // Display for Element
    // -------------------------------------------------------------------------
    #[cfg(test)]
    mod element_display_tests {
        use super::*;

        use std::collections::HashMap;

        fn jsx_config() -> Config {
            Config::new(ParserMode::JSX(None), HashMap::new())
        }

        #[test]
        fn simple_element() {
            assert_eq!(
                ok(parse_input("div", None, &Config::default())).to_string(),
                "<div></div>"
            );
        }

        #[test]
        fn self_closing_via_snippet() {
            let config = html_config(&[("br", "br/")]);
            // "br" expands to "br/" via snippet -> self_closing = true
            assert_eq!(ok(parse_input("br", None, &config)).to_string(), "<br />");
        }

        #[test]
        fn element_with_single_class() {
            assert_eq!(
                ok(parse_input("div.container", None, &Config::default())).to_string(),
                "<div class=\"container\"></div>"
            );
        }

        #[test]
        fn element_with_multiple_classes_preserves_order() {
            assert_eq!(
                ok(parse_input(
                    "div.flex.items-center",
                    None,
                    &Config::default()
                ))
                .to_string(),
                "<div class=\"flex items-center\"></div>"
            );
        }

        #[test]
        fn element_with_id() {
            assert_eq!(
                ok(parse_input("div#main", None, &Config::default())).to_string(),
                "<div id=\"main\"></div>"
            );
        }

        #[test]
        fn element_with_plain_prop_value_is_quoted() {
            assert_eq!(
                ok(parse_input("div:role=main", None, &Config::default())).to_string(),
                "<div role=\"main\"></div>"
            );
        }

        #[test]
        fn element_with_jsx_prop_value_is_not_quoted() {
            assert_eq!(
                ok(parse_input("div:onClick={handler}", None, &jsx_config())).to_string(),
                "<div onClick={handler}></div>"
            );
        }

        #[test]
        fn element_with_text_content() {
            assert_eq!(
                ok(parse_input("div>>Hello", None, &Config::default())).to_string(),
                "<div>Hello</div>"
            );
        }

        #[test]
        fn element_with_child_indents_one_level() {
            assert_eq!(
                ok(parse_input("div>p", None, &Config::default())).to_string(),
                "<div>\n\t<p></p>\n</div>"
            );
        }

        #[test]
        fn siblings_at_root_separated_by_newline() {
            assert_eq!(
                ok(parse_input("div+p", None, &Config::default())).to_string(),
                "<div></div>\n<p></p>"
            );
        }

        #[test]
        fn siblings_at_indented_level_carry_newline_prefix() {
            assert_eq!(
                ok(parse_input("div+p", Some(1), &Config::default())).to_string(),
                "\n\t<div></div>\n\t<p></p>"
            );
        }

        #[test]
        fn multiplied_element_at_root_separated_by_newline() {
            assert_eq!(
                ok(parse_input("li*3", None, &Config::default())).to_string(),
                "<li></li>\n<li></li>\n<li></li>"
            );
        }

        #[test]
        fn multiplied_element_at_indented_level_uses_embedded_prefix() {
            // the \n\t is part of each repeated value, so join("") gives correct output
            assert_eq!(
                ok(parse_input("li*3", Some(1), &Config::default())).to_string(),
                "\n\t<li></li>\n\t<li></li>\n\t<li></li>"
            );
        }

        #[test]
        fn nested_children_indent_accumulates() {
            let config = html_config(&[("a", "a:href")]);
            assert_eq!(
                ok(parse_input("ul>li>a", None, &config)).to_string(),
                "<ul>\n\t<li>\n\t\t<a href></a>\n\t</li>\n</ul>"
            );
        }

        #[test]
        fn attributes_are_sorted_id_then_props_then_class() {
            // AttributeType order: Id(0) < Props(1) < Class(2)
            assert_eq!(
                ok(parse_input(
                    "div.foo#bar:disabled",
                    None,
                    &Config::default()
                ))
                .to_string(),
                "<div id=\"bar\" disabled class=\"foo\"></div>"
            );
        }

        #[test]
        fn snippet_expansion_included_in_output() {
            let config = html_config(&[("a", "a:href")]);
            // "a" -> "a:href" -> href boolean attr in output
            assert_eq!(
                ok(parse_input("a", None, &config)).to_string(),
                "<a href></a>"
            );
        }

        #[test]
        fn group_renders_inner_element() {
            assert_eq!(
                ok(parse_group("(div)+span", None, &Config::default())).to_string(),
                "<div></div>\n<span></span>"
            );
        }

        #[test]
        fn group_with_multiplier_at_root_separated_by_newline() {
            assert_eq!(
                ok(parse_group("(li)*3", None, &Config::default())).to_string(),
                "<li></li>\n<li></li>\n<li></li>"
            );
        }

        #[test]
        fn group_with_multiplier_at_indented_level() {
            assert_eq!(
                ok(parse_group("(li)*3", Some(1), &Config::default())).to_string(),
                "\n\t<li></li>\n\t<li></li>\n\t<li></li>"
            );
        }

        #[test]
        fn multiplied_children_inside_parent() {
            assert_eq!(
                ok(parse_input("ul>li*3", None, &Config::default())).to_string(),
                "<ul>\n\t<li></li>\n\t<li></li>\n\t<li></li>\n</ul>"
            );
        }

        #[test]
        fn fragments() {
            assert_eq!(
                ok(parse_input("e", None, &jsx_config())).to_string(),
                "<></>"
            );
        }

        #[test]
        fn fragment_with_child() {
            assert_eq!(
                ok(parse_input("e>div", None, &jsx_config())).to_string(),
                "<>\n\t<div></div>\n</>"
            );
        }

        #[test]
        fn fragment_with_sibling() {
            assert_eq!(
                ok(parse_input("e+p", None, &jsx_config())).to_string(),
                "<></>\n<p></p>"
            );
        }

        #[test]
        fn fragment_multiplied() {
            assert_eq!(
                ok(parse_input("e*3", None, &jsx_config())).to_string(),
                "<></>\n<></>\n<></>"
            );
        }

        // ── implicit div ─────────────────────────────────────────────

        #[test]
        fn implicit_div_from_class() {
            assert_eq!(
                ok(parse_input(".container", None, &Config::default())).to_string(),
                "<div class=\"container\"></div>"
            );
        }

        #[test]
        fn implicit_div_from_id() {
            assert_eq!(
                ok(parse_input("#main", None, &Config::default())).to_string(),
                "<div id=\"main\"></div>"
            );
        }

        #[test]
        fn implicit_div_from_prop() {
            assert_eq!(
                ok(parse_input(":disabled", None, &Config::default())).to_string(),
                "<div disabled></div>"
            );
        }

        #[test]
        fn implicit_div_from_child_operator() {
            assert_eq!(
                ok(parse_input(">p", None, &Config::default())).to_string(),
                "<div>\n\t<p></p>\n</div>"
            );
        }

        #[test]
        fn implicit_div_class_with_own_child() {
            assert_eq!(
                ok(parse_input(".foo>p", None, &Config::default())).to_string(),
                "<div class=\"foo\">\n\t<p></p>\n</div>"
            );
        }

        #[test]
        fn implicit_div_class_with_implicit_div_sibling() {
            // .foo+.bar -> both become divs, separated by newline
            assert_eq!(
                ok(parse_input(".foo+.bar", None, &Config::default())).to_string(),
                "<div class=\"foo\"></div>\n<div class=\"bar\"></div>"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Custom snippets — parse_input + parse_group integration
    // -------------------------------------------------------------------------
    #[cfg(test)]
    mod custom_snippet_tests {
        use crate::parser::attribute::AttributeType;

        use super::*;

        // ── AST tests ─────────────────────────────────────────────────────────────────────

        #[test]
        fn custom_alias_resolves_to_identifier() {
            // "mc" is not a built-in; the custom map expands it to "MyComponent"
            let config = html_config(&[("mc", "MyComponent")]);
            let r = ok(parse_input("mc", None, &config));
            assert_eq!(r.identifier.as_deref(), Some("MyComponent"));
        }

        #[test]
        fn custom_overrides_builtin_identifier() {
            // built-in "btn" → "button"; custom entry shadows it with "MyButton"
            let config = html_config(&[("btn", "MyButton")]);
            let r = ok(parse_input("btn", None, &config));
            assert_eq!(r.identifier.as_deref(), Some("MyButton"));
        }

        #[test]
        fn custom_self_closing_snippet() {
            // expansion ending with "/" must set self_closing = true
            let config = html_config(&[("myimg", "MyImage/")]);
            let r = ok(parse_input("myimg", None, &config));
            assert_eq!(r.identifier.as_deref(), Some("MyImage"));
            assert!(r.self_closing);
        }

        #[test]
        fn custom_snippet_with_attributes() {
            // "mc" → "MyComponent:name" → identifier="MyComponent", one Props attr
            let config = html_config(&[("comp", "MyComponent:name")]);
            let r = ok(parse_input("comp", None, &config));
            assert_eq!(r.identifier.as_deref(), Some("MyComponent"));
            let attrs = r.attributes.expect("should have name attribute");
            assert_eq!(attrs[0], AttributeType::Props("name".into(), None));
        }

        #[test]
        fn custom_snippet_propagates_to_child() {
            // custom map must be forwarded when recursively parsing children
            let config = html_config(&[("mc", "MyComponent")]);
            let r = ok(parse_input("div>mc", None, &config));
            let child = r.node.expect("div should have child");
            assert_eq!(child.element().identifier.as_deref(), Some("MyComponent"));
        }

        #[test]
        fn custom_snippet_propagates_to_sibling() {
            // custom map must be forwarded when recursively parsing siblings
            let config = html_config(&[("mc", "MyComponent")]);
            let r = ok(parse_input("div+mc", None, &config));
            let sibling = r.node.expect("div should have sibling");
            assert_eq!(sibling.element().identifier.as_deref(), Some("MyComponent"));
        }

        #[test]
        fn custom_snippet_expands_inside_group() {
            // parse_group must also forward the custom map to its inner content
            let config = html_config(&[("mc", "MyComponent")]);
            let r = ok(parse_group("(mc)+div", None, &config));
            let inner = r.group.expect("group should contain inner element");
            assert_eq!(inner.identifier.as_deref(), Some("MyComponent"));
        }

        // ── Display / rendering tests ─────────────────────────────────────────────────────

        #[test]
        fn custom_alias_renders_as_tag() {
            let config = html_config(&[("mc", "MyComponent")]);
            assert_eq!(
                ok(parse_input("mc", None, &config)).to_string(),
                "<MyComponent></MyComponent>"
            );
        }

        #[test]
        fn custom_self_closing_renders() {
            // expansion "MyImage:src/" → <MyImage src />
            let config = html_config(&[("myimg", "MyImage:src/")]);
            assert_eq!(
                ok(parse_input("myimg", None, &config)).to_string(),
                "<MyImage src />"
            );
        }

        #[test]
        fn custom_override_produces_different_output_from_builtin() {
            // without custom: "btn" → "button" → <button></button>
            // with custom:    "btn" → "MyButton" → <MyButton></MyButton>
            let config1 = html_config(&[("btn", "button")]);
            assert_eq!(
                ok(parse_input("btn", None, &config1)).to_string(),
                "<button></button>"
            );
            let config2 = html_config(&[("btn", "MyButton")]);
            assert_eq!(
                ok(parse_input("btn", None, &config2)).to_string(),
                "<MyButton></MyButton>"
            );
        }

        #[test]
        fn custom_snippet_child_renders_correctly() {
            let config = html_config(&[("mc", "MyComponent")]);
            assert_eq!(
                ok(parse_input("div>mc", None, &config)).to_string(),
                "<div>\n\t<MyComponent></MyComponent>\n</div>"
            );
        }

        #[test]
        fn custom_snippet_sibling_renders_correctly() {
            let config = html_config(&[("mc", "MyComponent")]);
            assert_eq!(
                ok(parse_input("mc+div", None, &config)).to_string(),
                "<MyComponent></MyComponent>\n<div></div>"
            );
        }
    }
}
