//! `glyf-core` — A compact abbreviation expander for HTML and JSX.
//!
//! Write a brief symbolic abbreviation and get back a full, indented
//! HTML or JSX structure:
//!
//! ```
//! use glyf_core::expand;
//!
//! assert_eq!(
//!     expand("ul>li.item*2", None, None).unwrap(),
//!     "<ul>\n\t<li class=\"item\"></li>\n\t<li class=\"item\"></li>\n</ul>"
//! );
//! ```
//!
//! The library also works in reverse — given HTML markup it produces the
//! shortest Glyf abbreviation that would regenerate equivalent output:
//!
//! ```
//! use glyf_core::compress;
//!
//! assert_eq!(compress("<ul><li class=\"item\"></li></ul>").unwrap(), "ul>li.item");
//! ```
//!
//! The full syntax reference lives in the [`parser`] module documentation.
//!
//! ## Entry points
//!
//! | Direction | String output | Element tree |
//! |---|---|---|
//! | Abbreviation → HTML/JSX | [`expand`] | [`expand_to_tree`] |
//! | HTML/JSX → Glyf | [`compress`] | [`compress_to_tree`] |

use crate::{
    config::Config,
    parser::{
        Element, GlyfError,
        html::{html_to_glyf, parse_html},
        parse_input,
        validate::input_correctly_close,
    },
};

pub mod config;
pub mod parser;

/// Expands a Glyf abbreviation into an HTML or JSX string.
///
/// This is the primary entry point for the library.  The abbreviation is
/// validated for balanced brackets, parsed into a [`parser::Element`] tree,
/// and rendered to an indented string ready for editor insertion.
///
/// For direct access to the parsed [`Element`] tree instead of a string, see
/// [`expand_to_tree`].
///
/// # Arguments
///
/// - `abbr` — The abbreviation to expand (e.g. `"ul>li.item*3"`).
///   See the [`parser`] module documentation for the full syntax reference.
/// - `base_level` — Indentation depth of the root element.
///   `None` and `Some(0)` both produce unindented root output.
///   `Some(n)` prefixes every root-level element with `n` tabs, useful
///   when embedding the expansion inside an already-indented block.
/// - `config` — Optional [`Config`] that sets the parser mode and provides
///   user-defined snippet aliases. Pass `None` to use [`Config::default`]
///   (HTML mode, empty snippet table).
///
/// # Errors
///
/// - [`GlyfError::UnmatchedBrackets`] — the abbreviation contains
///   unclosed parentheses (e.g. `"div(unclosed"`).
/// - [`GlyfError::NoIdentifier`] — the abbreviation produces no valid
///   tag name (e.g. a bare `">"`).
///
/// # Examples
///
/// Basic expansion:
///
/// ```
/// use glyf_core::expand;
///
/// assert_eq!(expand("div", None, None).unwrap(), "<div></div>");
/// assert_eq!(
///     expand("ul>li", None, None).unwrap(),
///     "<ul>\n\t<li></li>\n</ul>"
/// );
/// ```
///
/// At an indented level — useful when the LSP inserts text inside
/// an already-indented block:
///
/// ```
/// use glyf_core::expand;
///
/// assert_eq!(expand("p", Some(1), None).unwrap(), "\n\t<p></p>");
/// ```
///
/// With a custom snippet alias:
///
/// ```
/// use std::collections::HashMap;
/// use glyf_core::expand;
/// use glyf_core::config::{ParserMode,Config};
///
///
/// let snippets = HashMap::from([
///     ("btn".to_string(), "MyButton".to_string()),
/// ]);
/// let config = Config::new(ParserMode::HTML, snippets);
///
/// assert_eq!(
///     expand("btn", None, Some(config)).unwrap(),
///     "<MyButton></MyButton>"
/// );
/// ```
///
/// Error on unmatched brackets:
///
/// ```
/// use glyf_core::{expand, parser::GlyfError};
///
/// assert!(matches!(
///     expand("div(unclosed", None, None),
///     Err(GlyfError::UnmatchedBrackets)
/// ));
/// ```
pub fn expand(
    abbr: &str,
    base_level: Option<usize>,
    config: Option<Config>,
) -> Result<String, GlyfError> {
    let config = config.unwrap_or_default();

    if !input_correctly_close(abbr) {
        return Err(GlyfError::UnmatchedBrackets);
    }

    parse_input(abbr, base_level, &config).map(|node| node.to_string())
}

