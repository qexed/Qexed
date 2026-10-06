//! 配置阶段握手（v4 connection::configuration 迁移）。
//!
//! 包名对齐 v6：CustomPayload 同名；FeatureFlags→UpdateEnabledFeatures；
//! SelectKnownPacks 同名（字段 known_packs）；RegistryData 同名；
//! Tags→UpdateTags；AddResourcePack→ResourcePackPush；
//! ResourcePackReceive→ResourcePack(id+action)；Settings→ClientInformation；
//! CodeOfConduct / FinishConfiguration / Disconnect 同名。
//!
//! 注册表数据加载从 v4 crate::registry_sync 改为 v6 的 qexed_mojang_data。
//! 资源包推送的来源解析（URL 直推 / 对象存储直链 / 本地包 HTTP 下载服务）
//! 在 super::resource_pack（连接域自持，v4 ResourcePackManager 的等价实现）；
//! qexed_server 组装层可通过 ServerContext::set_resource_pack_offer 注入
//! 自定义解析回调（如对象存储上传后回填 URL）覆盖默认行为。

use qexed_packet::Packet;
use qexed_protocol::{
    to_client,
    to_server::configuration::{
        accept_code_of_conduct::AcceptCodeOfConduct,
        client_information::ClientInformation,
        finish_configuration::FinishConfiguration as ServerboundFinishConfiguration,
        resource_pack::ResourcePack as ServerboundResourcePack,
        select_known_packs::SelectKnownPacks as ServerboundSelectKnownPacks,
    },
};

use super::{
    SERVER_BRAND, ServerContext,
    codec::{decode_payload, read_packet_id, string_payload},
    optional_text_component, text_component,
};
use crate::error::{ConnectionError, Result};

pub(super) async fn handle_configuration<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
    login_host: &str,
) -> Result<ClientConfiguration>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    log::debug!("{}", qexed_language::t("qexed.connection.config.send_brand"));
    sink.send(to_client::configuration::custom_payload::CustomPayload {
        channel: "minecraft:brand".to_string(),
        data: qexed_packet::net_types::RestBuffer(string_payload(SERVER_BRAND)?),
    })
    .await?;

    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.send_feature_flags").replace(
            "%{feature}",
            qexed_mojang_data::registry_sync::VANILLA_FEATURE,
        ),
    );
    sink.send(to_client::configuration::update_enabled_features::UpdateEnabledFeatures {
        features: vec![qexed_mojang_data::registry_sync::VANILLA_FEATURE.to_string()],
    })
    .await?;

    let known_packs = qexed_mojang_data::registry_sync::known_packs();
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.request_known_packs").replace(
            "%{packs}",
            &format!("{known_packs:?}"),
        ),
    );
    sink.send(
        to_client::configuration::select_known_packs::SelectKnownPacks { known_packs },
    )
    .await?;
    sink.flush().await?;

    let configuration_start = wait_for_known_packs(packets).await?;
    let selected_packs = configuration_start.selected_packs;
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.client_selected_packs").replace(
            "%{packs}",
            &format!("{:?}", selected_packs.entries),
        ),
    );
    let include_registry_contents =
        !qexed_mojang_data::registry_sync::accepts_vanilla_core_pack(&selected_packs.entries);
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.send_registry").replace(
            "%{full}",
            &include_registry_contents.to_string(),
        ),
    );

    let registry_packets = qexed_mojang_data::registry_sync::load_registry_packets(
        include_registry_contents,
    )
    .map_err(|err| ConnectionError::Registry(err.to_string()))?;
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.registry_packet_count").replace(
            "%{count}",
            &registry_packets.len().to_string(),
        ),
    );
    for packet in registry_packets {
        log::trace!("{}", qexed_language::t("qexed.connection.config.send_registry_packet").replace("%{id}", &packet.id));
        sink.send(packet).await?;
    }

    let tag_packet = qexed_mojang_data::registry_sync::load_tag_packet()
        .map_err(|err| ConnectionError::Registry(err.to_string()))?;
    log::debug!(
        "{}",
        qexed_language::t("qexed.connection.config.send_tags").replace(
            "%{count}",
            &tag_packet.tags.len().to_string(),
        ),
    );
    sink.send(tag_packet).await?;

    if let Some(code_of_conduct) = context
        .code_of_conducts
        .select(configuration_start.client.locale.as_deref())
    {
        log::debug!("{}", qexed_language::t("qexed.connection.config.send_code_of_conduct"));
        sink.send(to_client::configuration::code_of_conduct::CodeOfConduct {
            code_of_conduct: code_of_conduct.to_string(),
        })
        .await?;
        wait_for_code_of_conduct_accept(packets).await?;
        log::debug!("{}", qexed_language::t("qexed.connection.config.code_of_conduct_accepted"));
    }

    send_configured_resource_pack(packets, sink, context, login_host).await?;

    log::debug!("{}", qexed_language::t("qexed.connection.config.send_finish"));
    sink.send(to_client::configuration::finish_configuration::FinishConfiguration {})
        .await?;
    sink.flush().await?;
    wait_for_finish_configuration(packets).await?;
    log::debug!("{}", qexed_language::t("qexed.connection.config.client_finished"));

    Ok(configuration_start.client)
}

