use qexed_task::message::{MessageType, return_message::ReturnMessage};
use tokio::{process::Command, sync::mpsc::UnboundedSender};
use crate::message::{CommandData, CommandParameter, ManagerCommand};
pub mod help;

pub async fn register(api:&UnboundedSender<ReturnMessage<ManagerCommand>>)->anyhow::Result<()>{
    help::register(api).await?;
    Ok(())
}
// 简化的命令注册函数
pub async fn register_command<F, Fut>(
    name: &str,
    doc: &str,
    permission: &str,
    parameters: Vec<CommandParameter>,
    aliases: Vec<&str>,
    command_api: &UnboundedSender<ReturnMessage<ManagerCommand>>,
    mut handler: F,
) -> anyhow::Result<()>
where
    F: FnMut(tokio::sync::mpsc::UnboundedReceiver<CommandData>) -> Fut + Send + 'static,
    Fut: Future<Output = anyhow::Result<()>> + Send + 'static,
{
    // 创建命令通道
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    
    // 构建注册消息
    let register_msg = ManagerCommand::RegisterCommand {
        name: name.to_string(),
        doc: doc.to_string(),
        permission: permission.to_string(),
        parameters,
        aliases: aliases.into_iter().map(|s| s.to_string()).collect(),
        api: Some(cmd_tx),
        success: false,
    };
    
    // 发送注册请求
    let response = ReturnMessage::build(register_msg)
        .get(command_api)
        .await?;
    
    // 检查注册结果
    if let ManagerCommand::RegisterCommand { name, success, .. } = response {
        if !success {
            anyhow::bail!("命令 '{}' 注册失败", name);
        }
        
        // 启动命令处理任务
        tokio::spawn(async move {
            log::debug!("[指令] [{}] 服务启动", name);
            let _ = handler(cmd_rx).await;
            log::info!("[指令] [{}] 服务结束", name);
        });
    } else {
        anyhow::bail!("命令注册响应格式错误");
    }
    
    Ok(())
}