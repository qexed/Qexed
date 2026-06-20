use openssl::rsa::{Padding, Rsa};
use qexed_packet::net_types::{GameProfile, ProfileProperty};
use rand::RngCore as _;
use serde::Deserialize;
use sha1::{Digest as _, Sha1};
use std::sync::{Arc, Mutex};

const SERVER_ID: &str = "";
const VERIFY_TOKEN_LEN: usize = 16;

#[derive(Debug, Clone)]
pub struct Authenticator {
    config: qexed_config::app::qexed_auth::Auth,
    inner: Arc<Mutex<Option<Arc<AuthenticatorInner>>>>,
}

#[derive(Debug)]
struct AuthenticatorInner {
    public_key_der: Vec<u8>,
    private_key: Rsa<openssl::pkey::Private>,
    http: reqwest::Client,
}

#[derive(Debug, Clone)]
pub struct AuthSession {
    profile: GameProfile,
    online_mode: bool,
    shared_secret: Option<Vec<u8>>,
}

impl AuthSession {
    pub fn profile(&self) -> &GameProfile {
        &self.profile
    }

    pub fn into_profile(self) -> GameProfile {
        self.profile
    }

    pub fn online_mode(&self) -> bool {
        self.online_mode
    }

    pub fn shared_secret(&self) -> Option<&[u8]> {
        self.shared_secret.as_deref()
    }
}

impl Authenticator {
    pub fn new(config: qexed_config::app::qexed_auth::Auth) -> Self {
        Self {
            config,
            inner: Arc::new(Mutex::new(None)),
        }
    }

    pub fn config(&self) -> &qexed_config::app::qexed_auth::Auth {
        &self.config
    }

    pub fn is_online_mode(&self) -> bool {
        self.config.enabled
    }

    pub fn new_verify_token(&self) -> Vec<u8> {
        let mut token = [0_u8; VERIFY_TOKEN_LEN];
        rand::thread_rng().fill_bytes(&mut token);
        token.to_vec()
    }

    pub fn public_key_der(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.inner()?.public_key_der.clone())
    }

    pub fn decrypt_login_key(
        &self,
        key_packet: &qexed_protocol::to_server::login::encryption_begin::EncryptionBegin,
        expected_verify_token: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let inner = self.inner()?;
        let shared_secret = inner.decrypt_rsa(&key_packet.shared_secret.0)?;
        let verify_token = inner.decrypt_rsa(&key_packet.verify_token.0)?;

        if verify_token != expected_verify_token {
            anyhow::bail!("登录验证 token 不匹配");
        }
        if shared_secret.len() != 16 {
            anyhow::bail!("登录共享密钥长度无效: {}", shared_secret.len());
        }

        Ok(shared_secret)
    }

    pub async fn authenticate_online(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> anyhow::Result<AuthSession> {
        let profile = self
            .verify_session(username, shared_secret, remote_ip)
            .await?;
        Ok(AuthSession {
            profile,
            online_mode: true,
            shared_secret: Some(shared_secret.to_vec()),
        })
    }

    pub fn authenticate_offline(&self, username: &str, client_uuid: uuid::Uuid) -> AuthSession {
        AuthSession {
            profile: offline_profile_with_client_uuid(username, client_uuid),
            online_mode: false,
            shared_secret: None,
        }
    }

    async fn verify_session(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> anyhow::Result<GameProfile> {
        let inner = self.inner()?;
        let server_id = minecraft_server_hash(SERVER_ID, shared_secret, &inner.public_key_der);
        let mut request = inner
            .http
            .get(&self.config.yggdrasil.session_server_url)
            .query(&[("username", username), ("serverId", server_id.as_str())]);

        let ip;
        if self.config.yggdrasil.include_client_ip {
            if let Some(remote_ip) = remote_ip {
                ip = remote_ip.to_string();
                request = request.query(&[("ip", ip.as_str())]);
            }
        }

        let response = request.send().await?;
        if response.status() == reqwest::StatusCode::NO_CONTENT {
            anyhow::bail!("Yggdrasil 未确认玩家会话: {username}");
        }
        if !response.status().is_success() {
            anyhow::bail!("Yggdrasil 认证服务返回异常状态: {}", response.status());
        }

        response.json::<SessionProfile>().await?.try_into()
    }

    fn inner(&self) -> anyhow::Result<Arc<AuthenticatorInner>> {
        let mut guard = self
            .inner
            .lock()
            .map_err(|_| anyhow::anyhow!("authenticator state poisoned"))?;
        if let Some(inner) = guard.as_ref() {
            return Ok(inner.clone());
        }

        let inner = Arc::new(AuthenticatorInner::new(
            self.config.yggdrasil.http_timeout.max(1),
        )?);
        *guard = Some(inner.clone());
        Ok(inner)
    }
}

impl AuthenticatorInner {
    fn new(http_timeout_secs: u64) -> anyhow::Result<Self> {
        let private_key = Rsa::generate(1024)?;
        let public_key_der = private_key.public_key_to_der()?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(http_timeout_secs))
            .user_agent(concat!("qexed/", env!("CARGO_PKG_VERSION")))
            .build()?;

        Ok(Self {
            public_key_der,
            private_key,
            http,
        })
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

pub fn offline_profile(username: &str) -> GameProfile {
    GameProfile {
        uuid: offline_uuid(username),
        username: username.to_string(),
        properties: Vec::new(),
    }
}

pub fn offline_profile_with_client_uuid(username: &str, client_uuid: uuid::Uuid) -> GameProfile {
    if client_uuid == uuid::Uuid::nil() {
        return offline_profile(username);
    }

    GameProfile {
        uuid: client_uuid,
        username: username.to_string(),
        properties: Vec::new(),
    }
}

pub fn offline_uuid(username: &str) -> uuid::Uuid {
    let digest = openssl::hash::hash(
        openssl::hash::MessageDigest::md5(),
        format!("OfflinePlayer:{username}").as_bytes(),
    )
    .expect("MD5 digest should be available");

    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(digest.as_ref());
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

pub fn parse_mojang_uuid(value: &str) -> anyhow::Result<uuid::Uuid> {
    if value.len() == 32 {
        let hyphenated = format!(
            "{}-{}-{}-{}-{}",
            &value[0..8],
            &value[8..12],
            &value[12..16],
            &value[16..20],
            &value[20..32]
        );
        return Ok(uuid::Uuid::parse_str(&hyphenated)?);
    }

    Ok(uuid::Uuid::parse_str(value)?)
}

pub fn minecraft_server_hash(
    server_id: &str,
    shared_secret: &[u8],
    public_key_der: &[u8],
) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);

    java_signed_hex(&hasher.finalize())
}

pub fn java_signed_hex(bytes: &[u8]) -> String {
    let negative = bytes.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut value = bytes.to_vec();

    if negative {
        for byte in &mut value {
            *byte = !*byte;
        }

        for byte in value.iter_mut().rev() {
            let (next, overflow) = byte.overflowing_add(1);
            *byte = next;
            if !overflow {
                break;
            }
        }
    }

    let first_non_zero = value
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(value.len().saturating_sub(1));
    let mut hex = hex::encode(&value[first_non_zero..]);
    while hex.starts_with('0') && hex.len() > 1 {
        hex.remove(0);
    }

    if negative { format!("-{hex}") } else { hex }
}

#[derive(Debug, Deserialize)]
struct SessionProfile {
    id: String,
    name: String,
    #[serde(default)]
    properties: Vec<SessionProperty>,
}

#[derive(Debug, Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
    signature: Option<String>,
}

