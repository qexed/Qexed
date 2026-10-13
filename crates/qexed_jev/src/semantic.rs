//! 语义判定：规则快筛没命中的文本交给 jev 模型理解，判断是否违规。
//!
//! 与 [`crate::moderation`] 的分工：
//! - 规则引擎（关键词/号码正则）：零成本、零延迟，抓写死的词；
//! - 语义判定（本模块）：一次推理延迟，抓换写法、隐喻、谐音、拼音混写。
//!
//! 严格 JSON 输出协议：模型必须只回一个 JSON 对象（不做免责声明、不解释）。
//! 解析失败/超时一律视为「未命中」放行 —— 语义判定是增益层，
//! 不能因为模型抽风把正常聊天全部堵死。
//!
//! ```rust,ignore
//! // 典型用法（PLAY 层，规则未命中后）：
//! let moderator = Moderator::new(&config.moderation);
//! let hit = semantic::classify(jev.backend(), &config, &moderator, text).await;
//! if let Some(verdict) = hit {
//!     // 按策略处置：Block 拦截 / Sanitize 整段打码 / Allow 放行
//! }
//! ```

use crate::engine::JevRequest;
use crate::error::JevError;
use crate::moderation::{Category, Moderator};
use serde::Deserialize;

/// 模型返回的判定结果（严格 JSON）。
#[derive(Debug, Clone, Deserialize)]
pub struct SemanticJudgement {
    /// 是否违规。
    pub violation: bool,
    /// 命中类别（小写不敏感：ads / contact_numbers / illegal）。
    #[serde(default)]
    pub categories: Vec<String>,
    /// 一句话理由（日志用，模型可能不给）。
    #[serde(default)]
    pub reason: String,
}

/// 语义判定系统提示：三条红线 + few-shot 标注示例 + 输出协议。
///
/// 内嵌是小模型（0.5B 级）：只讲规则它会"客气地放行"，few-shot
/// 示例是对齐输出格式和判定边界的主要手段，示例要保持更新。
pub fn judgement_system_prompt() -> String {
    [
        "你是游戏聊天内容审核器。判断玩家消息是否违规，只看这一条消息本身。",
        "违规类别（可多选）：",
        "- ads：广告（代练/代充/出售/收购/推广/拉人加群等营销）",
        "- contact_numbers：留联系方式或各种号码（QQ/微信/手机号/一串号码）",
        "- illegal：违法内容（诈骗/赌博/毒品/色情交易/外挂/枪支等）",
        "判断要点：",
        "- 普通数字不是号码：物品数量、坐标、版本号、随手敲的短数字都是正常内容。",
        "- 只有出现联系方式平台词（qq/微信/vx/tg 等）加号码，或明显引导别人去加联系方式，才算 contact_numbers。",
        "- 玩家名字、问候、约人玩游戏、问路、闲聊都不违规。",
        "- 「炮」「约」等词的性邀约用法（如问别人“炮吗”、约人开房）算违法；游戏里的红石炮/TNT大炮/炮台是正常游戏内容。",
        "- 线下交易（人民币买卖游戏物品/账号/代充）算 ads：「收/出/售/卖 + 数字+字母r」（r=人民币）、单价报价（如 1r1组）都是。游戏内以物易物（用钻石换绿宝石）不违规。",
        "- 拿不准时判不违规。",
        "示例：",
        "用户：收钻石块1r1组 要的密我",
        r#"输出：{"violation": true, "categories": ["ads"], "reason": "线下人民币收购游戏物品"}"#,
        "用户：出绿宝石4r一组 支付宝秒发",
        r#"输出：{"violation": true, "categories": ["ads"], "reason": "人民币出售游戏物品"}"#,
        "用户：我拿铁锭换你两组煤炭行吗",
        r#"输出：{"violation": false, "categories": [], "reason": "游戏内以物易物"}"#,
        "用户：给我64个石头",
        r#"输出：{"violation": false, "categories": [], "reason": "正常物品数量"}"#,
        "用户：今晚八点老地方挖矿",
        r#"输出：{"violation": false, "categories": [], "reason": "正常游戏交流"}"#,
        "用户：出售金币 5元一组 加群123456",
        r#"输出：{"violation": true, "categories": ["ads"], "reason": "出售金币广告"}"#,
        "用户：加我微信 abc123",
        r#"输出：{"violation": true, "categories": ["contact_numbers"], "reason": "留下微信号"}"#,
        "用户：卖外挂 卖淫 加微信详聊",
        r#"输出：{"violation": true, "categories": ["illegal"], "reason": "违法交易"}"#,
        "用户：你好，炮吗？",
        r#"输出：{"violation": true, "categories": ["illegal"], "reason": "性邀约暗示"}"#,
        "用户：这个红石大炮怎么造",
        r#"输出：{"violation": false, "categories": [], "reason": "游戏内大炮建筑"}"#,
        "用户：加我qq 987654321 带你上分",
        r#"输出：{"violation": true, "categories": ["contact_numbers"], "reason": "留下QQ号码"}"#,
        "只输出一个 JSON 对象，禁止任何解释、前后缀或代码块围栏。",
    ]
    .join("\n")
}

