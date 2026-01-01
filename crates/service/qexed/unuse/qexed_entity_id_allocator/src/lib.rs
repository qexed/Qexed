use anyhow::{Ok, Result};
use async_trait::async_trait;
use dashmap::DashSet;
use qexed_task::{
    event::task::TaskEasyEvent,
    message::{MessageSender, MessageType, return_message::ReturnMessage}
};
use std::collections::VecDeque;
use tokio::sync::mpsc::UnboundedSender;

#[derive(Debug, Clone)]
pub enum Message {
    // 分配一个实体ID
    AllocateEntityId(i32),
    // 回收一个实体ID
    DeallocateEntityId(i32),
    // 获取当前分配状态
    GetStatus {
        next_id: i32,
        total_allocated: usize,
        recycled_count: usize,
    },
    // 重置分配器
    ResetAllocator(i32), // 起始ID
}

#[derive(Debug)]
pub struct EntityIdAllocator {
    // 下一个可分配的新ID
    next_id: i32,
    // 可回收的ID队列
    recycled_ids: VecDeque<i32>,
    // 当前已分配的ID集合（用于快速检查）
    allocated_ids: DashSet<i32>,
    // 最大可分配的实体ID
    max_entity_id: i32,
}

impl EntityIdAllocator {
    pub fn new(start_id: i32, max_entity_id: i32) -> Self {
        Self {
            next_id: start_id,
            recycled_ids: VecDeque::new(),
            allocated_ids: DashSet::new(),
            max_entity_id,
        }
    }

    // 分配实体ID
    fn allocate(&mut self) -> Option<i32> {
        // 优先使用回收的ID
        if let Some(id) = self.recycled_ids.pop_front() {
            self.allocated_ids.insert(id);
            Some(id)
        } else if self.next_id <= self.max_entity_id {
            // 分配新ID
            let id = self.next_id;
            self.next_id += 1;
            self.allocated_ids.insert(id);
            Some(id)
        } else {
            None // 达到最大限制
        }
    }

    // 回收实体ID
    fn deallocate(&mut self, id: i32) -> bool {
        if self.allocated_ids.remove(&id).is_some() {
            self.recycled_ids.push_back(id);
            true
        } else {
            false
        }
    }

    // 重置分配器
    fn reset(&mut self, start_id: i32) {
        self.next_id = start_id;
        self.recycled_ids.clear();
        self.allocated_ids.clear();
    }

    // 获取状态
    fn get_status(&self) -> (i32, usize, usize) {
        (
            self.next_id,
            self.allocated_ids.len(),
            self.recycled_ids.len(),
        )
    }
}

#[async_trait]
impl TaskEasyEvent<ReturnMessage<Message>> for EntityIdAllocator {
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<Message>>,
        mut data: ReturnMessage<Message>,
    ) -> Result<bool> {
        match &mut data.data {
            Message::AllocateEntityId(entity_id) => {
                // 分配实体ID
                if let Some(id) = self.allocate() {
                    *entity_id = id;
                } else {
                    // 分配失败，设置为-1表示失败
                    *entity_id = -1;
                }
            }
            Message::DeallocateEntityId(entity_id) => {
                // 回收实体ID
                let success = self.deallocate(*entity_id);
                // 可以在这里记录日志或做其他处理
                log::debug!("Deallocate entity id {}: {}", entity_id, success);
            }
            Message::GetStatus {
                next_id,
                total_allocated,
                recycled_count,
            } => {
                let (n, t, r) = self.get_status();
                *next_id = n;
                *total_allocated = t;
                *recycled_count = r;
            }
            Message::ResetAllocator(start_id) => {
                // 重置分配器
                self.reset(*start_id);
            }
        }

        // 发送返回消息
        if let Some(send) = data.get_return_send().await? {
            let _ = send.send(data.data);
        }
        Ok(false)
    }
}

// 创建分配器任务
pub async fn run(
    config:qexed_config::app::qexed_entity_id_allocator::EntityIdAllocator,
) -> Result<UnboundedSender<ReturnMessage<Message>>> {
    let allocator = EntityIdAllocator::new(config.start_id, config.max_entity_id);
    let (task, task_send) = qexed_task::task::task::TaskEasy::new(allocator);
    
    task.run().await?;
    log::info!("[服务] 实体ID分配器 已启用");
    Ok(task_send)
}

