use std::{
    fmt::Debug,
    ops::{Deref, DerefMut},
};

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_task::{
    event::{task::TaskEvent, task_manage::TaskManageEvent}, 
    message::{MessageSender, MessageType, return_message::ReturnMessage, unreturn_message::UnReturnMessage}, 
    task::{task::Task, task_manage::TaskManage}
};
use tokio::sync::{
    mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel}, 
    oneshot
};
use anyhow::anyhow;

/// 内部消息结构体
#[derive(Debug)]
struct TaskSharedTaskMessage<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    // 0:注册任务(clone_task)
    // 1:同步数据(请求者)
    // 2:同步数据(接收者)
    // 3:注销任务(drop)
    // 4:注销管理器(close_manage)
    mode: u8,
    data: Option<T>,
    api: Option<UnboundedSender<ReturnMessage<TaskSharedTaskMessage<T>>>>,
    id: Option<u64>,
    task_r: Option<UnboundedSender<T>>,
}

#[derive(Debug)]
struct TaskSharedManageMessage<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    /*
    0:创建子任务
    1:同步数据(发起者调用)
    2:注销子任务
    3:关闭管理器
    */
    mode: u8,
    api: Option<oneshot::Sender<TaskSharedTaskMessage<T>>>,
    data: Option<T>,
    id: Option<u64>,
    task_r: Option<UnboundedSender<T>>
}

/// TaskShared是更高级的Task与TaskManage的内部封装
#[derive(Debug)]
pub struct Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    id: u64,
    data: T,  // 直接存储数据，无锁
    api: Option<UnboundedSender<ReturnMessage<TaskSharedTaskMessage<T>>>>,
    manage_api: Option<MessageSender<UnReturnMessage<TaskSharedManageMessage<T>>>>,
    task_s: Option<UnboundedSender<T>>,
    task_r: Option<UnboundedReceiver<T>>,
    pending_updates: bool,  // 标记是否有待处理的更新
}

impl<T> Clone for Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            data: self.data.clone(),
            api: self.api.clone(),
            manage_api: self.manage_api.clone(),
            task_s: self.task_s.clone(),
            task_r: None,  // 接收端不能克隆
            pending_updates: false,
        }
    }
}

// 实现 Deref
impl<T> Deref for Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

// 实现 DerefMut
impl<T> DerefMut for Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

impl<T> Drop for Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    fn drop(&mut self) {
        // 如果还在运行时，尝试注销任务
        if self.api.is_some() {
            // 由于drop不能是async，我们只尝试发送，但不等待
            if let Some(api) = self.api.take() {
                // 尝试发送注销消息，但忽略错误（可能在drop时通道已关闭）
                let _ = api.send(ReturnMessage::build(TaskSharedTaskMessage {
                    mode: 3,  // 注销任务
                    data: None,
                    api: None,
                    id: Some(self.id),
                    task_r: None,
                }));
            }
        }
    }
}

impl<T> Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    pub async fn new(data: T) -> anyhow::Result<Self> {
        let manager_actor = TaskSharedManage::new(data.clone());
        let (manager_task, manager_sender) = TaskManage::new(manager_actor);
        manager_task.run().await?;
        
        let (ts, tr) = unbounded_channel();

        // 创建第一个任务
        let mut task: ReturnMessage<TaskSharedTaskMessage<_>> =
            ReturnMessage::build(TaskSharedTaskMessage {
                mode: 0,  // 创建任务
                data: None,
                api: None,
                id: None,
                task_r: None,
            });
        let task_receiver = task.manual();
        
        UnReturnMessage::build(TaskSharedManageMessage {
            mode: 0,
            api: task.get_return_send().await?.take(),
            data: None,
            id: None,
            task_r: Some(ts),
        })
        .post(&manager_sender)
        .await?;
        
        let subtask = ReturnMessage::get_return_data(task_receiver).await?;
        let subtask_api = match subtask.api {
            Some(v) => v,
            None => return Err(anyhow!("创建任务失败")),
        };
        
