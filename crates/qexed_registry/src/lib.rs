use qexed_config::tool::AppConfigTrait;

mod mojang;
mod registries;
mod registry_sync;
mod tags;
mod util;

pub use registries::{dimension_type_holder_id, dynamic_registry_entry_id, load_registry_packets};
pub use registry_sync::{
    STATIC_TAG_REGISTRIES, VANILLA_FEATURE, accepts_vanilla_core_pack, known_packs,
};
pub use tags::load_tag_packet;

pub fn init() -> anyhow::Result<()> {
    let join_handle = AppConfigTrait::load_or_create_default(config_update)?;
    tokio::task::block_in_place(|| {
        tokio::runtime::Handle::current().block_on(async {
            join_handle
                .await
                .map_err(|e| anyhow::anyhow!("启动后首次重载任务失败: {}", e))
        })
    })?
}

async fn config_update(config: qexed_config::app::qexed_registry::Registry) -> anyhow::Result<()> {
    tklog::info!("初始化/重载注册表中");
    mojang::init(&config).await?;
    registry_sync::ensure_data_ready()?;
    Ok(())
}

#[cfg(test)]
#[path = "../tests.rs"]
mod tests;