async fn send_configured_resource_pack<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    context: &ServerContext,
    login_host: &str,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    let resource_pack = &context.config.resource_pack;
    if !resource_pack.enable {
        return Ok(());
    }

    // 组装层注入的 offer 回调优先（qexed_server 侧可能有对象存储上传等更强实现）；
    // 未注入时用连接域自带的 resolve_offer：Url 直推 / 对象存储拼直链 /
    // Local 读 zip → sha1 → 起本地 HTTP 下载服务（v4 ResourcePackManager 全量行为）。
    let offer = match context.resource_pack_offer.clone() {
        Some(resolve) => resolve(resource_pack, login_host)
            .await
            .map_err(ConnectionError::msg)?
            .map(|(url, hash)| super::resource_pack::ResourcePackOffer { url, hash }),
        None => super::resource_pack::resolve_offer(resource_pack, login_host).await?,
    };

    let Some(offer) = offer else {
        return Ok(());
    };

    sink.send(to_client::configuration::resource_pack_push::ResourcePackPush {
        id: resource_pack.pack_id(),
        url: offer.url,
        hash: offer.hash,
        required: resource_pack.required,
        prompt: optional_text_component(&resource_pack.prompt),
    })
    .await?;
    sink.flush().await?;

    wait_for_resource_pack_result(packets, sink, resource_pack).await
}

