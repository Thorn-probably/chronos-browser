use crate::semantic::registry::{ElementRegistry, InteractiveElement};

/// Capitalizes the first letter of a role string for clean presentation.
fn format_role(role: &str) -> String {
    let mut chars = role.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut capitalized = first.to_uppercase().to_string();
            capitalized.push_str(chars.as_str());
            capitalized
        }
    }
}

/// Formats a single interactive element into a concise token-efficient line:
/// e.g. `[1] Button "Submit"`
/// or `[2] Input "Search" placeholder="Type here..."`
/// or `[3] Checkbox "Remember me" checked=false`
/// or `[4] Contenteditable "Chat input" value="Hello"`
pub fn format_element(el: &InteractiveElement) -> String {
    let role = format_role(&el.role);
    let mut line = format!("[{}] {}", el.id, role);

    if !el.name.is_empty() {
        let clean_name = el.name.replace('\n', " ").replace('"', "\\\"");
        line.push_str(&format!(" \"{}\"", clean_name));
    }

    if let Some(ref val) = el.value {
        if !val.is_empty() {
            let clean_val = val.replace('\n', " ").replace('"', "\\\"");
            line.push_str(&format!(" value=\"{}\"", clean_val));
        }
    }

    if let Some(ref placeholder) = el.placeholder {
        // Show placeholder only if value is absent or empty
        let has_value = el.value.as_ref().map(|v| !v.is_empty()).unwrap_or(false);
        if !has_value && !placeholder.is_empty() {
            let clean_ph = placeholder.replace('\n', " ").replace('"', "\\\"");
            line.push_str(&format!(" placeholder=\"{}\"", clean_ph));
        }
    }

    if let Some(checked) = el.checked {
        line.push_str(&format!(" checked={}", checked));
    }

    if el.disabled {
        line.push_str(" [disabled]");
    }

    if !el.in_viewport {
        line.push_str(" [offscreen]");
    }

    line
}

/// Formats the complete semantic snapshot of a page into a clean, token-efficient representation:
/// - If an active modal is open, it highlights `=== ACTIVE DIALOG: <title> ===` at the top with its interactive elements.
/// - Lists remaining page elements under `Page Elements:` (or `Interactive Elements:` if no modal).
pub fn format_snapshot(registry: &ElementRegistry) -> String {
    let mut output = String::new();

    // 1. Page Header
    let title = registry.title().trim();
    let url = registry.url().trim();
    if !title.is_empty() && !url.is_empty() {
        output.push_str(&format!("Page: {} - {}\n\n", title, url));
    } else if !title.is_empty() {
        output.push_str(&format!("Page: {}\n\n", title));
    } else if !url.is_empty() {
        output.push_str(&format!("Page: {}\n\n", url));
    }

    let modal = registry.active_modal();
    let modal_elements = registry.modal_elements();
    let page_elements = registry.page_elements();

    // 2. Active Dialog Section (if any)
    if let Some(modal_info) = modal {
        let title = if modal_info.title.is_empty() {
            "Dialog"
        } else {
            &modal_info.title
        };
        output.push_str(&format!("=== ACTIVE DIALOG: {} ===\n", title));

        if modal_elements.is_empty() {
            output.push_str("(No interactive elements in dialog)\n");
        } else {
            for el in &modal_elements {
                output.push_str(&format_element(el));
                output.push('\n');
            }
        }
        output.push('\n');
    }

    // 3. Data Tables Section (if any)
    if !registry.tables().is_empty() {
        output.push_str("Tables & Data:\n");
        for table in registry.tables() {
            output.push_str(table);
            output.push_str("\n\n");
        }
    }

    // 4. Page Elements Section
    if modal.is_some() {
        if !page_elements.is_empty() {
            output.push_str("Page Elements:\n");
            for el in &page_elements {
                output.push_str(&format_element(el));
                output.push('\n');
            }
        }
    } else {
        if page_elements.is_empty() {
            output.push_str("(No interactive elements detected)\n");
        } else {
            output.push_str("Interactive Elements:\n");
            for el in &page_elements {
                output.push_str(&format_element(el));
                output.push('\n');
            }
        }
    }

    output.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic::registry::{ActionType, ElementRegistry, ModalInfo, RawElement, RawSnapshot};

    #[test]
    fn test_format_snapshot_without_modal() {
        let raw = RawSnapshot {
            title: "Example Domain".to_string(),
            url: "https://example.com".to_string(),
            modal: None,
            elements: vec![
                RawElement {
                    tag: "input".to_string(),
                    role: "searchbox".to_string(),
                    name: "Search".to_string(),
                    value: None,
                    placeholder: Some("Query...".to_string()),
                    checked: None,
                    disabled: false,
                    center: (100.0, 50.0),
                    bounds: None,
                    in_modal: false,
                    in_viewport: true,
                    action_type: ActionType::Input,
                },
                RawElement {
                    tag: "button".to_string(),
                    role: "button".to_string(),
                    name: "Search".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (200.0, 50.0),
                    bounds: None,
                    in_modal: false,
                    in_viewport: true,
                    action_type: ActionType::Click,
                },
                RawElement {
                    tag: "input".to_string(),
                    role: "checkbox".to_string(),
                    name: "Exact match".to_string(),
                    value: None,
                    placeholder: None,
                    checked: Some(false),
                    disabled: false,
                    center: (100.0, 80.0),
                    bounds: None,
                    in_modal: false,
                    in_viewport: true,
                    action_type: ActionType::Toggle,
                },
            ],
            tables: Vec::new(),
        };

        let reg = ElementRegistry::from_raw(raw);
        let formatted = format_snapshot(&reg);

        assert!(formatted.contains("Page: Example Domain - https://example.com"));
        assert!(formatted.contains("Interactive Elements:"));
        assert!(formatted.contains("[1] Searchbox \"Search\" placeholder=\"Query...\""));
        assert!(formatted.contains("[2] Button \"Search\""));
        assert!(formatted.contains("[3] Checkbox \"Exact match\" checked=false"));
    }

    #[test]
    fn test_format_snapshot_with_active_modal() {
        let raw = RawSnapshot {
            title: "App Dashboard".to_string(),
            url: "https://app.example.com".to_string(),
            modal: Some(ModalInfo {
                title: "Confirm Delete".to_string(),
                role: "dialog".to_string(),
                bounds: None,
            }),
            elements: vec![
                RawElement {
                    tag: "button".to_string(),
                    role: "button".to_string(),
                    name: "Delete Forever".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (250.0, 300.0),
                    bounds: None,
                    in_modal: true,
                    in_viewport: true,
                    action_type: ActionType::Click,
                },
                RawElement {
                    tag: "button".to_string(),
                    role: "button".to_string(),
                    name: "Cancel".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (150.0, 300.0),
                    bounds: None,
                    in_modal: true,
                    in_viewport: true,
                    action_type: ActionType::Click,
                },
                RawElement {
                    tag: "a".to_string(),
                    role: "link".to_string(),
                    name: "Settings".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (20.0, 10.0),
                    bounds: None,
                    in_modal: false,
                    in_viewport: true,
                    action_type: ActionType::Link,
                },
            ],
            tables: Vec::new(),
        };

        let reg = ElementRegistry::from_raw(raw);
        let formatted = format_snapshot(&reg);

        assert!(formatted.contains("=== ACTIVE DIALOG: Confirm Delete ==="));
        assert!(formatted.contains("[1] Button \"Delete Forever\""));
        assert!(formatted.contains("[2] Button \"Cancel\""));
        assert!(formatted.contains("Page Elements:"));
        assert!(formatted.contains("[3] Link \"Settings\""));
    }
}
