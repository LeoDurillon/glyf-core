//! Attribute parsing for Glyf abbreviations.
//!
//! Handles the five attribute syntaxes supported by the parser:
//!
//! | Syntax | Type | Example | Output |
//! |--------|------|---------|--------|
//! | `.name` | Class | `div.flex` | `class="flex"` |
//! | `#value` | Id | `div#app` | `id="app"` |
//! | `#{expr}` | JSX dynamic id | `div#{myId}` | `id={myId}` |
//! | `:key=value` | Props | `a:href=url` | `href="url"` |
//! | `>>text` | Text content | `p>>Hello` | inner content `Hello` |

use std::sync::LazyLock;

use regex::Regex;

use crate::config::ParserMode;

pub trait Render {
    fn render(&self, mode: ParserMode) -> String;
    fn to_glyf(&self) -> String;
}

static ATTRIBUTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(:\{.+?\}|:[\w$-]+=\{.+?\}|:[\w$-]+=[^:>+]+|:[\w$-]+|\.[\w\/-]+|#\{.+?\}|#[\w-]+|>>.+$)",
    )
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
    Id(String),
    /// `:key` or `:key=value` — rendered as `key` or `key="value"` / `key={value}`.
    Props(String, Option<String>),
    /// `.name` — all classes are aggregated into a single `class="a b c"` attribute.
    Class(String),
    /// `>>text` — placed as inner content between opening and closing tags.
    Text(String),
}

impl Render for AttributeType {
    fn render(&self, mode: ParserMode) -> String {
        match self {
            AttributeType::Id(id) => {
                if mode == ParserMode::JSX && id.starts_with('{') {
                    format!(" id={}", id)
                } else {
                    format!(" id=\"{}\"", id)
                }
            }
            AttributeType::Class(class) => {
                if mode == ParserMode::HTML {
                    format!(" class=\"{}\"", class)
                } else {
                    format!(" className=\"{}\"", class)
                }
            }
            AttributeType::Props(identifier, value) => {
                if let Some(value) = value.as_deref() {
                    let formatted = if mode == ParserMode::HTML {
                        let stripped = if value.starts_with('{') && value.ends_with('}') {
                            &value[1..value.len() - 1]
                        } else {
                            value
                        };
                        &format!("\"{}\"", stripped)
                    } else {
                        value
                    };

                    format!(" {}={}", identifier, formatted)
                } else {
                    format!(" {}", identifier)
                }
            }
            AttributeType::Text(text) => text.to_string(),
        }
    }

    /// Converts this attribute to its Glyf abbreviation notation.
    ///
    /// | Type | Example HTML | Glyf output |
    /// |------|-------------|-------------|
    /// | `Class` | `class="foo"` | `.foo` |
    /// | `Id` | `id="main"` | `#main` |
    /// | `Props` with value | `href="url"` | `:href=url` |
    /// | `Props` boolean | `disabled` | `:disabled` |
    /// | `Text` | text content `Hello` | `>>Hello` |
    fn to_glyf(&self) -> String {
        match self {
            AttributeType::Class(class) => {
                format!(".{}", class)
            }
            AttributeType::Text(text) => {
                format!(">>{}", text)
            }
            AttributeType::Id(id) => {
                format!("#{}", id)
            }
            AttributeType::Props(identifier, value) => {
                if let Some(value) = value {
                    format!(":{}={}", identifier, value)
                } else {
                    format!(":{}", identifier)
                }
            }
        }
    }
}

