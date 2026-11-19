use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Semantic action type supported by an interactive element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionType {
    Click,
    Input,
    Toggle,
    Select,
    Link,
}

impl std::fmt::Display for ActionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Click => write!(f, "click"),
            Self::Input => write!(f, "input"),
            Self::Toggle => write!(f, "toggle"),
            Self::Select => write!(f, "select"),
            Self::Link => write!(f, "link"),
        }
    }
}

/// Bounding rectangle of an element or modal on the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Active modal or overlay context information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModalInfo {
    pub title: String,
    pub role: String,
    #[serde(default)]
    pub bounds: Option<BoundingBox>,
}

/// Raw element structure returned by `detector.js`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawElement {
    pub tag: String,
    pub role: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub checked: Option<bool>,
    #[serde(default)]
    pub disabled: bool,
    pub center: (f64, f64),
    #[serde(default)]
    pub bounds: Option<BoundingBox>,
    #[serde(default)]
    pub in_modal: bool,
    #[serde(default)]
    pub in_viewport: bool,
    pub action_type: ActionType,
}

/// Raw snapshot payload returned by `detector.js`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawSnapshot {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub url: String,
    #[serde(default)]
    pub modal: Option<ModalInfo>,
    #[serde(default)]
    pub elements: Vec<RawElement>,
    #[serde(default)]
    pub tables: Vec<String>,
}

/// Registered interactive element with a compact 1-based numeric ID `[1]`, `[2]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// Compact 1-based reference ID.
    pub id: u32,
    /// HTML tag name (e.g. "button", "input", "a", "div").
    pub tag: String,
    /// Semantic role (e.g. "button", "link", "input", "checkbox", "contenteditable").
    pub role: String,
    /// Accessible label or name.
    pub name: String,
    /// Current input or text value, if applicable.
    pub value: Option<String>,
    /// Placeholder text, if applicable.
    pub placeholder: Option<String>,
    /// Checkbox or radio checked state.
    pub checked: Option<bool>,
    /// Whether the element is disabled.
    pub disabled: bool,
    /// Bounding box center coordinates `(x, y)` for CDP click dispatch.
    pub center: (f64, f64),
    /// Bounding rectangle in viewport coordinates.
    pub bounds: Option<BoundingBox>,
    /// Whether the element belongs to an active modal/dialog.
    pub in_modal: bool,
    /// Whether the element is currently within the visible viewport.
    pub in_viewport: bool,
    /// Primary action type.
    pub action_type: ActionType,
}

/// In-memory registry storing interactive elements mapped by their 1-based numeric reference IDs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElementRegistry {
    title: String,
    url: String,
    modal: Option<ModalInfo>,
    elements: Vec<InteractiveElement>,
    #[serde(default)]
    tables: Vec<String>,
    #[serde(skip)]
    index: HashMap<u32, usize>,
}

impl Default for ElementRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ElementRegistry {
    /// Creates an empty ElementRegistry.
    pub fn new() -> Self {
        Self {
            title: String::new(),
            url: String::new(),
            modal: None,
            elements: Vec::new(),
            tables: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Builds a registry from raw snapshot data returned by `detector.js`.
    /// Elements inside active modals are prioritized first, followed by regular page elements.
    /// Consecutive 1-based IDs `[1]`, `[2]`, ... are assigned to all elements.
    pub fn from_raw(raw: RawSnapshot) -> Self {
        let mut elements = Vec::with_capacity(raw.elements.len());
        let mut index = HashMap::with_capacity(raw.elements.len());

        let has_modal = raw.modal.is_some();
        let (modal_items, page_items): (Vec<RawElement>, Vec<RawElement>) = if has_modal {
            raw.elements.into_iter().partition(|e| e.in_modal)
        } else {
            (Vec::new(), raw.elements)
        };

        let mut current_id = 1u32;

        for raw_el in modal_items.into_iter().chain(page_items.into_iter()) {
            let el = InteractiveElement {
                id: current_id,
                tag: raw_el.tag,
                role: raw_el.role,
                name: raw_el.name,
                value: raw_el.value,
                placeholder: raw_el.placeholder,
                checked: raw_el.checked,
                disabled: raw_el.disabled,
                center: raw_el.center,
                bounds: raw_el.bounds,
                in_modal: raw_el.in_modal,
                in_viewport: raw_el.in_viewport,
                action_type: raw_el.action_type,
            };

            let pos = elements.len();
            elements.push(el);
            index.insert(current_id, pos);
            current_id += 1;
        }

        Self {
            title: raw.title,
            url: raw.url,
            modal: raw.modal,
            elements,
            tables: raw.tables,
            index,
        }
    }

    /// Parses JSON string directly (handles both JSON string payload or raw JSON object).
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        // Handle double-encoded JSON string if present
        let trimmed = json_str.trim();
        if trimmed.starts_with('"') && trimmed.ends_with('"') {
            let inner: String = serde_json::from_str(trimmed)?;
            let raw: RawSnapshot = serde_json::from_str(&inner)?;
            return Ok(Self::from_raw(raw));
        }

        let raw: RawSnapshot = serde_json::from_str(trimmed)?;
        Ok(Self::from_raw(raw))
    }

    /// Parses from a `serde_json::Value`.
    pub fn from_value(value: serde_json::Value) -> Result<Self, serde_json::Error> {
        if let serde_json::Value::String(s) = value {
            return Self::from_json(&s);
        }
        let raw: RawSnapshot = serde_json::from_value(value)?;
        Ok(Self::from_raw(raw))
    }

    /// Rebuilds the internal ID lookup index if needed after deserialization.
    pub fn rebuild_index(&mut self) {
        self.index.clear();
        for (i, el) in self.elements.iter().enumerate() {
            self.index.insert(el.id, i);
        }
    }

    /// Gets an element by its 1-based numeric ref ID.
    pub fn get(&self, id: u32) -> Option<&InteractiveElement> {
        if self.index.is_empty() && !self.elements.is_empty() {
            // Fallback linear scan if index was not populated
            return self.elements.iter().find(|e| e.id == id);
        }
        self.index.get(&id).and_then(|&idx| self.elements.get(idx))
    }

    /// Gets a mutable reference to an element by its 1-based numeric ref ID.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut InteractiveElement> {
        if self.index.is_empty() && !self.elements.is_empty() {
            return self.elements.iter_mut().find(|e| e.id == id);
        }
        if let Some(&idx) = self.index.get(&id) {
            self.elements.get_mut(idx)
        } else {
            None
        }
    }

