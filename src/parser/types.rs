//! AST types produced by the Glyf parser.
//!
//! The core type is [`Element`], which represents a single node in the
//! parsed abbreviation tree. Elements are linked together via [`Node`]
//! which carries a [`NodeType`] to describe the relationship.

use std::{fmt::Display, iter::repeat_n, sync::LazyLock};

use regex::Regex;

use crate::{
    config::{Config, ParserMode},
    parser::{parse_input, utils::has_node_operator},
};

use super::{
    attribute::{AttributeType, Render, parse_attribute},
    error::GlyfError,
    snippet::parse_snippet,
};

static IDENTIFIER_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[\w-]+").unwrap());

/// The relationship between an element and its next node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeType {
    /// `+` — the next node is a sibling (same level)
    Sibling,
    /// `>` — the next node is a child (indented one level deeper)
    Children,
}

/// Links an [`Element`] to the next element in the abbreviation.
#[derive(Debug, Clone)]
pub struct Node {
    pub node_type: NodeType,
    /// The next element in the chain.
    pub node: Element,
}

/// A parsed Glyf node — the fundamental unit of the output tree.
///
/// Three distinct states are possible:
///
/// | `identifier` | `group` | Meaning |
/// |---|---|---|
/// | `Some("div")` | `None` | A concrete element: `<div>` |
/// | `Some("")` | `None` | A JSX fragment: `<></>` (JSX mode only) |
/// | `None` | `Some(inner)` | A `(...)` group, or a multi-element snippet expansion |
///
/// `Display` renders the element (and the full subtree rooted here) to
/// an indented HTML/JSX string.
#[derive(Debug, Clone)]
pub struct Element {
    /// HTML/JSX tag name.  `Some("")` = fragment, `None` = group wrapper.
    pub identifier: Option<String>,
    /// When `true` the element renders as `<tag />` (no closing tag, no children).
    pub self_closing: bool,
    /// Parsed attributes in declaration order (sorted by [`AttributeType`] at render time).
    pub attributes: Option<Vec<AttributeType>>,
    /// Set when this node is a `(...)` group or a multi-element snippet expansion;
    /// contains the inner element tree.
    pub group: Option<Box<Element>>,
    /// Number of times this element is repeated (`*N` in the abbreviation).
    pub multiplier: usize,
    /// The next element in the chain (child or sibling).
    pub node: Option<Box<Node>>,
    /// Indentation depth. `None` or `Some(0)` = root level, `Some(n)` = `n` tabs.
    pub level: Option<usize>,
    pub mode: ParserMode,
}

impl Default for Element {
    fn default() -> Self {
        Self {
            identifier: Some(String::new()),
            self_closing: false,
            attributes: None,
            group: None,
            multiplier: 1,
            node: None,
            level: None,
            mode: ParserMode::HTML,
        }
    }
}

impl Element {
    /// Wraps an already-parsed [`Element`] tree in a group node.
    ///
    /// Used by `parse_group` and internally when snippet expansion
    /// produces a compound expression containing `>` or `+`.
    pub(super) fn from_group(
        group: Box<Element>,
        multiplier: usize,
        node: Option<Box<Node>>,
        level: Option<usize>,
        mode: ParserMode,
    ) -> Self {
        Self {
            identifier: None,
            self_closing: false,
            attributes: None,
            group: Some(group),
            multiplier,
            node,
            level,
            mode,
        }
    }

    /// Parses a raw Glyf abbreviation fragment into a concrete [`Element`].
    ///
    /// Pipeline:
    /// 1. Snippet expansion.
    /// 2. If the result contains `>` or `+`, re-parses via `parse_input`
    ///    and delegates to [`Element::from_group`].
    /// 3. Otherwise extracts the identifier, self-closing flag, and attributes.
    ///
    /// # Errors
    /// Returns [`GlyfError::NoIdentifier`] when no valid tag name is found.
    pub(super) fn from_abbr(
        value: &str,
        multiplier: usize,
        node: Option<Box<Node>>,
        level: Option<usize>,
        config: &Config,
    ) -> Result<Self, GlyfError> {
        let mode = config.mode;

        // JSX fragment shorthand: bare "e" → <></>
        if mode == ParserMode::JSX && value == "e" {
            return Ok(Self {
                identifier: Some(String::new()),
                self_closing: false,
                attributes: None,
                group: None,
                multiplier,
                node,
                level,
                mode,
            });
        }

        let expanded = parse_snippet(value, &config.snippets);

        // Snippet expanded to a compound expression — re-parse as a tree
        if has_node_operator(&expanded) {
            let inner = parse_input(&expanded, level, config)?;
            return Ok(Self::from_group(
                Box::new(inner),
                multiplier,
                node,
                level,
                mode,
            ));
        }

        // Concrete element: extract identifier and attributes
        let identifier = IDENTIFIER_REGEX
            .find(&expanded)
            .ok_or(GlyfError::NoIdentifier)? // ← replaces is_none() + unwrap()
            .as_str()
            .to_string();

        let self_closing = expanded.ends_with('/');
        let attr_end = if self_closing {
            expanded.len().saturating_sub(1)
        } else {
            expanded.len()
        };
        let attributes = parse_attribute(&expanded[identifier.len()..attr_end]);

        Ok(Self {
            identifier: Some(identifier),
            self_closing,
            attributes: if attributes.is_empty() {
                None
            } else {
                Some(attributes)
            },
            group: None,
            multiplier,
            node,
            level,
            mode,
        })
    }

