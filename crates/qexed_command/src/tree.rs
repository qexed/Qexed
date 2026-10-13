//! 指令树：声明、协议扁平化、字符串解析分发。

use std::collections::HashMap;

use qexed_packet::net_types::VarInt;
use qexed_protocol::to_client::play::commands::{
    BrigadierString, CommandArgumentType as Parser, Commands, Node, Varies,
};

use crate::error::CommandError;

// ---------------------------------------------------------------------------
// 树声明
// ---------------------------------------------------------------------------

/// 参数节点声明（协议 argument 节点 + 服务端解析所需元数据）。
#[derive(Debug, Clone)]
pub struct ArgSpec {
    /// 参数名，出现在 usage 与 [`CommandContext`] 参数表。
    pub name: String,
    /// 协议参数类型（决定客户端补全 UI 与属性字节）。
    pub parser: Parser,
    /// true：按 brigadier GREEDY_PHRASE 下发，并吞掉行尾全部剩余文本。
    /// 仅对 `brigadier:string` 生效，其他类型忽略。
    pub greedy: bool,
    /// 服务端补全建议类型（如 `minecraft:ask_server`）。
    pub suggestion: Option<String>,
    /// 显式属性字节；None 时由协议侧默认序列化兜底
    /// （44..=48 注册表类解析器必须显式提供，否则序列化报错）。
    pub properties: Option<Varies>,
}

impl ArgSpec {
    /// 最简参数声明：仅名字 + 解析器。
    pub fn new(name: impl Into<String>, parser: Parser) -> Self {
        Self {
            name: name.into(),
            parser,
            greedy: false,
            suggestion: None,
            properties: None,
        }
    }

    /// 设置 greedy（吞掉行尾剩余文本）。
    pub fn greedy(mut self) -> Self {
        self.greedy = true;
        self
    }

    /// 设置服务端补全建议类型。
    pub fn suggest(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// 指令树节点：literal（含根）或 argument。
#[derive(Default)]
pub struct CommandNode {
    /// literal 名或参数名；根节点为空。
    pub name: String,
    /// argument 节点声明；None 即 literal。
    pub arg: Option<ArgSpec>,
    /// 子节点（同级顺序即协议 children 顺序，匹配时先到先得）。
    pub children: Vec<CommandNode>,
    /// 可执行回调；位于该节点且后续无词元时触发。
    pub executor: Option<Box<dyn FnMut(&CommandContext) -> Result<(), CommandError> + Send>>,
}

impl std::fmt::Debug for CommandNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandNode")
            .field("name", &self.name)
            .field("arg", &self.arg.as_ref().map(|a| &a.name))
            .field("children", &self.children.len())
            .field("executable", &self.executor.is_some())
            .finish()
    }
}

impl CommandNode {
    /// 字面量节点。
    pub fn literal(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            arg: None,
            children: Vec::new(),
            executor: None,
        }
    }

    /// 参数节点。
    pub fn argument(spec: ArgSpec) -> Self {
        Self {
            name: spec.name.clone(),
            arg: Some(spec),
            children: Vec::new(),
            executor: None,
        }
    }

    /// 附带执行回调（通常挂在参数链末尾，或直接挂 literal 上）。
    pub fn executes(
        mut self,
        f: impl FnMut(&CommandContext) -> Result<(), CommandError> + Send + 'static,
    ) -> Self {
        self.executor = Some(Box::new(f));
        self
    }

    /// 追加子节点，返回自身以便链式声明。
    pub fn child(mut self, node: CommandNode) -> Self {
        self.children.push(node);
        self
    }
}

/// 指令树根容器。
#[derive(Default)]
pub struct CommandTree {
    root: CommandNode,
}

