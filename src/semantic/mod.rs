pub mod formatter;
pub mod registry;
pub mod waiter;

pub use formatter::{format_element, format_snapshot};
pub use registry::{
    ActionType, BoundingBox, ElementRegistry, InteractiveElement, ModalInfo, RawElement,
    RawSnapshot,
};
pub use waiter::{
    generate_settle_script, generate_wait_for_selector_script, Waiter, WaiterConfig, DOM_SETTLE_JS,
};

/// The in-page JavaScript detector script evaluated via Chrome DevTools Protocol (CDP).
pub const DETECTOR_JS: &str = include_str!("detector.js");

/// Evaluates `detector.js` against the page target, constructs the `ElementRegistry`,
/// and returns the registry and the formatted textual snapshot.
pub async fn take_semantic_snapshot(
    target: &crate::cdp::PageTarget,
) -> crate::error::Result<(ElementRegistry, String)> {
    let val = target.evaluate(DETECTOR_JS).await?;
    let registry = ElementRegistry::from_value(val).map_err(|e| {
        crate::error::ChronosError::ExecutionFailed(format!(
            "Failed to parse semantic detector output: {}",
            e
        ))
    })?;
    let formatted = format_snapshot(&registry);
    Ok((registry, formatted))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detector_js_loaded() {
        assert!(!DETECTOR_JS.is_empty());
        assert!(DETECTOR_JS.contains("Chronos Browser Semantic Detector"));
        assert!(DETECTOR_JS.contains("detectActiveModal"));
        assert!(DETECTOR_JS.contains("contenteditable"));
    }
}