/// Expands a Glyf abbreviation into an [`Element`] AST.
///
/// The tree-returning counterpart of [`expand`]: same validation and parsing
/// pipeline, but hands you the [`Element`] directly instead of rendering it
/// to a string. Use this when you need to inspect or transform the structure
/// before (or instead of) producing HTML output.
///
/// Call `.to_string()` on the returned element to render it to HTML/JSX, or
/// [`Element::to_glyf`] to serialise it back to a Glyf abbreviation.
///
/// # Arguments
///
/// - `abbr` — The abbreviation to expand (e.g. `"ul>li.item*3"`).
///   See the [`parser`] module documentation for the full syntax reference.
/// - `base_level` — Indentation depth of the root element.
///   `None` and `Some(0)` both produce unindented root output.
/// - `config` — Optional [`Config`] for parser mode and custom snippets.
///   Pass `None` to use [`Config::default`] (HTML mode, empty snippet table).
///
/// # Errors
///
/// - [`GlyfError::UnmatchedBrackets`] — unclosed parentheses in the abbreviation.
/// - [`GlyfError::NoIdentifier`] — the abbreviation produces no valid tag name.
///
/// # Examples
///
/// Inspecting a parsed element:
///
/// ```
/// use glyf_core::expand_to_tree;
///
/// let el = expand_to_tree("div.foo", None, None).unwrap();
/// assert_eq!(el.identifier.as_deref(), Some("div"));
/// assert!(!el.self_closing);
/// ```
///
/// Traversing a child node:
///
/// ```
/// use glyf_core::expand_to_tree;
/// use glyf_core::parser::NodeType;
///
/// let el = expand_to_tree("ul>li", None, None).unwrap();
/// assert_eq!(el.identifier.as_deref(), Some("ul"));
/// let child = el.node.unwrap();
/// assert_eq!(child.node_type, NodeType::Children);
/// assert_eq!(child.node.identifier.as_deref(), Some("li"));
/// ```
///
/// Inspecting parsed attributes:
///
/// ```
/// use glyf_core::expand_to_tree;
/// use glyf_core::parser::attribute::AttributeType;
///
/// let el = expand_to_tree("div.card#main", None, None).unwrap();
/// let attrs = el.attributes.unwrap();
/// assert!(attrs.iter().any(|a| matches!(a, AttributeType::Class(c) if c == "card")));
/// assert!(attrs.iter().any(|a| matches!(a, AttributeType::Id(i) if i == "main")));
/// ```
///
/// Rendering back to HTML via `Display`:
///
/// ```
/// use glyf_core::expand_to_tree;
///
/// let el = expand_to_tree("div>p", None, None).unwrap();
/// assert_eq!(el.to_string(), "<div>\n\t<p></p>\n</div>");
/// ```
///
/// Error on unmatched brackets:
///
/// ```
/// use glyf_core::{expand_to_tree, parser::GlyfError};
///
/// assert!(matches!(
///     expand_to_tree("div(unclosed", None, None),
///     Err(GlyfError::UnmatchedBrackets)
/// ));
/// ```
pub fn expand_to_tree(
    abbr: &str,
    base_level: Option<usize>,
    config: Option<Config>,
) -> Result<Element, GlyfError> {
    let config = config.unwrap_or_default();

    if !input_correctly_close(abbr) {
        return Err(GlyfError::UnmatchedBrackets);
    }

    parse_input(abbr, base_level, &config)
}

/// Compresses an HTML or JSX string into its Glyf abbreviation.
///
/// This is the inverse of [`expand`]: given HTML markup, it produces the
/// shortest Glyf abbreviation that would regenerate equivalent output.
///
/// # Errors
///
/// Returns [`GlyfError::NoIdentifier`] if `html` is empty or contains
/// no valid HTML element.
///
/// # Examples
///
/// ```
/// use glyf_core::compress;
///
/// assert_eq!(compress("<div></div>").unwrap(), "div");
/// assert_eq!(compress("<div class=\"foo\"></div>").unwrap(), "div.foo");
/// assert_eq!(compress("<div><p></p></div>").unwrap(), "div>p");
/// assert_eq!(compress("<div></div><span></span>").unwrap(), "div+span");
/// assert_eq!(compress("<div><p></p></div><span></span>").unwrap(), "(div>p)+span");
/// ```
pub fn compress(html: &str) -> Result<String, GlyfError> {
    html_to_glyf(html)
}

