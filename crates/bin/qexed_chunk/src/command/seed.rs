use qexed_task::message::{MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::global::GlobalCommand;

pub async fn register_seed_command(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<UnReturnMessage<GlobalCommand>>,
) -> anyhow::Result<()> {
    // 克隆 api2 用于闭包
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "seed",
        "显示世界种子。",
        "qexed.seed",
        vec![],
        vec![], // 可以添加多个别名
        command_api,
        move |mut cmd_rx| {
            let api2 = api2_for_closure.clone();
            async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    // 创建新的命令数据发送给管理器
                    UnReturnMessage::build(GlobalCommand::CommandSeed(cmd))
                    .post(&api2)
                    .await?;
                }
                Ok(())
            }
        },
    )
    .await
}