        // 从管理器获取数据
        let task_data = match subtask.data {
            Some(data) => data,
            None => return Err(anyhow!("创建任务失败，未获取到数据")),
        };
        
        let id = match subtask.id {
            Some(v) => v,
            None => return Err(anyhow!("创建任务失败，未获取到ID")),
        };

        Ok(Self {
            id,
            data: task_data,  // 直接存储数据
            api: Some(subtask_api),
            manage_api: Some(manager_sender),
            task_s: None,
            task_r: Some(tr),
            pending_updates: false,
        })
    }
    
    pub async fn clone_task(&self) -> anyhow::Result<Self> {
        let api_ref = self.api.as_ref()
            .ok_or_else(|| anyhow!("任务API不存在"))?;
        let (ts, tr) = unbounded_channel();
        
        let mut data = ReturnMessage::build(TaskSharedTaskMessage::<T> {
            mode: 0,  // 创建任务
            data: None,
            api: None,
            id: None,
            task_r: Some(ts),
        })
        .get(api_ref)
        .await?;
        
        let api = match data.api.take() {
            Some(api) => api,
            None => return Err(anyhow!("复制任务体失败")),
        };
        
        // 从管理器获取数据
        let task_data = match data.data.take() {
            Some(data) => data,
            None => return Err(anyhow!("复制数据失败")),
        };
        
        let id = match data.id.take() {
            Some(id) => id,
            None => return Err(anyhow!("复制ID失败")),
        };
        
        Ok(Self {
            id,
            data: task_data,  // 直接存储数据
            api: Some(api),
            manage_api: self.manage_api.clone(),
            task_s: None,
            task_r: Some(tr),
            pending_updates: false,
        })
    }
    
    // 获取数据的不可变引用
    pub fn get(&self) -> &T {
        &self.data
    }

    // 获取数据的可变引用
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }

    // // 获取所有权
    // pub fn into_inner(self) -> T {
    //     self.data
    // }

    // 设置数据
    pub fn set(&mut self, data: T) {
        self.data = data;
    }

    // 提交同步
    pub async fn commit_data(&self) -> anyhow::Result<()> {
        if let Some(api) = &self.api {
            api.send(ReturnMessage::build(TaskSharedTaskMessage {
                mode: 1,  // 同步数据
                data: Some(self.data.clone()),
                api: None,
                id: Some(self.id),
                task_r: None,
            }))?;
        }
        Ok(())
    }
    
    // 检查并更新数据
    pub async fn check_data(&mut self) -> anyhow::Result<bool> {
        if let Some(r) = &mut self.task_r {
            let mut updated = false;
            while let Ok(v) = r.try_recv() {
                self.data = v;
                updated = true;
            }
            Ok(updated)
        } else {
            Ok(false)
        }
    }
    
    // 阻塞检查并更新数据
    pub async fn wait_data(&mut self) -> anyhow::Result<()> {
        if let Some(r) = &mut self.task_r {
            if let Some(v) = r.recv().await {
                self.data = v;
            }
        }
        Ok(())
    }
    
    // 获取ID
    pub fn id(&self) -> u64 {
        self.id
    }
    
    // 直接修改数据并提交
    pub async fn modify<F>(&mut self, f: F) -> anyhow::Result<()>
    where
        F: FnOnce(&mut T),
    {
        f(&mut self.data);
        self.commit_data().await
    }
    
    // 手动注销任务
    pub async fn unregister(&mut self) -> anyhow::Result<()> {
        if let Some(api) = self.api.take() {
            // 发送注销消息
            let _ = api.send(ReturnMessage::build(TaskSharedTaskMessage {
                mode: 3,  // 注销任务
                data: None,
                api: None,
                id: Some(self.id),
                task_r: None,
            }));
        }
        
        // 清理接收通道
        self.task_r = None;
        self.task_s = None;
        
        Ok(())
    }
    
    // 关闭管理器
    pub async fn close_manager(&self) -> anyhow::Result<()> {
        if let Some(api) = &self.manage_api {
            UnReturnMessage::build(TaskSharedManageMessage {
                mode: 3,  // 关闭管理器
                api: None,
                data: None,
                id: None,
                task_r: None,
            })
            .post(api)
            .await?;
        }
        Ok(())
    }
    
    // 标记有更新
    pub fn mark_updated(&mut self) {
        self.pending_updates = true;
    }
    
    // 检查是否有待处理的更新
    pub fn has_pending_updates(&self) -> bool {
        self.pending_updates
    }
    
    // 清除更新标记
    pub fn clear_pending_updates(&mut self) {
        self.pending_updates = false;
    }
}

