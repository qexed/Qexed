use bytes::{Bytes, BytesMut};
use qexed_packet::net_types::GameProfile;
use qexed_packet::{Packet, PacketCodec, net_types::VarInt};
use qexed_protocol::{
    to_client::play::{
        player_chat::{ChatTypeHolder, PackedMessageSignature, PlayerChat},
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
    /// 客户端最新上报的聊天会话原样保存：转发给所有客户端
    /// （INITIALIZE_CHAT），否则接收方客户端会把它当作未签名消息验签，
    /// 弹出 "chat.validation_error"。
    chat_session: Option<ChatSessionData>,
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
            chat_session: None,
        }
    }

    /// 客户端发送 `ChatSessionUpdate` 时调用。
    pub fn update_session(&mut self, chat_session: &ChatSessionData) -> Result<(), ChatError> {
        self.secure_session = Some(SecureChatSession::new(chat_session)?);
        self.chat_session = Some(chat_session.clone());
        Ok(())
    }

    /// 最近一次上报的聊天会话；供调用方向客户端广播 INITIALIZE_CHAT。
    pub fn chat_session(&self) -> Option<&ChatSessionData> {
        self.chat_session.as_ref()
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
    /// `filtered_message` 与原文不同（内容过滤改写了文本）时，安全聊天
    /// 路径验签后仍走 `PlayerChat` 签名链：plain_message 保持签名覆盖的
    /// 原文，改写文本放进 `unsigned_chat_content` 覆盖显示——客户端显示
    /// 打码文本并标记「已修改」（仅样式提示，不报错），签名链不断裂。
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
        // 内容过滤改写了文本时，链字段（index/last_seen/签名）原样转发保持
        // 连续；plain_message 携带打码文本，原文不出服务端。
        let content_modified = filtered_message != chat.message;
        self.handle_inner(profile, chat, filtered_message, content_modified)
    }

    /// 无过滤直通：消息原样广播（签名路径可用，行为与原版一致）。
    pub fn handle_unfiltered(
        &mut self,
        profile: &GameProfile,
        chat: &ChatMessage,
    ) -> Result<ChatAccepted, ChatError> {
        self.handle_inner(profile, chat, chat.message.clone(), false)
    }

    /// 带定稿文本直通：调用方（如 jev 桥）已完成内容定稿，这里不再过滤。
    /// 与 `handle` 一样：文本与原文不同时，安全聊天下走 unsigned 覆盖显示。
    pub fn handle_unfiltered_with(
        &mut self,
        profile: &GameProfile,
        chat: &ChatMessage,
        final_text: String,
    ) -> Result<ChatAccepted, ChatError> {
        let content_modified = final_text != chat.message;
        self.handle_inner(profile, chat, final_text, content_modified)
    }

    fn handle_inner(
        &mut self,
        profile: &GameProfile,
        chat: &ChatMessage,
        filtered_message: String,
        content_modified: bool,
    ) -> Result<ChatAccepted, ChatError> {
        // 1. 限速与长度检查。用原始消息，和 v4 行为一致。
        validate_player_chat_message(&mut self.rate_limit, &chat.message)?;

        // 2. 安全聊天或普通聊天。
        // secure_chat_enabled=false（默认）时不做签名验证/转发——26.x 的
        // 签名链尚未验证，直接转发会被客户端拒收；统一走 SystemChat 回显。
        // 有签名会话时：原文走签名直通，改写文本走 unsigned 覆盖显示。
        let secure = self.secure_session.as_mut().filter(|_| self.enforce_secure_chat);
        let packet = match (secure, content_modified) {
            (Some(session), false) => {
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
            (Some(session), true) => {
                // 内容被改写（打码）时对齐原版「过滤」机制，而不是丢包：
                // 26.x 签名链环环相扣（每条签名覆盖发送者见过的签名列表），
                // 任何丢包/换文本都会让链验证失败 → 客户端「聊天验证错误」。
                // 所以 index/last_seen/签名/plain_message 全部原样转发
                // （签名必须验得过），显示层双保险：
                //   1) unsigned_chat_content = 打码文本（客户端优先显示它）
                //   2) filter_type = FULLY_FILTERED（忽略 unsigned 的路径
                //      也按「已过滤」渲染，绝不显示原文）
                let verified = session.verify_message(profile.uuid, chat)?;
                let packet = PlayerChat {
                    global_index: VarInt(self.next_global_index),
                    sender: profile.uuid,
                    index: VarInt(verified.index),
                    signature: Some(verified.signature.clone()),
                    // 签名覆盖原文，必须原样（验签用；显示层不可见）
                    plain_message: chat.message.clone(),
                    timestamp: chat.timestamp,
                    salt: chat.salt,
                    previous_messages: verified
                        .last_seen
                        .into_iter()
                        .map(PackedMessageSignature::full)
                        .collect(),
                    // 客户端实际显示：打码文本
                    unsigned_chat_content: Some(text_component(filtered_message)),
                    filter_type: VarInt(1), // FILTER_MASK_FULLY_FILTERED
                    chat_type: ChatTypeHolder::default(),
                    network_name: text_component(&profile.username),
                    network_target_name: None,
                };
                session.add_pending_signature(&verified.signature);
                self.next_global_index += 1;
                encode_packet(packet)?
            }
            (None, _) => {
                if self.enforce_secure_chat {
                    return Err(ChatError::SecureChatRequired);
                }
                let packet = SystemChat {
                    content: text_component(format!("<{}> {}", profile.username, filtered_message)),
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

#[cfg(test)]
mod modified_content_tests {
    use super::*;
    use openssl::{
        hash::MessageDigest,
        pkey::PKey,
        rsa::Rsa,
        sign::Signer,
    };
    use qexed_packet::net_types::GameProfile;
    use qexed_protocol::types::MessageSignature;
    use uuid::Uuid;

    fn config() -> ChatConfig {
        ChatConfig::default()
    }

    /// 生成 RSA 密钥对：私钥签名 / 公钥进 ChatSessionData。
    fn key_pair() -> (PKey<openssl::pkey::Private>, PKey<openssl::pkey::Public>) {
        let rsa = Rsa::generate(2048).expect("rsa keygen");
        let private = PKey::from_rsa(rsa).expect("private pkey");
        let public_der = private.rsa().expect("rsa ref").public_key_to_der().expect("public der");
        let public = PKey::public_key_from_der(&public_der).expect("public pkey");
        (private, public)
    }

    /// 按服务端 signed_chat_payload 布局做客户端签名。
    fn sign_payload(
        private: &PKey<openssl::pkey::Private>,
        profile_id: Uuid,
        session_id: Uuid,
        index: i32,
        salt: i64,
        timestamp: i64,
        message: &str,
    ) -> MessageSignature {
        let mut payload = Vec::new();
        payload.extend_from_slice(&1_i32.to_be_bytes());
        payload.extend_from_slice(profile_id.as_bytes());
        payload.extend_from_slice(session_id.as_bytes());
        payload.extend_from_slice(&index.to_be_bytes());
        payload.extend_from_slice(&salt.to_be_bytes());
        payload.extend_from_slice(&((timestamp / 1000) as i64).to_be_bytes());
        payload.extend_from_slice(&(message.len() as i32).to_be_bytes());
        payload.extend_from_slice(message.as_bytes());
        payload.extend_from_slice(&0_i32.to_be_bytes());
        let mut signer = Signer::new(MessageDigest::sha256(), private).expect("signer");
        signer.update(&payload).expect("update");
        MessageSignature(signer.sign_to_vec().expect("sign"))
    }

    fn signed_chat(
        private: &PKey<openssl::pkey::Private>,
        profile_id: Uuid,
        session_id: Uuid,
        message: &str,
    ) -> ChatMessage {
        ChatMessage {
            message: message.to_string(),
            timestamp: 1_700_000_000_000,
            salt: 42,
            signature: Some(sign_payload(private, profile_id, session_id, 0, 42, 1_700_000_000_000, message)),
            offset: VarInt(0),
            acknowledged: [0; 3],
            checksum: 0,
        }
    }

    /// 读包首 VarInt（含包 ID），用于区分包类型。
    fn packet_id(bytes: &[u8]) -> i32 {
        let mut value: i32 = 0;
        for (i, byte) in bytes.iter().enumerate() {
            value |= i32::from(byte & 0x7F) << (i * 7);
            if byte & 0x80 == 0 {
                return value;
            }
        }
        panic!("no varint");
    }

    #[test]
    fn modified_content_forwards_filtered() {
        let (private, public) = key_pair();
        let session_id = Uuid::new_v4();
        let profile = GameProfile {
            uuid: Uuid::new_v4(),
            username: "tester".to_string(),
            properties: Vec::new(),
        };
        let chat_session = ChatSessionData {
            session_id,
            expires_at_epoch_millis: 0,
            public_key_der: public.public_key_to_der().expect("der"),
            key_signature: Vec::new(),
        };
        let mut handler = ChatMessageHandler::new(&config(), true);
        handler.update_session(&chat_session).expect("session");

        let chat = signed_chat(&private, profile.uuid, session_id, "加我qq 98765432");
        let accepted = handler
            .handle(&profile, &chat, "加我qq ＊＊＊＊＊＊＊＊".to_string())
            .expect("modified content should be accepted");

        // 必须是 PlayerChat（0x41）：链字段原样转发保持 index/last_seen 连续
        //（「缺失或已被重新排序」/「聊天验证错误」只由链断裂触发）。
        assert_eq!(packet_id(&accepted.packet), 0x41, "应为 PlayerChat 包");
        // 签名覆盖原文：plain 必须带原文（验签通过），显示层由
        // unsigned_chat_content（打码）+ FULLY_FILTERED 遮蔽
        assert!(
            accepted.packet.windows(b"98765432".len()).any(|w| w == b"98765432"),
            "plain 必须保留原文供验签"
        );
        assert!(
            accepted.packet.windows("＊＊".len()).any(|w| w == "＊＊".as_bytes()),
            "unsigned 应携带打码文本"
        );
    }

    #[test]
    fn unmodified_content_under_secure_chat_passes_through() {
        let (private, public) = key_pair();
        let session_id = Uuid::new_v4();
        let profile = GameProfile {
            uuid: Uuid::new_v4(),
            username: "tester".to_string(),
            properties: Vec::new(),
        };
        let chat_session = ChatSessionData {
            session_id,
            expires_at_epoch_millis: 0,
            public_key_der: public.public_key_to_der().expect("der"),
            key_signature: Vec::new(),
        };
        let mut handler = ChatMessageHandler::new(&config(), true);
        handler.update_session(&chat_session).expect("session");

        let chat = signed_chat(&private, profile.uuid, session_id, "大家好");
        let accepted = handler
            .handle(&profile, &chat, "大家好".to_string())
            .expect("unmodified content should pass signature path");
        assert_eq!(packet_id(&accepted.packet), 0x41, "原文应走签名转发");
    }
}
