use crate::message::{CommandParameter, ManagerCommand, ParameterType, StringBehavior};
use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::sync::mpsc::UnboundedSender;
pub async fn register(api: &UnboundedSender<ReturnMessage<ManagerCommand>>) -> anyhow::Result<()> {
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    if let crate::message::ManagerCommand::RegisterCommand { success, .. } =
        ReturnMessage::build(crate::message::ManagerCommand::RegisterCommand {
            name: "help".to_string(),
            doc: "查询帮助".to_string(),
            permission: "qexed.help".to_string(),
            parameters: vec![CommandParameter {
                name: "page_or_command".to_string(),
                description: "页码或命令名".to_string(),
                required: false,
                param_type: ParameterType::String {
                    behavior: StringBehavior::SingleWord,
                },
                suggestions: None, // 可以动态填充建议
            }],
            aliases: vec!["帮助".to_string(), "?".to_string(), "helpme".to_string()],
            api: Some(cmd_tx),
            success: false,
        })
        .get(api)
        .await?
    {
        if success == false {
            log::error!("系统命令/help注册失败关闭");
            // 这里可以添加服务器关闭逻辑
            return Ok(());
        };
        let api = api.clone();
        tokio::spawn(async move {
            // 给任务起个名字，便于日志追踪
            let task_name = "help_command_handler";
            log::debug!("[{}] 任务启动", task_name);

            while let Some(cmd) = cmd_rx.recv().await {
                // 是否是命令行
                match ReturnMessage::build(ManagerCommand::CommandHelp(cmd))
                    .get(&api)
                    .await
                {
                    Ok(_) => {}
                    Err(_) => {
                        // cmd.send_chat_message("命令执行失败").await?;
                    }
                }
            }

            log::info!(
                "[{}] 任务结束，指令接收通道已关闭或收到退出指令。",
                task_name
            );
        });
    } else {
        log::error!("系统命令/help注册失败关闭");
        return Ok(());
    }
    Ok(())
}