    /// Converts this element tree to its Glyf abbreviation string.
    ///
    /// The inverse of constructing an element via [`crate::expand`] — produces
    /// the Glyf abbreviation that would generate equivalent HTML/JSX.
    /// Attributes are emitted in compress order: `.class` before `#id` before
    /// `:prop` before `>>text`.
    ///
    /// # Examples
    ///
    /// ```
    /// use glyf_core::expand_to_tree;
    ///
    /// let el = expand_to_tree("div.foo>p", None, None).unwrap();
    /// assert_eq!(el.to_glyf(), "div.foo>p");
    /// ```
    pub fn to_glyf(&self) -> String {
        let mut result = String::new();
        if let Some(identifier) = &self.identifier {
            result.push_str(identifier);
            if let Some(attributes) = &self.attributes {
                let mut sorted = attributes.iter().collect::<Vec<&AttributeType>>();
                sorted.sort();
                let glyf_attribute = sorted
                    .iter()
                    .map(|attr| attr.to_glyf())
                    .collect::<Vec<String>>();
                result.push_str(&glyf_attribute.join(""));
            }
            if self.self_closing {
                result.push('/');
            }
        } else if let Some(group) = &self.group {
            result.push_str(&format!("({})", group.to_glyf()))
        }

        if self.multiplier > 1 {
            result.push_str(&format!("*{}", self.multiplier));
        }

        if let Some(node) = &self.node {
            result.push_str(
                match node.node_type {
                    NodeType::Sibling => format!("+{}", node.node.to_glyf()),
                    NodeType::Children => format!(">{}", node.node.to_glyf()),
                }
                .as_str(),
            );
        }

        result
    }
}

impl Display for Element {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut value = String::new();
        let mut child = String::new();
        let mut sibling = String::new();
        let is_first_level = self.level.is_none_or(|v| v == 0);
        let level = self.level.unwrap_or(0);

        let prefix = format!(
            "{}{}",
            if !is_first_level { "\n" } else { "" },
            "\t".repeat(level)
        );

        if let Some(node) = self.node.as_ref() {
            if node.node_type == NodeType::Children {
                child = node.node.to_string()
            } else if node.node_type == NodeType::Sibling {
                sibling = node.node.to_string()
            }
        }
        let suffix = if !child.is_empty() {
            format!("\n{}", "\t".repeat(level))
        } else {
            String::new()
        };

        if let Some(identifier) = self.identifier.as_ref() {
            let mut attributes = self.attributes.clone().unwrap_or_default();
            attributes.sort();

            let classes = &attributes
                .iter()
                .filter_map(|a| match a {
                    AttributeType::Class(name) => Some(name.as_str()),
                    _ => None,
                })
                .filter(|v| !v.is_empty())
                .collect::<Vec<&str>>()
                .join(" ");

            let props_attributes = &attributes
                .iter()
                .filter_map(|a| {
                    if matches!(a, AttributeType::Id(_) | AttributeType::Props(_, _)) {
                        Some(a.render(self.mode))
                    } else {
                        None
                    }
                })
                .collect::<Vec<String>>()
                .join("");

            let text_attribute = &attributes
                .iter()
                .find_map(|a| {
                    if matches!(a, AttributeType::Text(_)) {
                        Some(a.render(self.mode))
                    } else {
                        None
                    }
                })
                .unwrap_or(String::new());

            let class_attribute = if !classes.is_empty() {
                match self.mode {
                    ParserMode::HTML => format!(" class=\"{}\"", classes),
                    ParserMode::JSX => format!(" className=\"{}\"", classes),
                }
            } else {
                String::new()
            };

            let main = format!("{}{}{}", identifier, props_attributes, class_attribute);

            if self.self_closing && child.is_empty() && text_attribute.is_empty() {
                value = format!("{}<{} />", prefix, main);
            } else {
                value = format!(
                    "{}<{}>{}{}{}</{}>",
                    prefix, main, text_attribute, child, suffix, identifier
                );
            }
        }