async fn wait_for_resource_pack_result<R, W>(
    packets: &mut crate::transport::PacketStream<R>,
    sink: &mut crate::transport::PacketSink<W>,
    resource_pack: &crate::config::ResourcePack,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
    W: tokio::io::AsyncWrite + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            return Err(ConnectionError::msg(
                "connection closed while waiting for resource pack response",
            ));
        };

        let packet_id = read_packet_id(&mut payload)?;
        log::info!("[diag wait cfg] id=0x{:x} len={}", packet_id, payload.len());
        if packet_id != ServerboundResourcePack::ID {
            log::debug!(
                "{}",
                qexed_language::t(
                    "qexed.connection.config.skip_packet_wait_resource_pack",
                )
                .replace("%{id}", &packet_id.to_string()),
            );
            continue;
        }

        let result = decode_payload::<ServerboundResourcePack>(&mut payload)?;
        if result.id != resource_pack.pack_id() {
            log::debug!(
                "{}",
                qexed_language::t("qexed.connection.config.resource_pack_other_pack").replace(
                    "%{expected}",
                    &resource_pack.pack_id().to_string(),
                )
                .replace("%{actual}", &result.id.to_string())
                .replace("%{status}", &result.action.0.to_string()),
            );
            continue;
        }

        let status = ResourcePackStatus::from_protocol_id(result.action.0);
        log::debug!(
            "{}",
            qexed_language::t("qexed.connection.config.resource_pack_response").replace(
                "%{status}",
                status.as_str(),
            ),
        );
        if status.is_successfully_loaded() {
            return Ok(());
        }

        if resource_pack.required && status.is_terminal_failure() {
            sink.send(to_client::configuration::disconnect::Disconnect {
                reason: text_component(&resource_pack.disconnect_message),
            })
            .await?;
            sink.flush().await?;
            return Err(ConnectionError::msg(format!(
                "client did not accept required resource pack: {}",
                status.as_str()
            )));
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
    packets: &mut crate::transport::PacketStream<R>,
) -> Result<ConfigurationStart>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut client = ClientConfiguration::default();
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            return Err(ConnectionError::msg(
                "connection closed while waiting for known-pack selection",
            ));
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundSelectKnownPacks::ID {
            return Ok(ConfigurationStart {
                selected_packs: decode_payload::<ServerboundSelectKnownPacks>(&mut payload)?,
                client,
            });
        }

        if packet_id == ClientInformation::ID {
            let settings = match decode_payload::<ClientInformation>(&mut payload) {
                    Ok(s) => s,
                    Err(err) => {
                        log::error!("[diag] client_information decode 失败: {err:#} | payload hex={}", payload.iter().map(|b| format!("{b:02x}")).collect::<String>());
                        return Err(err.into());
                    }
                };
            log::debug!(
                "{}",
                qexed_language::t("qexed.connection.config.client_locale").replace(
                    "%{locale}",
                    &settings.language,
                ),
            );
            client.locale = Some(settings.language);
            client.displayed_skin_parts = settings.model_customisation;
            continue;
        }

        log::debug!(
            "{}",
            qexed_language::t("qexed.connection.config.skip_packet_wait_known_packs").replace(
                "%{id}",
                &packet_id.to_string(),
            ),
        );
    }
}

#[derive(Debug)]
struct ConfigurationStart {
    selected_packs: ServerboundSelectKnownPacks,
    client: ClientConfiguration,
}

/// 客户端配置信息（v4 ClientConfiguration 迁移；LoginOutcome 公开字段）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientConfiguration {
    pub locale: Option<String>,
    pub displayed_skin_parts: u8,
}

/// v4 引用 crate::players::DEFAULT_DISPLAYED_SKIN_PARTS（qexed_player crate）；
/// v6 该 crate 尚未迁移到此，先在连接域内复制常量，待 qexed_player 就绪后改引用。
pub(crate) const DEFAULT_DISPLAYED_SKIN_PARTS: u8 = 0x7f;

impl Default for ClientConfiguration {
    fn default() -> Self {
        Self {
            locale: None,
            displayed_skin_parts: DEFAULT_DISPLAYED_SKIN_PARTS,
        }
    }
}
async fn wait_for_code_of_conduct_accept<R>(
    packets: &mut crate::transport::PacketStream<R>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            return Err(ConnectionError::msg(
                "connection closed while waiting for code-of-conduct acceptance",
            ));
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == AcceptCodeOfConduct::ID {
            decode_payload::<AcceptCodeOfConduct>(&mut payload)?;
            return Ok(());
        }

        log::debug!(
            "{}",
            qexed_language::t(
                "qexed.connection.config.skip_packet_wait_code_of_conduct",
            )
            .replace("%{id}", &packet_id.to_string()),
        );
    }
}

async fn wait_for_finish_configuration<R>(
    packets: &mut crate::transport::PacketStream<R>,
) -> Result<()>
where
    R: tokio::io::AsyncRead + Unpin,
{
    loop {
        let Some(mut payload) = packets.read_packet().await? else {
            return Err(ConnectionError::msg(
                "connection closed while waiting for finish configuration",
            ));
        };

        let packet_id = read_packet_id(&mut payload)?;
        if packet_id == ServerboundFinishConfiguration::ID {
            decode_payload::<ServerboundFinishConfiguration>(&mut payload)?;
            return Ok(());
        }

        log::debug!(
            "{}",
            qexed_language::t("qexed.connection.config.skip_packet_wait_finish").replace(
                "%{id}",
                &packet_id.to_string(),
            ),
        );
    }
}