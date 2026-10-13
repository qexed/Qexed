//! 对话内容检测：广告 / 各种号码 / 违法内容 三类规则。
//!
//! 检测发生在 `JevConversation::ask` 的两端：玩家发给引擎的输入、引擎回给
//! 玩家的输出（回显/模型都可能带出违规内容）。每个类别可独立配置动作：
//!
//! - `Action::Allow`：只记录，不改内容；
//! - `Action::Sanitize`（默认）：命中片段打码后放行；
//! - `Action::Block`：输入侧直接拒绝（`JevError::Moderated`），
//!   输出侧替换为固定拒答文案（推理已完成，丢弃比报错对玩家更友好）。
//!
//! 归一化对抗最常见的绕过：全角/大小写、零宽字符、数字/关键词里插空格或标点。
//! 这是启发式规则（不是模型），`extra_*_keywords` 可加自定义词。
use qexed_doc_macros::{Doc, DocValue};
use serde::{Deserialize, Serialize};

/// 检测类别：与需求一一对应。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    /// 广告：代练/出售/加群/优惠 等营销话术。
    Ads,
    /// 各种号码：QQ、手机号、长数字串（含分隔符混淆）。
    ContactNumbers,
    /// 违法：外挂、诈骗、赌博、毒品等。
    Illegal,
}

impl Category {
    /// 日志/错误里显示的中文名。
    pub const fn label(self) -> &'static str {
        match self {
            Category::Ads => "广告",
            Category::ContactNumbers => "号码",
            Category::Illegal => "违法",
        }
    }
}

/// 对一个类别的处置动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, DocValue)]
pub enum Action {
    /// 放行（只记录违规，不改内容）。
    Allow,
    /// 命中片段打码后放行（默认）。
    Sanitize,
    /// 拦截：输入直接拒绝，输出替换为拒答文案。
    Block,
}

impl Default for Action {
    fn default() -> Self {
        Action::Sanitize
    }
}

/// 单条违规记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub category: Category,
    /// 规则名：keyword / qq / mobile / digits。
    pub rule: String,
    /// 命中的原文片段（日志用）。
    pub matched: String,
}

/// 检测结论：整体动作 + 全部违规明细。
#[derive(Debug, Clone, Default)]
pub struct Verdict {
    /// 全部「有违规的类别」里最严格的动作（Block > Sanitize > Allow）。
    pub action: Action,
    pub violations: Vec<Violation>,
}

impl Verdict {
    pub fn is_clean(&self) -> bool {
        self.violations.is_empty()
    }

    /// 去重后的类别列表（日志用）。
    pub fn categories(&self) -> Vec<Category> {
        let mut seen: Vec<Category> = Vec::new();
        for v in &self.violations {
            if !seen.contains(&v.category) {
                seen.push(v.category);
            }
        }
        seen
    }
}

/// 检测策略（进 `JevConfig`，toml 可调）。
#[derive(Debug, Clone, Serialize, Deserialize, Doc)]
pub struct ModerationPolicy {
    /// 广告类处置动作。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.ads</Name>
    /// <Default>Sanitize</Default>
    /// <Attr name="writable" />
    /// ```
    pub ads: Action,
    /// 号码类处置动作。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.contact_numbers</Name>
    /// <Default>Sanitize</Default>
    /// <Attr name="writable" />
    /// ```
    pub contact_numbers: Action,
    /// 违法类处置动作。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.illegal</Name>
    /// <Default>Sanitize</Default>
    /// <Attr name="writable" />
    /// ```
    pub illegal: Action,
    /// 追加的广告关键词（在内置词表之上）。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.extra_ads_keywords</Name>
    /// <Default>[]</Default>
    /// <Attr name="writable" />
    /// ```
    pub extra_ads_keywords: Vec<String>,
    /// 追加的违法关键词（在内置词表之上）。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.extra_illegal_keywords</Name>
    /// <Default>[]</Default>
    /// <Attr name="writable" />
    /// ```
    pub extra_illegal_keywords: Vec<String>,
    /// 长数字串判为号码的最少位数（内置号码规则已覆盖 QQ/手机）。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.min_digits</Name>
    /// <Default>5</Default>
    /// <Attr name="writable" />
    /// ```
    pub min_digits: usize,
    /// 语义判定（模型理解）开关：规则没命中的文本交给 jev 模型判违规。
    ///
    /// 关键词只能抓写死的词；换写法、隐喻、谐音要靠模型理解。开启后
    /// 规则快筛未命中的文本会再过一次模型（多一次推理延迟）。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.semantic_enable</Name>
    /// <Default>false</Default>
    /// <Attr name="writable" />
    /// ```
    pub semantic_enable: bool,
    /// 语义判定的最短消息长度（字符数）。更短的消息直接跳过模型：
    /// 人名、单个数字、语气词没有语义空间，小模型只会幻觉误判。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.semantic_min_chars</Name>
    /// <Default>6</Default>
    /// <Attr name="writable" />
    /// ```
    pub semantic_min_chars: usize,
    /// 单次语义判定的超时（毫秒）；超时放行（可用性优先于拦截率）。
    ///
    /// ```autodoc
    /// <Name>qexed.crates.jev.config.ModerationPolicy.semantic_timeout_ms</Name>
    /// <Default>4000</Default>
    /// <Attr name="writable" />
    /// ```
    pub semantic_timeout_ms: u64,
}

impl Default for ModerationPolicy {
    fn default() -> Self {
        Self {
            ads: Action::Sanitize,
            contact_numbers: Action::Sanitize,
            illegal: Action::Sanitize,
            extra_ads_keywords: Vec::new(),
            extra_illegal_keywords: Vec::new(),
            min_digits: 5,
            semantic_enable: false,
            semantic_min_chars: 6,
            semantic_timeout_ms: 4000,
        }
    }
}

impl ModerationPolicy {
    pub(crate) const fn action_for(&self, category: Category) -> Action {
        match category {
            Category::Ads => self.ads,
            Category::ContactNumbers => self.contact_numbers,
            Category::Illegal => self.illegal,
        }
    }