/// Parses an HTML or JSX string into an [`Element`] AST.
///
/// Where [`compress`] converts HTML directly to a Glyf abbreviation string,
/// `compress_to_tree` gives you the parsed [`Element`] tree so you can
/// inspect or transform it programmatically before rendering.
///
/// Call `.to_string()` on the returned element to render it back to HTML, or
/// call [`Element::to_glyf`] to get its Glyf abbreviation.
///
/// # Arguments
///
/// - `html` — The HTML markup to parse.
/// - `config` — Optional [`Config`] that sets the parser mode. Pass `None`
///   for the default (HTML mode). The mode does not affect parsing but
///   controls how the returned elements render via `Display` (e.g. `class`
///   vs `className` in JSX mode).
///
/// # Errors
///
/// Returns [`GlyfError::NoIdentifier`] if `html` is empty or contains no
/// recognisable HTML element.
///
/// # Examples
///
/// Inspecting a simple element:
///
/// ```
/// use glyf_core::compress_to_tree;
///
/// let el = compress_to_tree("<div></div>", None).unwrap();
/// assert_eq!(el.identifier.as_deref(), Some("div"));
/// assert!(!el.self_closing);
/// assert!(el.node.is_none());
/// ```
///
/// Self-closing element:
///
/// ```
/// use glyf_core::compress_to_tree;
///
/// let el = compress_to_tree("<br />", None).unwrap();
/// assert_eq!(el.identifier.as_deref(), Some("br"));
/// assert!(el.self_closing);
/// ```
///
/// Traversing a child node:
///
/// ```
/// use glyf_core::compress_to_tree;
/// use glyf_core::parser::NodeType;
///
/// let el = compress_to_tree("<ul><li></li></ul>", None).unwrap();
/// assert_eq!(el.identifier.as_deref(), Some("ul"));
/// let child = el.node.unwrap();
/// assert_eq!(child.node_type, NodeType::Children);
/// assert_eq!(child.node.identifier.as_deref(), Some("li"));
/// ```
///
/// Inspecting parsed attributes:
///
/// ```
/// use glyf_core::compress_to_tree;
/// use glyf_core::parser::attribute::AttributeType;
///
/// let el = compress_to_tree("<div class=\"card\" id=\"main\"></div>", None).unwrap();
/// let attrs = el.attributes.unwrap();
/// assert!(attrs.iter().any(|a| matches!(a, AttributeType::Class(c) if c == "card")));
/// assert!(attrs.iter().any(|a| matches!(a, AttributeType::Id(i) if i == "main")));
/// ```
///
/// The tree renders back to HTML via `Display`:
///
/// ```
/// use glyf_core::compress_to_tree;
///
/// let el = compress_to_tree("<section><h1></h1><p></p></section>", None).unwrap();
/// assert_eq!(
///     el.to_string(),
///     "<section>\n\t<h1></h1>\n\t<p></p>\n</section>"
/// );
/// ```
///
/// Error on empty input:
///
/// ```
/// use glyf_core::compress_to_tree;
///
/// assert!(compress_to_tree("", None).is_err());
/// ```
pub fn compress_to_tree(html: &str, config: Option<Config>) -> Result<Element, GlyfError> {
    parse_html(html, None, &config.unwrap_or_default())
}

#[cfg(test)]
mod compress_tests {
    use super::*;

    #[test]
    fn simple_element() {
        assert_eq!(compress("<div></div>").unwrap(), "div");
    }

    #[test]
    fn element_with_class() {
        assert_eq!(compress("<div class=\"foo\"></div>").unwrap(), "div.foo");
    }

    #[test]
    fn element_with_id() {
        assert_eq!(compress("<div id=\"main\"></div>").unwrap(), "div#main");
    }

    #[test]
    fn element_with_child() {
        assert_eq!(compress("<div><p></p></div>").unwrap(), "div>p");
    }

    #[test]
    fn element_with_siblings() {
        assert_eq!(compress("<div></div><span></span>").unwrap(), "div+span");
    }

    #[test]
    fn element_with_child_and_sibling() {
        assert_eq!(
            compress("<div><p></p></div><span></span>").unwrap(),
            "(div>p)+span"
        );
    }

    #[test]
    fn self_closing_explicit() {
        assert_eq!(compress("<br />").unwrap(), "br/");
    }

    #[test]
    fn void_element() {
        assert_eq!(compress("<br>").unwrap(), "br/");
    }

    #[test]
    fn empty_input_returns_err() {
        assert!(compress("").is_err());
    }
}