    /// Returns all registered interactive elements.
    pub fn elements(&self) -> &[InteractiveElement] {
        &self.elements
    }

    /// Returns elements belonging to an active modal dialog.
    pub fn modal_elements(&self) -> Vec<&InteractiveElement> {
        self.elements.iter().filter(|e| e.in_modal).collect()
    }

    /// Returns elements outside modal dialogs.
    pub fn page_elements(&self) -> Vec<&InteractiveElement> {
        self.elements.iter().filter(|e| !e.in_modal).collect()
    }

    /// Returns active modal info, if any.
    pub fn active_modal(&self) -> Option<&ModalInfo> {
        self.modal.as_ref()
    }

    /// Returns extracted data tables on the page.
    pub fn tables(&self) -> &[String] {
        &self.tables
    }

    /// Returns the page title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the page URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Total count of interactive elements.
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Whether registry contains no interactive elements.
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// Finds element matching role (case-insensitive) and name (case-insensitive substring or match).
    pub fn find_by_role_and_name(&self, role: &str, name: &str) -> Option<&InteractiveElement> {
        let role_lower = role.to_lowercase();
        let name_lower = name.to_lowercase();
        self.elements.iter().find(|e| {
            e.role.to_lowercase() == role_lower && e.name.to_lowercase().contains(&name_lower)
        })
    }

    /// Finds element matching accessible name or label (case-insensitive substring).
    pub fn find_by_name(&self, name: &str) -> Option<&InteractiveElement> {
        let name_lower = name.to_lowercase();
        self.elements.iter().find(|e| e.name.to_lowercase().contains(&name_lower))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_ordering_and_lookup() {
        let raw = RawSnapshot {
            title: "Test Page".to_string(),
            url: "https://example.com".to_string(),
            modal: Some(ModalInfo {
                title: "Confirm".to_string(),
                role: "dialog".to_string(),
                bounds: Some(BoundingBox {
                    x: 100.0,
                    y: 100.0,
                    width: 300.0,
                    height: 200.0,
                }),
            }),
            elements: vec![
                RawElement {
                    tag: "button".to_string(),
                    role: "button".to_string(),
                    name: "Page Action".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (50.0, 50.0),
                    bounds: None,
                    in_modal: false,
                    in_viewport: true,
                    action_type: ActionType::Click,
                },
                RawElement {
                    tag: "button".to_string(),
                    role: "button".to_string(),
                    name: "Modal OK".to_string(),
                    value: None,
                    placeholder: None,
                    checked: None,
                    disabled: false,
                    center: (150.0, 150.0),
                    bounds: None,
                    in_modal: true,
                    in_viewport: true,
                    action_type: ActionType::Click,
                },
            ],
            tables: Vec::new(),
        };

        let reg = ElementRegistry::from_raw(raw);
        assert_eq!(reg.len(), 2);

        // Modal element should be assigned [1]
        let el1 = reg.get(1).unwrap();
        assert_eq!(el1.name, "Modal OK");
        assert!(el1.in_modal);
        assert_eq!(el1.center, (150.0, 150.0));

        // Page element should be assigned [2]
        let el2 = reg.get(2).unwrap();
        assert_eq!(el2.name, "Page Action");
        assert!(!el2.in_modal);

        assert_eq!(reg.modal_elements().len(), 1);
        assert_eq!(reg.page_elements().len(), 1);

        assert!(reg.find_by_role_and_name("button", "Modal").is_some());
        assert!(reg.find_by_name("Page").is_some());
        assert!(reg.get(999).is_none());
    }

    #[test]
    fn test_from_json() {
        let json_str = r#"{
            "title": "Login",
            "url": "https://auth.example.com",
            "modal": null,
            "elements": [
                {
                    "tag": "input",
                    "role": "input",
                    "name": "Username",
                    "value": "admin",
                    "placeholder": "Enter username",
                    "checked": null,
                    "disabled": false,
                    "center": [100.0, 200.0],
                    "in_modal": false,
                    "in_viewport": true,
                    "action_type": "input"
                },
                {
                    "tag": "input",
                    "role": "checkbox",
                    "name": "Remember me",
                    "value": null,
                    "placeholder": null,
                    "checked": false,
                    "disabled": false,
                    "center": [100.0, 250.0],
                    "in_modal": false,
                    "in_viewport": true,
                    "action_type": "toggle"
                }
            ]
        }"#;

        let reg = ElementRegistry::from_json(json_str).unwrap();
        assert_eq!(reg.title(), "Login");
        assert_eq!(reg.len(), 2);

        let input_el = reg.get(1).unwrap();
        assert_eq!(input_el.role, "input");
        assert_eq!(input_el.value.as_deref(), Some("admin"));
        assert_eq!(input_el.action_type, ActionType::Input);

        let check_el = reg.get(2).unwrap();
        assert_eq!(check_el.role, "checkbox");
        assert_eq!(check_el.checked, Some(false));
        assert_eq!(check_el.action_type, ActionType::Toggle);
    }
}