    pub(crate) fn strictest(a: Action, b: Action) -> Action {
        use Action::{Allow, Block, Sanitize};
        match (a, b) {
            (Block, _) | (_, Block) => Block,
            (Sanitize, _) | (_, Sanitize) => Sanitize,
            _ => Allow,
        }
    }
}

/// 内置广告关键词（含常见变体；匹配前统一小写化，中文不受影响）。
pub const ADS_KEYWORDS: &[&str] = &[
    "代练", "代肝", "代充", "刷金", "刷币", "出售", "收购", "低价", "优惠", "折扣",
    "充值卡", "礼品卡", "加群", "加微信", "加qq", "加vx", "微信号", "客服",
    "群号", "进群", "拉群", "有偿", "红包",
    "工作室", "推广", "发单", "接单", "返利", "免费领取",
];

/// 内置违法关键词。
pub const ILLEGAL_KEYWORDS: &[&str] = &[
    "外挂", "开挂", "作弊器", "破解器", "钓鱼网站", "诈骗", "骗取",
    "洗钱", "博彩", "赌博", "赌场", "色情", "翻墙", "枪支", "弹药",
    "毒品", "冰毒", "迷药", "自杀教程",
    "卖淫", "嫖娼", "招妓", "援交", "裸聊", "淫秽",
    "传销", "代孕", "办证", "假钞", "约炮", "一夜情", "援交妹",
    "炮友", "打炮", "炮机", "约爱", "出台费", "包夜", "上门服务",
];

/// 字符归一折叠：谐音（裙→群、浩→号、威心→微信）、同形（西里尔/希腊
/// 字母→ASCII）、符号代字（+→加）、数学字母块→ASCII。1:1 映射，
/// NormView 的原文坐标表不受影响；只用于检测，不影响原文显示。
fn fold_char(c: char) -> char {
    // 符号代字先行：'+' 是 ASCII，必须赶在下面的 ASCII 早退之前折叠
    let c = match c {
        '+' | '＋' | '﹢' => '加',
        _ => c,
    };
    // 字母变体（全角/带圈/负圈等）
    let c = match c {
        'Ａ'..='Ｚ' | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        'Ⓐ'..='Ⓩ' => char::from_u32(c as u32 - 0x24B6 + 'a' as u32).unwrap_or(c),
        'ⓐ'..='ⓩ' => char::from_u32(c as u32 - 0x24D0 + 'a' as u32).unwrap_or(c),
        '🅐'..='🅩' => char::from_u32(c as u32 - 0x1F150 + 'a' as u32).unwrap_or(c),
        '🅰'..='🆉' => char::from_u32(c as u32 - 0x1F170 + 'a' as u32).unwrap_or(c),
        _ => c,
    };
    // 数学字母符号块（𝐀𝐚 𝑨𝒂 𝔄𝔞 𝔸𝕒 𝕬𝖆 𝖠𝖺 𝗔𝗮 𝘈𝘢 𝘼𝙖 𝙰𝚊）：
    // 每块 52 字符 = 大写26+小写26，按块内偏移 %26 折到小写
    const MATH_BLOCKS: [u32; 10] = [
        0x1D400, 0x1D434, 0x1D504, 0x1D538, 0x1D56C,
        0x1D5A0, 0x1D5D4, 0x1D608, 0x1D63C, 0x1D670,
    ];
    let cp = c as u32;
    for &start in &MATH_BLOCKS {
        if (start..start + 52).contains(&cp) {
            return char::from_u32('a' as u32 + (cp - start) % 26).unwrap_or(c);
        }
    }
    if c.is_ascii() {
        return c;
    }
    // 希腊/西里尔同形字母 → ASCII
    match c {
        'ο' | 'о' | 'о' | 'σ' => 'o',
        'е' | 'ε' | 'е' => 'e',
        'ρ' | 'р' => 'p',
        'с' | 'ς' => 'c',
        'у' | 'υ' => 'y',
        'х' | 'χ' => 'x',
        'і' | 'ι' => 'i',
        'ѕ' => 's',
        'ԁ' => 'd',
        'һ' => 'h',
        'ј' => 'j',
        'κ' | 'к' => 'k',
        'α' => 'a',
        'β' => 'b',
        'ζ' => 'z',
        'ν' => 'v',
        'τ' => 't',
        'η' => 'n',
        'ω' => 'w',
        'т' => 't',
        'м' => 'm',
        'ъ' => 'b',
        // 谐音代字（检测用）
        '裙' => '群',
        '浩' | '耗' => '号',
        '威' | '薇' | '围' | '维' | '唯' => '微',
        '心' | '芯' | '新' | '薪' => '信',
        '佳' | '迦' | '伽' => '加',
        '祥' | '翔' => '详',
        '辽' | '疗' => '聊',
        '莲' | '连' => '联',
        '细' | '戏' => '系',
        '斯' | '撕' | '丝' | '司' | '嘶' => '私',
        '泡' | '抛' | '袍' | '咆' => '炮',
        '月' | '越' | '悦' | '跃' | '粤' | '曰' => '约',
        '堵' | '睹' => '赌',
        '兽' | '受' => '售',
        '初' | '锄' => '出',
        '克' | '科' | '颗' => '客',
        '福' | '付' => '服',
        '勉' => '免',
        _ => c,
    }
}

/// 归一化视图：去零宽字符、ASCII 小写；（紧凑视图）再去空白/常见插字符。
/// map[i] = 归一化后第 i 个字符在原文的起始字节偏移，末尾哨兵为原文长度，
/// 用于把命中区间映回原文做打码。
struct NormView {
    text: String,
    map: Vec<usize>,
}

