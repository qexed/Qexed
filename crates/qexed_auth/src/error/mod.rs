#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("config error: {0}")]
    ConfigError(#[from] qexed_config::error::ConfigError),

    // ── 外部 crate 源错误 ──
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("crypto error: {0}")]
    Crypto(#[from] openssl::error::ErrorStack),
    #[error("base64 decode error: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("uuid parse error: {0}")]
    Uuid(#[from] uuid::Error),

    // ── 业务错误 ──
    #[error("verify token mismatch")]
    VerifyTokenMismatch,
    #[error("invalid shared secret length: {0}")]
    InvalidSharedSecretLength(usize),
    #[error("session server did not confirm player: {0}")]
    SessionNotConfirmed(String),
    #[error("session server returned unexpected status: {0}")]
    SessionServerStatus(reqwest::StatusCode),
    #[error("public keys service returned unexpected status: {0}")]
    PublicKeysStatus(reqwest::StatusCode),
    #[error("public keys service returned no player certificate keys")]
    NoPlayerCertificateKeys,
    #[error("chat public key expired")]
    ChatKeyExpired,
    #[error("chat public key signature invalid")]
    ChatKeySignatureInvalid,

    // ── 内部状态 ──
    #[error("authenticator state poisoned")]
    StatePoisoned,
}