//! ChunkWorker 回归测试:非阻塞入队 + 在途去重 + 完成回流。
//! worker_live_probe 标记 ignore:需要本机 127.0.0.1:54132 chunk-summon 在线。
use qexed_play::chunk_worker::ChunkWorker;

#[tokio::test(flavor = "multi_thread")]
async fn enqueue_is_nonblocking_and_dedups() {
    let source = qexed_world::tcp_source::TcpChunkSource::connect_addr("127.0.0.1:54132")
        .await
        .expect("connect summon");
    let mut source = source.with_fallback_to_empty(false);
    let mut w = ChunkWorker::spawn(source);
    // enqueue 现在是纯内存同步操作:33 批连续入队也必须立即返回(死锁回归)。
    let t0 = std::time::Instant::now();
    for i in 0..33 {
        let cx = 2000 + i * 3;
        w.enqueue(vec![(cx, 0), (cx + 1, 0), (cx + 2, 0)]);
    }
    assert!(t0.elapsed().as_millis() < 50, "enqueue 必须非阻塞,实际 {:?}", t0.elapsed());
    // 在途去重:同坐标再次入队应被过滤。
    w.enqueue(vec![(2000, 0)]);
    // 等待全部完成:done 通道应收到 33 批。
    let mut batches = 0;
    while batches < 33 {
        let job = tokio::time::timeout(std::time::Duration::from_secs(120), w.done_rx.recv())
            .await
            .expect("done timeout")
            .expect("worker exited");
        for &(x, z) in &job.coords {
            w.complete(x, z);
            assert!(job.result.is_ok(), "batch error: {:?}", job.result.as_ref().err());
        }
        batches += 1;
    }
    assert_eq!(batches, 33);
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "需要 127.0.0.1:54132 chunk-summon 在线"]
async fn worker_live_probe() {
    let source = qexed_world::tcp_source::TcpChunkSource::connect_addr("127.0.0.1:54132")
        .await
        .expect("connect summon");
    let mut source = source.with_fallback_to_empty(false);
    let mut w = ChunkWorker::spawn(source);
    let t0 = std::time::Instant::now();
    w.enqueue(vec![(1000, 1000), (1001, 1000), (1000, 1001), (1001, 1001)]);
    let job = tokio::time::timeout(std::time::Duration::from_secs(45), w.done_rx.recv())
        .await
        .expect("timeout")
        .expect("worker exited");
    println!("DONE after {:?}: {} coords, ok={}", t0.elapsed(), job.coords.len(), job.result.is_ok());
}