impl NormView {
    fn build(src: &str, drop_separators: bool) -> Self {
        let mut text = String::with_capacity(src.len());
        let mut map = Vec::with_capacity(src.len() + 1);
        for (idx, ch) in src.char_indices() {
            // 零宽/方向控制字符：纯规避用，一律丢
            if matches!(ch, '\u{200b}'..='\u{200f}' | '\u{feff}' | '\u{fe0f}' | '\u{20e3}') {
                continue;
            }
            if drop_separators
                && (ch.is_whitespace() || matches!(ch, '.' | '*' | '_' | '-' | '~' | '/' | '\\'))
            {
                continue;
            }
            map.push(idx);
            for lower in ch.to_lowercase() {
                // 谐音/同形折叠：检测文本与关键词走同一折叠，匹配才对得上
                text.push(fold_char(lower));
            }
        }
        map.push(src.len());
        Self { text, map }
    }

    /// 归一化字符区间 [start, end) → 原文字节区间。
    fn orig_range(&self, start: usize, end: usize) -> (usize, usize) {
        let last = self.map.len() - 1;
        (self.map[start.min(last)], self.map[end.min(last)])
    }

    /// 归一化文本内的字节偏移 → 字符序号（map 按字符序索引，两种偏移不能混用）。
    fn char_idx(&self, byte_off: usize) -> usize {
        self.text[..byte_off.min(self.text.len())].chars().count()
    }
}

/// 规则引擎：一次构建，多次检测（无内部可变状态，可全局共享）。
#[derive(Debug, Clone)]
pub struct Moderator {
    ads_keywords: Vec<String>,
    illegal_keywords: Vec<String>,
    policy: ModerationPolicy,
}

/// 线下交易动词：出现即说明数字是价码而不是数量/坐标/版本号
/// （「收」「出」「售」「卖」——谐音折叠已把「兽受→售」「初锄→出」归一）。
const RMT_TRADE_VERBS: &[&str] = &["收", "出", "售", "卖"];

/// 价码单位字：紧跟在「数字r」后面的量词（1r1组、2r1个、5r一组）。
const RMT_UNIT_CHARS: &[char] = &['组', '个', '只', '件', '套', '份', '枚', '条', '把', '张'];

/// 联系方式平台词（小写匹配）；出现即视为「引导添加联系方式」的强信号。
const CONTACT_PLATFORM_WORDS: &[&str] = &[
    "qq", "vx", "wx", "v信", "微信", "扣扣", "扣群", "q群", "tg", "电报",
    "skype", "discord", "whatsapp", "飞机",
];

/// 平台词后面跟字母数字混合 ID（≥4 位、至少 2 位数字）→ 命中 contact_id。
/// 这类「微信 abc123」形态是小模型稳定学不会的边界，规则层确定性兜住。
/// 坐标约定：输入/输出均为 view 字节偏移 → orig_range 映回原文。
fn scan_contact_ids(view: &NormView) -> Vec<(usize, usize, String)> {
    let lower = view.text.to_lowercase();
    let Some((wbyte, word)) = CONTACT_PLATFORM_WORDS
        .iter()
        .find_map(|w| lower.find(w).map(|p| (p, *w)))
    else {
        return Vec::new();
    };
    // 平台词本身在 view.text 里占的字节数（窗口内跳过，避免平台词自命中）
    let wbytes = view.text[wbyte..].starts_with(word) as usize * word.len();
    let window = &view.text[wbyte..];
    let bytes = window.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_alphanumeric() {
            i += 1;
        }
        // 紧凑视图里平台词与 ID 可能粘成一段（qqabc123）：裁掉与平台词
        // 重叠的头部再判定，避免整段被「起点落在平台词内」误杀
        let overlap = wbytes.saturating_sub(start).min(i - start);
        let run = &window[start + overlap..i];
        let digits = run.bytes().filter(|b| b.is_ascii_digit()).count();
        let has_alpha = run.bytes().any(|b| b.is_ascii_alphabetic());
        // ≥4 位、含字母、≥2 位数字 → 混合 ID（纯数字串归 scan_numbers 管）
        if run.len() >= 4 && digits >= 2 && has_alpha {
            let abs_start = wbyte + start;
            let abs_end = wbyte + i;
            let (cs, ce) = (view.char_idx(abs_start), view.char_idx(abs_end));
            let (os, oe) = view.orig_range(cs, ce);
            out.push((os, oe, "contact_id".to_string()));
        }
    }
    out
}

/// 游戏大炮语境词：「炮吗」出现在这些词后面是正常游戏讨论，不算涉黄。
const CANNON_CONTEXT: &[&str] = &[
    "红石", "tnt", "大炮", "炮台", "炮塔", "活塞", "轰炸", "攻城", "发射",
    "造", "教", "做",
];

/// 「炮吗」类性邀约暗语：小模型学不会俚语义，规则层兜底，
/// 但前文出现游戏大炮语境词时豁免（红石炮吗/TNT炮吗是正常讨论）。
fn scan_sexual_slang(view: &NormView) -> Vec<(usize, usize, String)> {
    let lower = view.text.to_lowercase();
    let needle = "炮吗";
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(rel) = lower[from..].find(needle) {
        let abs = from + rel;
        // 检查前 6 个字符是否有游戏语境
        let before: String = {
            let chars: Vec<char> = lower[..abs].chars().collect();
            let start = chars.len().saturating_sub(6);
            chars[start..].iter().collect()
        };
        if !CANNON_CONTEXT.iter().any(|w| before.contains(w)) {
            let (cs, ce) = (view.char_idx(abs), view.char_idx(abs + needle.len()));
            let (os, oe) = view.orig_range(cs, ce);
            out.push((os, oe, "sexual_slang".to_string()));
        }
        from = abs + needle.len();
        if from >= lower.len() {
            break;
        }
    }
    out
}

