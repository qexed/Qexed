//! Mojang 在线认证器（v4 auth::authenticator 迁移）。
//!
//! anyhow 全部替换为 ConnectionError::Auth/Http；
//! 用户可见文案走 qexed_language::t 并 .replace("%{name}", ...) 插值。

use base64::Engine as _;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private, Public},
    rsa::{Padding, Rsa},
    sign::Verifier,
};
use std::sync::{Arc, Mutex};

use super::{
    AuthenticatedProfile,
    model::{ServicesPublicKeys, SessionProfile},
    util::{current_epoch_millis, minecraft_server_hash},
};
use crate::error::{ConnectionError, Result};

const SESSION_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const SERVICES_PUBLIC_KEYS_URL: &str = "https://api.minecraftservices.com/publickeys";
const SERVER_ID: &str = "";

fn t(key: &str) -> String {
    qexed_language::t(key)
}

/// 懒初始化的 RSA 密钥对 + Mojang HTTP 客户端（v4 同名类型）。
#[derive(Clone, Default)]
pub struct Authenticator {
    inner: Arc<Mutex<Option<Arc<AuthenticatorInner>>>>,
}

struct AuthenticatorInner {
    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
    http: reqwest::Client,
    service_public_keys: tokio::sync::RwLock<Option<Vec<PKey<Public>>>>,
}

impl Authenticator {
    pub fn new() -> Result<Self> {
        Ok(Self::default())
    }

    pub fn public_key_der(&self) -> Result<Vec<u8>> {
        Ok(self.inner()?.public_key_der.clone())
    }

    /// 解密客户端密钥包（v4 decrypt_login_key；v6 包为 to_server::login::key::Key）。
    pub fn decrypt_login_key(
        &self,
        key_packet: &qexed_protocol::to_server::login::key::Key,
        expected_verify_token: &[u8],
    ) -> Result<Vec<u8>> {
        let inner = self.inner()?;
        let shared_secret = inner.decrypt_rsa(&key_packet.keybytes.0)?;
        let verify_token = inner.decrypt_rsa(&key_packet.encrypted_challenge.0)?;

        if verify_token != expected_verify_token {
            return Err(ConnectionError::Auth(
                t("qexed.connection.auth.verify_token_mismatch"),
            ));
        }

        if shared_secret.len() != 16 {
            return Err(ConnectionError::Auth(
                t("qexed.connection.auth.invalid_shared_secret_length")
                    .replace("%{length}", &shared_secret.len().to_string()),
            ));
        }

        Ok(shared_secret)
    }

    pub async fn verify_session(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> Result<AuthenticatedProfile> {
        let inner = self.inner()?;
        let digest = minecraft_server_hash(SERVER_ID, shared_secret, &inner.public_key_der);
        let mut request = inner
            .http
            .get(SESSION_SERVER_URL)
            .query(&[("username", username), ("serverId", digest.as_str())]);

        let ip;
        if let Some(remote_ip) = remote_ip {
            ip = remote_ip.to_string();
            request = request.query(&[("ip", ip.as_str())]);
        }

        let response = request
            .send()
            .await
            .map_err(|err| ConnectionError::Http(format!("{SESSION_SERVER_URL}: {err}")))?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            return Err(ConnectionError::Auth(
                t("qexed.connection.auth.session_unconfirmed")
                    .replace("%{name}", username),
            ));
        }
        if !response.status().is_success() {
            return Err(ConnectionError::Http(
                t("qexed.connection.auth.session_bad_status")
                    .replace("%{status}", &response.status().to_string()),
            ));
        }

        let profile = response
            .json::<SessionProfile>()
            .await
            .map_err(|err| {
                ConnectionError::Http(format!("session server response decode failed: {err}"))
            })?;
        Ok(profile.try_into()?)
    }

