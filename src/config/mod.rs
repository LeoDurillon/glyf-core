use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq, Clone)]
pub enum ParserMode {
    HTML,
    /// JSX mode. The inner value overrides the class attribute name.
    /// `None` defaults to `"className"` (standard React).
    /// Use `Some("class")` for frameworks like Qwik.
    JSX(Option<String>),
}

impl ParserMode {
    pub fn is_jsx(&self) -> bool {
        matches!(self, ParserMode::JSX(_))
    }
}

#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Config {
    pub mode: ParserMode,
    pub snippets: HashMap<String, String>,
}

// ---------------------------------------------------------------------------
// Config implementation
// ---------------------------------------------------------------------------

impl Config {
    pub fn new(mode: ParserMode, snippets: HashMap<String, String>) -> Self {
        Self { mode, snippets }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            mode: ParserMode::HTML,
            snippets: HashMap::new(),
        }
    }
}
