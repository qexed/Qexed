#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct ConfigFieldDoc {
    pub path: String,
    pub description: String,
    pub value_type: &'static str,
    pub default_value: Option<String>,
    pub warning: Option<String>,
    pub danger: Option<String>,
    pub pending_deprecated: Option<String>,
    pub deprecated: Option<String>,
    pub migration_notice: Option<String>,
}

pub trait AutoDocConfigTrait {
    fn doc_fields(lang: &str) -> Vec<(String, String)>;

    fn field_value_types() -> Vec<(String, &'static str)> {
        Vec::new()
    }

    fn default_display_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    fn sensitive_fields() -> Vec<String> {
        Vec::new()
    }

    fn deprecation_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    fn pending_deprecated_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    fn warning_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    fn migration_notice_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }

    fn danger_fields(_lang: &str) -> Vec<(String, String)> {
        Vec::new()
    }
}

pub fn autodoc_translate(key: &'static str, lang: &str) -> String {
    let _ = lang;
    key.to_string()
}

#[doc(hidden)]
pub fn autodoc_push_translated(
    target: &mut Vec<(String, String)>,
    field: &'static str,
    key: &'static str,
    lang: &str,
) {
    target.push((field.to_string(), autodoc_translate(key, lang)));
}

#[doc(hidden)]
pub fn autodoc_push_literal(
    target: &mut Vec<(String, String)>,
    field: &'static str,
    value: &'static str,
) {
    target.push((field.to_string(), value.to_string()));
}

#[doc(hidden)]
pub fn autodoc_push_value_type(
    target: &mut Vec<(String, &'static str)>,
    field: &'static str,
    value_type: &'static str,
) {
    target.push((field.to_string(), value_type));
}

#[doc(hidden)]
pub fn autodoc_extend_prefixed_strings(
    target: &mut Vec<(String, String)>,
    prefix: &'static str,
    entries: Vec<(String, String)>,
) {
    target.extend(
        entries
            .into_iter()
            .map(|(key, value)| (format!("{prefix}.{key}"), value)),
    );
}

#[doc(hidden)]
pub fn autodoc_extend_prefixed_value_types(
    target: &mut Vec<(String, &'static str)>,
    prefix: &'static str,
    entries: Vec<(String, &'static str)>,
) {
    target.extend(
        entries
            .into_iter()
            .map(|(key, value)| (format!("{prefix}.{key}"), value)),
    );
}

#[doc(hidden)]
pub fn autodoc_extend_prefixed_sensitive(
    target: &mut Vec<String>,
    prefix: &'static str,
    entries: Vec<String>,
) {
    target.extend(entries.into_iter().map(|key| format!("{prefix}.{key}")));
}
