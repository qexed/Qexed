use async_trait::async_trait;
use dashmap::DashMap;
use futures::future::try_join_all;
use qexed_task::{
    event::task_manage::TaskManageEvent,
    message::{MessageSender, MessageType, unreturn_message::UnReturnMessage},
};
use tokio::sync::oneshot;

use crate::{
    data_type::direction::{Direction, DirectionMap},
    engine::mini_lobby::event::{chunk::ChunkTask, region::RegionManage},
    message::{
        chunk::ChunkCommand,
        region::{RegionCommand, RegionCommandResult},
        world::WorldCommand,
    },
};

#[async_trait]
impl TaskManageEvent<[i64; 2], UnReturnMessage<RegionCommand>, UnReturnMessage<ChunkCommand>>
    for RegionManage
{
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<RegionCommand>>,
        task_map: &DashMap<[i64; 2], MessageSender<UnReturnMessage<ChunkCommand>>>,
        data: UnReturnMessage<RegionCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            RegionCommand::Init => {
                self.init(api, task_map).await?;
                Ok(false)
            }
            RegionCommand::GetChunkApi { pos, result } => {
                // 计算 pos 是否在本区域范围
                if self.is_chunk_in_region(pos) {
                    if let Some(chunk) = task_map.get(&pos) {
                        result
                            .send(
                                crate::message::region::RegionCommandResult::GetChunkApiResult {
                                    success: true,
                                    api: Some(chunk.clone()),
                                },
                            )
                            .map_err(|e| {
                                // 将 RegionCommandResult 转换为 anyhow::Error
                                anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                            })?;
                    } else {
                        result
                            .send(
                                crate::message::region::RegionCommandResult::GetChunkApiResult {
                                    success: false,
                                    api: None,
                                },
                            )
                            .map_err(|e| {
                                // 将 RegionCommandResult 转换为 anyhow::Error
                                anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                            })?;
                    }
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    region.send(UnReturnMessage::build(RegionCommand::GetChunkApi {
                        pos,
                        result,
                    }))?;
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api
                    .send(UnReturnMessage::build(WorldCommand::GetChunkApi {
                        pos,
                        result,
                    }))?;
                return Ok(false);
            }
            RegionCommand::GetOtherWorldChunkApi { pos, world, result } => {
                // 计算是否属于当前世界
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    let _ = api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::GetChunkApi { pos, result },
                        ),
                    );
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::GetOtherWorldChunkApi { pos, world, result },
                ))?;
                return Ok(false);
            }
            RegionCommand::GetRegionApi { pos, result } => {
                // 虽然可能有点怀疑，但是把玩意有人误操作呢。
                if pos == self.pos {
                    result
                        .send(
                            crate::message::region::RegionCommandResult::GetRegionApiResult {
                                success: true,
                                api: Some(api.clone()),
                            },
                        )
                        .map_err(|e| {
                            // 将 RegionCommandResult 转换为 anyhow::Error
                            anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                        })?;
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    result
                        .send(
                            crate::message::region::RegionCommandResult::GetRegionApiResult {
                                success: true,
                                api: Some(region.clone()),
                            },
                        )
                        .map_err(|e| {
                            // 将 RegionCommandResult 转换为 anyhow::Error
                            anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                        })?;
                    return Ok(false);
                }
                // 最后请求
                self.master_api
                    .send(UnReturnMessage::build(WorldCommand::GetRegionApi {
                        pos,
                        result,
                    }))?;
                return Ok(false);
            }
            RegionCommand::GetOtherWorldRegionApi { pos, world, result } => {
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::GetRegionApi { pos, result },
                        ),
                    )?;
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::GetOtherWorldRegionApi { pos, world, result },
                ))?;
                return Ok(false);
            }
            RegionCommand::SendChunkCommand { pos, event } => {
                // 跨区块请求都跑到这里了，绝对是因为自己没有这个区块
                if self.is_chunk_in_region(pos) {
                    if let Some(chunk) = task_map.get(&pos) {
                        let _ = chunk.send(
                            qexed_task::message::unreturn_message::UnReturnMessage::build(event),
                        );
                    }
                    // 注:区块请求没要求回调,意味着调用者只是为了广播事件
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    region.send(UnReturnMessage::build(RegionCommand::SendChunkCommand {
                        pos,
                        event,
                    }))?;
                    return Ok(false);
                }
                // 最后请求
                self.master_api
                    .send(UnReturnMessage::build(WorldCommand::SendChunkCommand {
                        pos,
                        event,
                    }))?;
                return Ok(false);
            }
            RegionCommand::SendChunkNeedReturnCommand { pos, event, result } => {
                // 跨区块请求都跑到这里了，绝对是因为自己没有这个区块
                if self.is_chunk_in_region(pos) {
                    if let Some(chunk) = task_map.get(&pos) {
                        let _ = chunk.send(
                            qexed_task::message::unreturn_message::UnReturnMessage::build(event),
                        );
                    } else {
                        let _ = result.send(false);
                    }
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    region.send(UnReturnMessage::build(
                        RegionCommand::SendChunkNeedReturnCommand { pos, event, result },
                    ))?;
                    return Ok(false);
                }
                // 最后请求
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::SendChunkNeedReturnCommand { pos, event, result },
                ))?;
                return Ok(false);
            }
            RegionCommand::SendOtherWorldChunkCommand { pos, world, event } => {
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::SendChunkCommand { pos, event },
                        ),
                    )?;
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::SendOtherWorldChunkCommand { pos, world, event },
                ))?;
                return Ok(false);
            }
            RegionCommand::SendOtherWorldChunkNeedReturnCommand {
                pos,
                world,
                event,
                result,
            } => {
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::SendChunkNeedReturnCommand { pos, event, result },
                        ),
                    )?;
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::SendOtherWorldChunkNeedReturnCommand {
                        pos,
                        world,
                        event,
                        result,
                    },
                ))?;
                return Ok(false);
            }
            RegionCommand::CreateChunk { pos, result } => {
                if self.is_chunk_in_region(pos) {
                    if let Some(chunk) = task_map.get(&pos) {
                        result
                            .send(
                                crate::message::region::RegionCommandResult::CreateChunkResult {
                                    success: true,
                                    api: Some(chunk.clone()),
                                },
                            )
                            .map_err(|e| {
                                // 将 RegionCommandResult 转换为 anyhow::Error
                                anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                            })?;
                    } else {
                        // 创建区块(只读模式不允许这么做)
                        result
                            .send(
                                crate::message::region::RegionCommandResult::CreateChunkResult {
                                    success: false,
                                    api: None,
                                },
                            )
                            .map_err(|e| {
                                // 将 RegionCommandResult 转换为 anyhow::Error
                                anyhow::anyhow!("Failed to send RegionCommandResult: {:?}", e)
                            })?;
                    }
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    region.send(UnReturnMessage::build(RegionCommand::CreateChunk {
                        pos,
                        result,
                    }))?;
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                // 让世界自己创建区域后再创建区块
                self.master_api
                    .send(UnReturnMessage::build(WorldCommand::CreateChunk {
                        pos,
                        result,
                    }))?;
                return Ok(false);
            }
            RegionCommand::CreateOtherWorldChunk { pos, world, result } => {
                // 计算是否属于当前世界
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::CreateChunk { pos, result },
                        ),
                    );
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::CreateOtherWorldChunk { pos, world, result },
                ))?;
                return Ok(false);
            }
            RegionCommand::CreateRegion { pos, result } => {
                // 计算是否属于当前世界
                if pos == self.pos {
                    result.send(RegionCommandResult::CreateRegionResult {
                        success: true,
                        api: Some(api.clone()),
                    });
                    return Ok(false);
                }
                // 请求周围区域
                if let Some(region) = self.direction_region.get_at_target(self.pos, pos) {
                    result.send(RegionCommandResult::CreateRegionResult {
                        success: true,
                        api: Some(region.clone()),
                    });
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                // 让世界自己创建区域
                self.master_api
                    .send(UnReturnMessage::build(WorldCommand::CreateRegion {
                        pos,
                        result,
                    }))?;
                return Ok(false);
            }
            RegionCommand::CreateOtherWorldRegion { pos, world, result } => {
                // 计算是否属于当前世界
                if world == self.world_uuid {
                    // 降级为当前世界并重构建数据包给自己处理
                    api.send(
                        qexed_task::message::unreturn_message::UnReturnMessage::build(
                            RegionCommand::CreateRegion { pos, result },
                        ),
                    );
                    return Ok(false);
                }
                // 什么?还不行，那就只能请求到世界了
                // 让世界自己创建区域
                self.master_api.send(UnReturnMessage::build(
                    WorldCommand::CreateOtherWorldRegion { pos, world, result },
                ))?;
                return Ok(false);
            }
            RegionCommand::ChunkClose { pos } => {
                // 肯定是属于自己的区块，我还检查别人区块干什么
                task_map.remove(&pos);
                return Ok(false);
            }
            RegionCommand::RegionClose { pos } => {
                // 1. 检查是否是相邻位置
                if !DirectionMap::<()>::is_adjacent_to_center(self.pos, pos) {
                    return Ok(false); // 不是相邻位置
                }

                // 2. 计算方向
                if let Some(direction) = Direction::from_coords(self.pos, pos) {
                    // 3. 检查这个方向是否有区域
                    if self.direction_region.has_value(direction) {
                        // 4. 移除这个方向的区域
                        self.direction_region.remove(direction);
                    }
                }
                return Ok(false);
            }
            RegionCommand::RegionCloseCommand { result } => {
                // 发动区块强制关闭命令,等待关闭集合
                // 创建任务向量
                if task_map.is_empty() {
                    return Ok(true);
                }
                let mut tasks = Vec::with_capacity(task_map.len());

                // 遍历 DashMap
                for entry in task_map.iter() {
                    let pos = *entry.key(); // 复制坐标
                    let sender = entry.value().clone(); // 克隆发送器

                    // 创建异步任务
                    let task = async move {
                        let (tx, rx) = oneshot::channel();

                        // 发送关闭命令
                        match sender.send(UnReturnMessage::build(ChunkCommand::CloseCommand {
                            result: tx,
                        })) {
                            Ok(_) => {
                                // 等待区块数据
                                match rx.await {
                                    Ok(data) => Ok((pos, data)),
                                    Err(e) => Err(anyhow::anyhow!("接收区块数据失败: {}", e)),
                                }
                            }
                            Err(e) => Err(anyhow::anyhow!("发送命令到区块失败: {}", e)),
                        }
                    };

                    tasks.push(task);
                }

                // 并发执行，收集所有结果
                let _results = try_join_all(tasks).await?;
                // 按照顺序写入并保存
                // pass(暂未实现)
                result.send(());
                return Ok(true);
            }
        }
    }
    // 计算区块是否是相邻区块，如果是则判定是否有自己的相邻区块
}
