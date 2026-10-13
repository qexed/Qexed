use std::collections::VecDeque;
use std::time::{Duration, Instant};

use crate::config::ChatConfig;
use crate::error::ChatError;

/// 单个玩家的聊天限速器。滑动窗口 + 最大长度检查。
#[derive(Debug)]
pub struct ChatRateLimit {
    window: Duration,
    max_messages: usize,
    max_length: usize,
    sent_at: VecDeque<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatLimitResult {
    Allowed,
    RateLimited,
    TooLong { max_length: usize },
}

impl ChatRateLimit {
    pub fn new(config: &ChatConfig) -> Self {
        Self {
            window: Duration::from_secs(config.rate_limit_window_secs.max(1)),
            max_messages: config.rate_limit_max_messages.max(1),
            max_length: config.max_length.max(1),
            sent_at: VecDeque::new(),
        }
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }

    pub fn check(&mut self, message: &str, now: Instant) -> ChatLimitResult {
        if message.chars().count() > self.max_length {
            return ChatLimitResult::TooLong {
                max_length: self.max_length,
            };
        }

        while self
            .sent_at
            .front()
            .is_some_and(|sent_at| now.duration_since(*sent_at) >= self.window)
        {
            self.sent_at.pop_front();
        }

        if self.sent_at.len() >= self.max_messages {
            return ChatLimitResult::RateLimited;
        }

        self.sent_at.push_back(now);
        ChatLimitResult::Allowed
    }
}

/// 校验一条聊天消息是否通过限速与长度检查。
///
/// 与原 `play.rs` 里的 `validate_player_chat_message` 行为一致。
pub fn validate_player_chat_message(
    rate_limit: &mut ChatRateLimit,
    message: &str,
) -> Result<(), ChatError> {
    match rate_limit.check(message, Instant::now()) {
        ChatLimitResult::Allowed => Ok(()),
        ChatLimitResult::RateLimited => Err(ChatError::RateLimited),
        ChatLimitResult::TooLong { max_length } => Err(ChatError::MessageTooLong {
            length: message.chars().count(),
            max_length,
        }),
    }
}
