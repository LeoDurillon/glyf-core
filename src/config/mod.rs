use std::{
    collections::HashMap,
    ops::Deref,
    sync::{OnceLock, RwLock, RwLockReadGuard},
};

#[cfg(test)]
use std::cell::RefCell;

static CONFIG: OnceLock<RwLock<Config>> = OnceLock::new();

#[cfg(test)]
thread_local! {
    // In test builds each thread gets its own config override so parallel
    // tests never interfere with each other.
    static THREAD_CONFIG: RefCell<Option<Config>> = RefCell::new(None);
}

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

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

/// Unified reference to the active [`Config`].
///
/// Returned by [`Config::get`]. Implements [`Deref<Target = Config>`] so
/// call sites are identical whether the config comes from the global
/// [`RwLock`] or from a per-test thread-local.
pub enum ConfigRef {
    /// The process-wide config protected by a [`RwLock`].
    Global(RwLockReadGuard<'static, Config>),
    /// An owned clone taken from the thread-local override (test builds only).
    #[cfg(test)]
    Test(Config),
}

impl Deref for ConfigRef {
    type Target = Config;

    fn deref(&self) -> &Config {
        match self {
            ConfigRef::Global(guard) => guard,
            #[cfg(test)]
            ConfigRef::Test(config) => config,
        }
    }
}

/// RAII guard that clears the thread-local config when dropped.
///
/// Returned by [`Config::for_test`]. The thread-local is reset even if the
/// test panics, so no config leaks to subsequent tests on the same thread.
#[cfg(test)]
pub struct TestConfigGuard;

#[cfg(test)]
impl Drop for TestConfigGuard {
    fn drop(&mut self) {
        THREAD_CONFIG.with(|c| *c.borrow_mut() = None);
    }
}

// ---------------------------------------------------------------------------
// Config implementation
// ---------------------------------------------------------------------------

impl Config {
    /// Initialises or replaces the **global** config.
    ///
    /// Safe to call multiple times — subsequent calls replace the previous
    /// value via the inner [`RwLock`]. Prefer `Config::for_test` inside
    /// unit tests to avoid shared-state races between parallel test threads.
    pub fn init(mode: ParserMode, snippets: HashMap<String, String>) {
        let lock = CONFIG.get_or_init(|| RwLock::new(Config::default()));
        *lock.write().unwrap() = Self { mode, snippets };
    }

    /// Sets an isolated config for the **current test thread only**.
    ///
    /// Returns a [`TestConfigGuard`] that resets the thread-local config when
    /// dropped — even if the test panics. Because each test thread gets its
    /// own slot, parallel tests never see each other's config.
    ///
    /// The guard should be bound to a named variable (conventionally `_guard`)
    /// so it lives for the entire test body:
    ///
    /// ```ignore
    /// let _guard = Config::for_test(
    ///     ParserMode::HTML,
    ///     HashMap::from([("btn".to_string(), "MyButton".to_string())]),
    /// );
    /// // thread-local config active here
    /// // _guard dropped → config cleared
    /// ```
    #[cfg(test)]
    pub fn for_test(mode: ParserMode, snippets: HashMap<String, String>) -> TestConfigGuard {
        THREAD_CONFIG.with(|c| {
            *c.borrow_mut() = Some(Config { mode, snippets });
        });
        TestConfigGuard
    }

    /// Returns a reference to the active config.
    ///
    /// **Resolution order:**
    /// 1. Thread-local override (test builds only) — set via `Config::for_test`.
    /// 2. Global config — set via [`Config::init`], defaulting to
    ///    [`Config::default`] if `init` was never called.
    ///
    /// The returned [`ConfigRef`] implements [`Deref<Target = Config>`].
    pub fn get() -> ConfigRef {
        #[cfg(test)]
        {
            let thread_config = THREAD_CONFIG.with(|c| c.borrow().clone());
            if let Some(config) = thread_config {
                return ConfigRef::Test(config);
            }
        }

        ConfigRef::Global(
            CONFIG
                .get_or_init(|| RwLock::new(Config::default()))
                .read()
                .unwrap(),
        )
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
