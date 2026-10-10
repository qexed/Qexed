use base64::Engine as _;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private, Public},
    rsa::{Padding, Rsa},
    sign::Verifier,
};
use std::sync::Arc;

use qexed_chat::profile_key_payload;

use crate::{
    AuthenticatedProfile, config::AuthConfig, error::AuthError, model::{ServicesPublicKeys, SessionProfile}, util::{current_epoch_millis, minecraft_server_hash},
};

/// Minecraft 协议固定的空 server_id（不做 IP 绑定）。
const SERVER_ID: &str = "";

#[derive(Clone)]
pub struct Authenticator {
    inner: Arc<AuthenticatorInner>,
}

struct AuthenticatorInner {
    session_server_url: String,
    public_keys_url: String,
    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
    http: reqwest::Client,
    service_public_keys: tokio::sync::RwLock<Option<Vec<PKey<Public>>>>,
}

impl Authenticator {
    /// 运行期构造：从 `crate::get()` 读全局配置。
    pub fn new() -> Result<Self, AuthError> {
        Self::from_config(crate::get())
    }

    /// 显式传配置构造，测试或需要自定义 URL 时使用。
    pub fn from_config(config: &AuthConfig) -> Result<Self, AuthError> {
        Ok(Self {
            inner: Arc::new(AuthenticatorInner::new(config)?),
        })
    }

    pub fn public_key_der(&self) -> Result<Vec<u8>, AuthError> {
        Ok(self.inner.public_key_der.clone())
    }

    pub fn decrypt_login_key(
        &self,
        key_packet: &qexed_protocol::to_server::login::encryption_begin::EncryptionBegin,
        expected_verify_token: &[u8],
    ) -> Result<Vec<u8>, AuthError> {
        let inner = &*self.inner;
        let shared_secret = inner.decrypt_rsa(&key_packet.shared_secret.0)?;
        let verify_token = inner.decrypt_rsa(&key_packet.verify_token.0)?;

        if verify_token != expected_verify_token {
            return Err(AuthError::VerifyTokenMismatch);
        }

        if shared_secret.len() != 16 {
            return Err(AuthError::InvalidSharedSecretLength(shared_secret.len()));
        }

        Ok(shared_secret)
    }

    pub async fn verify_session(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> Result<AuthenticatedProfile, AuthError> {
        let inner = &*self.inner;
        let digest = minecraft_server_hash(SERVER_ID, shared_secret, &inner.public_key_der);

        let mut request = inner
            .http
            .get(&inner.session_server_url)
            .query(&[("username", username), ("serverId", digest.as_str())]);

        let ip;
        if let Some(remote_ip) = remote_ip {
            ip = remote_ip.to_string();
            request = request.query(&[("ip", ip.as_str())]);
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Err(AuthError::SessionNotConfirmed(username.to_string()));
        }
        if !response.status().is_success() {
            return Err(AuthError::SessionServerStatus(response.status()));
        }

        let profile = response.json::<SessionProfile>().await?;
        Ok(profile.try_into()?)
    }

    pub async fn verify_chat_session(
        &self,
        profile_id: uuid::Uuid,
        chat_session: &qexed_protocol::types::ChatSessionData,
    ) -> Result<(), AuthError> {
        if chat_session.expires_at_epoch_millis < current_epoch_millis() {
            return Err(AuthError::ChatKeyExpired);
        }

        // 只做 DER 格式校验，非法直接抛 Crypto
        PKey::public_key_from_der(&chat_session.public_key_der).map(|_| ())?;

        let keys = self.service_public_keys().await?;
        let payload = profile_key_payload(profile_id, chat_session);
        for key in &keys {
            let mut verifier = Verifier::new(MessageDigest::sha1(), key)?;
            verifier.update(&payload)?;
            if verifier.verify(&chat_session.key_signature)? {
                return Ok(());
            }
        }

        Err(AuthError::ChatKeySignatureInvalid)
    }

    async fn service_public_keys(&self) -> Result<Vec<PKey<Public>>, AuthError> {
        let inner = &*self.inner;

        if let Some(keys) = inner.service_public_keys.read().await.as_ref() {
            return Ok(keys.clone());
        }

        let mut guard = inner.service_public_keys.write().await;
        if let Some(keys) = guard.as_ref() {
            return Ok(keys.clone());
        }

        let response = inner.http.get(&inner.public_keys_url).send().await?;
        if !response.status().is_success() {
            return Err(AuthError::PublicKeysStatus(response.status()));
        }

        let key_set = response.json::<ServicesPublicKeys>().await?;
        let keys = key_set
            .player_certificate_keys
            .into_iter()
            .map(|key| {
                let der = base64::engine::general_purpose::STANDARD
                    .decode(key.public_key)?;
                Ok(PKey::public_key_from_der(&der)?)
            })
            .collect::<Result<Vec<_>, AuthError>>()?;

        if keys.is_empty() {
            return Err(AuthError::NoPlayerCertificateKeys);
        }

        *guard = Some(keys.clone());
        Ok(keys)
    }
}

impl AuthenticatorInner {
    fn new(config: &AuthConfig) -> Result<Self, AuthError> {
        let private_key = Rsa::generate(1024)?;
        let public_key_der = private_key.public_key_to_der()?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("qexed/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            session_server_url: config.session_server_url.clone(),
            public_keys_url: config.services_public_keys_url.clone(),
            public_key_der,
            private_key,
            http,
            service_public_keys: tokio::sync::RwLock::new(None),
        })
    }

    fn decrypt_rsa(&self, data: &[u8]) -> Result<Vec<u8>, AuthError> {
        let mut output = vec![0_u8; self.private_key.size() as usize];
        let len = self
            .private_key
            .private_decrypt(data, &mut output, Padding::PKCS1)?;
        output.truncate(len);
        Ok(output)
    }
}