        if let Some(group) = self.group.as_ref() {
            value = group.to_string();
        }

        let repeated = repeat_n(value.as_str(), self.multiplier)
            .collect::<Vec<&str>>()
            .join(if is_first_level { "\n" } else { "" });

        let sibling_output = if is_first_level && !sibling.is_empty() {
            format!("\n{}", sibling)
        } else {
            sibling
        };

        write!(f, "{}{}", repeated, sibling_output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn html_config(snippets_list: &[(&str, &str)]) -> Config {
        Config::new(ParserMode::HTML, snippets(snippets_list))
    }

    fn snippets(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    // -------------------------------------------------------------------------
    // Element::new
    // -------------------------------------------------------------------------
    mod element_new_tests {
        use super::*;

        #[test]
        fn simple_identifier() {
            let e = Element::from_abbr("div", 1, None, None, &Config::default()).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("div"));
            assert!(!e.self_closing);
            assert!(e.attributes.is_none());
        }

        #[test]
        fn snippet_expands_self_closing_tag() {
            let config = html_config(&[("br", "br/")]);
            let e = Element::from_abbr("br", 1, None, None, &config).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("br"));
            assert!(e.self_closing);
        }

        #[test]
        fn explicit_self_closing_slash() {
            let e = Element::from_abbr("Input/", 1, None, None, &Config::default()).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("Input"));
            assert!(e.self_closing);
        }