impl CommandTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册一条顶层指令（literal），返回自身以便链式注册。
    pub fn command(mut self, node: CommandNode) -> Self {
        self.root.children.push(node);
        self
    }

    /// 按 literal 名取顶层指令。
    pub fn lookup(&self, name: &str) -> Option<&CommandNode> {
        self.root
            .children
            .iter()
            .find(|c| c.arg.is_none() && c.name == name)
    }

    /// 顶层指令名列表。
    pub fn command_names(&self) -> Vec<&str> {
        self.root
            .children
            .iter()
            .filter(|c| c.arg.is_none())
            .map(|c| c.name.as_str())
            .collect()
    }

    /// 深度优先收集某节点全部子孙的 literal 名（ask_server 补全用）。
    pub fn descendant_literals(node: &CommandNode, out: &mut Vec<String>) {
        for child in &node.children {
            if child.arg.is_none() {
                out.push(child.name.clone());
            }
            Self::descendant_literals(child, out);
        }
    }

    // -----------------------------------------------------------------------
    // 协议扁平化
    // -----------------------------------------------------------------------

    /// 扁平化为协议包：深度优先后序编号（子在前父在后），root_index 恒为 0。
    pub fn to_packet(&self) -> Commands {
        // 原版协议要求 nodes[0] 即 root（root_index = 0）；
        // root 的 children 指向后写的节点属合法前向引用（客户端读完整个列表再建树）。
        let mut nodes: Vec<Node> = vec![Node {
            flags: 0,
            ..Node::default()
        }];
        let root_children = Self::emit(&mut nodes, &self.root);
        nodes[0].children = root_children;
        Commands {
            nodes,
            root_index: VarInt(0),
        }
    }

    /// 后序下发：先编码全部子孙取得其索引，再挂到自身 children。
    fn emit(out: &mut Vec<Node>, node: &CommandNode) -> Vec<VarInt> {
        node.children
            .iter()
            .map(|child| {
                let child_children = Self::emit(out, child);
                out.push(Self::proto_node(child, child_children));
                VarInt(out.len() as i32 - 1)
            })
            .collect()
    }

    /// 单节点 -> 协议 Node（含 flags / parser id / 属性 / 补全类型）。
    fn proto_node(node: &CommandNode, children: Vec<VarInt>) -> Node {
        let executable = node.executor.is_some();
        match &node.arg {
            None => Node {
                // literal（0x01）；可执行加执行位 0x04
                flags: if executable { 0x05 } else { 0x01 },
                children,
                redirect_node: None,
                name: Some(node.name.clone()),
                parser_id: None,
                properties: None,
                suggestions_type: None,
            },
            Some(arg) => {
                // greedy：强制 brigadier:string + GREEDY_PHRASE(2) 属性
                if arg.greedy && matches!(arg.parser, Parser::BrigadierString(_)) {
                    return Node {
                        // argument（0x02）；可执行加执行位 0x04
                        flags: if executable { 0x06 } else { 0x02 },
                        children,
                        redirect_node: None,
                        name: Some(node.name.clone()),
                        parser_id: Some(VarInt(5)),
                        properties: Some(Varies::BrigadierString(BrigadierString {
                            behavior: VarInt(2),
                        })),
                        suggestions_type: arg.suggestion.clone(),
                    };
                }
                Node {
                    flags: if executable { 0x06 } else { 0x02 },
                    children,
                    redirect_node: None,
                    name: Some(node.name.clone()),
                    parser_id: Some(VarInt(arg.parser.id() as i32)),
                    properties: arg.properties.clone(),
                    suggestions_type: arg.suggestion.clone(),
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // 解析分发
    // -----------------------------------------------------------------------

    /// 解析并执行一行指令。`input` 为客户端 chat_command 的 command 字段
    /// （不含斜杠）。
    pub fn dispatch(&mut self, input: &str) -> Result<(), CommandError> {
        let tokens = tokenize(input);
        let Some(first) = tokens.first() else {
            return Err(CommandError::UnknownCommand(String::new()));
        };
        let Some(index) = self
            .root
            .children
            .iter()
            .position(|c| c.arg.is_none() && c.name == *first)
        else {
            return Err(CommandError::UnknownCommand(first.clone()));
        };
        let cmd = &mut self.root.children[index];
        let mut parsed = ParsedArgs::default();
        let mut bad_arg: Option<CommandError> = None;
        // 命中路径（自深向浅），用于定位真正停止的可执行节点
        let mut path: Vec<usize> = Vec::new();
        if Self::match_node(cmd, &tokens[1..], &mut parsed, &mut bad_arg, &mut path)? {
            path.reverse();
            let ctx = CommandContext {
                name: cmd.name.clone(),
                args: parsed,
            };
            let mut target: &mut CommandNode = cmd;
            for &i in &path {
                target = &mut target.children[i];
            }
            let executor = target.executor.as_mut().expect("可执行路径必有回调");
            executor(&ctx)
        } else if let Some(err) = bad_arg {
            // 有分支走到参数值解析失败 -> 报具体参数错误
            Err(err)
        } else {
            Err(CommandError::InvalidUsage {
                usage: Self::usage_of(cmd),
            })
        }
    }

    /// 递归匹配：tokens 对 `node` 的子层。
    /// 返回 true = tokens 在该子树内被完整消费并停在可执行节点。
    /// 参数值解析失败记入 `bad_arg`（首次），结构不匹配则静默回溯。
    fn match_node(
        node: &mut CommandNode,
        tokens: &[String],
        parsed: &mut ParsedArgs,
        bad_arg: &mut Option<CommandError>,
        path: &mut Vec<usize>,
    ) -> Result<bool, CommandError> {
        if tokens.is_empty() {
            return Ok(node.executor.is_some());
        }
        for (index, child) in node.children.iter_mut().enumerate() {
            match &child.arg {
                None => {
                    if child.name == tokens[0]
                        && Self::match_node(child, &tokens[1..], parsed, bad_arg, path)?
                    {
                        path.push(index);
                        return Ok(true);
                    }
                }
                Some(spec) => {
                    let greedy = spec.greedy && matches!(spec.parser, Parser::BrigadierString(_));
                    let width = if greedy { tokens.len() } else { 1 };
                    let consumed: Vec<String> = tokens[..width].to_vec();
                    let Ok(value) = parse_value(spec, &consumed) else {
                        if bad_arg.is_none() {
                            *bad_arg = Some(CommandError::BadArgument {
                                name: spec.name.clone(),
                                expected: expected_of(&spec.parser),
                                raw: consumed.join(" "),
                            });
                        }
                        continue;
                    };
                    let saved = parsed.clone();
                    parsed.insert(&spec.name, value);
                    if Self::match_node(child, &tokens[width..], parsed, bad_arg, path)? {
                        path.push(index);
                        return Ok(true);
                    }
                    // 回滚：换兄弟分支重试
                    *parsed = saved;
                }
            }
        }
        Ok(false)
    }

    /// 生成一条指令的 usage（多签名时以换行连接），
    /// 如 `msg <target> <message>`。
    pub fn usage_of(node: &CommandNode) -> String {
        let mut lines = Vec::new();
        Self::usage_lines(node, &node.name, &mut lines);
        if lines.is_empty() {
            node.name.clone()
        } else {
            lines.join("\n")
        }
    }

    fn usage_lines(node: &CommandNode, prefix: &str, out: &mut Vec<String>) {
        for child in &node.children {
            let name = match &child.arg {
                None => child.name.clone(),
                Some(spec) => format!("<{}>", spec.name),
            };
            let line = if prefix.is_empty() {
                name
            } else {
                format!("{prefix} {name}")
            };
            if child.executor.is_some() {
                out.push(line.clone());
            }
            if !child.children.is_empty() {
                Self::usage_lines(child, &line, out);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// 参数值与解析
// ---------------------------------------------------------------------------

/// 单个参数解析结果。
#[derive(Debug, Clone, PartialEq)]
pub enum ArgValue {
    Bool(bool),
    Float(f32),
    Double(f64),
    Int(i32),
    Long(i64),
    /// 原样字符串（普通 / greedy / 各类 minecraft 解析器统一保留原文，
    /// 语义校验交给执行回调）。
    Text(String),
}

impl ArgValue {
    /// 数值视图（bool 除外）：整型走 i64，浮点走 f64。
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Self::Int(v) => Some(*v as i64),
            Self::Long(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Float(v) => Some(*v as f64),
            Self::Double(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v),
            _ => None,
        }
    }
}

/// 一条指令的参数表（保序 + 按名索引）。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedArgs {
    order: Vec<(String, ArgValue)>,
    by_name: HashMap<String, ArgValue>,
}

impl ParsedArgs {
    pub fn get(&self, name: &str) -> Option<&ArgValue> {
        self.by_name.get(name)
    }

    pub fn bool(&self, name: &str) -> Option<bool> {
        self.get(name)?.as_bool()
    }

    /// 整数视图：Int / Long（Long 超出 i32 时经 as_i64 自行转换）。
    pub fn int(&self, name: &str) -> Option<i32> {
        i32::try_from(self.get(name)?.as_i64()?).ok()
    }

    pub fn long(&self, name: &str) -> Option<i64> {
        self.get(name)?.as_i64()
    }

    pub fn double(&self, name: &str) -> Option<f64> {
        self.get(name)?.as_f64()
    }

    pub fn text(&self, name: &str) -> Option<&str> {
        self.get(name)?.as_text()
    }

    /// 按声明顺序遍历。
    pub fn iter(&self) -> impl Iterator<Item = (&String, &ArgValue)> {
        self.order.iter().map(|(k, v)| (k, v))
    }

    fn insert(&mut self, name: &str, value: ArgValue) {
        self.order.push((name.to_string(), value.clone()));
        self.by_name.insert(name.to_string(), value);
    }
}

/// 执行回调拿到的上下文。
#[derive(Debug, Clone)]
pub struct CommandContext {
    /// 顶层指令名。
    pub name: String,
    /// 已解析参数。
    pub args: ParsedArgs,
}

/// 错误信息里期望类型的可读名。
fn expected_of(parser: &Parser) -> &'static str {
    match parser {
        Parser::BrigadierBool => "true|false",
        Parser::BrigadierFloat(_) => "f32",
        Parser::BrigadierDouble(_) => "f64",
        Parser::BrigadierInteger(_) => "i32",
        Parser::BrigadierLong(_) => "i64",
        _ => "text",
    }
}

/// 按协议类型解析词元；greedy 参数已由调用方聚合为单 token。
fn parse_value(spec: &ArgSpec, raw: &[String]) -> Result<ArgValue, CommandError> {
    let joined = raw.join(" ");
    let bad = || CommandError::BadArgument {
        name: spec.name.clone(),
        expected: expected_of(&spec.parser),
        raw: joined.clone(),
    };
    let head = raw.first().map(String::as_str).unwrap_or("");
    Ok(match &spec.parser {
        Parser::BrigadierBool => match head {
            "true" => ArgValue::Bool(true),
            "false" => ArgValue::Bool(false),
            _ => return Err(bad()),
        },
        Parser::BrigadierFloat(_) => head.parse::<f32>().map(ArgValue::Float).map_err(|_| bad())?,
        Parser::BrigadierDouble(_) => {
            head.parse::<f64>().map(ArgValue::Double).map_err(|_| bad())?
        }
        Parser::BrigadierInteger(_) => head.parse::<i32>().map(ArgValue::Int).map_err(|_| bad())?,
        Parser::BrigadierLong(_) => head.parse::<i64>().map(ArgValue::Long).map_err(|_| bad())?,
        _ => ArgValue::Text(joined),
    })
}

/// 词法切分：空白分隔；双引号段整体成 token，段内 \" 转义。
pub fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut has_token = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' if in_quotes => match chars.peek() {
                // 引号内 \" 与 \\ 转义
                Some('"') | Some('\\') => {
                    cur.push(chars.next().unwrap());
                }
                _ => cur.push('\\'),
            },
            '"' if in_quotes => in_quotes = false,
            '"' => {
                in_quotes = true;
                has_token = true;
            }
            c if c.is_whitespace() && !in_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut cur));
                    has_token = false;
                }
            }
            c => {
                cur.push(c);
                has_token = true;
            }
        }
    }
    if has_token {
        tokens.push(cur);
    }
    tokens
}