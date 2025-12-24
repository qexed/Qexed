use qexed_command::message::CommandData;
use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::ManagerMessage;

pub async fn register_tell_command(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<ReturnMessage<ManagerMessage>>,
) -> anyhow::Result<()> {
    // 克隆 api2 用于闭包
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "tell",
        "与玩家私聊",
        "qexed.tell",
        vec![
            // 第一个参数：玩家
            qexed_command::message::CommandParameter {
                name: "player".to_string(),
                description: "要私聊的玩家".to_string(),
                required: true,
                param_type: qexed_command::message::ParameterType::String {
                    behavior: qexed_command::message::StringBehavior::Greedy,
                },
                suggestions: None,
            },
            // 第二个参数：消息内容
            qexed_command::message::CommandParameter {
                name: "message".to_string(),
                description: "私聊消息内容".to_string(),
                required: true,
                param_type: qexed_command::message::ParameterType::String {
                    behavior: qexed_command::message::StringBehavior::Greedy,
                },
                suggestions: None,
            },
        ],
        vec![], // 可以添加多个别名
        command_api,
        move |mut cmd_rx| {
            let api2 = api2_for_closure.clone();
            async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    // 创建新的命令数据发送给管理器
                    ReturnMessage::build(ManagerMessage::Command(cmd))
                    .get(&api2)
                    .await?;
                }
                Ok(())
            }
        },
    )
    .await
}

pub async fn register_me_command(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<ReturnMessage<ManagerMessage>>,
) -> anyhow::Result<()> {
    // 克隆 api2 用于闭包
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "me",
        "广播一条关于你的信息。",
        "qexed.me",
        vec![
            qexed_command::message::CommandParameter {
                name: "message".to_string(),
                description: "指定要显示的消息。".to_string(),
                required: true,
                param_type: qexed_command::message::ParameterType::String {
                    behavior: qexed_command::message::StringBehavior::Greedy,
                },
                suggestions: None,
            },
        ],
        vec![], // 可以添加多个别名
        command_api,
        move |mut cmd_rx| {
            let api2 = api2_for_closure.clone();
            async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    // 创建新的命令数据发送给管理器
                    ReturnMessage::build(ManagerMessage::CommandMe(cmd))
                    .get(&api2)
                    .await?;
                }
                Ok(())
            }
        },
    )
    .await
}