/// 字符能否被看出是数字：能 → 返回它表示的数字串（❿→"10"、❷→"2"、伍→"5"）。
/// 覆盖：ASCII/全角/中文数字/带圈①-⑳/括号⑴/加点⒈/双圈⓫/黑圈⓵/❶❿/➀➉/➊⓿/
/// 带圈中文㈠㊀/罗马Ⅰ-Ⅹ/数学粗斜体𝟎-𝟡/键帽🔟。判定标准：人眼可辨识。
fn digit_contrib(c: char) -> Option<String> {
    if c.is_ascii_digit() {
        return Some(c.to_string());
    }
    if matches!(c, '０'..='９') {
        return Some(char::from_u32(c as u32 - 0xFEE0).expect("fullwidth digit").to_string());
    }
    if let Some(d) = match c {
        '零' | '〇' => Some('0'),
        '一' => Some('1'),
        '二' | '两' => Some('2'),
        '三' => Some('3'),
        '四' => Some('4'),
        '五' => Some('5'),
        '六' => Some('6'),
        '七' => Some('7'),
        '八' => Some('8'),
        '九' => Some('9'),
        _ => None,
    } {
        return Some(d.to_string());
    }
    let v: u32 = match c {
        '①'..='⑳' => c as u32 - 0x2460 + 1,
        '⑴'..='⒇' => c as u32 - 0x2474 + 1,
        '⒈'..='⒛' => c as u32 - 0x2488 + 1,
        '⓫'..='⓴' => c as u32 - 0x24EB + 11,
        '⓵'..='⓾' => c as u32 - 0x24F5 + 1,
        '❶'..='❿' => c as u32 - 0x2776 + 1,
        '➀'..='➉' => c as u32 - 0x2780 + 1,
        '➊'..='➓' => c as u32 - 0x278A + 1,
        '㈠'..='㈩' => c as u32 - 0x3220 + 1,
        '㊀'..='㊉' => c as u32 - 0x3280 + 1,
        'Ⅰ'..='Ⅹ' => c as u32 - 0x2160 + 1,
        '\u{1D7CE}'..='\u{1D7FF}' => (c as u32 - 0x1D7CE) % 10,
        '🔟' => 10,
        _ => return None,
    };
    Some(v.to_string())
}

/// 字母变体归一：全角/带圈/负圈/负方框字母 → ASCII 小写；其余原样。
fn fold_alpha(c: char) -> char {
    match c {
        'Ａ'..='Ｚ' | 'ａ'..='ｚ' => char::from_u32(c as u32 - 0xFEE0).expect("fullwidth alpha"),
        'Ⓐ'..='Ⓩ' => char::from_u32(c as u32 - 0x24B6 + 'a' as u32).expect("circled alpha upper"),
        'ⓐ'..='ⓩ' => char::from_u32(c as u32 - 0x24D0 + 'a' as u32).expect("circled alpha"),
        '🅐'..='🅩' => char::from_u32(c as u32 - 0x1F150 + 'a' as u32).expect("neg circled"),
        '🅰'..='🆉' => char::from_u32(c as u32 - 0x1F170 + 'a' as u32).expect("neg squared"),
        _ => c,
    }
}

/// 数字变体混排：「1二删叁掉肆伍⑥字❼符❿」把能认出的数字全抽出来拼流 →
/// 1234567+10 = 9 位 ≥ min_digits → 命中，干扰字（删/掉/字/符）不参与自然剔除。
/// 中文计数读法豁免：数字间出现「十百千万亿」是正常大数书写（一百二十三万）。
/// 纯 ASCII 数字串归 scan_numbers 管；这里要求 ≥2 个非 ASCII 贡献字符，
/// 「2024年12月」这类正常日期不误伤。
fn scan_mixed_numbers(view: &NormView, min_digits: usize) -> Vec<(usize, usize, String)> {
    let mut contrib: Vec<(usize, String)> = Vec::new();
    let mut non_ascii = 0usize;
    for (idx, ch) in view.text.chars().enumerate() {
        if let Some(d) = digit_contrib(ch) {
            if !ch.is_ascii() {
                non_ascii += 1;
            }
            contrib.push((idx, d));
        }
    }
    let total: usize = contrib.iter().map(|(_, d)| d.len()).sum();
    if contrib.is_empty() || total < min_digits || non_ascii < 2 {
        return Vec::new();
    }
    let first = contrib[0].0;
    let last = contrib[contrib.len() - 1].0;
    let between: String = view.text.chars().skip(first).take(last - first + 1).collect();
    if between.contains(['十', '百', '千', '万', '亿']) {
        return Vec::new();
    }
    let (os, oe) = view.orig_range(first, last + 1);
    vec![(os, oe, "digits_mixed".to_string())]
}

/// 外部交易暗语：「32r,z还是w」（r=人民币，z=支付宝，w=微信，q=QQ）。
/// 任一条件即命中：价码（数字紧跟 r，1 位也拦——线上漏过「收钻石块1r1组」）/
/// 单位报价（数字 r 数字 单位字：1r1组、2r1个）/ ≥2 个独立 z/w/q /
/// 独立 z/w/q 前有「加找私聊联系问」引诱词 / 后面跟「群号我」。
fn scan_offsite_trade(view: &NormView) -> Vec<(usize, usize, String)> {
    let chars: Vec<char> = view.text.chars().collect();
    let is_alnum = |i: usize| -> bool {
        match chars.get(i).copied().map(fold_alpha) {
            Some(c) => c.is_ascii_alphanumeric() || digit_contrib(c).is_some(),
            None => false,
        }
    };
    let text_lower: String = chars.iter().collect();
    let has_trade_verb = RMT_TRADE_VERBS.iter().any(|w| text_lower.contains(w));
    let mut singles: Vec<usize> = Vec::new();
    let mut price: Option<(usize, usize)> = None;
    let mut run_start: Option<usize> = None;
    let mut run_digits = 0usize;
    for (i, &raw) in chars.iter().enumerate() {
        if let Some(d) = digit_contrib(raw) {
            if run_digits == 0 {
                run_start = Some(i);
            }
            run_digits += d.len();
            continue;
        }
        if run_digits >= 1 && fold_alpha(raw) == 'r' {
            // 「收/出/售/卖」在场 ⇒ 数字必是价码，1 位也拦（收钻石块1r1组）；
            // 否则保持 ≥2 位门槛（「第1名」这类序号不误伤）。
            let unit_ahead = chars[(i + 1).min(chars.len())..(i + 3).min(chars.len())]
                .iter()
                .any(|c| RMT_UNIT_CHARS.contains(c));
            if has_trade_verb || run_digits >= 2 || unit_ahead {
                let rs = run_start.expect("run started");
                price = Some(match price {
                    Some((s, e)) => (s.min(rs), e.max(i + 1)),
                    None => (rs, i + 1),
                });
            }
        }
        run_digits = 0;
        let c = fold_alpha(raw);
        if matches!(c, 'z' | 'w' | 'q') && !is_alnum(i.wrapping_sub(1)) && !is_alnum(i + 1) {
            singles.push(i);
        }
    }
    let before: String = {
        let s = singles.first().copied().unwrap_or(0).saturating_sub(2);
        chars[s..singles.first().copied().unwrap_or(0)].iter().collect()
    };
    let triggered = singles.iter().any(|&i| {
        let pre: String = chars[i.saturating_sub(2)..i].iter().collect();
        let post: String = chars[(i + 1).min(chars.len())..(i + 3).min(chars.len())].iter().collect();
        ["加", "找", "私", "聊", "联系", "问"].iter().any(|w| pre.contains(w))
            || ["群", "号", "我"].iter().any(|w| post.contains(w))
            || before.contains("收费")
    });
    if singles.len() < 2 && price.is_none() && !triggered {
        return Vec::new();
    }
    let mut start = usize::MAX;
    let mut end = 0usize;
    if let Some((ps, pe)) = price {
        start = start.min(ps);
        end = end.max(pe);
    }
    for &i in &singles {
        start = start.min(i);
        end = end.max(i + 1);
    }
    let (os, oe) = view.orig_range(start, end);
    vec![(os, oe, "offsite_trade".to_string())]
}

