use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::ManagerMessage;

pub async fn register_commands(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<ReturnMessage<ManagerMessage>>,
) -> anyhow::Result<()> {
    // 克隆 api2 用于闭包
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "quard",
        "封禁规则指令",
        "qexed.quard",
        vec![],
        vec![],
        command_api,
        move |mut cmd_rx| {
            // 使用 move 关键字
            let api2 = api2_for_closure.clone(); // 在闭包内部再克隆一次
            async move {
                // 处理命令，直到通道关闭
                while let Some(cmd) = cmd_rx.recv().await {
                    ReturnMessage::build(ManagerMessage::Command(cmd))
                        .get(&api2) // 使用闭包内部的 api2
                        .await?;
                }

                // 通道关闭，正常结束
                Ok(())
            }
        },
    )
    .await
}
