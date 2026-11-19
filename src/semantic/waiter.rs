use crate::error::{ChronosError, Result};
use std::time::Duration;

/// Default JavaScript snippet for detecting DOM mutation quiescence.
/// It uses a MutationObserver to verify that the DOM has stopped mutating
/// for at least `settle_ms` (default 50ms), with an upper bound `max_timeout_ms` (default 1500ms).
pub const DOM_SETTLE_JS: &str = r#"
(() => new Promise((resolve) => {
    let timeout;
    let observer;
    const settleMs = 50;
    const maxTimeoutMs = 1500;

    const cleanup = () => {
        if (observer) {
            try { observer.disconnect(); } catch (e) {}
        }
        clearTimeout(maxTimer);
        clearTimeout(timeout);
    };

    const done = () => {
        cleanup();
        resolve({ settled: true });
    };

    const resetTimer = () => {
        clearTimeout(timeout);
        timeout = setTimeout(done, settleMs);
    };

    try {
        observer = new MutationObserver(() => {
            resetTimer();
        });

        const root = document.body || document.documentElement;
        if (root) {
            observer.observe(root, {
                childList: true,
                subtree: true,
                attributes: true,
                characterData: true
            });
        }
    } catch (e) {
        done();
        return;
    }

    const maxTimer = setTimeout(done, maxTimeoutMs);
    resetTimer();
}))()
"#;

/// Generates a customized JavaScript snippet for waiting for DOM quiescence.
pub fn generate_settle_script(settle_ms: u64, max_timeout_ms: u64) -> String {
    format!(
        r#"
(() => new Promise((resolve) => {{
    let timeout;
    let observer;
    const settleMs = {settle_ms};
    const maxTimeoutMs = {max_timeout_ms};

    const cleanup = () => {{
        if (observer) {{
            try {{ observer.disconnect(); }} catch (e) {{}}
        }}
        clearTimeout(maxTimer);
        clearTimeout(timeout);
    }};

    const done = () => {{
        cleanup();
        resolve({{ settled: true }});
    }};

    const resetTimer = () => {{
        clearTimeout(timeout);
        timeout = setTimeout(done, settleMs);
    }};

    try {{
        observer = new MutationObserver(() => {{
            resetTimer();
        }});

        const root = document.body || document.documentElement;
        if (root) {{
            observer.observe(root, {{
                childList: true,
                subtree: true,
                attributes: true,
                characterData: true
            }});
        }}
    }} catch (e) {{
        done();
        return;
    }}

    const maxTimer = setTimeout(done, maxTimeoutMs);
    resetTimer();
}}))()
"#
    )
}

/// Generates a JavaScript expression waiting for a specific CSS selector to appear in DOM.
pub fn generate_wait_for_selector_script(selector: &str, timeout_ms: u64) -> String {
    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
    format!(
        r#"
(() => new Promise((resolve, reject) => {{
    const selector = '{escaped}';
    if (document.querySelector(selector)) {{
        resolve(true);
        return;
    }}

    let observer;
    const timer = setTimeout(() => {{
        if (observer) observer.disconnect();
        reject(new Error("Timeout waiting for selector: " + selector));
    }}, {timeout_ms});

    observer = new MutationObserver(() => {{
        if (document.querySelector(selector)) {{
            clearTimeout(timer);
            observer.disconnect();
            resolve(true);
        }}
    }});

    const root = document.body || document.documentElement;
    if (root) {{
        observer.observe(root, {{ childList: true, subtree: true }});
    }} else {{
        clearTimeout(timer);
        reject(new Error("Document body not available"));
    }}
}}))()
"#
    )
}

/// Configuration settings for the DOM settlement waiter.
#[derive(Debug, Clone)]
pub struct WaiterConfig {
    /// Milliseconds of DOM silence required to consider the page settled.
    pub settle_ms: u64,
    /// Maximum milliseconds to wait before resolving anyway.
    pub max_timeout_ms: u64,
}

impl Default for WaiterConfig {
    fn default() -> Self {
        Self {
            settle_ms: 50,
            max_timeout_ms: 1500,
        }
    }
}

/// Smart auto-wait helper that coordinates DOM mutation settle and quiescence checks.
#[derive(Debug, Clone)]
pub struct Waiter {
    config: WaiterConfig,
}

impl Default for Waiter {
    fn default() -> Self {
        Self::new()
    }
}

impl Waiter {
    /// Creates a Waiter with default configuration (50ms quiet window, 1500ms max timeout).
    pub fn new() -> Self {
        Self {
            config: WaiterConfig::default(),
        }
    }

    /// Creates a Waiter with custom configuration.
    pub fn with_config(config: WaiterConfig) -> Self {
        Self { config }
    }

    /// Returns the active configuration.
    pub fn config(&self) -> &WaiterConfig {
        &self.config
    }

    /// Returns the JS script expression configured with the waiter's parameters.
    pub fn script(&self) -> String {
        generate_settle_script(self.config.settle_ms, self.config.max_timeout_ms)
    }

    /// Waits for the active PageTarget to settle DOM mutations.
    pub async fn wait_for_target_settle(&self, target: &crate::cdp::PageTarget) -> Result<bool> {
        let script = self.script();
        let timeout_dur = Duration::from_millis(self.config.max_timeout_ms + 500);

        match tokio::time::timeout(timeout_dur, target.evaluate(&script)).await {
            Ok(Ok(_)) => Ok(true),
            Ok(Err(e)) => Err(e),
            Err(_) => Err(ChronosError::Timeout(format!(
                "DOM quiescence wait exceeded {} ms",
                self.config.max_timeout_ms
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_generation() {
        let script = generate_settle_script(75, 2000);
        assert!(script.contains("const settleMs = 75;"));
        assert!(script.contains("const maxTimeoutMs = 2000;"));
        assert!(script.contains("MutationObserver"));
    }

    #[test]
    fn test_selector_script_generation() {
        let script = generate_wait_for_selector_script("button.confirm-btn", 3000);
        assert!(script.contains("button.confirm-btn"));
        assert!(script.contains("3000"));
    }
}
