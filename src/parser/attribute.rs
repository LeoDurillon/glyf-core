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

use std::{collections::HashMap, sync::LazyLock};

use regex::Regex;

use crate::config::ParserMode;

static ATTRIBUTE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(:\{.+?\}|:[\w$-]+=\{.+?\}|:[\w$-]+=[^:>+]*|:[\w$-]+|\.[\w\/-]+|#\{.+?\}|#[\w-]+|>>.+$)",
    )
    .unwrap()
});

// Regex check for escaped string
// it's a string that contains only word space or subset of glyf identifier
// that need to be escaped for it to be parsed successfully
//
// e.g url in href need to be escaped
static ESCAPE_STRING_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\{[\w\s.:><\/-]+\}$").unwrap());

/// Classifies how a parsed attribute maps to its HTML/JSX output.
///
/// The ordering of variants is meaningful: `sort_by` on `AttributeType`
/// places `Class` before `Id` before `Props` before `Text`, which ensures
/// a predictable attribute output order regardless of abbreviation order.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone)]
pub enum AttributeType {
    /// `.name` — all classes are aggregated into a single `class="a b c"` attribute.
    Class(String),
    /// `#value` — rendered as `id="value"`.
    Id(String),
    /// `:key` or `:key=value` — rendered as `key` or `key="value"` / `key={value}`.
    Props(String, Option<String>),
    /// `>>text` — placed as inner content between opening and closing tags.
    Text(String),
}

