//! Attribute parsing for Glyf abbreviations.
//!
//! Handles the four attribute syntaxes supported by the parser:
//!
//! | Syntax | Type | Example | Output |
//! |--------|------|---------|--------|
//! | `.name` | Class | `div.flex` | `class="flex"` |
//! | `#value` | Id | `div#app` | `id="app"` |
//! | `:key=value` | Props | `a:href=url` | `href="url"` |
//! | `<text` | Text | `p<Hello` | `>Hello` (inner content) |

use std::{fmt::Display, sync::LazyLock};

use regex::Regex;

use crate::config::{Config, ParserMode};

static ATTRIBUTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(:\{.+?\}|:[\w$-]+=\{.+?\}|:[\w$-]+=[^:<]+|:[\w$-]+|\.[\w\/-]+|#[\w-]+|<.+$)")
        .unwrap()
});

/// Classifies how a parsed attribute maps to its HTML/JSX output.
///
/// The ordering of variants is meaningful: `sort_by` on `AttributeType`
/// places `Id` before `Props` before `Class` before `Text`, which ensures
/// a predictable attribute output order regardless of abbreviation order.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AttributeType {
    /// `#value` — rendered as `id="value"`.
    Id,
    /// `:key` or `:key=value` — rendered as `key` or `key="value"` / `key={value}`.
    Props,
    /// `.name` — all classes are aggregated into a single `class="a b c"` attribute.
    Class,
    /// `<text` — placed as inner content between opening and closing tags.
    Text,
}

/// A single parsed attribute from an Glyf abbreviation.
#[derive(Debug, Clone)]
pub struct Attribute {
    /// For `Id`: the id string. For `Props`: the key name. For `Class`: the class name. For `Text`: the content.
    pub identifier: String,
    /// The attribute value, if present. Only used by `Id` (the id string) and `Props` (`:key=value`).
    pub value: Option<String>,
    pub attribute_type: AttributeType,
}

impl Attribute {
    pub fn new(identifier: String, value: Option<String>, attribute_type: AttributeType) -> Self {
        Self {
            identifier,
            value,
            attribute_type,
        }
    }
}

impl Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mode = &Config::get().mode;

        match self.attribute_type {
            AttributeType::Id => write!(f, " id=\"{}\"", self.value.as_deref().unwrap_or("")),
            AttributeType::Class => {
                if mode == &ParserMode::HTML {
                    write!(f, " class=\"{}\"", self.identifier)
                } else {
                    write!(f, " className=\"{}\"", self.identifier)
                }
            }
            AttributeType::Props => {
                if let Some(value) = self.value.as_deref() {
                    let formatted = if !value.starts_with("{") || mode == &ParserMode::HTML {
                        format!("\"{}\"", value)
                    } else {
                        value.to_string()
                    };

                    write!(f, " {}={}", self.identifier, formatted)
                } else {
                    write!(f, " {}", self.identifier)
                }
            }
            AttributeType::Text => write!(f, "{}", self.identifier),
        }
    }
}

