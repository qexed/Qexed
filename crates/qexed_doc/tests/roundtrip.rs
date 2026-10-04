// DocValue 手工实现的配套类型（宏的对照样例）。
use qexed_doc::DocValue;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum Mode {
    Fast,
    Slow,
}

impl qexed_doc::DocValue for Mode {
    const TYPE: &'static str = "Mode";
    const VARIANTS: &'static [&'static str] = &["Fast", "Slow"];
    fn validate(value: &serde_json::Value) -> Result<(), String> {
        match value.as_str() {
            Some("Fast") | Some("Slow") => Ok(()),
            _ => Err(format!("expected one of {}", Self::VARIANTS.join(", "))),
        }
    }
}

#[derive(Serialize)]
pub struct OnlyDefault {
    pub mode: Mode,
}

#[test]
fn eval_default_executes_expression() {
    let v = qexed_doc::eval_default(&OnlyDefault { mode: Mode::Fast });
    assert_eq!(v, serde_json::json!({ "mode": "Fast" }));
}

#[test]
fn default_from_literal_parses_json_or_string() {
    assert_eq!(qexed_doc::default_from_literal("42"), serde_json::json!(42));
    assert_eq!(
        qexed_doc::default_from_literal("hello"),
        serde_json::json!("hello")
    );
}

#[test]
fn primitive_validation() {
    assert!(<u32 as DocValue>::validate(&serde_json::json!(3)).is_ok());
    assert!(<u32 as DocValue>::validate(&serde_json::json!("3")).is_err());
    assert!(<String as DocValue>::validate(&serde_json::json!("x")).is_ok());
    assert!(Mode::validate(&serde_json::json!("Fast")).is_ok());
    assert!(Mode::validate(&serde_json::json!("Meh")).is_err());
}