impl AttributeType {
    pub fn render(&self, mode: &ParserMode) -> String {
        match self {
            AttributeType::Id(id) => match (mode, id.starts_with('{') && id.ends_with('}')) {
                (ParserMode::JSX(_), true) => format!(" id={}", id),
                (ParserMode::HTML, true) => format!(" id=\"{}\"", &id[1..id.len() - 1]),
                _ => format!(" id=\"{}\"", id),
            },
            AttributeType::Class(class) => match mode {
                ParserMode::HTML => format!(" class=\"{}\"", class),
                ParserMode::JSX(value) => {
                    format!(" {}=\"{}\"", value.as_deref().unwrap_or("className"), class)
                }
            },
            AttributeType::Props(identifier, value) => {
                if let Some(value) = value.as_deref() {
                    let formatted = match (mode, value.starts_with('{') && value.ends_with('}')) {
                        (ParserMode::HTML, true) => {
                            let stripped = &value[1..value.len() - 1];
                            format!("\"{}\"", stripped)
                        }
                        (_, false) => format!("\"{}\"", value),
                        (ParserMode::JSX(_), true) => {
                            // If attribute is encapsulated in {}
                            // and contain only letters/number and at least one space or glyf identifier char
                            // then it's probably an escaped string literal
                            if ESCAPE_STRING_REGEX.is_match(value)
                                && value.contains([' ', ':', '>'])
                            {
                                let stripped = &value[1..value.len() - 1];
                                format!("\"{}\"", stripped)
                            } else {
                                value.to_string()
                            }
                        }
                    };

                    format!(" {}={}", identifier, formatted)
                } else {
                    format!(" {}", identifier)
                }
            }
            // Trim surrounding whitespace that may come from formatted HTML source.
            AttributeType::Text(text) => text.trim().to_string(),
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
    pub fn to_glyf(&self) -> String {
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
    let mut prop_pos: HashMap<String, usize> = HashMap::new();
    let mut id_pos: Option<usize> = None;
    let mut result: Vec<AttributeType> = Vec::new();

    ATTRIBUTE_REGEX.find_iter(attributes).for_each(|cap| {
        let s = cap.as_str();
        let attribute = match s.chars().next() {
            Some(':') => {
                let mut parts = s[1..].splitn(2, '=');
                let key = parts.next();
                if key.is_none_or(|k| k.is_empty()) {
                    return;
                }
                Some(AttributeType::Props(
                    key.unwrap().into(),
                    parts.next().map(str::to_owned),
                ))
            }
            Some('.') => Some(AttributeType::Class(s[1..].into())),
            Some('#') => Some(AttributeType::Id(s[1..].into())),
            Some('>') => Some(AttributeType::Text(s[2..].into())),
            _ => None,
        };

        // clean duplicate logic
        match attribute {
            Some(AttributeType::Props(name, value)) => {
                if let Some(&pos) = prop_pos.get(&name) {
                    result[pos] = AttributeType::Props(name, value);
                } else {
                    prop_pos.insert(name.clone(), result.len());
                    result.push(AttributeType::Props(name, value));
                }
            }
            Some(AttributeType::Id(value)) => {
                if let Some(pos) = id_pos {
                    result[pos] = AttributeType::Id(value);
                } else {
                    id_pos = Some(result.len());
                    result.push(AttributeType::Id(value));
                }
            }
            Some(attr) => result.push(attr),
            _ => {}
        }
    });

    result
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

        #[test]
        fn duplicate_prop_last_value_wins() {
            // snippet produces ":href", user suffix appends ":href=abc"
            // position of :href is preserved, value updated
            let attrs = parse_attribute(":href:href=abc");
            assert_eq!(attrs.len(), 1);
            assert_eq!(
                attrs[0],
                AttributeType::Props("href".into(), Some("abc".into()))
            );
        }

        #[test]
        fn duplicate_prop_with_all_snippet_attrs() {
            // simulates img:src:alt:src=foo coming out of parse_snippet
            let attrs = parse_attribute(":src:alt:src=foo");
            assert_eq!(attrs.len(), 2);
            assert_eq!(
                attrs[0],
                AttributeType::Props("src".into(), Some("foo".into()))
            );
            assert_eq!(attrs[1], AttributeType::Props("alt".into(), None));
        }

        #[test]
        fn non_prop_attrs_are_never_deduplicated() {
            // two identical classes are both kept (valid HTML: just redundant)
            let attrs = parse_attribute(".foo.foo");
            assert_eq!(attrs.len(), 2);
        }

        #[test]
        fn duplicate_id_last_value_wins() {
            let attrs = parse_attribute("#first#second");
            assert_eq!(attrs.len(), 1);
            assert_eq!(attrs[0], AttributeType::Id("second".into()));
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
            assert_eq!(a.render(&ParserMode::HTML), " id=\"main\"");
        }

        #[test]
        fn id_renders_empty_string_when_value_is_none() {
            let a = AttributeType::Id("".into());
            assert_eq!(a.render(&ParserMode::HTML), " id=\"\"");
        }

        #[test]
        fn class_renders_with_leading_space() {
            let a = AttributeType::Class("flex".into());
            assert_eq!(a.render(&ParserMode::HTML), " class=\"flex\"");
        }

        #[test]
        fn props_plain_value_is_quoted() {
            let a = AttributeType::Props("href".into(), Some("https://example.com".into()));
            assert_eq!(a.render(&ParserMode::HTML), " href=\"https://example.com\"");
        }

        #[test]
        fn props_braced_value_is_not_quoted() {
            let a = AttributeType::Props("onClick".into(), Some("{handler}".into()));
            assert_eq!(a.render(&ParserMode::JSX(None)), " onClick={handler}");
        }

        #[test]
        fn props_no_value_renders_as_boolean_attribute() {
            let a = AttributeType::Props("disabled".into(), None);
            assert_eq!(a.render(&ParserMode::HTML), " disabled");
        }

        #[test]
        fn text_renders_content_without_prefix() {
            let a = AttributeType::Text("Hello World".into());
            assert_eq!(a.render(&ParserMode::HTML), "Hello World");
        }
        #[test]
        fn render_escaped_string_correctly() {
            let a = AttributeType::Props("title".into(), Some("{Hello World}".into()));
            assert_eq!(a.render(&ParserMode::JSX(None)), " title=\"Hello World\"");
            let b =
                AttributeType::Props("href".into(), Some("{https://example-domain.com}".into()));
            assert_eq!(
                b.render(&ParserMode::JSX(None)),
                " href=\"https://example-domain.com\""
            );
        }

        #[test]
        fn shouldnt_capture_function_as_escaped_string() {
            let a = AttributeType::Props(
                "onClick".into(),
                Some("{()=>{setState(old=>old+1)}}".into()),
            );
            assert_eq!(
                a.render(&ParserMode::JSX(None)),
                " onClick={()=>{setState(old=>old+1)}}"
            );
        }

        #[test]
        fn class_renders_as_classname_in_jsx_by_default() {
            let a = AttributeType::Class("foo".into());
            assert_eq!(a.render(&ParserMode::JSX(None)), " className=\"foo\"");
        }

        #[test]
        fn class_renders_with_custom_prop_name() {
            let a = AttributeType::Class("foo".into());
            assert_eq!(
                a.render(&ParserMode::JSX(Some("class".into()))),
                " class=\"foo\""
            );
        }

        #[test]
        fn class_renders_with_explicit_classname_same_as_default() {
            let a = AttributeType::Class("foo".into());
            assert_eq!(
                a.render(&ParserMode::JSX(Some("className".into()))),
                a.render(&ParserMode::JSX(None))
            );
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
