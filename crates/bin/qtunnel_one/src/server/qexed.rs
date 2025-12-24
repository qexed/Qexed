use qexed_task::message::return_message::ReturnMessage;
use tokio::sync::mpsc::UnboundedSender;

pub async fn register_version_command(
    command_api: &UnboundedSender<ReturnMessage<qexed_command::message::ManagerCommand>>,
) -> anyhow::Result<()> {
    qexed_command::register::register_command(
        "version",
        "查看服务器版本信息",
        "qexed.console.version",
        vec![qexed_command::message::CommandParameter {
            name: "page".to_string(),
            description: "页码".to_string(),
            required: false,
            param_type: qexed_command::message::ParameterType::String{behavior:qexed_command::message::StringBehavior::SingleWord},
            suggestions: None,
        }],
        vec!["ver"],
        command_api,
        move |mut cmd_rx| {
            // 使用 move 关键字
            async move {
                // 处理命令，直到通道关闭
                while let Some(cmd) = cmd_rx.recv().await {
                    let version = format!("Qexed {}",qexed_config::QEXED_VERSION);
                    cmd.send_chat_message(&version).await?;
                }

                // 通道关闭，正常结束
                Ok(())
            }
        },
    )
    .await
}