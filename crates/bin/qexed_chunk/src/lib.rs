use qexed_task::message::{MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

pub mod message;
pub mod engine;
pub mod data_type;
pub mod command;


pub async fn run(
    config: qexed_config::app::qexed_chunk::ChunkConfig,
) -> anyhow::Result<UnboundedSender<UnReturnMessage<message::world::WorldCommand>>> {
    let app = match config.engine {
        // qexed_config::app::qexed_chunk::engine::Engine::Original => {},
        qexed_config::app::qexed_chunk::engine::Engine::MiniLobby => engine::mini_lobby::run(config.engine_setting.minilobby).await?,
        // qexed_config::app::qexed_chunk::engine::Engine::OpenLobby => {},
        // qexed_config::app::qexed_chunk::engine::Engine::VoidOnlyRead => {},
        // qexed_config::app::qexed_chunk::engine::Engine::BedWar => {},
        // qexed_config::app::qexed_chunk::engine::Engine::SkyWar => {},
        // qexed_config::app::qexed_chunk::engine::Engine::JumpRope => {},
        // qexed_config::app::qexed_chunk::engine::Engine::Custom => {},
        _ => {
            return Err(anyhow::anyhow!("当前版本暂时仅支持MiniLobby引擎"));
        }

    };
    log::info!("[服务] 区块 已启用");
    Ok(app)
    // let manager_actor =engine::original::event::global::GlobalManage::new(config);
    // let (manager_task, manager_sender) =
    //     qexed_task::task::task_manage::TaskManage::new(manager_actor);
    
    // manager_task.run().await?;
    // manager_sender.send(UnReturnMessage::build(message::global::GlobalCommand::Init))?;
    // log::info!("[服务] 区块 已启用");
    // Ok(manager_sender)
}