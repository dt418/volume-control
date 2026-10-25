//! Accessible node properties and semantic role descriptors.

use accesskit::Role;

/// Accessible metadata attached to a scene node for assistive technologies.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yNodeData {
    pub role: Role,
    pub name: Option<String>,
    pub value: Option<f64>,
    pub min_value: Option<f64>,
    pub max_value: Option<f64>,
    pub step: Option<f64>,
    pub toggled: Option<bool>,
    pub disabled: bool,
}

impl Default for A11yNodeData {
    fn default() -> Self {
        Self {
            role: Role::GenericContainer,
            name: None,
            value: None,
            min_value: None,
            max_value: None,
            step: None,
            toggled: None,
            disabled: false,
        }
    }
}

impl A11yNodeData {
    /// Construct a slider node with min, max, current value, and step.
    pub fn slider(name: impl Into<String>, value: f64, min: f64, max: f64, step: f64) -> Self {
        Self {
            role: Role::Slider,
            name: Some(name.into()),
            value: Some(value),
            min_value: Some(min),
            max_value: Some(max),
            step: Some(step),
            toggled: None,
            disabled: false,
        }
    }

    /// Construct a toggle button or checkbox (e.g. mute control).
    pub fn toggle_button(name: impl Into<String>, is_toggled: bool) -> Self {
        Self {
            role: Role::CheckBox,
            name: Some(name.into()),
            value: None,
            min_value: None,
            max_value: None,
            step: None,
            toggled: Some(is_toggled),
            disabled: false,
        }
    }

    /// Construct a standard push button.
    pub fn button(name: impl Into<String>) -> Self {
        Self {
            role: Role::Button,
            name: Some(name.into()),
            value: None,
            min_value: None,
            max_value: None,
            step: None,
            toggled: None,
            disabled: false,
        }
    }

    /// Construct a text input field.
    pub fn text_input(name: impl Into<String>) -> Self {
        Self {
            role: Role::TextInput,
            name: Some(name.into()),
            value: None,
            min_value: None,
            max_value: None,
            step: None,
            toggled: None,
            disabled: false,
        }
    }
}
