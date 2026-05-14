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
    attribute::{Attribute, AttributeType, parse_attribute},
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
    pub attributes: Option<Vec<Attribute>>,
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
    /// Constructs an [`Element`] from raw parsed data.
    ///
    /// - If `value` is `Some`, snippet expansion and attribute parsing are applied.
    /// - If the expanded snippet contains `>` or `+` (child/sibling operators), it is
    ///   re-parsed via [`parse_input`] and wrapped as a group (`identifier = None`,
    ///   `group = Some(inner)`), exactly like an explicit `(...)` group expression.
    /// - If `value` is `None`, the element is a group wrapper (`identifier = None`).
    /// - In JSX mode ([`crate::config::ParserMode::JSX`]), the literal identifier `"e"`
    ///   is recognised directly and produces a JSX fragment
    ///   (`identifier = Some("")`, renders as `<></>`) — no snippet entry needed.
    ///
    /// # Errors
    /// Returns [`GlyfError::NoIdentifier`] when `value` is non-empty but contains
    /// no leading word characters (e.g. a lone operator with no tag name).
    pub fn new(
        value: Option<String>,
        group: Option<Box<Element>>,
        multiplier: usize,
        node: Option<Box<Node>>,
        level: Option<usize>,
        config: &Config,
    ) -> Result<Self, GlyfError> {
        let mode = config.mode();
        if let Some(value) = &value {
            if mode == ParserMode::JSX && value == "e" {
                return Ok(Self {
                    identifier: Some(String::new()),
                    self_closing: false,
                    attributes: None,
                    group,
                    multiplier,
                    node,
                    level,
                    mode,
                });
            }

            let transformed_value = parse_snippet(value, config.snippets());
            if has_node_operator(&transformed_value) {
                let group = parse_input(&transformed_value, level, config);
                return match group {
                    Err(e) => Err(e),
                    Ok(element) => Ok(Self {
                        identifier: None,
                        group: Some(Box::new(element)),
                        multiplier,
                        level,
                        node,
                        ..Default::default()
                    }),
                };
            }

            let identifier_match = IDENTIFIER_REGEX.find(&transformed_value);
            if identifier_match.is_none() {
                return Err(GlyfError::NoIdentifier);
            }

            let identifier = identifier_match.unwrap().as_str().to_string();

            let self_closing = transformed_value.ends_with("/");
            let attributes = parse_attribute(
                &transformed_value[identifier.len()..(if self_closing {
                    transformed_value.len() - 1
                } else {
                    transformed_value.len()
                })],
                &mode,
            );
            return Ok(Self {
                identifier: Some(identifier),
                self_closing,
                attributes: if !attributes.is_empty() {
                    Some(attributes)
                } else {
                    None
                },
                group,
                multiplier,
                node,
                level,
                mode,
            });
        }

        // If no value then we return default
        Ok(Self {
            identifier: None,
            self_closing: false,
            attributes: None,
            group,
            multiplier,
            node,
            level,
            mode,
        })
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
            attributes.sort_by(|a, b| a.attribute_type.cmp(&b.attribute_type));

            let classes = &attributes
                .iter()
                .filter(|a| matches!(a.attribute_type, AttributeType::Class))
                .map(|a| a.identifier.clone())
                .collect::<Vec<String>>()
                .join(" ");

            let props_attributes = &attributes
                .iter()
                .filter(|a| {
                    matches!(a.attribute_type, AttributeType::Props)
                        || matches!(a.attribute_type, AttributeType::Id)
                })
                .map(|a| a.to_string())
                .collect::<Vec<String>>()
                .join("");

            let text_attribute = if let Some(attribute) = &attributes
                .iter()
                .find(|a| matches!(a.attribute_type, AttributeType::Text))
            {
                attribute.to_string()
            } else {
                String::new()
            };

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
            let e =
                Element::new(Some("div".into()), None, 1, None, None, &Config::default()).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("div"));
            assert!(!e.self_closing);
            assert!(e.attributes.is_none());
        }

        #[test]
        fn snippet_expands_self_closing_tag() {
            let config = html_config(&[("br", "br/")]);
            let e = Element::new(Some("br".into()), None, 1, None, None, &config).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("br"));
            assert!(e.self_closing);
        }

        #[test]
        fn explicit_self_closing_slash() {
            let e = Element::new(
                Some("Input/".into()),
                None,
                1,
                None,
                None,
                &Config::default(),
            )
            .unwrap();
            assert_eq!(e.identifier.as_deref(), Some("Input"));
            assert!(e.self_closing);
        }

        #[test]
        fn snippet_expands_and_parses_attributes() {
            let config = html_config(&[("img", "img:src:alt")]);
            let e = Element::new(Some("img".into()), None, 1, None, None, &config).unwrap();
            assert_eq!(e.identifier.as_deref(), Some("img"));
            let attrs = e.attributes.expect("img should have attributes");
            assert_eq!(attrs.len(), 2);
            assert_eq!(attrs[0].identifier, "src");
            assert_eq!(attrs[1].identifier, "alt");
        }

        #[test]
        fn class_attribute_is_parsed() {
            let e = Element::new(
                Some("div.container".into()),
                None,
                1,
                None,
                None,
                &Config::default(),
            )
            .unwrap();
            assert_eq!(e.identifier.as_deref(), Some("div"));
            let attrs = e.attributes.expect("should have attributes");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "container");
            assert!(matches!(attrs[0].attribute_type, AttributeType::Class));
        }

        #[test]
        fn prop_with_value_is_parsed() {
            let e = Element::new(
                Some("div:role=main".into()),
                None,
                1,
                None,
                None,
                &Config::default(),
            )
            .unwrap();
            let attrs = e.attributes.expect("should have attributes");
            assert_eq!(attrs[0].identifier, "role");
            assert_eq!(attrs[0].value.as_deref(), Some("main"));
        }

        #[test]
        fn none_value_produces_group_element() {
            let e = Element::new(None, None, 1, None, None, &Config::default()).unwrap();
            assert!(e.identifier.is_none());
            assert!(!e.self_closing);
            assert!(e.attributes.is_none());
        }

        #[test]
        fn multiplier_and_level_are_passed_through() {
            let e = Element::new(
                Some("li".into()),
                None,
                5,
                None,
                Some(2),
                &Config::default(),
            )
            .unwrap();
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
            let e = Element::new(Some("card".into()), None, 1, None, None, &config).unwrap();
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
            let e = Element::new(Some("duo".into()), None, 1, None, None, &config).unwrap();
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
            let e = Element::new(Some("card".into()), None, 1, None, None, &config).unwrap();
            assert!(e.identifier.is_none());
            let inner = e.group.expect("should have group");
            assert_eq!(inner.identifier.as_deref(), Some("div"));
            let div_attrs = inner.attributes.as_ref().expect("div should have class");
            assert!(div_attrs.iter().any(|a| a.identifier == "card"));
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
            let e = Element::new(Some("card".into()), None, 3, None, None, &config).unwrap();
            assert_eq!(e.multiplier, 3);
        }

        #[test]
        fn outer_sibling_node_is_preserved_on_group_expansion() {
            let config = html_config(&[("card", "div.card>p")]);
            let footer = Element::new(Some("footer".into()), None, 1, None, None, &config).unwrap();
            let node = Box::new(Node {
                node_type: NodeType::Sibling,
                node: footer,
            });
            let e = Element::new(Some("card".into()), None, 1, Some(node), None, &config).unwrap();
            let sibling = e.node.expect("wrapper must carry the sibling node");
            assert!(matches!(sibling.node_type, NodeType::Sibling));
            assert_eq!(sibling.node.identifier.as_deref(), Some("footer"));
        }

        #[test]
        fn child_expansion_renders_correctly() {
            let config = html_config(&[("card", "div.card>p")]);
            let e = Element::new(Some("card".into()), None, 1, None, None, &config).unwrap();
            assert_eq!(e.to_string(), "<div class=\"card\">\n\t<p></p>\n</div>");
        }

        #[test]
        fn sibling_expansion_renders_correctly() {
            let config = html_config(&[("duo", "h1+p")]);
            let e = Element::new(Some("duo".into()), None, 1, None, None, &config).unwrap();
            assert_eq!(e.to_string(), "<h1></h1>\n<p></p>");
        }

        #[test]
        fn complex_card_expansion_renders_correctly() {
            let config = html_config(&[("card", "div.card>p.card-header+p.card-body")]);
            let e = Element::new(Some("card".into()), None, 1, None, None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<div class=\"card\">\n\t<p class=\"card-header\"></p>\n\t<p class=\"card-body\"></p>\n</div>"
            );
        }

        #[test]
        fn multiplied_group_expansion_renders_correctly() {
            let config = html_config(&[("duo", "h1+p")]);
            let e = Element::new(Some("duo".into()), None, 3, None, None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<h1></h1>\n<p></p>\n<h1></h1>\n<p></p>\n<h1></h1>\n<p></p>"
            );
        }

        #[test]
        fn group_expansion_with_outer_sibling_renders_correctly() {
            let config = html_config(&[("card", "div.card>p")]);
            let footer = Element::new(Some("footer".into()), None, 1, None, None, &config).unwrap();
            let node = Box::new(Node {
                node_type: NodeType::Sibling,
                node: footer,
            });
            let e = Element::new(Some("card".into()), None, 1, Some(node), None, &config).unwrap();
            assert_eq!(
                e.to_string(),
                "<div class=\"card\">\n\t<p></p>\n</div>\n<footer></footer>"
            );
        }
    }
}