impl TryFrom<SessionProfile> for GameProfile {
    type Error = anyhow::Error;

    fn try_from(value: SessionProfile) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_mojang_uuid(&value.id)?,
            username: value.name,
            properties: value
                .properties
                .into_iter()
                .map(|property| ProfileProperty {
                    name: property.name,
                    value: property.value,
                    signature: property.signature,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Authenticator, java_signed_hex, minecraft_server_hash, offline_uuid, parse_mojang_uuid,
    };

    #[test]
    fn parses_compact_mojang_uuid() {
        let uuid = parse_mojang_uuid("00112233445566778899aabbccddeeff").unwrap();

        assert_eq!(uuid.to_string(), "00112233-4455-6677-8899-aabbccddeeff");
    }

    #[test]
    fn formats_java_signed_sha1_hash() {
        assert_eq!(java_signed_hex(&[0, 0, 0x0f]), "f");
        assert_eq!(java_signed_hex(&[0xff; 20]), "-1");
        assert_eq!(java_signed_hex(&[0x80, 0, 0]), "-800000");
    }

    #[test]
    fn generates_java_compatible_offline_uuid() {
        assert_eq!(
            offline_uuid("Steve").to_string(),
            "5627dd98-e6be-3c21-b8a8-e92344183641"
        );
    }

    #[test]
    fn verifier_token_is_expected_length() {
        let auth = Authenticator::new(Default::default());

        assert_eq!(auth.new_verify_token().len(), 16);
    }

    #[tokio::test]
    async fn verifies_yggdrasil_session_profile() {
        let mut server = mockito::Server::new_async().await;
        let mut config = qexed_config::app::qexed_auth::Auth::default();
        config.enabled = true;
        config.yggdrasil.session_server_url =
            format!("{}/session/minecraft/hasJoined", server.url());
        let auth = Authenticator::new(config);
        let server_id = minecraft_server_hash("", &[], &auth.public_key_der().unwrap());

        let mock = server
            .mock("GET", "/session/minecraft/hasJoined")
            .match_query(mockito::Matcher::AllOf(vec![
                mockito::Matcher::UrlEncoded("username".to_string(), "Steve".to_string()),
                mockito::Matcher::UrlEncoded("serverId".to_string(), server_id),
            ]))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"id":"00112233445566778899aabbccddeeff","name":"Steve","properties":[{"name":"textures","value":"abc","signature":"sig"}]}"#,
            )
            .create_async()
            .await;

        let profile = auth.verify_session("Steve", &[], None).await.unwrap();

        assert_eq!(
            profile.uuid.to_string(),
            "00112233-4455-6677-8899-aabbccddeeff"
        );
        assert_eq!(profile.username, "Steve");
        assert_eq!(profile.properties.len(), 1);
        mock.assert_async().await;
    }
}