// 任务管理器
#[derive(Debug)]
struct TaskSharedManage<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    data: T,
    next_id: u64,
    recycled: Vec<u64>,
    task_senders: DashMap<u64, UnboundedSender<T>>,  // 存储每个任务的发送端
}

impl<T> TaskSharedManage<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    pub fn new(data: T) -> Self {
        Self {
            data,
            next_id: 0,
            recycled: Vec::new(),
            task_senders: DashMap::new(),
        }
    }

    pub fn acquire_id(&mut self) -> u64 {
        self.recycled.pop().unwrap_or_else(|| {
            let id = self.next_id;
            self.next_id += 1;
            id
        })
    }

    pub fn release_id(&mut self, id: u64) {
        if !self.recycled.contains(&id) {
            self.recycled.push(id);
        }
        // 清理任务的发送端
        self.task_senders.remove(&id);
    }
}

// 为TaskEvent trait实现
#[async_trait]
impl<T> TaskEvent<ReturnMessage<TaskSharedTaskMessage<T>>, UnReturnMessage<TaskSharedManageMessage<T>>> for Shared<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    async fn event(
        &mut self,
        _api: &MessageSender<ReturnMessage<TaskSharedTaskMessage<T>>>,
        manage_api: &MessageSender<UnReturnMessage<TaskSharedManageMessage<T>>>,
        mut data: ReturnMessage<TaskSharedTaskMessage<T>>,
    ) -> anyhow::Result<bool> {
        match data.data.mode {
            // 注册新任务
            0 => {
                UnReturnMessage::build(TaskSharedManageMessage {
                    mode: 0,
                    api: data.get_return_send().await?,
                    data: None,
                    id: None,
                    task_r: data.data.task_r.take(),
                })
                .post(manage_api)
                .await?;
            }
            // 同步数据(请求者)
            1 => {
                UnReturnMessage::build(TaskSharedManageMessage {
                    mode: 1,
                    api: data.get_return_send().await?,
                    data: data.data.data.take(),  // 传递请求者的数据
                    id: Some(self.id),
                    task_r: None,
                })
                .post(manage_api)
                .await?;
            }
            // 同步数据(接受者) - 直接更新本地数据
            2 => {
                if let Some(new_data) = data.data.data {
                    self.data = new_data.clone();
                    self.mark_updated();
                    
                    // 如果设置了task_s，则广播更新
                    if let Some(send) = &self.task_s {
                        let _ = send.send(new_data);
                    }
                }
            }
            // 注销任务
            3 => {
                UnReturnMessage::build(TaskSharedManageMessage {
                    mode: 2,  // 注销子任务
                    api: data.get_return_send().await?,
                    data: None,
                    id: Some(self.id),
                    task_r: None,
                })
                .post(manage_api)
                .await?;
                return Ok(true);  // 任务结束
            }
            // 关闭管理器
            4 => {
                return Ok(true);  // 任务结束
            }
            _ => {
                return Err(anyhow!("未知的操作模式: {}", data.data.mode));
            }
        }
        Ok(false)
    }
}