/// 链接变体：全角域名、点号改写（。·点）、去点连接（taobaocom）。
/// 紧凑视图已丢 `.`，「example.com」「example。com」都归一成
/// 「examplecom」，按「域名体+顶级域」模式命中；顶级域后必须不是
/// 字母数字（"come" 不是 .com）。
const URL_TLDS: &[&str] = &[
    "com", "cn", "net", "org", "top", "xyz", "vip", "shop", "site",
    "club", "icu", "fun", "cc", "pro", "app", "live", "io", "hk", "tw",
];

fn scan_url_variants(view: &NormView) -> Vec<(usize, usize, String)> {
    // 全程字符坐标：folded[i] 是第 i 个字符，避开字节/字符序错位
    let folded: Vec<char> = view.text.chars().map(fold_alpha).collect();
    let mut out = Vec::new();
    for tld in URL_TLDS {
        let tc: Vec<char> = tld.chars().collect();
        for i in 0..folded.len() {
            if !folded[i..].starts_with(&tc) {
                continue;
            }
            let tld_end = i + tc.len();
            let after_ok = folded
                .get(tld_end)
                .copied()
                .map_or(true, |c| !c.is_ascii_alphanumeric() && digit_contrib(c).is_none());
            // 域名体：顶级域前 ≥2 个字母/数字/汉字（单独 "cc" 不算）；
            // 点号替代符（。。．·・，）可跳过（taobao。cc 一样成域）
            let mut s = i;
            let mut body = 0usize;
            let mut seps = 0usize;
            while s > 0 && body < 32 {
                let c = folded[s - 1];
                if c.is_ascii_alphanumeric()
                    || ('\u{4E00}'..='\u{9FFF}').contains(&c)
                    || digit_contrib(c).is_some()
                {
                    body += 1;
                    s -= 1;
                } else if matches!(c, '。' | '．' | '·' | '・' | '，' | '、') && seps < 2 && body == 0 {
                    seps += 1;
                    s -= 1;
                } else {
                    break;
                }
            }
            if after_ok && body >= 2 {
                let (os, oe) = view.orig_range(s, tld_end);
                out.push((os, oe, "url_variant".to_string()));
            }
        }
    }
    out
}

impl Moderator {
    pub fn new(policy: &ModerationPolicy) -> Self {
        let mut ads_keywords: Vec<String> = ADS_KEYWORDS.iter().map(|s| s.to_string()).collect();
        let mut illegal_keywords: Vec<String> =
            ILLEGAL_KEYWORDS.iter().map(|s| s.to_string()).collect();
        ads_keywords.extend(policy.extra_ads_keywords.iter().cloned());
        illegal_keywords.extend(policy.extra_illegal_keywords.iter().cloned());
        Self {
            ads_keywords,
            illegal_keywords,
            policy: policy.clone(),
        }
    }

