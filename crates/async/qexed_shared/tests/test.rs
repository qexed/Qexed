use qexed_shared::Shared;
use tokio::time::{sleep, Duration};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Barrier;

#[derive(Clone, Debug)]
struct A {
    b: i32,
}

#[tokio::test]
async fn test_concurrent_behavior() -> Result<()> {
    println!("=== 测试并发修改的时间线 ===");
    
    let mut c = Shared::<A>::new(A { b: 0 }).await?;
    c.b = 2;
    let mut d = c.clone_task().await?;
    let mut observer = c.clone_task().await?;
    // 使用屏障让两个任务同时开始
    let barrier = Arc::new(Barrier::new(2));
    
    let barrier_c = barrier.clone();
    let handle_c = tokio::spawn(async move {
        println!("[C] 任务开始");
        barrier_c.wait().await;
        
        println!("[C] 本地修改: 0 -> 2");
        c.b = 2;
        
        println!("[C] 提交修改...");
        c.commit_data().await.expect("C提交失败");
        println!("[C] 提交完成");
        
        sleep(Duration::from_millis(50)).await;
        
        println!("[C] 第一次检查...");
        c.check_data().await.expect("C检查失败");
        println!("[C] 第一次检查结果: b = {}", c.b);
        
        sleep(Duration::from_millis(100)).await;
        
        println!("[C] 第二次检查...");
        c.check_data().await.expect("C检查失败");
        println!("[C] 第二次检查结果: b = {}", c.b);
    });
    
    let barrier_d = barrier.clone();
    let handle_d = tokio::spawn(async move {
        println!("[D] 任务开始");
        barrier_d.wait().await;
        
        println!("[D] 本地修改: 0 -> 45");
        d.b = 45;
        
        // 让C先提交
        sleep(Duration::from_millis(10)).await;
        
        println!("[D] 提交修改...");
        d.commit_data().await.expect("D提交失败");
        println!("[D] 提交完成");
        
        sleep(Duration::from_millis(100)).await;
        
        println!("[D] 第一次检查...");
        d.check_data().await.expect("D检查失败");
        println!("[D] 第一次检查结果: b = {}", d.b);
        
        sleep(Duration::from_millis(50)).await;
        
        println!("[D] 第二次检查...");
        d.check_data().await.expect("D检查失败");
        println!("[D] 第二次检查结果: b = {}", d.b);
    });
    
    handle_c.await?;
    handle_d.await?;
    
    // 创建新的观察者
   
    sleep(Duration::from_millis(50)).await;
    observer.check_data().await?;
    println!("\n[观察者] 最终值: b = {}", observer.b);
    
    Ok(())
}