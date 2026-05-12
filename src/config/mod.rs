use std::collections::HashMap;

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum ParserMode {
    HTML,
    JSX,
}

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

    pub fn mode(&self) -> ParserMode {
        self.mode
    }

    pub fn snippets(&self) -> &HashMap<String, String> {
        &self.snippets
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
