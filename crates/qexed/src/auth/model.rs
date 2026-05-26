use serde::Deserialize;

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

#[derive(Debug, Deserialize)]
pub(super) struct SessionProfile {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(default)]
    pub(super) properties: Vec<SessionProperty>,
}

#[derive(Debug, Deserialize)]
pub(super) struct SessionProperty {
    name: String,
    value: String,
    signature: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ServicesPublicKeys {
    #[serde(rename = "playerCertificateKeys", default)]
    pub(super) player_certificate_keys: Vec<ServicesPublicKey>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ServicesPublicKey {
    #[serde(rename = "publicKey")]
    pub(super) public_key: String,
}

impl TryFrom<SessionProfile> for AuthenticatedProfile {
    type Error = anyhow::Error;

    fn try_from(value: SessionProfile) -> Result<Self, Self::Error> {
        Ok(Self {
            uuid: super::util::parse_mojang_uuid(&value.id)?,
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
