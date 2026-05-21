use openssl::{
    pkey::Private,
    rsa::{Padding, Rsa},
};
use serde::Deserialize;
use sha1::{Digest, Sha1};

const SESSION_SERVER_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const SERVER_ID: &str = "";

#[derive(Clone)]
pub struct Authenticator {
    public_key_der: Vec<u8>,
    private_key: std::sync::Arc<Rsa<Private>>,
    http: reqwest::Client,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthenticatedProfile {
    pub uuid: uuid::Uuid,
    pub name: String,
    pub properties: Vec<qexed_packet::net_types::ProfileProperty>,
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

    fn decrypt_rsa(&self, data: &[u8]) -> anyhow::Result<Vec<u8>> {
        let mut output = vec![0_u8; self.private_key.size() as usize];
        let len = self
            .private_key
            .private_decrypt(data, &mut output, Padding::PKCS1)?;
        output.truncate(len);
        Ok(output)
    }
}

pub fn offline_profile(username: &str) -> qexed_packet::net_types::GameProfile {
    qexed_packet::net_types::GameProfile {
        uuid: offline_uuid(username),
        username: username.to_string(),
        properties: Vec::new(),
    }
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

impl TryFrom<SessionProfile> for AuthenticatedProfile {
    type Error = anyhow::Error;

    fn try_from(value: SessionProfile) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: parse_mojang_uuid(&value.id)?,
            name: value.name,
            properties: value
                .properties
                .into_iter()
                .map(|property| qexed_packet::net_types::ProfileProperty {
                    name: property.name,
                    value: property.value,
                    signature: property.signature,
                })
                .collect(),
        })
    }
}

fn offline_uuid(username: &str) -> uuid::Uuid {
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

fn parse_mojang_uuid(value: &str) -> anyhow::Result<uuid::Uuid> {
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

fn minecraft_server_hash(server_id: &str, shared_secret: &[u8], public_key_der: &[u8]) -> String {
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

#[cfg(test)]
mod tests {
    use super::{java_signed_hex, offline_uuid, parse_mojang_uuid};

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
}