/// Parses the attribute portion of an Glyf element string into a list of [`Attribute`]s.
///
/// `attributes` is the raw string **after** the identifier has been stripped,
/// e.g. for `div.foo#bar:disabled` this receives `.foo#bar:disabled`.
///
/// Prop values are handled in two ways:
/// - `{expr}` — kept as-is (JSX expression, no quotes added)
/// - `plain`  — wrapped in `"quotes"` (standard HTML attribute)
pub fn parse_attribute(attributes: &str) -> Vec<Attribute> {
    let matcher = ATTRIBUTE_REGEX.find_iter(attributes);
    let mut attributes: Vec<Attribute> = Vec::new();
    for capture in matcher.into_iter() {
        let element = capture.as_str();
        match element.chars().next() {
            Some(':') => {
                let parts: Vec<&str> = element[1..].splitn(2, '=').collect();
                let identifier = parts[0];
                let value = if parts.len() > 1 {
                    Some(parts[1].to_string())
                } else {
                    None
                };
                attributes.push(Attribute::new(
                    identifier.to_string(),
                    value,
                    AttributeType::Props,
                ));
            }
            Some('.') => {
                let class = &element[1..];
                attributes.push(Attribute::new(
                    class.to_string(),
                    None,
                    AttributeType::Class,
                ));
            }
            Some('#') => {
                let id = &element[1..];
                attributes.push(Attribute::new(
                    "id".to_string(),
                    Some(id.to_string()),
                    AttributeType::Id,
                ));
            }
            Some('<') => {
                let text = &element[1..];
                attributes.push(Attribute::new(text.to_string(), None, AttributeType::Text));
            }
            _ => {}
        }
    }

    attributes
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // parse_attribute
    // -------------------------------------------------------------------------
    mod parse_attribute_tests {
        use super::*;

        #[test]
        fn empty_string_returns_no_attributes() {
            assert!(parse_attribute("").is_empty());
        }

        #[test]
        fn parses_single_class() {
            let attrs = parse_attribute(".foo");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "foo");
            assert!(attrs[0].value.is_none());
            assert!(matches!(attrs[0].attribute_type, AttributeType::Class));
        }

        #[test]
        fn parses_multiple_classes() {
            let attrs = parse_attribute(".flex.items-center.text-lg");
            assert_eq!(attrs.len(), 3);
            assert_eq!(attrs[0].identifier, "flex");
            assert_eq!(attrs[1].identifier, "items-center");
            assert_eq!(attrs[2].identifier, "text-lg");
            assert!(
                attrs
                    .iter()
                    .all(|a| matches!(a.attribute_type, AttributeType::Class))
            );
        }

        #[test]
        fn parses_id() {
            let attrs = parse_attribute("#my-id");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "id");
            assert_eq!(attrs[0].value, Some("my-id".to_string()));
            assert!(matches!(attrs[0].attribute_type, AttributeType::Id));
        }

        #[test]
        fn parses_prop_without_value() {
            let attrs = parse_attribute(":disabled");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "disabled");
            assert!(attrs[0].value.is_none());
            assert!(matches!(attrs[0].attribute_type, AttributeType::Props));
        }

        #[test]
        fn parses_prop_with_simple_value() {
            let attrs = parse_attribute(":type=text");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "type");
            assert_eq!(attrs[0].value.as_deref(), Some("text"));
            assert!(matches!(attrs[0].attribute_type, AttributeType::Props));
        }

        #[test]
        fn parses_prop_with_braced_value() {
            let attrs = parse_attribute(":onClick={handler}");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "onClick");
            assert_eq!(attrs[0].value.as_deref(), Some("{handler}"));
        }

        #[test]
        fn two_braced_props_are_separate() {
            // non-greedy +? must not merge :a={x}:b={y} into one match
            let attrs = parse_attribute(":a={x}:b={y}");
            assert_eq!(attrs.len(), 2);
            assert_eq!(attrs[0].identifier, "a");
            assert_eq!(attrs[0].value.as_deref(), Some("{x}"));
            assert_eq!(attrs[1].identifier, "b");
            assert_eq!(attrs[1].value.as_deref(), Some("{y}"));
        }

        #[test]
        fn parses_spread_syntax() {
            let attrs = parse_attribute(":{...props}");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "{...props}");
            assert!(attrs[0].value.is_none());
            assert!(matches!(attrs[0].attribute_type, AttributeType::Props));
        }

        #[test]
        fn parses_text_content() {
            let attrs = parse_attribute("<Hello World");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0].identifier, "Hello World");
            assert!(matches!(attrs[0].attribute_type, AttributeType::Text));
        }

        #[test]
        fn parses_mixed_attributes_in_order() {
            // .class first, then #id, then :prop
            let attrs = parse_attribute(".card#sidebar:aria-label=nav");
            assert_eq!(attrs.len(), 3);
            assert!(matches!(attrs[0].attribute_type, AttributeType::Class));
            assert!(matches!(attrs[1].attribute_type, AttributeType::Id));
            assert!(matches!(attrs[2].attribute_type, AttributeType::Props));
        }
    }

    // -------------------------------------------------------------------------
    // Display for Attribute
    // -------------------------------------------------------------------------
    mod attribute_display_tests {
        use super::*;

        #[test]
        fn id_renders_with_value() {
            let a = Attribute::new("id".into(), Some("main".into()), AttributeType::Id);
            assert_eq!(a.to_string(), " id=\"main\"");
        }

        #[test]
        fn id_renders_empty_string_when_value_is_none() {
            let a = Attribute::new("id".into(), None, AttributeType::Id);
            assert_eq!(a.to_string(), " id=\"\"");
        }

        #[test]
        fn class_renders_with_leading_space() {
            let a = Attribute::new("flex".into(), None, AttributeType::Class);
            assert_eq!(a.to_string(), " class=\"flex\"");
        }

        #[test]
        fn props_plain_value_is_quoted() {
            let a = Attribute::new(
                "href".into(),
                Some("https://example.com".into()),
                AttributeType::Props,
            );
            assert_eq!(a.to_string(), " href=\"https://example.com\"");
        }

        #[test]
        fn props_braced_value_is_not_quoted() {
            let _guard = Config::for_test(ParserMode::JSX, Default::default());
            let a = Attribute::new(
                "onClick".into(),
                Some("{handler}".into()),
                AttributeType::Props,
            );
            assert_eq!(a.to_string(), " onClick={handler}");
        }

        #[test]
        fn props_no_value_renders_as_boolean_attribute() {
            let a = Attribute::new("disabled".into(), None, AttributeType::Props);
            assert_eq!(a.to_string(), " disabled");
        }

        #[test]
        fn text_renders_content_without_prefix() {
            let a = Attribute::new("Hello World".into(), None, AttributeType::Text);
            assert_eq!(a.to_string(), "Hello World");
        }
    }
}