    /// 规则快检：关键词 + 号码正则，零成本零延迟。命中即给出处置动作；
    /// 干净文本返回 Allow，调用方可继续走语义层。
    pub fn inspect(&self, text: &str) -> Verdict {
        let loose = NormView::build(text, false);
        let compact = NormView::build(text, true);
        let mut violations: Vec<Violation> = Vec::new();
        let mut action = Action::Allow;

        // 关键词类：广告 / 违法。松散视图先查（保留原始间隔），紧凑视图兜底。
        for (category, keywords) in [
            (Category::Ads, &self.ads_keywords),
            (Category::Illegal, &self.illegal_keywords),
        ] {
            let mut hit = false;
            for keyword in keywords {
                let found = find_in(&loose, keyword).or_else(|| find_in(&compact, keyword));
                if let Some((s, e)) = found {
                    hit = true;
                    violations.push(Violation {
                        category,
                        rule: "keyword".into(),
                        matched: slice_text(text, s, e),
                    });
                }
            }
            if hit {
                action = ModerationPolicy::strictest(action, self.policy.action_for(category));
            }
        }

        // 号码规则跑在紧凑视图上：插空格/标点的号码一样成段
        let mut numbers = scan_numbers(&compact, self.policy.min_digits.max(1));
        // 平台词 + 字母数字混合 ID（如「微信 abc123」）：小模型学不稳的边界，
        // 规则层确定性兜住
        numbers.extend(scan_contact_ids(&compact));
        // 数字变体混排（全角/中文数字/带圈+插干扰字）
        numbers.extend(scan_mixed_numbers(&compact, self.policy.min_digits.max(1)));
        if !numbers.is_empty() {
            for (s, e, rule) in &numbers {
                violations.push(Violation {
                    category: Category::ContactNumbers,
                    rule: rule.clone(),
                    matched: slice_text(text, *s, *e),
                });
            }
            action = ModerationPolicy::strictest(
                action,
                self.policy.action_for(Category::ContactNumbers),
            );
        }

        // 性邀约暗语（如「炮吗」）：规则层兜底，游戏大炮语境豁免
        let slang = scan_sexual_slang(&compact);
        if !slang.is_empty() {
            for (s, e, rule) in &slang {
                violations.push(Violation {
                    category: Category::Illegal,
                    rule: rule.clone(),
                    matched: slice_text(text, *s, *e),
                });
            }
            action = ModerationPolicy::strictest(action, self.policy.action_for(Category::Illegal));
        }

        // 外部交易暗语（32r / z还是w：z=支付宝 w=微信 q=QQ）→ 按广告引流处置
        let offsite = scan_offsite_trade(&compact);
        if !offsite.is_empty() {
            for (s, e, rule) in &offsite {
                violations.push(Violation {
                    category: Category::Ads,
                    rule: rule.clone(),
                    matched: slice_text(text, *s, *e),
                });
            }
            action = ModerationPolicy::strictest(action, self.policy.action_for(Category::Ads));
        }

        // 链接变体（全角域名/点号改写/去点连接）→ 按广告引流处置
        let urls = scan_url_variants(&compact);
        if !urls.is_empty() {
            for (s, e, rule) in &urls {
                violations.push(Violation {
                    category: Category::Ads,
                    rule: rule.clone(),
                    matched: slice_text(text, *s, *e),
                });
            }
            action = ModerationPolicy::strictest(action, self.policy.action_for(Category::Ads));
        }

        Verdict { action, violations }
    }

    /// 语义层结论（无具体命中区间）→ Verdict：按类别处置动作聚合。
    pub fn verdict_from_categories(&self, categories: Vec<Category>) -> Verdict {
        let mut violations = Vec::new();
        let mut action = Action::Allow;
        for category in categories {
            violations.push(Violation {
                category,
                rule: "semantic".into(),
                matched: String::new(),
            });
            action = ModerationPolicy::strictest(action, self.policy.action_for(category));
        }
        Verdict { action, violations }
    }

    /// 按结论打码（对外主入口）：无具体命中区间（语义命中）时整段打码。
    pub fn sanitize_verdict(&self, text: &str, verdict: &Verdict) -> String {
        if verdict.is_clean() {
            return text.to_string();
        }
        // 语义命中没有区间：matched 全空 → 整段打码
        if verdict.violations.iter().all(|v| v.matched.is_empty()) {
            let count = text.chars().count();
            return "＊".repeat(count);
        }
        self.sanitize(text, verdict)
    }

    /// 按结论打码：每个命中区间替换为等字符数的全角星号。
    pub fn sanitize(&self, text: &str, verdict: &Verdict) -> String {
        if verdict.is_clean() {
            return text.to_string();
        }
        let loose = NormView::build(text, false);
        let compact = NormView::build(text, true);
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        for keyword in self.ads_keywords.iter().chain(&self.illegal_keywords) {
            if let Some((s, e)) = find_in(&loose, keyword).or_else(|| find_in(&compact, keyword)) {
                ranges.push((s, e));
            }
        }
        for (s, e, _) in scan_numbers(&compact, self.policy.min_digits.max(1)) {
            ranges.push((s, e));
        }
        for (s, e, _) in scan_contact_ids(&compact) {
            ranges.push((s, e));
        }
        for (s, e, _) in scan_sexual_slang(&compact) {
            ranges.push((s, e));
        }
        for (s, e, _) in scan_mixed_numbers(&compact, self.policy.min_digits.max(1)) {
            ranges.push((s, e));
        }
        for (s, e, _) in scan_offsite_trade(&compact) {
            ranges.push((s, e));
        }
        for (s, e, _) in scan_url_variants(&compact) {
            ranges.push((s, e));
        }
        ranges.sort_unstable();

        let mut out = String::with_capacity(text.len());
        let mut copy_from = 0;
        let mut last_end = 0;
        for (start, end) in ranges {
            if start < last_end {
                continue; // 与上一段重叠，已覆盖
            }
            out.push_str(&text[copy_from..start]);
            for _ in text[start..end].chars() {
                out.push('＊');
            }
            copy_from = end;
            last_end = end;
        }
        out.push_str(&text[copy_from..]);
        out
    }

    /// 输出侧 Block 的固定拒答文案。
    pub fn refusal_text(&self) -> &'static str {
        "（该回复因包含违规内容被拦截）"
    }
}

/// 在视图里找关键词；关键词本身也按紧凑方式归一化（"加 群" 一样能命中）。
fn find_in(view: &NormView, keyword: &str) -> Option<(usize, usize)> {
    let needle = NormView::build(keyword, true).text;
    if needle.is_empty() {
        return None;
    }
    let found = view.text.find(&needle)?;
    // 字节偏移 → 字符序号（map 按字符序记录）
    let start = view.char_idx(found);
    let end = start + needle.chars().count();
    Some(view.orig_range(start, end))
}

/// 扫描紧凑视图里的号码形态：手机（1 开头 11 位）/ QQ（qq 前缀）/ 长数字串。
/// 返回原文字节区间 + 规则名。
fn scan_numbers(view: &NormView, min_digits: usize) -> Vec<(usize, usize, String)> {
    let text = &view.text;
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let digits = i - start;
        if digits < min_digits {
            continue;
        }
        let rule = if digits == 11 && bytes[start] == b'1' {
            "mobile"
        } else if text[..start].ends_with("qq") {
            "qq"
        } else {
            "digits"
        };
        // 字节偏移先换算成字符序号，再映回原文字节区间
        let (cs, ce) = (view.char_idx(start), view.char_idx(i));
        let (os, oe) = view.orig_range(cs, ce);
        out.push((os, oe, rule.to_string()));
    }
    out
}

