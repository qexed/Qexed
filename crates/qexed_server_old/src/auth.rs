//! Mojang 会话认证（online mode）与离线档案。移植自 v4 的 auth 模块。

use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::{Padding, Rsa},
};
use std::sync::{Arc, Mutex};

const SESSION_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const SERVER_ID: &str = "";

#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedProfile {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub properties: Vec<qexed_packet::net_types::ProfileProperty>,
}

impl From<AuthenticatedProfile> for qexed_packet::net_types::GameProfile {
    fn from(value: AuthenticatedProfile) -> Self {
        Self {
            uuid: value.uuid,
            username: value.name,
            properties: value.properties,
        }
    }
}

#[derive(Debug, serde::Deserialize)]
struct SessionProfile {
    id: String,
    name: String,
    #[serde(default)]
    properties: Vec<SessionProperty>,
}

#[derive(Debug, serde::Deserialize)]
struct SessionProperty {
    name: String,
    value: String,
    signature: Option<String>,
}

#[derive(Clone, Default)]
pub struct Authenticator {
    inner: Arc<Mutex<Option<Arc<AuthenticatorInner>>>>,
}

struct AuthenticatorInner {
    public_key_der: Vec<u8>,
    private_key: Rsa<Private>,
    http: reqwest::Client,
}

impl Authenticator {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self::default())
    }

    pub fn public_key_der(&self) -> anyhow::Result<Vec<u8>> {
        Ok(self.inner()?.public_key_der.clone())
    }

    /// 解密客户端登录密钥包（RSA），校验 verify token，返回共享密钥。
    pub fn decrypt_login_key(
        &self,
        shared_secret_enc: &[u8],
        verify_token_enc: &[u8],
        expected_verify_token: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let inner = self.inner()?;
        let shared_secret = decrypt_rsa(&inner.private_key, shared_secret_enc)?;
        let verify_token = decrypt_rsa(&inner.private_key, verify_token_enc)?;
        if verify_token != expected_verify_token {
            anyhow::bail!("login verify token mismatch");
        }
        if shared_secret.len() != 16 {
            anyhow::bail!("invalid shared secret length: {}", shared_secret.len());
        }
        Ok(shared_secret)
    }

    /// 向 Mojang 会话服务器验证（hasJoined）。
    pub async fn verify_session(
        &self,
        username: &str,
        shared_secret: &[u8],
        remote_ip: Option<std::net::IpAddr>,
    ) -> anyhow::Result<AuthenticatedProfile> {
        let inner = self.inner()?;
        let digest = minecraft_server_hash(SERVER_ID, shared_secret, &inner.public_key_der);
        let mut request = inner
            .http
            .get(SESSION_SERVER_URL)
            .query(&[("username", username), ("serverId", digest.as_str())]);
        if let Some(remote_ip) = remote_ip {
            let ip = remote_ip.to_string();
            request = request.query(&[("ip", ip.as_str())]);
        }
        let response = request.send().await?;
        if !response.status().is_success() {
            anyhow::bail!("session server returned {}", response.status());
        }
        let profile: SessionProfile = response.json().await?;
        Ok(AuthenticatedProfile {
            uuid: parse_mojang_uuid(&profile.id)?,
            name: profile.name,
            properties: profile
                .properties
                .into_iter()
                .map(|p| qexed_packet::net_types::ProfileProperty {
                    name: p.name,
                    value: p.value,
                    signature: p.signature,
                })
                .collect(),
        })
    }

    fn inner(&self) -> anyhow::Result<Arc<AuthenticatorInner>> {
        let mut guard = self.inner.lock().expect("authenticator poisoned");
        if let Some(inner) = guard.as_ref() {
            return Ok(inner.clone());
        }
        let inner = Arc::new(AuthenticatorInner::create()?);
        *guard = Some(inner.clone());
        Ok(inner)
    }
}

impl AuthenticatorInner {
    fn create() -> anyhow::Result<Self> {
        let private_key = Rsa::generate(1024)?;
        let public_key_der = {
            let pkey = PKey::from_rsa(private_key.public_key())?;
            pkey.public_key_to_der()?
        };
        let http = reqwest::Client::builder()
            .user_agent(format!("qexed/{}", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { public_key_der, private_key, http })
    }
}

fn decrypt_rsa(key: &Rsa<Private>, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut out = vec![0u8; key.size() as usize];
    let len = key.private_decrypt(data, &mut out, Padding::PKCS1)?;
    out.truncate(len);
    Ok(out)
}

/// 离线档案（vanilla 兼容的 OfflinePlayer MD5 UUID）。
pub fn offline_profile(username: &str) -> qexed_packet::net_types::GameProfile {
    qexed_packet::net_types::GameProfile {
        uuid: offline_uuid(username),
        username: username.to_string(),
        properties: Vec::new(),
    }
}

/// vanilla 的离线 UUID：MD5("OfflinePlayer:{name}") + version 3 / variant 位。
pub fn offline_uuid(username: &str) -> uuid::Uuid {
    let digest = openssl::hash::hash(
        MessageDigest::md5(),
        format!("OfflinePlayer:{username}").as_bytes(),
    )
    .expect("MD5 digest should be available");
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(digest.as_ref());
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}

fn parse_mojang_uuid(value: &str) -> anyhow::Result<uuid::Uuid> {
    if value.len() == 32 {
        let hyphenated = format!(
            "{}-{}-{}-{}-{}",
            &value[0..8], &value[8..12], &value[12..16], &value[16..20], &value[20..32],
        );
        return Ok(uuid::Uuid::parse_str(&hyphenated)?);
    }
    Ok(uuid::Uuid::parse_str(value)?)
}

pub fn minecraft_server_hash(server_id: &str, shared_secret: &[u8], public_key_der: &[u8]) -> String {
    use sha1::{Digest, Sha1};
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);
    java_signed_hex(&hasher.finalize())
}

fn java_signed_hex(bytes: &[u8]) -> String {
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
        .unwrap_or(value.len() - 1);
    let mut hex = hex::encode(&value[first_non_zero..]);
    while hex.starts_with('0') && hex.len() > 1 {
        hex.remove(0);
    }
    if negative { format!("-{hex}") } else { hex }
}