/// 把模型的类别字符串解析成检测类别（未知的忽略）。
fn parse_categories(raw: &[String]) -> Vec<Category> {
    raw.iter().filter_map(|c| match c.trim().to_ascii_lowercase().as_str() {
        "ads" | "ad" | "广告" => Some(Category::Ads),
        "contact_numbers" | "numbers" | "contact" | "号码" => Some(Category::ContactNumbers),
        "illegal" | "违法" => Some(Category::Illegal),
        _ => None,
    }).collect()
}

/// 联系方式平台词：与号码强相关的字面标记（小写匹配）。
const CONTACT_MARKERS: &[&str] = &[
    "qq", "vx", "wx", "v信", "微信", "扣扣", "扣群", "q群", "tg", "电报",
    "电话", "手机", "号码", "联系",
];

/// 消息里是否具备「留联系方式」的字面证据：数字或平台词。
/// 用于交叉验证模型的 contact_numbers 判定，挡住复读 few-shot 的幻觉。
fn has_contact_markers(text: &str) -> bool {
    let lower = text.to_lowercase();
    if lower.bytes().any(|b| b.is_ascii_digit()) {
        return true;
    }
    CONTACT_MARKERS.iter().any(|m| lower.contains(m))
}

/// 从模型输出里抠出 JSON 对象再解析（容忍包一层代码块/前后废话）。
/// 供引擎冒烟测试复用：把模型原文解析成判定。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn parse_judgement_for_tests(text: &str) -> Option<SemanticJudgement> {
    parse_judgement(text)
}

fn parse_judgement(text: &str) -> Option<SemanticJudgement> {
    let (open, close) = (text.find('{')?, text.rfind('}')?);
    if open >= close {
        return None;
    }
    serde_json::from_str(&text[open..=close]).ok()
}

