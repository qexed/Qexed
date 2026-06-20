use base64::Engine as _;
use openssl::{
    hash::MessageDigest,
    pkey::{PKey, Public},
    rsa::{Padding, Rsa},
    sign::Verifier,
};
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
    private_key_der: Vec<u8>,
    http: reqwest::Client,
    service_public_keys: tokio::sync::RwLock<Option<Vec<PKey<Public>>>>,
    blocking_runtime: Option<DedicatedBlockingRuntime>,
}

#[derive(Debug)]
struct DedicatedBlockingRuntime {
    runtime: Option<tokio::runtime::Runtime>,
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

    pub async fn decrypt_login_key(
        &self,
        key_packet: &qexed_protocol::to_server::login::encryption_begin::EncryptionBegin,
        expected_verify_token: &[u8],
    ) -> anyhow::Result<Vec<u8>> {
        let inner = self.inner()?;
        let private_key_der = inner.private_key_der.clone();
        let shared_secret_ciphertext = key_packet.shared_secret.0.clone();
        let verify_token_ciphertext = key_packet.verify_token.0.clone();
        let expected_verify_token = expected_verify_token.to_vec();

        let (shared_secret, verify_token) = inner
            .run_blocking(move || {
                let private_key = Rsa::private_key_from_der(&private_key_der)?;
                let shared_secret = decrypt_rsa(&private_key, &shared_secret_ciphertext)?;
                let verify_token = decrypt_rsa(&private_key, &verify_token_ciphertext)?;
                Ok::<_, anyhow::Error>((shared_secret, verify_token))
            })
            .await?;

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
        let payload = profile_key_payload(profile_id, chat_session);
        for key in &keys {
            let mut verifier = Verifier::new(MessageDigest::sha1(), key)?;
            verifier.update(&payload)?;
            if verifier.verify(&chat_session.key_signature)? {
                return Ok(());
            }
        }

        anyhow::bail!("Mojang 聊天公钥签名无效");
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

    async fn service_public_keys(&self) -> anyhow::Result<Vec<PKey<Public>>> {
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
            .get(&self.config.yggdrasil.services_public_keys_url)
            .send()
            .await?;
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
            &self.config.blocking_pool,
        )?);
        *guard = Some(inner.clone());
        Ok(inner)
    }
}

impl AuthenticatorInner {
    fn new(
        http_timeout_secs: u64,
        blocking_pool: &qexed_config::app::qexed_auth::BlockingPool,
    ) -> anyhow::Result<Self> {
        let private_key = Rsa::generate(1024)?;
        let public_key_der = private_key.public_key_to_der()?;
        let private_key_der = private_key.private_key_to_der()?;
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(http_timeout_secs))
            .user_agent(concat!("qexed/", env!("CARGO_PKG_VERSION")))
            .build()?;
        let blocking_runtime = build_blocking_runtime(blocking_pool)?;

        Ok(Self {
            public_key_der,
            private_key_der,
            http,
            service_public_keys: tokio::sync::RwLock::new(None),
            blocking_runtime,
        })
    }

    async fn run_blocking<F, T>(&self, task: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        if let Some(runtime) = &self.blocking_runtime {
            return runtime.spawn_blocking(task).await?;
        }

        task()
    }
}

impl DedicatedBlockingRuntime {
    fn new(runtime: tokio::runtime::Runtime) -> Self {
        Self {
            runtime: Some(runtime),
        }
    }

    fn spawn_blocking<F, T>(&self, task: F) -> tokio::task::JoinHandle<anyhow::Result<T>>
    where
        F: FnOnce() -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        self.runtime
            .as_ref()
            .expect("dedicated blocking runtime must exist before drop")
            .spawn_blocking(task)
    }
}

impl Drop for DedicatedBlockingRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

fn build_blocking_runtime(
    config: &qexed_config::app::qexed_auth::BlockingPool,
) -> anyhow::Result<Option<DedicatedBlockingRuntime>> {
    if !config.enabled {
        return Ok(None);
    }

    let worker_threads = config.worker_threads.max(1);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(1)
        .max_blocking_threads(worker_threads)
        .thread_name("qexed-auth-blocking")
        .enable_all()
        .build()?;

    Ok(Some(DedicatedBlockingRuntime::new(runtime)))
}