fn slice_text(text: &str, start: usize, end: usize) -> String {
    text.get(start..end.min(text.len())).unwrap_or("").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn moderator() -> Moderator {
        Moderator::new(&ModerationPolicy::default())
    }

    #[test]
    fn clean_text_passes() {
        let m = moderator();
        let v = m.inspect("村长你好，附近的钻石矿在哪里？");
        assert!(v.is_clean());
        assert_eq!(v.action, Action::Allow);
    }

    #[test]
    fn detects_ad_keywords() {
        let m = moderator();
        let v = m.inspect("低价代练，加群了解详情");
        assert!(v.violations.iter().any(|x| x.category == Category::Ads));
        assert_eq!(v.action, Action::Sanitize);
    }

    #[test]
    fn detects_qq_and_mobile_numbers() {
        let m = moderator();
        let v = m.inspect("加我qq 12345678 聊");
        assert!(
            v.violations.iter().any(|x| x.rule == "qq"),
            "expect qq hit, got {:?}",
            v.violations
        );
        let v2 = m.inspect("联系电话 13812345678");
        assert!(v2.violations.iter().any(|x| x.rule == "mobile"));
    }

    #[test]
    fn detects_spaced_out_bypass() {
        let m = moderator();
        // 插空格/星号的号码与关键词
        let v = m.inspect("q q 1 2 3 4 5 6 7 8");
        assert!(v.violations.iter().any(|x| x.rule == "qq"), "got {:?}", v.violations);
        let v2 = m.inspect("低*价*代*练");
        assert!(v2.violations.iter().any(|x| x.category == Category::Ads));
    }

    #[test]
    fn detects_illegal_keywords() {
        let m = moderator();
        let v = m.inspect("这里有外挂出售");
        assert!(v.violations.iter().any(|x| x.category == Category::Illegal));
    }

    #[test]
    fn block_action_wins_when_configured() {
        let policy = ModerationPolicy {
            illegal: Action::Block,
            ..ModerationPolicy::default()
        };
        let m = Moderator::new(&policy);
        let v = m.inspect("卖外挂");
        assert_eq!(v.action, Action::Block);
        // 干净文本不受 Block 配置影响
        let clean = m.inspect("今天天气不错");
        assert_eq!(clean.action, Action::Allow);
    }

    #[test]
    fn sanitize_masks_matched_spans() {
        let m = moderator();
        let text = "低价代练加群12345678";
        let v = m.inspect(text);
        let out = m.sanitize(text, &v);
        assert!(!out.contains("代练"), "out={out}");
        assert!(!out.contains("12345678"), "out={out}");
        assert!(out.contains('＊'));
    }

    #[test]
    fn contact_id_with_platform_word_is_hit() {
        // 线上冒烟回归：字母数字混合 ID 跟在平台词后 → 规则层直接命中
        let m = moderator();
        let v = m.inspect("这是我的微信 abc123");
        assert!(
            v.violations.iter().any(|x| x.rule == "contact_id"),
            "混合 ID 应命中 contact_id，got {:?}",
            v.violations
        );
        assert_eq!(v.action, Action::Sanitize);
        // 打码只盖 ID，不伤「这是我的微信」
        let out = m.sanitize("这是我的微信 abc123", &v);
        assert!(out.contains("这是我的微信"), "out={out}");
        assert!(!out.contains("abc123"), "out={out}");
        // 普通聊天不受影响
        let v2 = m.inspect("今天谁来挖矿");
        assert!(v2.is_clean(), "got {:?}", v2.violations);
        // 纯数字串仍走原规则
        let v3 = m.inspect("加我qq 987654321");
        assert!(v3.violations.iter().any(|x| x.rule == "qq" || x.rule == "digits"));
    }

    #[test]
    fn homophone_and_lookalike_folding_hits() {
        let m = moderator();
        // 裙→群 浩→号：裙号 114514（keyword 命中折叠后的「裙号」，digits 命中号码）
        let v = m.inspect("裙号 114514");
        assert!(
            v.violations.iter().any(|x| x.rule == "keyword" || x.rule == "contact_id"),
            "裙号应命中，got {:?}",
            v.violations
        );
        assert!(
            v.violations.iter().any(|x| x.rule == "digits"),
            "号码应命中，got {:?}",
            v.violations
        );
        // 谐音「+裙」→「加群」（广告关键词命中）
        assert!(m.inspect("+裙 114514").violations.iter().any(|x| x.rule == "keyword"));
        // 威心→微信
        assert!(m.inspect("威心 abc123").violations.iter().any(|x| x.rule == "contact_id"));
        // 数学字母块 qq（纯数字串走 digits 规则，混合 ID 走 contact_id）
        assert!(
            m.inspect("加𝐪𝐪 987654321").violations.iter().any(|x| x.rule == "qq" || x.rule == "digits"),
            "数学qq纯数字，got {:?}",
            m.inspect("加𝐪𝐪 987654321").violations
        );
        assert!(
            m.inspect("加𝐪𝐪 abc123").violations.iter().any(|x| x.rule == "contact_id"),
            "数学qq混合ID，got {:?}",
            m.inspect("加𝐪𝐪 abc123").violations
        );
        // 不误伤：普通聊天（裙=衣服）
        assert!(m.inspect("这条裙子真好看").is_clean(), "got {:?}", m.inspect("这条裙子真好看").violations);
        // 不误伤：常见词「群聊」本身不在关键词表（加群才是）
        assert!(m.inspect("群里见").is_clean(), "got {:?}", m.inspect("群里见").violations);
    }

    #[test]
    fn mixed_digit_variants_are_numbers() {
        let m = moderator();
        // 线上样本：中文数字+带圈数字+插干扰字（「删掉」「字」「符」是干扰）
        let v = m.inspect("1二删叁掉肆伍⑥字❼符❿");
        assert!(
            v.violations.iter().any(|x| x.rule == "digits_mixed"),
            "混排数字应命中，got {:?}",
            v.violations
        );
        // 打码覆盖从首到尾的整段（含干扰字）
        let out = m.sanitize_verdict("1二删叁掉肆伍⑥字❼符❿", &v);
        assert!(!out.contains("叁") && !out.contains("删"), "out={out}");
        // 全角手机号
        assert!(m.inspect("１３８１２３４５６７８").violations.iter().any(|x| x.rule == "digits_mixed"));
        // 中文数字枚举（无数量级字）
        assert!(m.inspect("号是一三八八九七六五四三二").violations.iter().any(|x| x.rule == "digits_mixed"));
        // 不误伤：正常日期（无变体字符）
        assert!(m.inspect("2024年12月上线").is_clean(), "got {:?}", m.inspect("2024年12月上线").violations);
        // 不误伤：中文大数（含数量级字）
        assert!(m.inspect("一百二十三万四千个石头").is_clean(), "got {:?}", m.inspect("一百二十三万四千个石头").violations);
        // 不误伤：正常聊天
        assert!(m.inspect("给我64个石头").is_clean());
    }

    #[test]
    fn offsite_trade_shorthand_is_hit() {
        let m = moderator();
        // 价码 + 支付暗语：r=人民币 z=支付宝 w=微信
        let v = m.inspect("出售钻石 32r,z还是w");
        assert!(
            v.violations.iter().any(|x| x.rule == "offsite_trade"),
            "外部交易暗语应命中，got {:?}",
            v.violations
        );
        let out = m.sanitize_verdict("出售钻石 32r,z还是w", &v);
        assert!(!out.contains("32r") && !out.contains("z"), "out={out}");
        // 纯支付暗语
        assert!(m.inspect("买火力包私我，z还是w都行").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 引诱词+暗语
        assert!(m.inspect("加w详聊").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 不误伤：正常聊天
        assert!(m.inspect("wsl 玩什么是了").is_clean(), "got {:?}", m.inspect("wsl 玩什么是了").violations);
    }

    #[test]
    fn rmt_price_with_trade_verb_is_hit() {
        let m = moderator();
        // 线上漏网样本：交易动词 + 1 位价码 + 单位报价（收钻石块1r1组）
        let v = m.inspect("收钻石块1r1组");
        assert!(
            v.violations.iter().any(|x| x.rule == "offsite_trade"),
            "线下收购应命中，got {:?}",
            v.violations
        );
        let out = m.sanitize_verdict("收钻石块1r1组", &v);
        assert!(!out.contains("1r"), "价码应被打码，out={out}");
        // 交易动词 + 1 位价码（无单位）
        assert!(m.inspect("出铁锭3r").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 单位报价本身独立成规则（无交易动词：2r1组）
        assert!(m.inspect("钻石剑2r1把").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 谐音折叠后同样命中：兽=售
        assert!(m.inspect("兽附魔书5r1本").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 大写变体：32R 照样拦
        assert!(m.inspect("出售钻石32R").violations.iter().any(|x| x.rule == "offsite_trade"));
        // 不误伤：序号/数量/坐标/版本号
        assert!(m.inspect("第1名是你").is_clean(), "got {:?}", m.inspect("第1名是你").violations);
        assert!(m.inspect("我1个人玩").is_clean(), "got {:?}", m.inspect("我1个人玩").violations);
        assert!(m.inspect("来x1z2坐标挖矿").is_clean(), "got {:?}", m.inspect("来x1z2坐标挖矿").violations);
    }

    #[test]
    fn url_variants_are_hit() {
        let m = moderator();
        assert!(m.inspect("来 example.com 看").violations.iter().any(|x| x.rule == "url_variant"));
        // 点号改写
        assert!(m.inspect("上 taobao。cc 买").violations.iter().any(|x| x.rule == "url_variant"));
        // 全角域名
        assert!(m.inspect("ｗｗｗ．ａｂｃ．ｃｏｍ").violations.iter().any(|x| x.rule == "url_variant"));
        // 不误伤：come 含 com 但后面是字母
        assert!(m.inspect("please come here").is_clean(), "got {:?}", m.inspect("please come here").violations);
        // 不误伤：普通聊天
        assert!(m.inspect("帮我看看这个截图").is_clean());
    }

    #[test]
    fn sexual_slang_pao_ma_hits_with_context_exemption() {
        let m = moderator();
        // 线上漏网样本：性邀约暗语必须拦
        let v = m.inspect("你好，炮吗？");
        assert!(
            v.violations.iter().any(|x| x.rule == "sexual_slang"),
            "炮吗 应命中 sexual_slang，got {:?}",
            v.violations
        );
        // 打码只盖「炮吗」两字，问号和称呼保留
        let out = m.sanitize_verdict("你好，炮吗？", &v);
        assert_eq!(out, "你好，＊＊？", "out={out}");
        // 游戏大炮语境豁免：正常讨论不得误伤
        let v2 = m.inspect("红石炮吗？教我造");
        assert!(
            v2.is_clean(),
            "红石炮吗 是游戏讨论不应命中，got {:?}",
            v2.violations
        );
    }

    #[test]
    fn five_digit_run_is_contact_number() {
        // 线上实测回归：min_digits=5 时 "123456" 必须在规则层命中，
        // 不能漏给语义层（模型会幻觉成违法）。
        let m = moderator();
        let v = m.inspect("123456");
        assert!(
            v.violations.iter().any(|x| x.rule == "digits"),
            "6 位数字串应命中 digits 规则，got {:?}",
            v.violations
        );
        assert_eq!(v.action, Action::Sanitize);
        // 四位数字（坐标、数量常见形态）不触发
        let v2 = m.inspect("给我1234个");
        assert!(
            v2.violations.iter().all(|x| x.rule != "digits"),
            "4 位数字不应触发，got {:?}",
            v2.violations
        );
    }

    #[test]
    fn extra_keywords_take_effect() {
        let policy = ModerationPolicy {
            extra_ads_keywords: vec!["神秘商店".into()],
            ..ModerationPolicy::default()
        };
        let m = Moderator::new(&policy);
        let v = m.inspect("欢迎来到神秘商店");
        assert!(v.violations.iter().any(|x| x.category == Category::Ads));
    }
}