// 为TaskManageEvent trait实现
#[async_trait]
impl<T> TaskManageEvent<
    u64,
    UnReturnMessage<TaskSharedManageMessage<T>>,
    ReturnMessage<TaskSharedTaskMessage<T>>,
> for TaskSharedManage<T>
where
    T: Send + 'static + Sync + Debug + Unpin + Clone,
{
    async fn event(
        &mut self,
        _api: &MessageSender<UnReturnMessage<TaskSharedManageMessage<T>>>,
        task_map: &DashMap<u64, MessageSender<ReturnMessage<TaskSharedTaskMessage<T>>>>,
        mut data: UnReturnMessage<TaskSharedManageMessage<T>>,
    ) -> anyhow::Result<bool> {
        match data.data.mode {
            // 创建子任务
            0 => {
                let id = self.acquire_id();
                // 使用管理器的数据创建任务
                let raw_task = Shared { 
                    id, 
                    data: self.data.clone(),  // 使用管理器的数据
                    api: None,
                    manage_api: None,  // 不传递给任务
                    task_s: data.data.task_r.take(),
                    task_r: None,
                    pending_updates: false,
                };
                
                let (task, task_sand) = Task::new(_api.clone(), raw_task);
                task.run().await?;
                task_map.insert(id, task_sand.clone());
                
                // 保存任务的发送端用于广播
                if let Some(task_s) = data.data.task_r {
                    self.task_senders.insert(id, task_s);
                }
                
                if let Some(task_return_api) = data.data.api {
                    let _ = task_return_api.send(TaskSharedTaskMessage {
                        mode: 0,
                        data: Some(self.data.clone()),  // 发送管理器的数据给任务
                        api: Some(task_sand),
                        id: Some(id),
                        task_r: None,
                    });
                }
            },
            // 同步数据(发起) - 更新管理器数据并广播给其他任务
            1 => {
                let id = match data.data.id {
                    Some(v) => v,
                    None => { 
                        return Err(anyhow!("同步数据时缺少任务ID"));
                    }
                };
                
                // 更新管理器的数据
                if let Some(new_data) = data.data.data {
                    self.data = new_data.clone();
                    
                    // 向所有任务广播更新
                    for entry in task_map.iter() {
                        let task_id = *entry.key();
                        // if task_id != id {  // 不给自己发
                            let _ = entry.value().send(ReturnMessage::build(TaskSharedTaskMessage {
                                mode: 2,  // 同步(接受者)
                                data: Some(new_data.clone()),  // 发送更新后的数据
                                api: None,
                                id: None,
                                task_r: None,
                            }));
                        // }
                    }
                    
                    // 向所有任务的task_s发送更新
                    for entry in self.task_senders.iter() {
                        let _ = entry.value().send(new_data.clone());
                    }
                }
                
                // 确认同步完成
                if let Some(task_return_api) = data.data.api {
                    let _ = task_return_api.send(TaskSharedTaskMessage {
                        mode: 2,  // 同步(接受者) - 给请求者确认
                        data: Some(self.data.clone()),
                        api: None,
                        id: None,
                        task_r: None,
                    });
                }
            },
            // 注销子任务
            2 => {
                if let Some(id) = data.data.id {
                    task_map.remove(&id);
                    self.release_id(id);
                }
            },
            // 关闭管理器
            3 => {
                // 向所有任务发送关闭消息
                for entry in task_map.iter() {
                    let _ = entry.value().send(ReturnMessage::build(TaskSharedTaskMessage {
                        mode: 4,  // 关闭任务
                        data: None,
                        api: None,
                        id: None,
                        task_r: None,
                    }));
                }
                
                // 清理所有资源
                task_map.clear();
                self.task_senders.clear();
                return Ok(true);  // 管理器结束
            },
            _ => {
                return Err(anyhow!("未知的管理器操作模式: {}", data.data.mode));
            }
        }
        Ok(false)
    }
}