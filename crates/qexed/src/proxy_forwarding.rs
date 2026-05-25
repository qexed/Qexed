use bytes::{Buf as _, BytesMut};
use hmac::{Hmac, Mac};
#[cfg(test)]
use qexed_packet::PacketWriter;
use qexed_packet::{
    PacketCodec, PacketReader,
    net_types::{GameProfile, ProfileProperty, VarInt},
};
use serde::Deserialize;
use sha2::Sha256;

pub(crate) const VELOCITY_FORWARDING_CHANNEL: &str = "velocity:player_info";
pub(crate) const VELOCITY_FORWARDING_MESSAGE_ID: i32 = 0;

const VELOCITY_FORWARDING_VERSION: i32 = 1;
const VELOCITY_SIGNATURE_BYTES: usize = 32;
const MAX_PROFILE_PROPERTIES: usize = 16;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ForwardedLogin {
    pub profile: GameProfile,
    pub login_host: String,
}

pub(crate) fn parse_bungeecord_forwarding(
    server_host: &str,
    username: &str,
) -> anyhow::Result<ForwardedLogin> {
    let parts = server_host.split('\0').collect::<Vec<_>>();
    if parts.len() < 3 {
        anyhow::bail!("missing BungeeCord forwarding data");
    }

    let uuid = uuid::Uuid::parse_str(parts[2])
        .map_err(|err| anyhow::anyhow!("invalid BungeeCord forwarded UUID: {err}"))?;
    let properties = parts
        .get(3)
        .filter(|value| !value.trim().is_empty())
        .map_or_else(
            || Ok(Vec::new()),
            |value| parse_bungeecord_properties(value),
        )?;

    Ok(ForwardedLogin {
        login_host: parts[0].to_string(),
        profile: GameProfile {
            uuid,
            username: username.to_string(),
            properties,
        },
    })
}

pub(crate) fn parse_velocity_forwarding_response(
    data: &[u8],
    secret: &str,
) -> anyhow::Result<GameProfile> {
    if secret.is_empty() {
        anyhow::bail!("Velocity forwarding secret is empty");
    }
    if data.len() < VELOCITY_SIGNATURE_BYTES {
        anyhow::bail!("Velocity forwarding response is too short");
    }

    let (signature, payload) = data.split_at(VELOCITY_SIGNATURE_BYTES);
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|err| anyhow::anyhow!("invalid Velocity forwarding secret: {err}"))?;
    mac.update(payload);
    mac.verify_slice(signature)
        .map_err(|_| anyhow::anyhow!("invalid Velocity forwarding signature"))?;

    let mut payload = BytesMut::from(payload);
    let mut reader = PacketReader::new(&mut payload);

    let mut version = VarInt::default();
    version.deserialize(&mut reader)?;
    if version.0 != VELOCITY_FORWARDING_VERSION {
        anyhow::bail!("unsupported Velocity forwarding version: {}", version.0);
    }

    let mut _forwarded_address = String::new();
    _forwarded_address.deserialize(&mut reader)?;

    let mut uuid = uuid::Uuid::nil();
    uuid.deserialize(&mut reader)?;

    let mut username = String::new();
    username.deserialize(&mut reader)?;

    let properties = read_profile_properties(&mut reader)?;
    if reader.buf.has_remaining() {
        anyhow::bail!(
            "Velocity forwarding response has {} trailing bytes",
            reader.buf.remaining()
        );
    }

    Ok(GameProfile {
        uuid,
        username,
        properties,
    })
}

#[derive(Debug, Deserialize)]
struct BungeecordProperty {
    name: String,
    value: String,
    signature: Option<String>,
}

fn parse_bungeecord_properties(value: &str) -> anyhow::Result<Vec<ProfileProperty>> {
    let properties = serde_json::from_str::<Vec<BungeecordProperty>>(value)
        .map_err(|err| anyhow::anyhow!("invalid BungeeCord forwarded properties: {err}"))?;
    ensure_property_count(properties.len())?;
    Ok(properties
        .into_iter()
        .map(|property| ProfileProperty {
            name: property.name,
            value: property.value,
            signature: property.signature,
        })
        .collect())
}

