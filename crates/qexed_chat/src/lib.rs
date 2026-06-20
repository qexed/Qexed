use std::sync::Arc;

use anyhow::Context as _;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Public},
    sign::Verifier,
};

const LAST_SEEN_WINDOW_LEN: usize = 20;
const MAX_PENDING_LAST_SEEN_MESSAGES: usize = 1024;
const MAX_TRACKED_LAST_SEEN_MESSAGES: usize = LAST_SEEN_WINDOW_LEN + MAX_PENDING_LAST_SEEN_MESSAGES;

#[derive(Debug, Clone)]
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

#[derive(Debug)]
struct ChatServiceInner {
    config: qexed_config::app::qexed_chat::Chat,
    io_runtime: Option<DedicatedRuntime>,
}

#[derive(Debug)]
struct DedicatedRuntime {
    runtime: Option<tokio::runtime::Runtime>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub sender: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatResult {
    Enabled {
        message: ChatMessage,
        system_chat_only: bool,
    },
    Disabled,
}

impl ChatService {
    pub fn new(config: qexed_config::app::qexed_chat::Chat) -> anyhow::Result<Self> {
        Ok(Self {
            inner: Arc::new(ChatServiceInner {
                io_runtime: build_io_runtime(&config)?,
                config,
            }),
        })
    }

    pub fn config(&self) -> &qexed_config::app::qexed_chat::Chat {
        &self.inner.config
    }

    pub async fn process(&self, message: ChatMessage) -> anyhow::Result<ChatResult> {
        if let Some(runtime) = &self.inner.io_runtime {
            let config = self.inner.config.clone();
            let task = runtime.spawn(async move { process_chat(config, message).await });
            return task.await?;
        }

        process_chat(self.inner.config.clone(), message).await
    }
}

impl DedicatedRuntime {
    fn new(runtime: tokio::runtime::Runtime) -> Self {
        Self {
            runtime: Some(runtime),
        }
    }

