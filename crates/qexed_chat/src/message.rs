use bytes::{Bytes, BytesMut};
use qexed_packet::{Packet, PacketCodec, net_types::VarInt};
use qexed_packet::net_types::GameProfile;
use qexed_protocol::{
    to_client::play::{
        player_chat::{PackedMessageSignature, PlayerChat},
        system_chat::SystemChat,
    },
    to_server::play::chat_message::ChatMessage,
    types::ChatSessionData,
};

use crate::config::ChatConfig;
use crate::error::ChatError;
use crate::formatting::text_component;
use crate::rate_limit::{ChatRateLimit, validate_player_chat_message};
use crate::session::SecureChatSession;

/// 处理单个玩家连接内所有的聊天消息。
///
/// 负责：
/// - 限速与长度检查
/// - 安全聊天签名验证（启用时）
/// - 生成广播包（PlayerChat 或 SystemChat）
///
/// 不负责：
/// - 内容过滤（由调用方在外部异步执行）
/// - 实际广播 / 写 socket（由调用方执行）
pub struct ChatMessageHandler {
    rate_limit: ChatRateLimit,
    secure_session: Option<SecureChatSession>,
    enforce_secure_chat: bool,
    next_global_index: i32,
}

/// 聊天消息通过校验后返回的广播包。
///
/// 调用方需要把这个包同时发送给：
/// - 发送者本人（`sink.send_raw(packet)`）
/// - 其他所有玩家（`players.broadcast_packets_except(sender_uuid, vec![packet])`）
#[derive(Debug, Clone)]
pub struct ChatAccepted {
    pub packet: Bytes,
}

impl ChatMessageHandler {
    /// `enforce_secure_chat` 一般等于服务器在线模式
    /// （见 v4 `play.rs` 里的 `effective_online_mode`）。
    pub fn new(config: &ChatConfig, enforce_secure_chat: bool) -> Self {
        Self {
            rate_limit: ChatRateLimit::new(config),
            secure_session: None,
            enforce_secure_chat,
            next_global_index: 0,
        }
    }

    /// 客户端发送 `ChatSessionUpdate` 时调用。
    pub fn update_session(&mut self, chat_session: &ChatSessionData) -> Result<(), ChatError> {
        self.secure_session = Some(SecureChatSession::new(chat_session)?);
        Ok(())
    }

    pub fn has_session(&self) -> bool {
        self.secure_session.is_some()
    }

    /// 客户端发送 `ChatAck` 时调用。
    pub fn apply_offset(&mut self, offset: VarInt) -> Result<(), ChatError> {
        match self.secure_session.as_mut() {
            Some(session) => session.apply_offset(offset),
            None => Ok(()),
        }
    }

    pub fn max_length(&self) -> usize {
        self.rate_limit.max_length()
    }

    /// 处理一条来自客户端的 `ChatMessage`。
    ///
    /// - `chat` 是原始 serverbound 包。
    /// - `filtered_message` 是内容过滤后的消息内容；如果没启用内容过滤，
    ///   直接传 `chat.message.clone()`。
    ///
    /// 成功时返回 [`ChatAccepted`]，调用方负责广播。
    /// 失败时返回 [`ChatError`]，调用方负责根据错误类型构造面向用户的
    /// `SystemChat` 提示。
    pub fn handle(
        &mut self,
        profile: &GameProfile,
        chat: &ChatMessage,
        filtered_message: String,
    ) -> Result<ChatAccepted, ChatError> {
        // 1. 限速与长度检查。用原始消息，和 v4 行为一致。
        validate_player_chat_message(&mut self.rate_limit, &chat.message)?;

        // 2. 安全聊天或普通聊天。
        let packet = match self.secure_session.as_mut() {
            Some(session) => {
                let verified = session.verify_message(profile.uuid, chat)?;
                let packet = PlayerChat::pass_through(
                    self.next_global_index,
                    profile.uuid,
                    verified.index,
                    verified.signature.clone(),
                    verified
                        .last_seen
                        .into_iter()
                        .map(PackedMessageSignature::full)
                        .collect(),
                    filtered_message,
                    chat.timestamp,
                    chat.salt,
                    text_component(&profile.username),
                );
                session.add_pending_signature(&verified.signature);
                self.next_global_index += 1;
                encode_packet(packet)?
            }
            None => {
                if self.enforce_secure_chat {
                    return Err(ChatError::SecureChatRequired);
                }
                let packet = SystemChat {
                    content: text_component(format!(
                        "<{}> {}",
                        profile.username, filtered_message
                    )),
                    overlay: false,
                };
                encode_packet(packet)?
            }
        };

        Ok(ChatAccepted { packet })
    }
}

fn encode_packet<T: Packet>(packet: T) -> Result<Bytes, ChatError> {
    let mut buf = BytesMut::new();
    let mut writer = qexed_packet::PacketWriter::new(&mut buf);
    VarInt(T::ID)
        .serialize(&mut writer)
        .map_err(|err| ChatError::Encode(err.to_string()))?;
    packet
        .serialize(&mut writer)
        .map_err(|err| ChatError::Encode(err.to_string()))?;
    Ok(buf.freeze())
}