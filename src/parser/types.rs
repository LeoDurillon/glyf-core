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
    /// use glyf_core::parser::parse_input;
    /// use glyf_core::config::Config;
    ///
    /// let el = parse_input("div.foo>p", None, &Config::default()).unwrap();
    /// assert_eq!(el.to_glyf(), "div.foo>p");
    /// ```
    pub fn to_glyf(&self) -> String {
        let mut result = String::new();
        if let Some(identifier) = &self.identifier {
            result.push_str(identifier);
            if let Some(attributes) = &self.attributes {
                let mut glyf_attribute = attributes
                    .iter()
                    .map(|attr| attr.to_glyf())
                    .collect::<Vec<String>>();
                glyf_attribute.sort_by_key(|k| match k.chars().next() {
                    Some('.') => 0, // class
                    Some('#') => 1, // id
                    Some(':') => 2, // props
                    Some('>') => 3, // text content
                    _ => 4,
                });
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

static ATTRIBUTE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-zA-Z-]+=(?:\{.+?\}|["'].+?["'])|[a-zA-Z]+|>.+$)"#).unwrap());

const VOID_ELEMENTS: &[&str] = &[
    "br", "hr", "img", "input", "col", "area", "base", "link", "meta", "param", "source", "track",
    "wbr", "embed", "keygen", "command",
];

/// Extracts the Glyf identifier and closing-tag position from a slice of HTML tokens.
///
/// Each token in `tags` is a `<tag ...>text` string as produced by the HTML
/// tokeniser (`TAG_REGEX`). Returns:
/// - The Glyf identifier string (e.g. `"div.foo#main"`) with attributes already
///   sorted in compress order (class → id → props → text).
/// - `Some(index)` — the index in `tags` of the matching closing tag.
/// - `None` — the element is self-closing (void element or `/>` suffix).
///
/// # Errors
/// Returns [`GlyfError::NoIdentifier`] when `tags` is empty, the tag name
/// cannot be extracted, or a non-self-closing element has no matching close.
pub(super) fn get_identifier_from_html(
    tags: &[&str],
) -> Result<(String, Option<usize>), GlyfError> {
    let Some(&first_tag) = tags.first() else {
        return Err(GlyfError::NoIdentifier);
    };

    let Some(tagname) = first_tag.split(['>', ' ', '<']).nth(1) else {
        return Err(GlyfError::NoIdentifier);
    };

    let mut attributes = ATTRIBUTE_REGEX
        .find_iter(&first_tag[tagname.len() + 1..])
        .map(|t| {
            let str = t.as_str();
            let mut parts = str.splitn(2, '=');
            let identifier = parts.next().unwrap();
            let Some(value) = parts.next() else {
                if str.starts_with('>') {
                    return format!(">{}", &str);
                }
                return format!(":{}", str);
            };
            match identifier {
                "class" | "className" => {
                    let clean = value.replace(['"', '\''], "");
                    clean.split_whitespace().map(|c| format!(".{c}")).collect()
                }
                "id" => format!("#{}", value.replace(['"', '\''], "")),
                _ => {
                    let mut cleaned = value.replace(['"', '\''], "");
                    if cleaned.contains([':', '.', '>']) {
                        cleaned = format!("{{{}}}", cleaned);
                    }
                    format!(":{}={}", identifier, cleaned)
                }
            }
        })
        .collect::<Vec<String>>();

    attributes.sort_by_key(|k| match k.chars().next() {
        Some('.') => 0, // class
        Some('#') => 1, // id
        Some(':') => 2, // props
        Some('>') => 3, // text content
        _ => 4,
    });

    let is_self_closing = first_tag.ends_with("/>") || VOID_ELEMENTS.contains(&tagname);

    let mut closing_tag_index = None;
    if !is_self_closing {
        let mut depth = 0;
        let open_prefix = format!("<{}", tagname);
        let close_prefix = format!("</{}", tagname);
        for (i, tag) in tags[1..].iter().enumerate() {
            if tag.split(['>', ' ']).next() == Some(&open_prefix) {
                depth += 1;
                continue;
            }
            if tag.split(['>', ' ']).next() == Some(&close_prefix) {
                if depth > 0 {
                    depth -= 1;
                    continue;
                }
                closing_tag_index = Some(i + 1);
                break;
            }
        }
    }

    if !is_self_closing && closing_tag_index.is_none() {
        return Err(GlyfError::NoIdentifier);
    }

    Ok((
        format!("{}{}", tagname, &attributes.join("")),
        closing_tag_index,
    ))
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
            let e = Element::new(Some("div".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div");
        }

        #[test]
        fn element_with_class() {
            let e = Element::new(Some("div.foo".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div.foo");
        }

        #[test]
        fn element_with_id() {
            let e = Element::new(Some("div#main".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div#main");
        }

        #[test]
        fn element_with_prop() {
            let e = Element::new(Some("a:href=url".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "a:href=url");
        }

        #[test]
        fn element_with_text_content() {
            let e = Element::new(Some("p>>Hello".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "p>>Hello");
        }

        #[test]
        fn self_closing_element() {
            let e = Element::new(Some("br/".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "br/");
        }

        #[test]
        fn element_with_multiplier() {
            let e = Element::new(Some("li".into()), None, 3, None, None, &cfg()).unwrap();
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
            let e = Element::new(Some("div#main.foo".into()), None, 1, None, None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div.foo#main");
        }

        #[test]
        fn chained_children() {
            let e = parse_input("div>p>span", None, &cfg()).unwrap();
            assert_eq!(e.to_glyf(), "div>p>span");
        }
    }

    // -------------------------------------------------------------------------
    // get_identifier_from_html
    // -------------------------------------------------------------------------
    mod get_identifier_from_html_tests {
        use super::*;

        #[test]
        fn simple_tag_returns_tagname() {
            let tags = vec!["<div>", "</div>"];
            let (id, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "div");
            assert_eq!(close, Some(1));
        }

        #[test]
        fn tag_with_class() {
            let tags = vec!["<div class=\"foo\">", "</div>"];
            let (id, _) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "div.foo");
        }

        #[test]
        fn tag_with_id() {
            let tags = vec!["<div id=\"main\">", "</div>"];
            let (id, _) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "div#main");
        }

        #[test]
        fn tag_with_prop() {
            let tags = vec!["<a href=\"url\">", "</a>"];
            let (id, _) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "a:href=url");
        }

        #[test]
        fn tag_with_multiple_classes() {
            let tags = vec!["<div class=\"foo bar\">", "</div>"];
            let (id, _) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "div.foo.bar");
        }

        #[test]
        fn tag_with_text_content() {
            // TAG_REGEX produces "<p>Hello" when text follows the tag immediately
            let tags = vec!["<p>Hello", "</p>"];
            let (id, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "p>>Hello");
            assert_eq!(close, Some(1));
        }

        #[test]
        fn self_closing_explicit() {
            let tags = vec!["<br />"];
            let (id, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "br");
            assert_eq!(close, None);
        }

        #[test]
        fn void_element_without_slash() {
            let tags = vec!["<br>"];
            let (id, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "br");
            assert_eq!(close, None);
        }

        #[test]
        fn closing_tag_index_accounts_for_children() {
            // <div><p></p></div>
            let tags = vec!["<div>", "<p>", "</p>", "</div>"];
            let (_, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(close, Some(3));
        }

        #[test]
        fn nested_same_name_depth_tracking() {
            // <div><div></div></div> — inner </div> must not close the outer
            let tags = vec!["<div>", "<div>", "</div>", "</div>"];
            let (_, close) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(close, Some(3));
        }

        #[test]
        fn attributes_sorted_class_before_id() {
            // Compress order: class(0) < id(1)
            let tags = vec!["<div id=\"main\" class=\"card\">", "</div>"];
            let (id, _) = get_identifier_from_html(&tags).unwrap();
            assert_eq!(id, "div.card#main");
        }

        #[test]
        fn empty_tags_returns_err() {
            assert!(get_identifier_from_html(&[]).is_err());
        }
    }
}
