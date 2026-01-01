use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::sync::mpsc::UnboundedSender;

use crate::message::ManagerMessage;

pub async fn register_title_command_full(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
    api2: UnboundedSender<ReturnMessage<ManagerMessage>>,
) -> anyhow::Result<()> {
    let api2_for_closure = api2.clone();

    qexed_command::register::register_command(
        "title",
        "向玩家显示屏幕标题",
        "qexed.title",
        vec![
            // 第二个参数：消息内容
            qexed_command::message::CommandParameter {
                name: "message".to_string(),
                description: "标题内容".to_string(),
                required: true,
                param_type: qexed_command::message::ParameterType::String {
                    behavior: qexed_command::message::StringBehavior::Greedy,
                },
                suggestions: None,
            },
            // // 目标玩家
            // qexed_command::message::CommandParameter {
            //     name: "target".to_string(),
            //     description: "目标玩家（@a, @p, @r, @s 或玩家名）".to_string(),
            //     required: true,
            //     param_type: qexed_command::message::ParameterType::String {
            //         behavior: qexed_command::message::StringBehavior::SingleWord,
            //     },
            //     suggestions: Some(vec![
            //         "@a".to_string(),
            //         "@p".to_string(),
            //         "@r".to_string(),
            //         "@s".to_string(),
            //     ]),
            // },
            // // 子命令
            // qexed_command::message::CommandParameter {
            //     name: "action".to_string(),
            //     description: "标题动作".to_string(),
            //     required: true,
            //     param_type: qexed_command::message::ParameterType::String {
            //         behavior: qexed_command::message::StringBehavior::SingleWord,
            //     },
            //     suggestions: Some(vec![
            //         "clear".to_string(),
            //         "reset".to_string(),
            //         "title".to_string(),
            //         "subtitle".to_string(),
            //         "actionbar".to_string(),
            //     ]),
            // },
            // // 标题文本（可选的JSON文本）
            // qexed_command::message::CommandParameter {
            //     name: "title_text".to_string(),
            //     description: "标题文本（JSON格式）".to_string(),
            //     required: false,
            //     param_type: qexed_command::message::ParameterType::String {
            //         behavior: qexed_command::message::StringBehavior::Greedy,
            //     },
            //     suggestions: None,
            // },
            // // 可选的时间参数
            // qexed_command::message::CommandParameter {
            //     name: "fadein".to_string(),
            //     description: "淡入时间（刻）".to_string(),
            //     required: false,
            //     param_type: qexed_command::message::ParameterType::Integer {
            //         min: Some(0),
            //         max: Some(1000),
            //     },
            //     suggestions: None,
            // },
            // qexed_command::message::CommandParameter {
            //     name: "stay".to_string(),
            //     description: "停留时间（刻）".to_string(),
            //     required: false,
            //     param_type: qexed_command::message::ParameterType::Integer {
            //         min: Some(0),
            //         max: Some(1000),
            //     },
            //     suggestions: None,
            // },
            // qexed_command::message::CommandParameter {
            //     name: "fadeout".to_string(),
            //     description: "淡出时间（刻）".to_string(),
            //     required: false,
            //     param_type: qexed_command::message::ParameterType::Integer {
            //         min: Some(0),
            //         max: Some(1000),
            //     },
            //     suggestions: None,
            // },
        ],
        vec![], // 别名
        command_api,
        move |mut cmd_rx| {
            let api2 = api2_for_closure.clone();
            async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    // 发送到管理器
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