/// 单次语义判定：返回 Some(verdict) 表示模型判定违规（已按策略求出动作）。
///
/// - 仅当 policy.semantic_enable 时真正推理；关闭时恒为 Ok(None)；
/// - 超时（policy.semantic_timeout_ms）或解析失败按未命中放行并记日志。
pub async fn classify(
    backend: &crate::backend::JevBackend,
    config: &crate::config::JevConfig,
    moderator: &Moderator,
    text: &str,
) -> Result<Option<crate::moderation::Verdict>, JevError> {
    let policy = &config.moderation;
    if !policy.semantic_enable {
        return Ok(None);
    }
    if text.trim().is_empty() {
        return Ok(None);
    }
    // 短消息（人名、单个数字、语气词）没有语义空间，规则层已覆盖其风险；
    // 小模型对这类输入极易复读 few-shot 示例产生幻觉误判，直接跳过。
    let char_count = text.chars().count();
    if char_count < policy.semantic_min_chars {
        log::debug!("semantic moderation skipped (short message, {char_count} chars)");
        return Ok(None);
    }
    let request = JevRequest {
        model: None,
        messages: vec![
            ("system".to_string(), judgement_system_prompt()),
            ("user".to_string(), text.to_string()),
        ],
        temperature: Some(0.0),
    };
    let reply = match backend.complete(request, policy.semantic_timeout_ms).await {
        Ok(reply) => reply,
        Err(err @ (JevError::Timeout { .. } | JevError::QueueFull { .. })) => {
            log::warn!("semantic moderation unavailable, allowing message: {err}");
            return Ok(None);
        }
        Err(err) => return Err(err),
    };
    let Some(judgement) = parse_judgement(&reply) else {
        log::warn!("semantic moderation unparseable reply, allowing message: {reply:?}");
        return Ok(None);
    };
    if !judgement.violation {
        return Ok(None);
    }
    let mut categories = parse_categories(&judgement.categories);
    // 交叉验证：contact_numbers 必然伴随数字或联系方式平台词。
    // 模型报了号码类但消息里两者皆无，几乎一定是复读示例的幻觉，丢弃该类。
    if categories.contains(&Category::ContactNumbers) && !has_contact_markers(text) {
        log::info!(
            "semantic contact_numbers dropped (no digits/contact markers in message)"
        );
        categories.retain(|c| *c != Category::ContactNumbers);
    }
    if categories.is_empty() {
        log::info!(
            "semantic violation without credible categories: {:?} (allowing)",
            judgement.categories
        );
        return Ok(None);
    }
    log::info!(
        "semantic moderation hit: categories={:?} reason={}",
        judgement.categories, judgement.reason
    );
    Ok(Some(moderator.verdict_from_categories(categories)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_plain_json() {
        let j = parse_judgement(r#"{"violation": true, "categories": ["Illegal"], "reason": "涉赌"}"#)
            .expect("plain json should parse");
        assert!(j.violation);
        assert_eq!(parse_categories(&j.categories), vec![Category::Illegal]);
    }

    #[test]
    fn parses_fenced_json_and_ignores_prose() {
        let j = parse_judgement("审核结果：{\"violation\": true, \"categories\": [\"ads\", \"contact_numbers\"]}
以上");
        let j = j.expect("embedded json should parse");
        assert_eq!(
            parse_categories(&j.categories),
            vec![Category::Ads, Category::ContactNumbers]
        );
    }

    #[test]
    fn rejects_garbage_and_clean_verdicts() {
        assert!(parse_judgement("我不是审核器，但我觉得没事").is_none());
        assert!(parse_judgement("{broken json}").is_none());
        let clean = parse_judgement(r#"{"violation": false, "categories": []}"#).unwrap();
        assert!(!clean.violation);
    }

    #[test]
    fn unknown_categories_are_dropped() {
        assert!(parse_categories(&["spam".to_string()]).is_empty());
    }

    #[test]
    fn system_prompt_states_three_categories() {
        let prompt = judgement_system_prompt();
        assert!(prompt.contains("ads") && prompt.contains("contact_numbers") && prompt.contains("illegal"));
    }

    /// 真实权重冒烟：线上误杀回归（玩家实测「打啥都是＊」）。
    /// 人名/短消息/纯数字必须放行，真联系方式仍要拦。
    /// 需要真模型，默认 ignore；排查误杀时带 --ignored 跑。
    #[test]
    #[ignore = "needs real GGUF weights in run/models"]
    fn real_model_benign_messages_pass() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("rt");
        rt.block_on(async {
            let mut config = crate::config::JevConfig::default();
            config.embedded.model_path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../run/models/qwen2.5-0.5b-instruct-q4_k_m.gguf"
            )
            .into();
            config.moderation.semantic_enable = true;
            let backend = crate::backend::build_backend(&config).expect("build embedded backend");
            let moderator = crate::moderation::Moderator::new(&config.moderation);
            let _policy = config.moderation.clone();

            // (输入, 期望违规?)
            // 镜像生产门控顺序（jev_bridge::gate_with）：先规则层 inspect，
            // 干净才进语义层 classify。任一层命中即违规。
            let cases = [
                ("流萤", false),                 // 玩家名字（线上误杀样本）
                ("1", false),                    // 单字符（线上误杀样本）
                ("123456", true),                // 纯数字：号码规则层应拦（min_digits=5）
                ("今天谁来挖矿", false),         // 普通闲聊
                ("这是我的微信 abc123", true),   // 混合 ID：contact_id 规则层应拦
                ("加我qq 987654321 带你上分", true), // 真留联系方式：规则层拦
                ("你好，炮吗？", true),          // 线上漏网：性邀约暗语，语义层应拦
                ("这个红石大炮怎么造", false),   // 语境对比：游戏内容不得误伤
            ];
            for (text, expect_violation) in cases {
                let rule_verdict = moderator.inspect(text);
                let verdict = if rule_verdict.is_clean() {
                    super::classify(&backend, &config, &moderator, text)
                        .await
                        .expect("classify works")
                } else {
                    Some(rule_verdict)
                };
                println!("[smoke] {text:?} -> {verdict:?}");
                if expect_violation {
                    let reply = backend
                        .complete(
                            crate::engine::JevRequest {
                                model: None,
                                messages: vec![
                                    ("system".to_string(), super::judgement_system_prompt()),
                                    ("user".to_string(), text.to_string()),
                                ],
                                temperature: Some(0.0),
                            },
                            _policy.semantic_timeout_ms,
                        )
                        .await
                        .expect("raw complete");
                    println!("[raw] {text:?} => {reply}");
                }
                if expect_violation {
                    assert!(verdict.is_some(), "{text:?} 应被判违规");
                } else {
                    assert!(verdict.is_none(), "{text:?} 不应被判违规，got {verdict:?}");
                }
            }
        });
    }
}
