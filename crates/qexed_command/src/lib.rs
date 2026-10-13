//! qexed_command — 指令声明、下发与解析分发
//!
//! 职责链路：
//! 1. [`CommandTree`] 声明式构建指令树（literal / argument 节点）；
//! 2. [`CommandTree::to_packet`] 扁平化为协议包
//!    `qexed_protocol::to_client::play::commands::Commands`（nodes[0] 恒为 root，其余节点后序编号）；
//! 3. [`CommandTree::dispatch`] 把客户端上行 `chat_command` 字符串
//!    按树逐词匹配并解析参数，回调节点上的执行闭包；
//! 4. 解析失败回 [`CommandError`]（Unknown / Usage / BadArgument），
//!    由接入方转成聊天消息下发。
//!
//! 属性（properties）复用协议侧的 `Varies` 枚举；未提供属性时，
//! `Node::serialize` 内部走 `serialize_default_argument_properties`。

pub mod error;
pub mod tree;

pub use error::CommandError;
pub use tree::{ArgSpec, ArgValue, CommandContext, CommandNode, CommandTree, ParsedArgs, tokenize};

use qexed_protocol::to_client::play::commands::CommandArgumentType;

/// 便捷构造：字面量节点（子命令 / 指令名）
pub fn literal(name: impl Into<String>) -> CommandNode {
    CommandNode::literal(name)
}

/// 便捷构造：参数节点
///
/// * `name` — 出现在 usage 与 [`CommandContext`] 参数表中的名字；
/// * `parser` — 协议参数类型（决定客户端补全 UI 与属性字节）；
/// * `greedy` — true 时该参数吞掉行尾全部剩余文本（brigadier greedy 字符串）。
pub fn argument(
    name: impl Into<String>,
    parser: CommandArgumentType,
    greedy: bool,
) -> CommandNode {
    CommandNode::argument(ArgSpec {
        name: name.into(),
        parser,
        greedy,
        suggestion: None,
        properties: None,
    })
}

/// 便捷构造：带建议类型的参数节点（服务端补全，tab 时客户端回 CommandSuggestion 询问）
pub fn argument_with_suggestion(
    name: impl Into<String>,
    parser: CommandArgumentType,
    suggestion: impl Into<String>,
) -> CommandNode {
    CommandNode::argument(ArgSpec::new(name, parser).suggest(suggestion))
}