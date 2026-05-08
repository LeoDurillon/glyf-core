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

use std::collections::HashMap;

use crate::{
    checker::input_correctly_close,
    parser::{GlyfError, parse_input},
};

pub mod checker;
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
/// - `custom_snippets` — User-defined snippet aliases that extend or
///   shadow the built-in table.  Pass `None` to use built-ins only.
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
///
/// let snippets = HashMap::from([
///     ("btn".to_string(), "MyButton".to_string()),
/// ]);
/// assert_eq!(
///     expand("btn", None, Some(&snippets)).unwrap(),
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
    custom_snippets: Option<&HashMap<String, String>>,
) -> Result<String, GlyfError> {
    if !input_correctly_close(abbr) {
        return Err(GlyfError::UnmatchedBrackets);
    }

    return match parse_input(abbr, base_level, custom_snippets) {
        Ok(node) => Ok(node.to_string()),
        Err(e) => Err(e),
    };
}