fn read_profile_properties(reader: &mut PacketReader<'_>) -> anyhow::Result<Vec<ProfileProperty>> {
    let mut count = VarInt::default();
    count.deserialize(reader)?;
    if count.0 < 0 {
        anyhow::bail!("negative profile property count: {}", count.0);
    }
    let count = count.0 as usize;
    ensure_property_count(count)?;

    let mut properties = Vec::with_capacity(count);
    for _ in 0..count {
        let mut name = String::new();
        name.deserialize(reader)?;

        let mut value = String::new();
        value.deserialize(reader)?;

        let mut has_signature = false;
        has_signature.deserialize(reader)?;
        let signature = if has_signature {
            let mut signature = String::new();
            signature.deserialize(reader)?;
            Some(signature)
        } else {
            None
        };

        properties.push(ProfileProperty {
            name,
            value,
            signature,
        });
    }

    Ok(properties)
}

fn ensure_property_count(count: usize) -> anyhow::Result<()> {
    if count > MAX_PROFILE_PROPERTIES {
        anyhow::bail!(
            "profile property count {} exceeds max {}",
            count,
            MAX_PROFILE_PROPERTIES
        );
    }
    Ok(())
}

#[cfg(test)]
fn velocity_forwarding_data(
    secret: &str,
    address: &str,
    profile: &GameProfile,
) -> anyhow::Result<Vec<u8>> {
    let mut payload = BytesMut::new();
    {
        let mut writer = PacketWriter::new(&mut payload);
        VarInt(VELOCITY_FORWARDING_VERSION).serialize(&mut writer)?;
        address.to_string().serialize(&mut writer)?;
        profile.uuid.serialize(&mut writer)?;
        profile.username.serialize(&mut writer)?;
        write_profile_properties(&profile.properties, &mut writer)?;
    }

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())?;
    mac.update(&payload);
    let signature = mac.finalize().into_bytes();

    let mut data = Vec::with_capacity(VELOCITY_SIGNATURE_BYTES + payload.len());
    data.extend_from_slice(&signature);
    data.extend_from_slice(&payload);
    Ok(data)
}

#[cfg(test)]
fn write_profile_properties(
    properties: &[ProfileProperty],
    writer: &mut PacketWriter<'_>,
) -> anyhow::Result<()> {
    VarInt(properties.len() as i32).serialize(writer)?;
    for property in properties {
        property.name.serialize(writer)?;
        property.value.serialize(writer)?;
        property.signature.is_some().serialize(writer)?;
        if let Some(signature) = &property.signature {
            signature.serialize(writer)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use qexed_packet::net_types::{GameProfile, ProfileProperty};

    use super::{
        parse_bungeecord_forwarding, parse_velocity_forwarding_response, velocity_forwarding_data,
    };

    #[test]
    fn parses_bungeecord_forwarding_data() {
        let host = concat!(
            "play.example.org\0",
            "203.0.113.10\0",
            "00112233445566778899aabbccddeeff\0",
            r#"[{"name":"textures","value":"skin","signature":"sig"}]"#
        );

        let forwarded = parse_bungeecord_forwarding(host, "Player").unwrap();

        assert_eq!(forwarded.login_host, "play.example.org");
        assert_eq!(forwarded.profile.username, "Player");
        assert_eq!(
            forwarded.profile.uuid,
            uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff)
        );
        assert_eq!(
            forwarded.profile.properties,
            vec![ProfileProperty {
                name: "textures".to_string(),
                value: "skin".to_string(),
                signature: Some("sig".to_string()),
            }]
        );
    }

    #[test]
    fn parses_velocity_forwarding_response() {
        let profile = GameProfile {
            uuid: uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff),
            username: "Player".to_string(),
            properties: vec![ProfileProperty {
                name: "textures".to_string(),
                value: "skin".to_string(),
                signature: Some("sig".to_string()),
            }],
        };
        let data = velocity_forwarding_data("secret", "203.0.113.10", &profile).unwrap();

        let parsed = parse_velocity_forwarding_response(&data, "secret").unwrap();

        assert_eq!(parsed, profile);
    }

    #[test]
    fn rejects_velocity_forwarding_response_with_wrong_secret() {
        let profile = GameProfile {
            uuid: uuid::Uuid::from_u128(0x00112233_4455_6677_8899_aabbccddeeff),
            username: "Player".to_string(),
            properties: Vec::new(),
        };
        let data = velocity_forwarding_data("secret", "203.0.113.10", &profile).unwrap();

        assert!(parse_velocity_forwarding_response(&data, "wrong").is_err());
    }
}