    pub async fn verify_chat_session(
        &self,
        profile_id: uuid::Uuid,
        chat_session: &qexed_protocol::types::ChatSessionData,
    ) -> Result<()> {
        if chat_session.expires_at_epoch_millis < current_epoch_millis() {
            return Err(ConnectionError::Auth(t("qexed.connection.auth.chat_key_expired")));
        }

        PKey::public_key_from_der(&chat_session.public_key_der)
            .map(|_| ())
            .map_err(|err| {
                ConnectionError::Auth(
                    t("qexed.connection.auth.chat_key_invalid")
                        .replace("%{error}", &err.to_string()),
                )
            })?;

        let keys = self.service_public_keys().await?;
        let payload = crate::secure_chat::profile_key_payload(profile_id, chat_session);
        for key in &keys {
            let mut verifier = Verifier::new(MessageDigest::sha1(), key)
                .map_err(|e| ConnectionError::Auth(format!("verifier create failed: {e}")))?;
            verifier
                .update(&payload)
                .map_err(|e| ConnectionError::Auth(format!("verifier update failed: {e}")))?;
            if verifier
                .verify(&chat_session.key_signature)
                .map_err(|e| ConnectionError::Auth(format!("verify failed: {e}")))?
            {
                return Ok(());
            }
        }

        Err(ConnectionError::Auth(t("qexed.connection.auth.chat_signature_invalid")))
    }

    async fn service_public_keys(&self) -> Result<Vec<PKey<Public>>> {
        let inner = self.inner()?;
        if let Some(keys) = inner.service_public_keys.read().await.as_ref() {
            return Ok(keys.clone());
        }

        let mut guard = inner.service_public_keys.write().await;
        if let Some(keys) = guard.as_ref() {
            return Ok(keys.clone());
        }

        let response = inner
            .http
            .get(SERVICES_PUBLIC_KEYS_URL)
            .send()
            .await
            .map_err(|err| ConnectionError::Http(format!("{SERVICES_PUBLIC_KEYS_URL}: {err}")))?;
        if !response.status().is_success() {
            return Err(ConnectionError::Http(
                t("qexed.connection.auth.publickeys_bad_status")
                    .replace("%{status}", &response.status().to_string()),
            ));
        }

        let key_set = response
            .json::<ServicesPublicKeys>()
            .await
            .map_err(|err| {
                ConnectionError::Http(format!("publickeys response decode failed: {err}"))
            })?;
        let keys = key_set
            .player_certificate_keys
            .into_iter()
            .map(|key| {
                let der = base64::engine::general_purpose::STANDARD
                    .decode(key.public_key)
                    .map_err(|err| {
                        ConnectionError::Auth(
                            t("qexed.connection.auth.public_key_base64_invalid")
                                .replace("%{error}", &err.to_string()),
                        )
                    })?;
                PKey::public_key_from_der(&der).map_err(|err| {
                    ConnectionError::Auth(
                        t("qexed.connection.auth.public_key_der_invalid")
                            .replace("%{error}", &err.to_string()),
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;

        if keys.is_empty() {
            return Err(ConnectionError::Auth(t(
                "qexed.connection.auth.publickeys_empty",
            )));
        }

        *guard = Some(keys.clone());
        Ok(keys)
    }

    fn inner(&self) -> Result<Arc<AuthenticatorInner>> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| ConnectionError::Auth(t("qexed.connection.auth.state_poisoned")))?;
        if let Some(inner) = inner.as_ref() {
            return Ok(inner.clone());
        }

        let created = Arc::new(AuthenticatorInner::new()?);
        *inner = Some(created.clone());
        Ok(created)
    }
}

impl AuthenticatorInner {
    fn new() -> Result<Self> {
        let private_key =
            Rsa::generate(1024).map_err(|e| ConnectionError::Auth(format!("RSA key generation failed: {e}")))?;
        let public_key_der = private_key
            .public_key_to_der()
            .map_err(|e| ConnectionError::Auth(format!("RSA public key export failed: {e}")))?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("qexed/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|err| {
                ConnectionError::Http(format!("http client build failed: {err}"))
            })?;

        Ok(Self {
            public_key_der,
            private_key,
            http,
            service_public_keys: tokio::sync::RwLock::new(None),
        })
    }

    fn decrypt_rsa(&self, data: &[u8]) -> Result<Vec<u8>> {
        let mut output = vec![0_u8; self.private_key.size() as usize];
        let len = self
            .private_key
            .private_decrypt(data, &mut output, Padding::PKCS1)
            .map_err(|e| ConnectionError::Auth(format!("RSA decrypt failed: {e}")))?;
        output.truncate(len);
        Ok(output)
    }
}