    fn spawn<F>(&self, future: F) -> tokio::task::JoinHandle<F::Output>
    where
        F: std::future::Future + Send + 'static,
        F::Output: Send + 'static,
    {
        self.runtime
            .as_ref()
            .expect("dedicated runtime must exist before drop")
            .spawn(future)
    }
}

impl Drop for DedicatedRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn build_io_runtime(
    config: &qexed_config::app::qexed_chat::Chat,
) -> anyhow::Result<Option<DedicatedRuntime>> {
    if !config.io_pool.enabled {
        return Ok(None);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(config.io_pool.worker_threads.max(1))
        .thread_name("qexed-chat-io")
        .enable_all()
        .build()?;
    Ok(Some(DedicatedRuntime::new(runtime)))
}

async fn process_chat(
    config: qexed_config::app::qexed_chat::Chat,
    message: ChatMessage,
) -> anyhow::Result<ChatResult> {
    if !config.enabled {
        return Ok(ChatResult::Disabled);
    }

    Ok(ChatResult::Enabled {
        message,
        system_chat_only: config.system_chat_only,
    })
}

pub struct SecureChatSession {
    session_id: uuid::Uuid,
    player_key: PKey<Public>,
    last_seen: LastSeenValidator,
    next_index: i32,
    last_timestamp: i64,
}

impl SecureChatSession {
    pub fn new(chat_session: &qexed_protocol::types::ChatSessionData) -> anyhow::Result<Self> {
        Ok(Self {
            session_id: chat_session.session_id,
            player_key: PKey::public_key_from_der(&chat_session.public_key_der)
                .context("invalid player chat public key")?,
            last_seen: LastSeenValidator::default(),
            next_index: 0,
            last_timestamp: i64::MIN,
        })
    }

    pub fn apply_offset(&mut self, offset: qexed_packet::net_types::VarInt) -> anyhow::Result<()> {
        self.last_seen.apply_offset(offset.0)
    }

    pub fn add_pending_signature(
        &mut self,
        signature: &qexed_protocol::types::MessageSignature,
    ) -> anyhow::Result<()> {
        self.last_seen.add_pending(signature.clone())
    }

    pub fn verify_message(
        &mut self,
        profile_id: uuid::Uuid,
        chat: &qexed_protocol::to_server::play::chat_message::ChatMessage,
    ) -> anyhow::Result<VerifiedChatMessage> {
        let signature = chat
            .signature
            .as_ref()
            .context("missing signed chat message signature")?;

        validate_last_seen_update(chat)?;
        if chat.timestamp < self.last_timestamp {
            anyhow::bail!("out-of-order signed chat timestamp");
        }

        let last_seen = self.last_seen.apply_update(chat)?;
        let index = self.next_index;
        let mut verifier = Verifier::new(MessageDigest::sha256(), &self.player_key)?;
        write_signed_chat_payload(
            |bytes| verifier.update(bytes).map_err(Into::into),
            profile_id,
            self.session_id,
            index,
            chat.salt,
            chat.timestamp,
            &chat.message,
            &last_seen,
        )?;
        if !verifier.verify(&signature.0)? {
            anyhow::bail!("invalid signed chat message signature");
        }

        self.next_index = self
            .next_index
            .checked_add(1)
            .context("signed chat message index overflow")?;
        self.last_timestamp = chat.timestamp;

        Ok(VerifiedChatMessage {
            index,
            signature: signature.clone(),
            last_seen,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VerifiedChatMessage {
    pub index: i32,
    pub signature: qexed_protocol::types::MessageSignature,
    pub last_seen: Vec<qexed_protocol::types::MessageSignature>,
}

pub fn signed_chat_payload(
    profile_id: uuid::Uuid,
    session_id: uuid::Uuid,
    index: i32,
    salt: i64,
    timestamp_millis: i64,
    message: &str,
    last_seen_signatures: &[qexed_protocol::types::MessageSignature],
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(signed_chat_payload_len(message, last_seen_signatures));
    write_signed_chat_payload(
        |bytes| {
            payload.extend_from_slice(bytes);
            Ok(())
        },
        profile_id,
        session_id,
        index,
        salt,
        timestamp_millis,
        message,
        last_seen_signatures,
    )
    .expect("writing signed chat payload to Vec should not fail");
    payload
}

fn write_signed_chat_payload(
    mut write: impl FnMut(&[u8]) -> anyhow::Result<()>,
    profile_id: uuid::Uuid,
    session_id: uuid::Uuid,
    index: i32,
    salt: i64,
    timestamp_millis: i64,
    message: &str,
    last_seen_signatures: &[qexed_protocol::types::MessageSignature],
) -> anyhow::Result<()> {
    write(&1_i32.to_be_bytes())?;
    write(profile_id.as_bytes())?;
    write(session_id.as_bytes())?;
    write(&index.to_be_bytes())?;
    write(&salt.to_be_bytes())?;
    write(&(timestamp_millis / 1000).to_be_bytes())?;
    write(&(message.len() as i32).to_be_bytes())?;
    write(message.as_bytes())?;
    write(&(last_seen_signatures.len() as i32).to_be_bytes())?;
    for signature in last_seen_signatures {
        write(&signature.0)?;
    }
    Ok(())
}

fn signed_chat_payload_len(
    message: &str,
    last_seen_signatures: &[qexed_protocol::types::MessageSignature],
) -> usize {
    4 + 16
        + 16
        + 4
        + 8
        + 8
        + 4
        + message.len()
        + 4
        + last_seen_signatures
            .iter()
            .map(|signature| signature.0.len())
            .sum::<usize>()
}

fn validate_last_seen_update(
    chat: &qexed_protocol::to_server::play::chat_message::ChatMessage,
) -> anyhow::Result<()> {
    if chat.offset.0 < 0 {
        anyhow::bail!("negative signed chat last-seen offset {}", chat.offset.0);
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct LastSeenValidator {
    tracked_messages: Vec<Option<LastSeenTrackedEntry>>,
    last_pending_message: Option<qexed_protocol::types::MessageSignature>,
}

impl Default for LastSeenValidator {
    fn default() -> Self {
        Self {
            tracked_messages: vec![None; LAST_SEEN_WINDOW_LEN],
            last_pending_message: None,
        }
    }
}

impl LastSeenValidator {
    fn add_pending(
        &mut self,
        signature: qexed_protocol::types::MessageSignature,
    ) -> anyhow::Result<()> {
        if self.last_pending_message.as_ref() == Some(&signature) {
            return Ok(());
        }

        if self.tracked_messages.len() >= MAX_TRACKED_LAST_SEEN_MESSAGES {
            anyhow::bail!(
                "too many pending signed chat messages: {}",
                self.tracked_messages
                    .len()
                    .saturating_sub(LAST_SEEN_WINDOW_LEN)
            );
        }

        self.last_pending_message = Some(signature.clone());
        self.tracked_messages.push(Some(LastSeenTrackedEntry {
            signature,
            pending: true,
        }));
        Ok(())
    }

    fn apply_update(
        &mut self,
        chat: &qexed_protocol::to_server::play::chat_message::ChatMessage,
    ) -> anyhow::Result<Vec<qexed_protocol::types::MessageSignature>> {
        validate_last_seen_update(chat)?;
        self.apply_offset(chat.offset.0)?;

        let mut last_seen = Vec::with_capacity(LAST_SEEN_WINDOW_LEN);
        for i in 0..LAST_SEEN_WINDOW_LEN {
            let acknowledged = acknowledged_bit(chat.acknowledged, i);
            let message = self
                .tracked_messages
                .get_mut(i)
                .context("last-seen window missing entry")?;
            if acknowledged {
                let Some(entry) = message.as_mut() else {
                    anyhow::bail!("last-seen update acknowledged unknown message at index {i}");
                };
                entry.pending = false;
                last_seen.push(entry.signature.clone());
            } else {
                if message.as_ref().is_some_and(|entry| !entry.pending) {
                    anyhow::bail!(
                        "last-seen update ignored previously acknowledged message at index {i}"
                    );
                }
                *message = None;
            }
        }

        let checksum = last_seen_checksum(&last_seen);
        if chat.checksum != 0 && chat.checksum != checksum {
            anyhow::bail!(
                "last-seen checksum mismatch: expected {}, got {}",
                checksum,
                chat.checksum
            );
        }

        Ok(last_seen)
    }

    fn apply_offset(&mut self, offset: i32) -> anyhow::Result<()> {
        if offset < 0 {
            anyhow::bail!("negative last-seen offset {offset}");
        }

        let offset = offset as usize;
        let max_offset = self
            .tracked_messages
            .len()
            .saturating_sub(LAST_SEEN_WINDOW_LEN);
        if offset > max_offset {
            anyhow::bail!("last-seen offset {offset} exceeds max {max_offset}");
        }

        self.tracked_messages.drain(0..offset);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct LastSeenTrackedEntry {
    signature: qexed_protocol::types::MessageSignature,
    pending: bool,
}

fn acknowledged_bit(bytes: [u8; 3], index: usize) -> bool {
    let byte = bytes[index / 8];
    let bit = index % 8;
    byte & (1 << bit) != 0
}

fn last_seen_checksum(signatures: &[qexed_protocol::types::MessageSignature]) -> u8 {
    let mut checksum = 1_i32;
    for signature in signatures {
        checksum = checksum
            .wrapping_mul(31)
            .wrapping_add(java_byte_array_hash(&signature.0));
    }

    let value = checksum as i8 as u8;
    if value == 0 { 1 } else { value }
}

fn java_byte_array_hash(bytes: &[u8]) -> i32 {
    let mut result = 1_i32;
    for byte in bytes {
        result = result.wrapping_mul(31).wrapping_add(*byte as i8 as i32);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::{
        ChatMessage, ChatResult, ChatService, LastSeenValidator, MAX_PENDING_LAST_SEEN_MESSAGES,
        signed_chat_payload,
    };

    #[tokio::test]
    async fn allows_chat_when_chat_is_enabled() {
        let service = ChatService::new(Default::default()).unwrap();

        let decision = service
            .process(ChatMessage {
                sender: "Steve".to_string(),
                content: "hello".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(
            decision,
            ChatResult::Enabled {
                message: ChatMessage {
                    sender: "Steve".to_string(),
                    content: "hello".to_string()
                },
                system_chat_only: false,
            }
        );
    }

    #[tokio::test]
    async fn disables_chat_when_chat_is_disabled() {
        let mut config = qexed_config::app::qexed_chat::Chat::default();
        config.enabled = false;
        let service = ChatService::new(config).unwrap();

        let decision = service
            .process(ChatMessage {
                sender: "Steve".to_string(),
                content: "hello".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(decision, ChatResult::Disabled);
    }

    #[tokio::test]
    async fn can_use_dedicated_io_pool() {
        let mut config = qexed_config::app::qexed_chat::Chat::default();
        config.io_pool.enabled = true;
        config.io_pool.worker_threads = 1;
        let service = ChatService::new(config).unwrap();

        let decision = service
            .process(ChatMessage {
                sender: "Steve".to_string(),
                content: "io-pool".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(
            decision,
            ChatResult::Enabled {
                message: ChatMessage {
                    sender: "Steve".to_string(),
                    content: "io-pool".to_string(),
                },
                system_chat_only: false,
            }
        );
    }

    #[tokio::test]
    async fn can_force_system_chat() {
        let mut config = qexed_config::app::qexed_chat::Chat::default();
        config.system_chat_only = true;
        let service = ChatService::new(config).unwrap();

        let decision = service
            .process(ChatMessage {
                sender: "Steve".to_string(),
                content: "hello".to_string(),
            })
            .await
            .unwrap();

        assert_eq!(
            decision,
            ChatResult::Enabled {
                message: ChatMessage {
                    sender: "Steve".to_string(),
                    content: "hello".to_string()
                },
                system_chat_only: true,
            }
        );
    }

    #[test]
    fn signed_chat_payload_uses_epoch_seconds_for_timestamp() {
        let profile_id = uuid::Uuid::from_u128(1);
        let session_id = uuid::Uuid::from_u128(2);
        let payload = signed_chat_payload(profile_id, session_id, 3, 4, 5_678, "hi", &[]);

        assert!(payload.ends_with(&[0, 0, 0, 2, b'h', b'i', 0, 0, 0, 0]));
        assert!(
            payload
                .windows(8)
                .any(|window| window == 5_i64.to_be_bytes())
        );
    }

    #[test]
    fn signed_chat_payload_preallocates_exact_capacity() {
        let signatures = vec![
            qexed_protocol::types::MessageSignature(vec![1; 256]),
            qexed_protocol::types::MessageSignature(vec![2; 256]),
        ];

        let payload = signed_chat_payload(
            uuid::Uuid::from_u128(1),
            uuid::Uuid::from_u128(2),
            3,
            4,
            5_678,
            "hello",
            &signatures,
        );

        assert_eq!(payload.len(), payload.capacity());
    }

    #[test]
    fn last_seen_pending_messages_are_bounded() {
        let mut validator = LastSeenValidator::default();

        for i in 0..MAX_PENDING_LAST_SEEN_MESSAGES {
            validator
                .add_pending(qexed_protocol::types::MessageSignature(vec![i as u8; 256]))
                .unwrap();
        }

        let err = validator
            .add_pending(qexed_protocol::types::MessageSignature(vec![42; 256]))
            .unwrap_err();

        assert!(
            err.to_string()
                .contains("too many pending signed chat messages")
        );
    }
}