/// Parses the attribute portion of an Glyf element string into a list of [`AttributeType`]s.
///
/// `attributes` is the raw string **after** the identifier has been stripped,
/// e.g. for `div.foo#bar:disabled` this receives `.foo#bar:disabled`.
///
/// Prop values are handled in two ways:
/// - `{expr}` — kept as-is in JSX mode; wrapped in `"quotes"` in HTML mode
/// - `plain`  — always wrapped in `"quotes"`
pub fn parse_attribute(attributes: &str) -> Vec<AttributeType> {
    let matcher = ATTRIBUTE_REGEX.find_iter(attributes);
    let mut attributes: Vec<AttributeType> = Vec::new();
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
                attributes.push(AttributeType::Props(identifier.into(), value));
            }
            Some('.') => {
                let class = &element[1..];
                attributes.push(AttributeType::Class(class.into()));
            }
            Some('#') => {
                let id = &element[1..];
                attributes.push(AttributeType::Id(id.into()));
            }
            Some('>') => {
                let text = &element[2..];
                attributes.push(AttributeType::Text(text.into()));
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
            assert_eq!(attrs[0], AttributeType::Class("foo".into()));
        }

        #[test]
        fn parses_multiple_classes() {
            let attrs = parse_attribute(".flex.items-center.text-lg");
            assert_eq!(attrs.len(), 3);
            assert_eq!(attrs[0], AttributeType::Class("flex".into()));
            assert_eq!(attrs[1], AttributeType::Class("items-center".into()));
            assert_eq!(attrs[2], AttributeType::Class("text-lg".into()));
        }

        #[test]
        fn parses_id() {
            let attrs = parse_attribute("#my-id");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Id("my-id".into()));
        }

        #[test]
        fn parses_prop_without_value() {
            let attrs = parse_attribute(":disabled");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Props("disabled".into(), None));
        }

        #[test]
        fn parses_prop_with_simple_value() {
            let attrs = parse_attribute(":type=text");
            assert_eq!(attrs.len(), 1);
            assert_eq!(
                attrs[0],
                AttributeType::Props("type".into(), Some("text".into()))
            );
        }

        #[test]
        fn parses_prop_with_braced_value() {
            let attrs = parse_attribute(":onClick={handler}");
            assert_eq!(attrs.len(), 1);
            assert_eq!(
                attrs[0],
                AttributeType::Props("onClick".into(), Some("{handler}".into()))
            );
        }

        #[test]
        fn two_braced_props_are_separate() {
            // non-greedy +? must not merge :a={x}:b={y} into one match
            let attrs = parse_attribute(":a={x}:b={y}");
            assert_eq!(attrs.len(), 2);
            assert_eq!(
                attrs[0],
                AttributeType::Props("a".into(), Some("{x}".into()))
            );
            assert_eq!(
                attrs[1],
                AttributeType::Props("b".into(), Some("{y}".into()))
            );
        }

        #[test]
        fn parses_spread_syntax() {
            let attrs = parse_attribute(":{...props}");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Props("{...props}".into(), None));
        }

        #[test]
        fn parses_text_content() {
            let attrs = parse_attribute(">>Hello World");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Text("Hello World".into()));
        }

        #[test]
        fn parses_mixed_attributes_in_order() {
            // .class first, then #id, then :prop
            let attrs = parse_attribute(".card#sidebar:aria-label=nav");
            assert_eq!(attrs.len(), 3);
            assert_eq!(attrs[0], AttributeType::Class("card".into()));
            assert_eq!(attrs[1], AttributeType::Id("sidebar".into()));
            assert_eq!(
                attrs[2],
                AttributeType::Props("aria-label".into(), Some("nav".into()))
            );
        }
    }

    // -------------------------------------------------------------------------
    // Display for Attribute
    // -------------------------------------------------------------------------
    mod attribute_display_tests {
        use super::*;

        #[test]
        fn id_renders_with_value() {
            let a = AttributeType::Id("main".into());
            assert_eq!(a.render(ParserMode::HTML), " id=\"main\"");
        }

        #[test]
        fn id_renders_empty_string_when_value_is_none() {
            let a = AttributeType::Id("".into());
            assert_eq!(a.render(ParserMode::HTML), " id=\"\"");
        }

        #[test]
        fn class_renders_with_leading_space() {
            let a = AttributeType::Class("flex".into());
            assert_eq!(a.render(ParserMode::HTML), " class=\"flex\"");
        }

        #[test]
        fn props_plain_value_is_quoted() {
            let a = AttributeType::Props("href".into(), Some("https://example.com".into()));
            assert_eq!(a.render(ParserMode::HTML), " href=\"https://example.com\"");
        }

        #[test]
        fn props_braced_value_is_not_quoted() {
            let a = AttributeType::Props("onClick".into(), Some("{handler}".into()));
            assert_eq!(a.render(ParserMode::JSX), " onClick={handler}");
        }

        #[test]
        fn props_no_value_renders_as_boolean_attribute() {
            let a = AttributeType::Props("disabled".into(), None);
            assert_eq!(a.render(ParserMode::HTML), " disabled");
        }

        #[test]
        fn text_renders_content_without_prefix() {
            let a = AttributeType::Text("Hello World".into());
            assert_eq!(a.render(ParserMode::HTML), "Hello World");
        }
    }

    // -------------------------------------------------------------------------
    // Attribute::to_glyf
    // -------------------------------------------------------------------------
    mod attribute_to_glyf_tests {
        use super::*;

        #[test]
        fn class_produces_dot_prefix() {
            let a = AttributeType::Class("foo".into());
            assert_eq!(a.to_glyf(), ".foo");
        }

        #[test]
        fn id_produces_hash_with_value() {
            let a = AttributeType::Id("main".into());
            assert_eq!(a.to_glyf(), "#main");
        }

        #[test]
        fn props_with_value_produces_colon_eq() {
            let a = AttributeType::Props("href".into(), Some("url".into()));
            assert_eq!(a.to_glyf(), ":href=url");
        }

        #[test]
        fn props_boolean_produces_colon_only() {
            let a = AttributeType::Props("disabled".into(), None);
            assert_eq!(a.to_glyf(), ":disabled");
        }

        #[test]
        fn text_produces_double_gt() {
            let a = AttributeType::Text("Hello".into());
            assert_eq!(a.to_glyf(), ">>Hello");
        }

        #[test]
        fn props_with_braced_value_preserved() {
            // JSX expression values are kept as-is
            let a = AttributeType::Props("onClick".into(), Some("{handler}".into()));
            assert_eq!(a.to_glyf(), ":onClick={handler}");
        }
    }
}
