use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Public},
    sign::Verifier,
};
use qexed_packet::net_types::VarInt;
use qexed_protocol::{
    to_server::play::chat_message::ChatMessage,
    types::{ChatSessionData, MessageSignature},
};

use crate::error::ChatError;

pub struct SecureChatSession {
    session_id: uuid::Uuid,
    player_key: PKey<Public>,
    last_seen: LastSeenValidator,
    next_index: i32,
    last_timestamp: i64,
}

impl SecureChatSession {
    pub fn new(chat_session: &ChatSessionData) -> Result<Self, ChatError> {
        Ok(Self {
            session_id: chat_session.session_id,
            player_key: PKey::public_key_from_der(&chat_session.public_key_der)?,
            last_seen: LastSeenValidator::default(),
            next_index: 0,
            last_timestamp: i64::MIN,
        })
    }

    pub fn apply_offset(&mut self, offset: VarInt) -> Result<(), ChatError> {
        self.last_seen.apply_offset(offset.0)
    }

    pub fn add_pending_signature(&mut self, signature: &MessageSignature) {
        self.last_seen.add_pending(signature.clone());
    }

    pub fn verify_message(
        &mut self,
        profile_id: uuid::Uuid,
        chat: &ChatMessage,
    ) -> Result<VerifiedChatMessage, ChatError> {
        let signature = chat
            .signature
            .as_ref()
            .ok_or(ChatError::MissingSignature)?;

        validate_last_seen_update(chat)?;
        if chat.timestamp < self.last_timestamp {
            return Err(ChatError::OutOfOrderTimestamp);
        }

        let last_seen = self.last_seen.apply_update(chat)?;
        let index = self.next_index;
        let payload = signed_chat_payload(
            profile_id,
            self.session_id,
            index,
            chat.salt,
            chat.timestamp,
            &chat.message,
            &last_seen,
        );
        let mut verifier = Verifier::new(MessageDigest::sha256(), &self.player_key)?;
        verifier.update(&payload)?;
        if !verifier.verify(&signature.0)? {
            return Err(ChatError::InvalidSignature);
        }

        self.next_index = self
            .next_index
            .checked_add(1)
            .ok_or(ChatError::IndexOverflow)?;
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
    pub signature: MessageSignature,
    pub last_seen: Vec<MessageSignature>,
}

pub fn profile_key_payload(profile_id: uuid::Uuid, chat_session: &ChatSessionData) -> Vec<u8> {
    let mut payload = Vec::with_capacity(24 + chat_session.public_key_der.len());
    payload.extend_from_slice(profile_id.as_bytes());
    payload.extend_from_slice(&chat_session.expires_at_epoch_millis.to_be_bytes());
    payload.extend_from_slice(&chat_session.public_key_der);
    payload
}

pub fn signed_chat_payload(
    profile_id: uuid::Uuid,
    session_id: uuid::Uuid,
    index: i32,
    salt: i64,
    timestamp_millis: i64,
    message: &str,
    last_seen_signatures: &[MessageSignature],
) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&1_i32.to_be_bytes());
    payload.extend_from_slice(profile_id.as_bytes());
    payload.extend_from_slice(session_id.as_bytes());
    payload.extend_from_slice(&index.to_be_bytes());
    payload.extend_from_slice(&salt.to_be_bytes());
    payload.extend_from_slice(&(timestamp_millis / 1000).to_be_bytes());
    payload.extend_from_slice(&(message.len() as i32).to_be_bytes());
    payload.extend_from_slice(message.as_bytes());
    payload.extend_from_slice(&(last_seen_signatures.len() as i32).to_be_bytes());
    for signature in last_seen_signatures {
        payload.extend_from_slice(&signature.0);
    }
    payload
}

fn validate_last_seen_update(chat: &ChatMessage) -> Result<(), ChatError> {
    if chat.offset.0 < 0 {
        return Err(ChatError::NegativeOffset(chat.offset.0));
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct LastSeenValidator {
    tracked_messages: Vec<Option<LastSeenTrackedEntry>>,
    last_pending_message: Option<MessageSignature>,
}

impl Default for LastSeenValidator {
    fn default() -> Self {
        Self {
            tracked_messages: vec![None; 20],
            last_pending_message: None,
        }
    }
}

impl LastSeenValidator {
    fn add_pending(&mut self, signature: MessageSignature) {
        if self.last_pending_message.as_ref() == Some(&signature) {
            return;
        }

        self.last_pending_message = Some(signature.clone());
        self.tracked_messages.push(Some(LastSeenTrackedEntry {
            signature,
            pending: true,
        }));
    }

    fn apply_update(&mut self, chat: &ChatMessage) -> Result<Vec<MessageSignature>, ChatError> {
        validate_last_seen_update(chat)?;
        self.apply_offset(chat.offset.0)?;

        let mut last_seen = Vec::new();
        for i in 0..20 {
            let acknowledged = acknowledged_bit(chat.acknowledged, i);
            let message = self
                .tracked_messages
                .get_mut(i)
                .ok_or(ChatError::MissingWindowEntry)?;
            if acknowledged {
                let Some(entry) = message.as_mut() else {
                    return Err(ChatError::AcknowledgedUnknown(i));
                };
                entry.pending = false;
                last_seen.push(entry.signature.clone());
            } else {
                if message.as_ref().is_some_and(|entry| !entry.pending) {
                    return Err(ChatError::IgnoredAcknowledged(i));
                }
                *message = None;
            }
        }

        let checksum = last_seen_checksum(&last_seen);
        if chat.checksum != 0 && chat.checksum != checksum {
            return Err(ChatError::ChecksumMismatch {
                expected: checksum,
                actual: chat.checksum,
            });
        }

        Ok(last_seen)
    }

    fn apply_offset(&mut self, offset: i32) -> Result<(), ChatError> {
        if offset < 0 {
            return Err(ChatError::NegativeOffset(offset));
        }

        let offset = offset as usize;
        let max_offset = self.tracked_messages.len().saturating_sub(20);
        if offset > max_offset {
            return Err(ChatError::OffsetExceedsWindow {
                offset,
                max: max_offset,
            });
        }

        self.tracked_messages.drain(0..offset);
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct LastSeenTrackedEntry {
    signature: MessageSignature,
    pending: bool,
}

fn acknowledged_bit(bytes: [u8; 3], index: usize) -> bool {
    let byte = bytes[index / 8];
    let bit = index % 8;
    byte & (1 << bit) != 0
}

fn last_seen_checksum(signatures: &[MessageSignature]) -> u8 {
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
    use super::{profile_key_payload, signed_chat_payload};

    #[test]
    fn profile_key_payload_matches_minecraft_layout() {
        let profile_id = uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff);
        let chat_session = qexed_protocol::types::ChatSessionData {
            expires_at_epoch_millis: 0x0102030405060708,
            public_key_der: vec![9, 10, 11],
            ..Default::default()
        };

        let payload = profile_key_payload(profile_id, &chat_session);
        assert_eq!(
            payload,
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11
            ]
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
}