        #[test]
        fn snippet_expands_and_parses_attributes() {
            let config = html_config(&[("img", "img:src:alt")]);
            let e = Element::from_abbr("img", 1, None, None, &config).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("img"));
            let attrs = e.attributes.expect("img should have attributes");
            assert_eq!(attrs.len(), 2);
            assert_eq!(attrs[0], AttributeType::Props("src".into(), None));
            assert_eq!(attrs[1], AttributeType::Props("alt".into(), None));
        }

        #[test]
        fn class_attribute_is_parsed() {
            let e = Element::from_abbr("div.container", 1, None, None, &Config::default()).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("div"));
            let attrs = e.attributes.expect("should have attributes");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Class("container".into()));
        }

        #[test]
        fn prop_with_value_is_parsed() {
            let e = Element::from_abbr("div:role=main", 1, None, None, &Config::default()).unwrap();
            let attrs = e.attributes.expect("should have attributes");
            assert_eq!(
                attrs[0],
                AttributeType::Props("role".into(), Some("main".into()))
            );
        }

        #[test]
        fn multiplier_and_level_are_passed_through() {
            let e = Element::from_abbr("li", 5, None, Some(2), &Config::default()).unwrap();
            assert_eq!(e.multiplier, 5);
            assert_eq!(e.level, Some(2));
        }
    }

    // -------------------------------------------------------------------------
    // Element::new — multi-element snippet expansion
    // -------------------------------------------------------------------------
    mod multi_element_snippet_tests {
        use super::*;

        #[test]
        fn child_operator_in_expansion_produces_group() {
            let config = html_config(&[("card", "div.card>p")]);
            let e = Element::from_abbr("card", 1, None, None, &config).unwrap();
            assert!(
                e.identifier.is_none(),
                "group wrapper must have identifier = None"
            );
            assert!(e.group.is_some());
            assert_eq!(e.group.unwrap().identifier.as_deref(), Some("div"));
        }

        #[test]
        fn sibling_operator_in_expansion_produces_group() {
            let config = html_config(&[("duo", "h1+p")]);
            let e = Element::from_abbr("duo", 1, None, None, &config).unwrap();
            assert!(e.identifier.is_none());
            let inner = e.group.expect("should have a group");
            assert_eq!(inner.identifier.as_deref(), Some("h1"));
            let sibling = inner.node.expect("h1 should have sibling p");
            assert!(matches!(sibling.node_type, NodeType::Sibling));
            assert_eq!(sibling.node.identifier.as_deref(), Some("p"));
        }

        #[test]
        fn complex_expansion_builds_nested_tree() {
            let config = html_config(&[("card", "div.card>p.card-header+p.card-body")]);
            let e = Element::from_abbr("card", 1, None, None, &config).unwrap();
            assert!(e.identifier.is_none());
            let inner = e.group.expect("should have group");
            assert_eq!(inner.identifier.as_deref(), Some("div"));
            let div_attrs = inner.attributes.as_ref().expect("div should have class");
            assert!(
                div_attrs
                    .iter()
                    .any(|a| a == &AttributeType::Class("card".into()))
            );
            let child_node = inner.node.expect("div should have a child node");
            assert!(matches!(child_node.node_type, NodeType::Children));
            assert_eq!(child_node.node.identifier.as_deref(), Some("p"));
            let sibling_node = child_node.node.node.expect("should have sibling");
            assert!(matches!(sibling_node.node_type, NodeType::Sibling));
            assert_eq!(sibling_node.node.identifier.as_deref(), Some("p"));
        }

        #[test]
        fn multiplier_is_preserved_on_group_expansion() {
            let config = html_config(&[("card", "div.card>p")]);
            let e = Element::from_abbr("card", 3, None, None, &config).unwrap();
            assert_eq!(e.multiplier, 3);
        }

        #[test]
        fn outer_sibling_node_is_preserved_on_group_expansion() {
            let config = html_config(&[("card", "div.card>p")]);
            let footer = Element::from_abbr("footer", 1, None, None, &config).unwrap();
            let node = Box::new(Node {
                node_type: NodeType::Sibling,
                node: footer,
            });
            let e = Element::from_abbr("card", 1, Some(node), None, &config).unwrap();
            let sibling = e.node.expect("wrapper must carry the sibling node");
            assert!(matches!(sibling.node_type, NodeType::Sibling));
            assert_eq!(sibling.node.identifier.as_deref(), Some("footer"));
        }

        #[test]
        fn child_expansion_renders_correctly() {
            let config = html_config(&[("card", "div.card>p")]);
            let e = Element::from_abbr("card", 1, None, None, &config).unwrap();
            assert_eq!(e.to_string(), "<div class=\"card\">\n\t<p></p>\n</div>");
        }

        #[test]
        fn sibling_expansion_renders_correctly() {
            let config = html_config(&[("duo", "h1+p")]);
            let e = Element::from_abbr("duo", 1, None, None, &config).unwrap();
            assert_eq!(e.to_string(), "<h1></h1>\n<p></p>");
        }

        #[test]
        fn complex_card_expansion_renders_correctly() {
            let config = html_config(&[("card", "div.card>p.card-header+p.card-body")]);
            let e = Element::from_abbr("card", 1, None, None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<div class=\"card\">\n\t<p class=\"card-header\"></p>\n\t<p class=\"card-body\"></p>\n</div>"
            );
        }

        #[test]
        fn multiplied_group_expansion_renders_correctly() {
            let config = html_config(&[("duo", "h1+p")]);
            let e = Element::from_abbr("duo", 3, None, None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<h1></h1>\n<p></p>\n<h1></h1>\n<p></p>\n<h1></h1>\n<p></p>"
            );
        }

        #[test]
        fn group_expansion_with_outer_sibling_renders_correctly() {
            let config = html_config(&[("card", "div.card>p")]);
            let footer = Element::from_abbr("footer", 1, None, None, &config).unwrap();
            let node = Box::new(Node {
                node_type: NodeType::Sibling,
                node: footer,
            });
            let e = Element::from_abbr("card", 1, Some(node), None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<div class=\"card\">\n\t<p></p>\n</div>\n<footer></footer>"
            );
        }
    }

    // -------------------------------------------------------------------------
    // Element::to_glyf
    // -------------------------------------------------------------------------
    mod element_to_glyf_tests {
        use super::*;

        fn cfg() -> Config {
            Config::default()
        }

        #[test]
        fn simple_element() {
            let e = Element::from_abbr("div", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div");
        }

        #[test]
        fn element_with_class() {
            let e = Element::from_abbr("div.foo", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div.foo");
        }

        #[test]
        fn element_with_id() {
            let e = Element::from_abbr("div#main", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div#main");
        }

        #[test]
        fn element_with_prop() {
            let e = Element::from_abbr("a:href=url", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "a:href=url");
        }

        #[test]
        fn element_with_text_content() {
            let e = Element::from_abbr("p>>Hello", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "p>>Hello");
        }

        #[test]
        fn self_closing_element() {
            let e = Element::from_abbr("br/", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "br/");
        }

        #[test]
        fn element_with_multiplier() {
            let e = Element::from_abbr("li", 3, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "li*3");
        }

        #[test]
        fn element_with_child() {
            let e = parse_input("div>p", None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div>p");
        }

        #[test]
        fn element_with_sibling() {
            let e = parse_input("div+span", None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div+span");
        }

        #[test]
        fn attributes_sorted_class_before_id() {
            // to_glyf uses compress order: class first, then id
            let e = Element::from_abbr("div#main.foo", 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div.foo#main");
        }

        #[test]
        fn chained_children() {
            let e = parse_input("div>p>span", None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div>p>span");
        }
    }
}
