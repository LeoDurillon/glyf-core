use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ParserMode {
    HTML,
    JSX,
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
