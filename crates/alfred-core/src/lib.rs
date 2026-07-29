use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Feedback {
    #[serde(rename = "skipknowledge", skip_serializing_if = "Option::is_none")]
    pub skip_knowledge: Option<bool>,
    pub items: Vec<Item>,
}

impl Feedback {
    pub fn new(items: Vec<Item>) -> Self {
        Self {
            skip_knowledge: None,
            items,
        }
    }

    pub fn with_skip_knowledge(mut self, skip_knowledge: bool) -> Self {
        self.skip_knowledge = Some(skip_knowledge);
        self
    }

    pub fn single_error(code: &str, message: impl Into<String>) -> Self {
        Self::new(vec![
            Item::new(format!("Error [{code}]"))
                .with_subtitle(message)
                .with_valid(false),
        ])
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Item {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autocomplete: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<ItemIcon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mods: Option<BTreeMap<String, ItemModifier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<BTreeMap<String, String>>,
}

impl Item {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            uid: None,
            title: title.into(),
            subtitle: None,
            arg: None,
            valid: None,
            autocomplete: None,
            icon: None,
            mods: None,
            variables: None,
        }
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_uid(mut self, uid: impl Into<String>) -> Self {
        self.uid = Some(uid.into());
        self
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.arg = Some(arg.into());
        self
    }

    pub fn with_valid(mut self, valid: bool) -> Self {
        self.valid = Some(valid);
        self
    }

    pub fn with_autocomplete(mut self, autocomplete: impl Into<String>) -> Self {
        self.autocomplete = Some(autocomplete.into());
        self
    }

    pub fn with_icon(mut self, icon: ItemIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_mod(mut self, modifier: impl Into<String>, config: ItemModifier) -> Self {
        self.mods
            .get_or_insert_with(BTreeMap::new)
            .insert(modifier.into(), config);
        self
    }

    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ItemModifier {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub valid: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<ItemIcon>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<BTreeMap<String, String>>,
}

impl ItemModifier {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = Some(subtitle.into());
        self
    }

    pub fn with_arg(mut self, arg: impl Into<String>) -> Self {
        self.arg = Some(arg.into());
        self
    }

    pub fn with_valid(mut self, valid: bool) -> Self {
        self.valid = Some(valid);
        self
    }

    pub fn with_icon(mut self, icon: ItemIcon) -> Self {
        self.icon = Some(icon);
        self
    }

    pub fn with_variable(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.variables
            .get_or_insert_with(BTreeMap::new)
            .insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ItemIcon {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub r#type: Option<String>,
}

impl ItemIcon {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            r#type: None,
        }
    }

    pub fn with_type(mut self, icon_type: impl Into<String>) -> Self {
        self.r#type = Some(icon_type.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feedback_serializes() {
        let payload = Feedback::new(vec![Item::new("hello").with_subtitle("world")]);
        let json = payload.to_json().expect("serialize feedback");
        assert!(json.contains("items"), "json should contain items field");
        assert!(
            !json.contains("skipknowledge"),
            "skipknowledge must be omitted unless explicitly configured"
        );
    }

    #[test]
    fn feedback_serializes_skip_knowledge_for_ordered_uid_rows() {
        let payload =
            Feedback::new(vec![Item::new("hello").with_uid("stable-id")]).with_skip_knowledge(true);
        let json = payload.to_json().expect("serialize feedback");

        assert!(
            json.contains("\"skipknowledge\":true"),
            "ordered uid feedback must disable Alfred knowledge sorting"
        );
    }

    #[test]
    fn feedback_single_error_creates_non_valid_item() {
        let payload = Feedback::single_error("demo.code", "boom");
        let json = payload.to_json().expect("serialize error feedback");
        assert!(json.contains("Error [demo.code]"));
        assert!(json.contains("\"valid\":false"));
        assert!(json.contains("\"subtitle\":\"boom\""));
    }

    #[test]
    fn item_optional_fields_serialize_only_when_present() {
        let base = Item::new("project");
        let json = serde_json::to_string(&base).expect("serialize item");

        assert!(json.contains("title"), "title must always serialize");
        assert!(
            !json.contains("subtitle"),
            "subtitle must be omitted when absent"
        );
        assert!(!json.contains("uid"), "uid must be omitted when absent");
        assert!(
            !json.contains("autocomplete"),
            "autocomplete must be omitted when absent"
        );
        assert!(!json.contains("mods"), "mods must be omitted when absent");
        assert!(
            !json.contains("variables"),
            "variables must be omitted when absent"
        );
    }

    #[test]
    fn modifier_and_variables_are_serialized() {
        let item = Item::new("project")
            .with_arg("/tmp/project")
            .with_autocomplete("project")
            .with_valid(true)
            .with_mod(
                "shift",
                ItemModifier::new()
                    .with_subtitle("Open on GitHub")
                    .with_arg("/tmp/project")
                    .with_valid(true)
                    .with_icon(ItemIcon::new("assets/icon-github.png"))
                    .with_variable("mode", "github"),
            )
            .with_variable("source", "open-project");

        let json = serde_json::to_string(&item).expect("serialize item with modifiers");
        assert!(json.contains("\"mods\""), "modifiers should be present");
        assert!(
            json.contains("\"shift\""),
            "shift modifier should be present"
        );
        assert!(
            json.contains("\"variables\""),
            "variables should be present"
        );
        assert!(
            json.contains("\"icon\""),
            "modifier icon should be present when configured"
        );
    }
}
