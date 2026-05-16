use std::fmt;

/// Errors that can occur while parsing an Glyf abbreviation.
#[derive(Debug, PartialEq, Clone)]
pub enum GlyfError {
    EmptyInput,
    /// The input string contains no valid HTML/JSX tag name.
    ///
    /// Triggered when the input is empty, consists entirely of operators,
    /// or a snippet expands to an empty string with no usable identifier.
    NoIdentifier,

    /// A group opened with `(` was never closed with `)`.
    ///
    /// Example: `(div>p` — the opening parenthesis has no matching `)`.
    UnmatchedBrackets,
    MalformedHtml(String),
}

impl fmt::Display for GlyfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GlyfError::EmptyInput => write!(f, "input is empty"),
            GlyfError::NoIdentifier => write!(f, "no valid tag name found in abbreviation"),
            GlyfError::UnmatchedBrackets => write!(f, "unmatched brackets in abbreviation"),
            GlyfError::MalformedHtml(details) => write!(f, "malformed HTML: {}", details),
        }
    }
}

impl std::error::Error for GlyfError {}
