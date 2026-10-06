use std::collections::HashMap;

use qexed_packet::net_types::RestBuffer;
use qexed_protocol::to_client::play::custom_payload::CustomPayload as ClientboundCustomPayload;
use qexed_plugins::api::{GeyserPlayerInfoResponse, PlayerClientPayload};

use crate::error::{PlayError, Result};

pub(super) const FLOODGATE_FORM_CHANNEL: &str = "floodgate:form";
const MAX_FORM_JSON_BYTES: usize = 64 * 1024;
const MAX_FORM_RESPONSE_BYTES: usize = 64 * 1024;

#[derive(Debug, Default)]
pub(super) struct GeyserRuntime {
    bedrock: bool,
    floodgate: bool,
    xuid: String,
    device_os: String,
    input_mode: String,
    ui_profile: String,
    next_form_id: u16,
    forms: HashMap<u16, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BedrockFormType {
    Modal,
    Simple,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct BedrockFormResponse {
    pub(super) form_id: u16,
    pub(super) plugin_form_id: String,
    pub(super) response: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct GeyserPayloadOutcome {
    pub(super) recognized: bool,
    pub(super) form_response: Option<BedrockFormResponse>,
}

impl GeyserRuntime {
    pub(super) fn mark_floodgate_player(&mut self) {
        self.bedrock = true;
        self.floodgate = true;
    }

    pub(super) fn apply_custom_payload(
        &mut self,
        payload: &qexed_protocol::to_server::play::custom_payload::CustomPayload,
    ) -> GeyserPayloadOutcome {
        match payload.channel.as_str() {
            FLOODGATE_FORM_CHANNEL => GeyserPayloadOutcome {
                recognized: true,
                form_response: self.apply_form_response(&payload.data.0),
            },
            "floodgate:skin" | "floodgate:packet" => {
                self.mark_floodgate_player();
                GeyserPayloadOutcome {
                    recognized: true,
                    form_response: None,
                }
            }
            _ => GeyserPayloadOutcome {
                recognized: false,
                form_response: None,
            },
        }
    }

    pub(super) fn player_info(
        &self,
        player: &qexed_player::OnlinePlayer,
    ) -> GeyserPlayerInfoResponse {
        GeyserPlayerInfoResponse {
            online: true,
            bedrock: self.bedrock || likely_bedrock_username(&player.profile.username),
            floodgate: self.floodgate,
            username: player.profile.username.clone(),
            java_uuid: player.profile.uuid.to_string(),
            xuid: self.xuid.clone(),
            device_os: self.device_os.clone(),
            input_mode: self.input_mode.clone(),
            ui_profile: self.ui_profile.clone(),
            language: player.language.clone(),
        }
    }

    pub(super) fn client_payload(&self, username: &str) -> PlayerClientPayload {
        PlayerClientPayload {
            bedrock: self.bedrock || likely_bedrock_username(username),
            floodgate: self.floodgate,
            xuid: self.xuid.clone(),
            device_os: self.device_os.clone(),
            input_mode: self.input_mode.clone(),
            ui_profile: self.ui_profile.clone(),
        }
    }

    pub(super) fn form_packet(
        &mut self,
        form_type: BedrockFormType,
        requested_form_id: Option<u16>,
        plugin_form_id: String,
        json: &str,
    ) -> Result<ClientboundCustomPayload> {
        let json = json.trim();
        if json.is_empty() {
            return Err(PlayError::msg("bedrock form json is empty"));
        }
        if json.len() > MAX_FORM_JSON_BYTES {
            return Err(PlayError::msg(format!(
                "bedrock form json exceeds {MAX_FORM_JSON_BYTES} bytes"
            )));
        }
        let form_id = requested_form_id.unwrap_or_else(|| self.next_allocated_form_id());
        self.forms.insert(form_id, plugin_form_id);
        Ok(ClientboundCustomPayload {
            channel: FLOODGATE_FORM_CHANNEL.to_string(),
            data: RestBuffer(encode_form_payload(form_type, form_id, json.as_bytes())),
        })
    }

    fn next_allocated_form_id(&mut self) -> u16 {
        let form_id = self.next_form_id & 0x7fff;
        self.next_form_id = if form_id == 0x7fff { 0 } else { form_id + 1 };
        form_id
    }

    fn apply_form_response(&mut self, data: &[u8]) -> Option<BedrockFormResponse> {
        self.mark_floodgate_player();
        if data.len() < 2 || data.len() > MAX_FORM_RESPONSE_BYTES {
            return None;
        }
        let form_id = u16::from_be_bytes([data[0], data[1]]);
        let response = String::from_utf8_lossy(&data[2..]).to_string();
        Some(BedrockFormResponse {
            form_id,
            plugin_form_id: self.forms.remove(&form_id).unwrap_or_default(),
            response,
        })
    }
}

impl BedrockFormType {
    pub(super) fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "modal" | "modal_form" | "modalform" => Self::Modal,
            "custom" | "custom_form" | "customform" => Self::Custom,
            _ => Self::Simple,
        }
    }

    fn wire_id(self) -> u8 {
        match self {
            Self::Modal => 0,
            Self::Simple => 1,
            Self::Custom => 2,
        }
    }
}

fn encode_form_payload(form_type: BedrockFormType, form_id: u16, json: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(json.len() + 3);
    data.push(form_type.wire_id());
    data.extend_from_slice(&form_id.to_be_bytes());
    data.extend_from_slice(json);
    data
}

fn likely_bedrock_username(username: &str) -> bool {
    username.starts_with('.') || username.starts_with('*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floodgate_form_payload_uses_type_id_form_id_and_json() {
        let data = encode_form_payload(BedrockFormType::Simple, 0x1234, br#"{"type":"form"}"#);
        assert_eq!(&data[..3], &[1, 0x12, 0x34]);
        assert_eq!(&data[3..], br#"{"type":"form"}"#);
    }

    #[test]
    fn form_response_maps_plugin_form_id() {
        let mut runtime = GeyserRuntime::default();
        let _ = runtime
            .form_packet(
                BedrockFormType::Simple,
                Some(7),
                "menu:main".to_string(),
                "{}",
            )
            .unwrap();
        let response = runtime.apply_form_response(b"\0\x07true").unwrap();

        assert!(runtime.floodgate);
        assert_eq!(response.form_id, 7);
        assert_eq!(response.plugin_form_id, "menu:main");
        assert_eq!(response.response, "true");
    }
}
