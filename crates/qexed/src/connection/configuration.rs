use qexed_packet::Packet;
use qexed_protocol::{
    to_client,
    to_server::configuration::{
        accept_code_of_conduct::AcceptCodeOfConduct,
        finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
        resource_pack_receive::ResourcePackReceive,
        select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
        settings::Settings as ServerboundSettings,
    },
};

use super::{
    SERVER_BRAND, ServerContext,
    codec::{decode_payload, read_packet_id, string_payload},
    optional_text_component, text_component,
};

pub(super) async fn handle_configuration<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
    login_host: &str,
) -> anyhow::Result<Option<String>>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    log::debug!("send configuration server brand: {SERVER_BRAND}");
    sink.send(to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(string_payload(SERVER_BRAND)?),
    })
    .await?;

    log::debug!(
        "send configuration feature flag: {}",
        crate::registry_sync::VANILLA_FEATURE
    );
    sink.send(to_client::configuration::feature_flags::FeatureFlags {
        features: vec![crate::registry_sync::VANILLA_FEATURE.to_string()],
    })
    .await?;

    let known_packs = crate::registry_sync::known_packs();
    log::debug!("request client known-pack selection: {known_packs:?}");
    sink.send(to_client::configuration::select_known_packs::SelectKnownPacks { known_packs })
        .await?;
    sink.flush().await?;

    let configuration_start = wait_for_known_packs(packets).await?;
    let selected_packs = configuration_start.selected_packs;
    log::debug!("client selected known packs: {:?}", selected_packs.entries);
    let include_registry_contents =
        !crate::registry_sync::accepts_vanilla_core_pack(&selected_packs.entries);
    log::debug!("send registry data, include full contents: {include_registry_contents}");

    let registry_packets = crate::registry_sync::load_registry_packets(include_registry_contents)?;
    log::debug!(
        "prepared registry data packet count: {}",
        registry_packets.len()
    );
    for packet in registry_packets {
        log::trace!("send registry: {}", packet.id);
        sink.send(packet).await?;
    }

    let tag_packet = crate::registry_sync::load_tag_packet()?;
    log::debug!("send tags, registry count: {}", tag_packet.tags.len());
    sink.send(tag_packet).await?;

    if let Some(code_of_conduct) = context
        .code_of_conducts
        .select(configuration_start.client_locale.as_deref())
    {
        log::debug!("send code of conduct and wait for client acceptance");
        sink.send(to_client::configuration::code_of_conduct::CodeOfConduct {
            code_of_conduct: code_of_conduct.to_string(),
        })
        .await?;
        wait_for_code_of_conduct_accept(packets).await?;
        log::debug!("client accepted code of conduct");
    }

    send_configured_resource_pack(packets, sink, context, login_host).await?;

    log::debug!("send finish configuration packet");
    sink.send(to_client::configuration::finish_configuration::FinishConfiguration {})
        .await?;
    sink.flush().await?;
    wait_for_finish_configuration(packets).await?;
    log::debug!("client finished configuration");

    Ok(configuration_start.client_locale)
}

async fn send_configured_resource_pack<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    context: &ServerContext,
    login_host: &str,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let resource_pack = &context.config.server.resource_pack;
    if !resource_pack.enable {
        return Ok(());
    }

    let Some(offer) = context.resource_pack.offer(resource_pack, login_host) else {
        log::warn!("resource pack is enabled but no download URL is available; skipping");
        return Ok(());
    };

    sink.send(
        to_client::configuration::add_resource_pack::AddResourcePack {
            id: resource_pack.id,
            url: offer.url,
            hash: offer.hash,
            required: resource_pack.required,
            prompt: optional_text_component(&resource_pack.prompt),
        },
    )
    .await?;
    sink.flush().await?;

    wait_for_resource_pack_result(packets, sink, resource_pack).await
}