fn decrypt_rsa(private_key: &Rsa<openssl::pkey::Private>, data: &[u8]) -> anyhow::Result<Vec<u8>> {
    let mut output = vec![0_u8; private_key.size() as usize];
    let len = private_key.private_decrypt(data, &mut output, Padding::PKCS1)?;
    output.truncate(len);
    Ok(output)
}

pub fn profile_key_payload(
    profile_id: uuid::Uuid,
    chat_session: &qexed_protocol::types::ChatSessionData,
) -> Vec<u8> {
    let mut payload = Vec::with_capacity(24 + chat_session.public_key_der.len());
    payload.extend_from_slice(profile_id.as_bytes());
    payload.extend_from_slice(&chat_session.expires_at_epoch_millis.to_be_bytes());
    payload.extend_from_slice(&chat_session.public_key_der);
    payload
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

#[derive(Debug, Deserialize)]
struct ServicesPublicKeys {
    #[serde(rename = "playerCertificateKeys", default)]
    player_certificate_keys: Vec<ServicesPublicKey>,
}

#[derive(Debug, Deserialize)]
struct ServicesPublicKey {
    #[serde(rename = "publicKey")]
    public_key: String,
}

fn current_epoch_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
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
        profile_key_payload,
    };
    use base64::Engine as _;
    use openssl::{pkey::Public, rsa::Rsa};

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
    async fn decrypts_login_key_on_configured_blocking_pool() {
        let auth = Authenticator::new(Default::default());
        let shared_secret = [7_u8; 16];
        let verify_token = auth.new_verify_token();
        let packet = encrypted_login_key_packet(&auth, &shared_secret, &verify_token);

        let decrypted = auth
            .decrypt_login_key(&packet, &verify_token)
            .await
            .unwrap();

        assert_eq!(decrypted, shared_secret);
    }

    #[tokio::test]
    async fn decrypts_login_key_when_blocking_pool_is_disabled() {
        let mut config = qexed_config::app::qexed_auth::Auth::default();
        config.blocking_pool.enabled = false;
        let auth = Authenticator::new(config);
        let shared_secret = [9_u8; 16];
        let verify_token = auth.new_verify_token();
        let packet = encrypted_login_key_packet(&auth, &shared_secret, &verify_token);

        let decrypted = auth
            .decrypt_login_key(&packet, &verify_token)
            .await
            .unwrap();

        assert_eq!(decrypted, shared_secret);
    }

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

    #[tokio::test]
    async fn loads_services_public_keys_from_configured_url() {
        let mut server = mockito::Server::new_async().await;
        let private_key = Rsa::generate(1024).unwrap();
        let public_key_der = private_key.public_key_to_der().unwrap();
        let public_key = base64::engine::general_purpose::STANDARD.encode(public_key_der);

        let mut config = qexed_config::app::qexed_auth::Auth::default();
        config.yggdrasil.services_public_keys_url = format!("{}/publickeys", server.url());
        let auth = Authenticator::new(config);

        let mock = server
            .mock("GET", "/publickeys")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"playerCertificateKeys":[{{"publicKey":"{public_key}"}}]}}"#
            ))
            .create_async()
            .await;

        let keys = auth.service_public_keys().await.unwrap();

        assert_eq!(keys.len(), 1);
        mock.assert_async().await;
    }

    fn encrypted_login_key_packet(
        auth: &Authenticator,
        shared_secret: &[u8; 16],
        verify_token: &[u8],
    ) -> qexed_protocol::to_server::login::encryption_begin::EncryptionBegin {
        let public_key =
            Rsa::<Public>::public_key_from_der(&auth.public_key_der().unwrap()).unwrap();

        qexed_protocol::to_server::login::encryption_begin::EncryptionBegin {
            shared_secret: qexed_packet::net_types::ByteArray(encrypt_rsa(
                &public_key,
                shared_secret,
            )),
            verify_token: qexed_packet::net_types::ByteArray(encrypt_rsa(
                &public_key,
                verify_token,
            )),
        }
    }

    fn encrypt_rsa(public_key: &Rsa<Public>, data: &[u8]) -> Vec<u8> {
        let mut output = vec![0_u8; public_key.size() as usize];
        let len = public_key
            .public_encrypt(data, &mut output, openssl::rsa::Padding::PKCS1)
            .unwrap();
        output.truncate(len);
        output
    }
}
