//! 端到端集成测试：需要 Java chunk-summon 服务已在本机 54132 端口运行
//! （tools/mc-vanilla-chunk-summon：gradle runServer）。未运行时测试自动跳过。

use qexed_chunk_by_tcp::{ChunkTcpClient, Dimension};

/// 服务不可达时跳过。Java 首次引导（下载/解析 MC 数据）约 30-60 秒。
async fn try_connect() -> Option<ChunkTcpClient> {
    match ChunkTcpClient::connect_local().await {
        Ok(c) => Some(c),
        Err(e) => {
            eprintln!("skip: chunk-summon service not reachable on 54132: {e}");
            None
        }
    }
}

/// (0,0) 线上契约：服务端可带存档（--world，命中读盘）或纯生成模式运行，
/// 两种模式下该测试都必须通过——因此默认只校验结构性与进程内确定性；
/// 对固定 seed 生成字节的逐字节 golden 比对需要纯生成模式，用
/// `CHUNK_SUMMON_STRICT_GOLDEN=1` 显式开启（服务端不带 --world 启动时用）。
#[tokio::test]
async fn e2e_chunk_0_0_wire_contract() {
    let Some(mut client) = try_connect().await else {
        return;
    };
    let chunk = client
        .request_chunk(0, 0, Dimension::Overworld)
        .await
        .expect("chunk request failed");
    assert_eq!(chunk.chunk_x, 0);
    assert_eq!(chunk.chunk_z, 0);
    // 完整 LevelChunk 包体：8 字节坐标 + 3 张 heightmap（各 298 字节）+
    // section 数据，远大于一个空壳
    assert!(chunk.packet.len() > 10_000, "包体过小: {}", chunk.packet.len());

    // 服务端无内容缓存：同一 JVM 内同坐标两次请求各自现编码，字节必须一致
    //（heightmap 三槽排列差异只出现在跨 JVM 重启之间）
    let again = client
        .request_chunk(0, 0, Dimension::Overworld)
        .await
        .expect("second chunk request failed");
    assert_eq!(chunk.packet, again.packet, "同进程两次请求应逐字节一致");

    // 严格 golden：仅纯生成模式（不带 --world）。
    // 唯一的已知非确定性：前 3 个 heightmap 块（每块 298 字节，块首类型字节位于
    // 偏移 9/307/605）的存放顺序随 JVM 启动的 HashMap 迭代序变化，块内容一致；
    // 因此这三个槽位按多重集比较，其余字节逐字节必须完全一致。
    if std::env::var("CHUNK_SUMMON_STRICT_GOLDEN").is_err() {
        return;
    }
    let golden = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/chunk_0_0.bin"))
        .expect("golden fixture missing");
    assert_eq!(chunk.len(), golden.len(), "chunk packet body size mismatch");
    const HM_TYPE_POS: [usize; 3] = [9, 307, 605];
    let mut got_types: Vec<u8> = HM_TYPE_POS.iter().map(|&i| chunk.packet[i]).collect();
    got_types.sort_unstable();
    let mut want_types: Vec<u8> = HM_TYPE_POS.iter().map(|&i| golden[i]).collect();
    want_types.sort_unstable();
    assert_eq!(got_types, want_types, "heightmap type slot set mismatch");
    for (i, b) in chunk.packet.iter().enumerate() {
        if HM_TYPE_POS.contains(&i) {
            continue;
        }
        assert_eq!(*b, golden[i], "byte {i} differs from golden fixture");
    }
}

#[tokio::test]
async fn e2e_keep_alive_echo() {
    let Some(mut client) = try_connect().await else {
        return;
    };
    let echoed = client.ping(0x1234_5678_9abc_def0).await.expect("ping failed");
    assert_eq!(echoed, 0x1234_5678_9abc_def0);
}

#[tokio::test]
async fn e2e_requested_coords_are_echoed() {
    let Some(mut client) = try_connect().await else {
        return;
    };
    let chunk = client
        .request_chunk(3, -4, Dimension::Overworld)
        .await
        .expect("chunk request failed");
    assert_eq!((chunk.chunk_x, chunk.chunk_z), (3, -4));
}