async fn wait_for_resource_pack_result<R, W>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
    sink: &mut qexed_tcp_connect::PacketSink<W>,
    resource_pack: &qexed_config::app::qexed::server::ResourcePack,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("connection closed while waiting for resource pack response");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id != ResourcePackReceive::ID {
            log::debug!(
                "skip configuration packet while waiting for resource pack response: {packet_id}"
            );
            continue;
        }

        let result = decode_payload::<ResourcePackReceive>(&mut payload)?;
        if result.uuid != resource_pack.id {
            log::debug!(
                "ignore resource pack response for another pack: expected={}, actual={}, status={}",
                resource_pack.id,
                result.uuid,
                result.result.0
            );
            continue;
        }

        let status = ResourcePackStatus::from_protocol_id(result.result.0);
        log::debug!("client resource pack response: {}", status.as_str());
        if status.is_successfully_loaded() {
            return Ok(());
        }

        if resource_pack.required && status.is_terminal_failure() {
            sink.send(to_client::configuration::disconnect::Disconnect {
                reason: text_component(&resource_pack.disconnect_message),
            })
            .await?;
            sink.flush().await?;
            anyhow::bail!(
                "client did not accept required resource pack: {}",
                status.as_str()
            );
        }

        if !resource_pack.required && status.is_terminal_failure() {
            return Ok(());
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResourcePackStatus {
    SuccessfullyLoaded,
    Declined,
    FailedDownload,
    Accepted,
    Downloaded,
    InvalidUrl,
    FailedReload,
    Discarded,
    Unknown(i32),
}

impl ResourcePackStatus {
    fn from_protocol_id(id: i32) -> Self {
        match id {
            0 => Self::SuccessfullyLoaded,
            1 => Self::Declined,
            2 => Self::FailedDownload,
            3 => Self::Accepted,
            4 => Self::Downloaded,
            5 => Self::InvalidUrl,
            6 => Self::FailedReload,
            7 => Self::Discarded,
            other => Self::Unknown(other),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::SuccessfullyLoaded => "successfully_loaded",
            Self::Declined => "declined",
            Self::FailedDownload => "failed_download",
            Self::Accepted => "accepted",
            Self::Downloaded => "downloaded",
            Self::InvalidUrl => "invalid_url",
            Self::FailedReload => "failed_reload",
            Self::Discarded => "discarded",
            Self::Unknown(_) => "unknown",
        }
    }

    fn is_successfully_loaded(self) -> bool {
        matches!(self, Self::SuccessfullyLoaded)
    }

    fn is_terminal_failure(self) -> bool {
        matches!(
            self,
            Self::Declined
                | Self::FailedDownload
                | Self::InvalidUrl
                | Self::FailedReload
                | Self::Discarded
                | Self::Unknown(_)
        )
    }
}

async fn wait_for_known_packs<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<ConfigurationStart>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut client_locale = None;
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("connection closed while waiting for known-pack selection");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundSelectKnownPacks::ID {
            return Ok(ConfigurationStart {
                selected_packs: decode_payload::<ServerboundSelectKnownPacks>(&mut payload)?,
                client_locale,
            });
        }

        if packet_id == ServerboundSettings::ID {
            let settings = decode_payload::<ServerboundSettings>(&mut payload)?;
            log::debug!("client locale: {}", settings.locale);
            client_locale = Some(settings.locale);
            continue;
        }

        log::debug!(
            "skip configuration packet while waiting for known-pack selection: {packet_id}"
        );
    }
}

#[derive(Debug)]
struct ConfigurationStart {
    selected_packs: ServerboundSelectKnownPacks,
    client_locale: Option<String>,
}

async fn wait_for_code_of_conduct_accept<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("connection closed while waiting for code-of-conduct acceptance");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == AcceptCodeOfConduct::ID {
            decode_payload::<AcceptCodeOfConduct>(&mut payload)?;
            return Ok(());
        }

        log::debug!(
            "skip configuration packet while waiting for code-of-conduct acceptance: {packet_id}"
        );
    }
}

async fn wait_for_finish_configuration<R>(
    packets: &mut qexed_tcp_connect::PacketStream<R>,
) -> anyhow::Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            anyhow::bail!("connection closed while waiting for finish configuration");
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundFinishConfiguration::ID {
            decode_payload::<ServerboundFinishConfiguration>(&mut payload)?;
            return Ok(());
        }

        log::debug!(
            "skip configuration packet while waiting for finish configuration: {packet_id}"
        );
    }
}
