use std::sync::LazyLock;

use regex::Regex;

use crate::{
    config::Config,
    parser::{Element, GlyfError, Node, NodeType},
};

fn extract_tags_from_html(html: &str) -> Vec<&str> {
    TAG_REGEX.find_iter(html).map(|t| t.as_str()).collect()
}

static TAG_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(<[^<]*)").unwrap());

static ATTRIBUTE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-zA-Z-]+=(?:\{.+?\}|["'].+?["'])|[a-zA-Z]+|>.+$)"#).unwrap());

const VOID_ELEMENTS: &[&str] = &[
    "br", "hr", "img", "input", "col", "area", "base", "link", "meta", "param", "source", "track",
    "wbr", "embed", "keygen", "command",
];

pub fn parse_html(html: &str, level: Option<usize>, config: &Config) -> Result<Element, GlyfError> {
    let cleaned = html.replace("\n", "").replace("\t", "").trim().to_string();
    if cleaned.is_empty() {
        return Err(GlyfError::NoIdentifier);
    }
    let tags = extract_tags_from_html(&cleaned);

    let (identifier, closing_tag_index) = get_identifier_from_html(&tags)?;

    let close = closing_tag_index.map_or(1, |c| c + 1);
    let siblings = if close < tags.len() {
        &tags[close..]
    } else {
        &[]
    };
    let sibling = if !siblings.is_empty() {
        Some(parse_html(&siblings.join(""), level, config)?)
    } else {
        None
    };

    // Self-closing: return immediately with optional sibling
    if closing_tag_index.is_none() {
        return Element::new(
            Some(format!("{}/", identifier)),
            None,
            1,
            sibling.map(|s| {
                Box::new(Node {
                    node: s,
                    node_type: NodeType::Sibling,
                })
            }),
            None,
            config,
        );
    }

    let mut children = None;
    if let Some(close_idx) = closing_tag_index {
        let inner = &tags[1..close_idx];
        if let Ok(e) = parse_html(
            &inner.join(""),
            level.map_or(Some(1), |l| Some(l + 1)),
            config,
        ) {
            children = Some(e);
        }
    }

    // When both children and a sibling exist, wrap in a group so the
    // sibling attaches at the right level: (element>children)+sibling
    let mut group = None;
    if sibling.is_some() && children.is_some() {
        group = Some(Box::new(Element::new(
            Some(identifier.clone()),
            None,
            1,
            Some(Box::new(Node {
                node: children.clone().unwrap(),
                node_type: NodeType::Children,
            })),
            level,
            config,
        )?));
    }

    Element::new(
        if group.is_some() {
            None
        } else {
            Some(identifier)
        },
        group,
        1,
        sibling.map_or(
            children.map(|c| {
                Box::new(Node {
                    node: c,
                    node_type: NodeType::Children,
                })
            }),
            |s| {
                Some(Box::new(Node {
                    node: s,
                    node_type: NodeType::Sibling,
                }))
            },
        ),
        level,
        config,
    )
}

/// Converts an HTML/JSX string directly to a Glyf abbreviation without
/// building an [`Element`] tree.
///
/// This is the fast path used by [`crate::compress`]. Children are connected
/// with `>`, siblings with `+`, and an element that has both children and a
/// sibling is wrapped in `(...)` so the sibling attaches at the correct level.
///
/// # Errors
/// Returns [`GlyfError::NoIdentifier`] if `html` is empty or contains no
/// valid HTML element.
pub fn html_to_glyf(html: &str) -> Result<String, GlyfError> {
    let cleaned = html.replace("\n", "").replace("\t", "").trim().to_string();
    if cleaned.is_empty() {
        return Err(GlyfError::NoIdentifier);
    }
    let tags = extract_tags_from_html(&cleaned);
    let (mut identifier, closing_tag_index) = get_identifier_from_html(&tags)?;

    let is_self_closing = closing_tag_index.is_none();
    if is_self_closing {
        identifier.push('/')
    }

    let close = closing_tag_index.map_or(1, |c| c + 1);
    let siblings = if close < tags.len() {
        &tags[close..]
    } else {
        &[]
    };

    let child = if !is_self_closing {
        let close_idx = closing_tag_index.unwrap();
        let inner = &tags[1..close_idx];
        if !inner.is_empty() {
            html_to_glyf(&inner.join("")).ok()
        } else {
            None
        }
    } else {
        None
    };

    let sibling = if !siblings.is_empty() {
        Some(html_to_glyf(&siblings.join(""))?)
    } else {
        None
    };

    Ok(match (child, sibling) {
        (Some(c), Some(s)) => format!("({}>{})+{}", identifier, c, s),
        (Some(c), None) => format!("{}>{}", identifier, c),
        (None, Some(s)) => format!("{}+{}", identifier, s),
        (None, None) => identifier,
    })
}

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
    // -------------------------------------------------------------------------
    // parse_html
    // -------------------------------------------------------------------------
    #[cfg(test)]
    mod parse_html_tests {
        use crate::config::Config;

        use super::super::parse_html;

        fn cfg() -> Config {
            Config::default()
        }

        /// Parse HTML and immediately return the rendered string.
        fn compress(html: &str) -> String {
            parse_html(html, None, &cfg())
                .expect("parse_html failed")
                .to_string()
        }

        // ── simple elements ─────────────────────────────────────────────────

        #[test]
        fn simple_element_round_trips() {
            assert_eq!(compress("<div></div>"), "<div></div>");
        }

        #[test]
        fn element_with_single_class() {
            assert_eq!(
                compress("<div class=\"foo\"></div>"),
                "<div class=\"foo\"></div>"
            );
        }

        #[test]
        fn element_with_multiple_classes() {
            // "foo bar" → .foo.bar → class="foo bar" preserved
            assert_eq!(
                compress("<div class=\"bar foo\"></div>"),
                "<div class=\"bar foo\"></div>"
            );
        }

        #[test]
        fn element_with_id() {
            assert_eq!(compress("<div id=\"app\"></div>"), "<div id=\"app\"></div>");
        }

        #[test]
        fn element_with_prop() {
            assert_eq!(
                compress("<a href=\"https://example.com\"></a>"),
                "<a href=\"https://example.com\"></a>"
            );
        }

        #[test]
        fn element_with_text_content() {
            assert_eq!(compress("<p>Hello world</p>"), "<p>Hello world</p>");
        }

        // ── attribute ordering ───────────────────────────────────────────────
        // Glyf renders: Id < Props < Class regardless of HTML input order.

        #[test]
        fn id_and_class_both_preserved() {
            let html = compress("<div id=\"main\" class=\"card\"></div>");
            assert!(html.contains("id=\"main\""), "id missing from output");
            assert!(html.contains("class=\"card\""), "class missing from output");
        }

        #[test]
        fn id_renders_before_class_regardless_of_html_order() {
            // Glyf attribute ordering: Id(0) < Props(1) < Class(2)
            let html = compress("<div class=\"card\" id=\"main\"></div>");
            assert!(html.find("id").unwrap() < html.find("class").unwrap());
        }

        // ── self-closing ────────────────────────────────────────────────────

        #[test]
        fn explicit_self_closing() {
            assert_eq!(compress("<br />"), "<br />");
        }

        #[test]
        fn void_element_without_slash() {
            // HTML4-style void element treated as self-closing
            assert_eq!(compress("<br>"), "<br />");
        }

        #[test]
        fn self_closing_with_attribute() {
            assert_eq!(
                compress("<img src=\"photo.jpg\" />"),
                "<img src=\"photo.jpg\" />"
            );
        }

        #[test]
        fn self_closing_with_sibling() {
            assert_eq!(compress("<br /><span></span>"), "<br />\n<span></span>");
        }

        // ── siblings ───────────────────────────────────────────────────────

        #[test]
        fn two_sibling_elements() {
            assert_eq!(
                compress("<div></div><span></span>"),
                "<div></div>\n<span></span>"
            );
        }

        #[test]
        fn three_sibling_elements() {
            assert_eq!(
                compress("<h1></h1><p></p><footer></footer>"),
                "<h1></h1>\n<p></p>\n<footer></footer>"
            );
        }

        // ── children ───────────────────────────────────────────────────────

        #[test]
        fn element_with_single_child() {
            assert_eq!(compress("<div><p></p></div>"), "<div>\n\t<p></p>\n</div>");
        }

        #[test]
        fn deeply_nested_children() {
            assert_eq!(
                compress("<div><p><span></span></p></div>"),
                "<div>\n\t<p>\n\t\t<span></span>\n\t</p>\n</div>"
            );
        }

        #[test]
        fn child_with_class() {
            assert_eq!(
                compress("<div><p class=\"title\"></p></div>"),
                "<div>\n\t<p class=\"title\"></p>\n</div>"
            );
        }

        #[test]
        fn child_with_text_content() {
            assert_eq!(
                compress("<div><p>Hello</p></div>"),
                "<div>\n\t<p>Hello</p>\n</div>"
            );
        }

        // ── children + siblings ─────────────────────────────────────────────

        #[test]
        fn element_with_child_and_sibling() {
            // <div><p></p></div><span></span>  →  (div>p)+span
            assert_eq!(
                compress("<div><p></p></div><span></span>"),
                "<div>\n\t<p></p>\n</div>\n<span></span>"
            );
        }

        // ── errors ───────────────────────────────────────────────────────

        #[test]
        fn empty_input_returns_err() {
            assert!(parse_html("", None, &cfg()).is_err());
        }

        #[test]
        fn whitespace_only_returns_err() {
            assert!(parse_html("  ", None, &cfg()).is_err());
        }
    }

    // -------------------------------------------------------------------------
    // html_to_glyf
    // -------------------------------------------------------------------------
    mod html_to_glyf_tests {
        use super::super::html_to_glyf;

        #[test]
        fn simple_element() {
            assert_eq!(html_to_glyf("<div></div>").unwrap(), "div");
        }

        #[test]
        fn element_with_class() {
            assert_eq!(
                html_to_glyf("<div class=\"foo\"></div>").unwrap(),
                "div.foo"
            );
        }

        #[test]
        fn element_with_id() {
            assert_eq!(html_to_glyf("<div id=\"main\"></div>").unwrap(), "div#main");
        }

        #[test]
        fn element_with_prop() {
            assert_eq!(html_to_glyf("<a href=\"url\"></a>").unwrap(), "a:href=url");
        }

        #[test]
        fn element_with_text_content() {
            assert_eq!(html_to_glyf("<p>Hello</p>").unwrap(), "p>>Hello");
        }

        #[test]
        fn element_with_multiple_classes() {
            assert_eq!(
                html_to_glyf("<div class=\"foo bar\"></div>").unwrap(),
                "div.foo.bar"
            );
        }

        #[test]
        fn self_closing_explicit() {
            assert_eq!(html_to_glyf("<br />").unwrap(), "br/");
        }

        #[test]
        fn void_element() {
            assert_eq!(html_to_glyf("<br>").unwrap(), "br/");
        }

        #[test]
        fn self_closing_with_sibling() {
            assert_eq!(html_to_glyf("<br /><span></span>").unwrap(), "br/+span");
        }

        #[test]
        fn element_with_child() {
            assert_eq!(html_to_glyf("<div><p></p></div>").unwrap(), "div>p");
        }

        #[test]
        fn element_with_siblings() {
            assert_eq!(
                html_to_glyf("<div></div><span></span>").unwrap(),
                "div+span"
            );
        }

        #[test]
        fn element_with_child_and_sibling() {
            assert_eq!(
                html_to_glyf("<div><p></p></div><span></span>").unwrap(),
                "(div>p)+span"
            );
        }

        #[test]
        fn deeply_nested() {
            assert_eq!(
                html_to_glyf("<div><p><span></span></p></div>").unwrap(),
                "div>p>span"
            );
        }

        #[test]
        fn url_prop_colon_wrapped_in_braces() {
            // colons in values are escaped with {} to survive Glyf attribute parsing
            assert_eq!(
                html_to_glyf("<a href=\"https://example.com\"></a>").unwrap(),
                "a:href={https://example.com}"
            );
        }

        #[test]
        fn empty_input_returns_err() {
            assert!(html_to_glyf("").is_err());
        }
    }

    // -------------------------------------------------------------------------
    // get_identifier_from_html
    // -------------------------------------------------------------------------
    mod get_identifier_from_html_tests {
        use super::super::get_identifier_from_html;

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
