use base64::Engine as _;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private, Public},
    rsa::{Padding, Rsa},
    sign::Verifier,
};

use super::{
    AuthenticatedProfile,
    model::{ServicesPublicKeys, SessionProfile},
    util::{current_epoch_millis, minecraft_server_hash},
};

const SESSION_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const SERVICES_PUBLIC_KEYS_URL: &str = "https://api.minecraftservices.com/publickeys";
const SERVER_ID: &str = "";

#[derive(Clone)]
pub struct Authenticator {
    public_key_der: Vec<u8>,
    private_key: std::sync::Arc<Rsa<Private>>,
    http: reqwest::Client,
    service_public_keys: std::sync::Arc<tokio::sync::RwLock<Option<Vec<PKey<Public>>>>>,
}

impl Authenticator {
    pub fn new() -> anyhow::Result<Self> {
        let private_key = Rsa::generate(1024)?;
        let public_key_der = private_key.public_key_to_der()?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("qexed/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            public_key_der,
            private_key: std::sync::Arc::new(private_key),
            http,
            service_public_keys: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        })
    }

    pub fn public_key_der(&self) -> &[u8] {
        &self.public_key_der
    }

    pub fn decrypt_login_key(
        &self,
        key_packet: &qexed_protocol::to_server::login::encryption_begin::EncryptionBegin,
        expected_verify_token: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let shared_secret = self.decrypt_rsa(&key_packet.shared_secret.0)?;
        let verify_token = self.decrypt_rsa(&key_packet.verify_token.0)?;

        if verify_token != expected_verify_token {
            anyhow::bail!("登录验证 token 不匹配");
        }

        if shared_secret.len() != 16 {
            anyhow::bail!("登录共享密钥长度无效: {}", shared_secret.len());
        }

        Ok(shared_secret)
    }

    pub async fn verify_session(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> anyhow::Result<AuthenticatedProfile> {
        let digest = minecraft_server_hash(SERVER_ID, shared_secret, &self.public_key_der);
        let mut request = self
            .http
            .get(SESSION_SERVER_URL)
            .query(&[("username", username), ("serverId", digest.as_str())]);

        let ip;
        if let Some(remote_ip) = remote_ip {
            ip = remote_ip.to_string();
            request = request.query(&[("ip", ip.as_str())]);
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            anyhow::bail!("Mojang 会话服务器未确认玩家 {}", username);
        }
        if !response.status().is_success() {
            anyhow::bail!("Mojang 会话服务器返回异常状态: {}", response.status());
        }

        let profile = response.json::<SessionProfile>().await?;
        Ok(profile.try_into()?)
    }

    pub async fn verify_chat_session(
        &self,
        profile_id: uuid::Uuid,
        chat_session: &qexed_protocol::types::ChatSessionData,
    ) -> anyhow::Result<()> {
        if chat_session.expires_at_epoch_millis < current_epoch_millis() {
            anyhow::bail!("Mojang 聊天公钥已过期");
        }

        PKey::public_key_from_der(&chat_session.public_key_der)
            .map(|_| ())
            .map_err(|err| anyhow::anyhow!("Mojang 聊天公钥格式无效: {err}"))?;

        let keys = self.service_public_keys().await?;
        let payload = crate::secure_chat::profile_key_payload(profile_id, chat_session);
        for key in &keys {
            let mut verifier = Verifier::new(MessageDigest::sha1(), key)?;
            verifier.update(&payload)?;
            if verifier.verify(&chat_session.key_signature)? {
                return Ok(());
            }
        }

        anyhow::bail!("Mojang 聊天公钥签名无效");
    }

    async fn service_public_keys(&self) -> anyhow::Result<Vec<PKey<Public>>> {
        if let Some(keys) = self.service_public_keys.read().await.as_ref() {
            return Ok(keys.clone());
        }

        let mut guard = self.service_public_keys.write().await;
        if let Some(keys) = guard.as_ref() {
            return Ok(keys.clone());
        }

        let response = self.http.get(SERVICES_PUBLIC_KEYS_URL).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("Mojang publickeys 服务返回异常状态: {}", response.status());
        }

        let key_set = response.json::<ServicesPublicKeys>().await?;
        let keys = key_set
            .player_certificate_keys
            .into_iter()
            .map(|key| {
                let der = base64::engine::general_purpose::STANDARD
                    .decode(key.public_key)
                    .map_err(|err| anyhow::anyhow!("Mojang public key base64 无效: {err}"))?;
                PKey::public_key_from_der(&der)
                    .map_err(|err| anyhow::anyhow!("Mojang public key DER 无效: {err}"))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;

        if keys.is_empty() {
            anyhow::bail!("Mojang publickeys 服务未返回 playerCertificateKeys");
        }

        *guard = Some(keys.clone());
        Ok(keys)
    }

    fn decrypt_rsa(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut output = vec![0_u8; self.private_key.size() as usize];
        let len = self
            .private_key
            .private_decrypt(data, &mut output, Padding::PKCS1)?;
        output.truncate(len);
        Ok(output)
    }
}
