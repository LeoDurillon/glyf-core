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
//! The full syntax reference lives in the [`parser`] module documentation.
//!
//! ## Entry points
//!
//! | Use case | API |
//! |---|---|
//! | Expand abbreviation to string | [`expand`] |
//! | Parse abbreviation to AST | [`parser::parse_input`] |
//! | Validate bracket balance | [`checker::input_correctly_close`] |

use crate::{
    checker::input_correctly_close,
    config::Config,
    parser::{GlyfError, html_to_glyf, parse_input},
};

pub mod checker;
pub mod config;
pub mod parser;

/// Expands a Glyf abbreviation into an HTML or JSX string.
///
/// This is the primary entry point for the library.  The abbreviation is
/// validated for balanced brackets, parsed into a [`parser::Element`] tree,
/// and rendered to an indented string ready for editor insertion.
///
/// For direct access to the parsed AST instead of a string, see
/// [`parser::parse_input`].
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
