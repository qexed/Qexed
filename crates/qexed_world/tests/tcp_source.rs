//! TCP 地形来源 e2e：需要 chunk-summon 服务在 127.0.0.1:54132 运行，
//! 否则自动跳过（与 qexed_chunk_by_tcp 的 e2e 约定一致）。
//!
//! 验证用户需求的三条链路：
//! 1. qexed 服务端（TCP 客户端）请求 → 服务端产出（存档读盘或生成）→ 解码 MapChunk；
//! 2. 同一 JVM 内服务端确定：同坐标两次请求解码结果一致（服务端无内容缓存，每次现编码）；
//! 3. TcpChunkProvider 适配 SpawnChunkStreamer：出生圈 9 区块全部经 TCP 真实产出。

use qexed_world::stream::SpawnChunkStreamer;
use qexed_world::tcp_source::TcpChunkSource;

/// 锚定 workspace 的 mojang 缓存（cargo test 的 CWD 是 crate 目录，
/// tcp_light 光照重算要读 blocks.json 光学表）。
fn setup_mojang_cache() {
    let manifest_dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mojang_cache = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|root| root.join("run").join("cache").join("mojang"))
        .expect("workspace root");
    let _ = qexed_mojang_data::set_cache_dir(&mojang_cache);
}

async fn try_connect() -> Option<TcpChunkSource> {
    setup_mojang_cache();
    match TcpChunkSource::connect_local().await {
        Ok(source) => Some(source),
        Err(e) => {
            eprintln!("skip: chunk-summon 服务未运行 (127.0.0.1:54132): {e}");
            None
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_tcp_source_requests_and_decodes_chunk() {
    let Some(mut source) = try_connect().await else {
        return;
    };
    let chunk = source
        .request_map_chunk(3, -4)
        .await
        .expect("请求并解码区块");
    assert_eq!((chunk.chunk_x, chunk.chunk_z), (3, -4), "坐标一致");
    assert!(!chunk.data.data.is_empty(), "section 数据非空");
    assert!(
        !chunk.data.heightmaps.is_empty(),
        "heightmap 至少携带一种类型"
    );
    assert!(
        chunk
            .data
            .heightmaps
            .iter()
            .all(|hm| hm.data.len() == 37),
        "每张 heightmap 37 个 long（9×18=162 bit）"
    );
    // 光照覆写：Java 发来空光照（掩码全零），qexed_light 重算后必须非空，
    // 且数组数 = 掩码填充数（客户端按掩码索引取数组的硬约束）。
    assert!(
        chunk.light.sky_light_mask.0.iter().any(|&w| w != 0),
        "TCP 回包应被 qexed_light 重算出非空天光掩码"
    );
    let sky_bits: usize = chunk
        .light
        .sky_light_mask
        .0
        .iter()
        .map(|w| w.count_ones() as usize)
        .sum();
    assert_eq!(
        chunk.light.sky_light_arrays.len(),
        sky_bits,
        "天光数组数必须等于天光掩码填充数"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_tcp_source_serves_far_coords_exploration() {
    let Some(mut source) = try_connect().await else {
        return;
    };
    // 玩家跑图：任意坐标即取即得。远坐标（region 文件大概率不存在）
    // 必须仍成功返回——存档未命中时服务端原版生成兜底。
    let chunk = source
        .request_map_chunk(1024, -1024)
        .await
        .expect("远坐标请求（存档未命中应生成兜底）");
    assert_eq!((chunk.chunk_x, chunk.chunk_z), (1024, -1024));
    assert!(!chunk.data.data.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_tcp_source_is_deterministic_within_server_lifetime() {
    let Some(mut source) = try_connect().await else {
        return;
    };
    // 服务端无内容缓存：两次独立现编码；同一 JVM 内（heightmap HashMap 序固定）应一致。
    let a = source.request_map_chunk(0, 0).await.expect("第一次请求");
    let b = source.request_map_chunk(0, 0).await.expect("第二次请求");
    assert_eq!(a, b, "同一服务进程内同坐标两次请求应产出一致区块");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_tcp_provider_feeds_spawn_streamer() {
    let Some(mut source) = try_connect().await else {
        return;
    };
    let mut provider = qexed_world::tcp_source::TcpChunkProvider::new(&mut source);
    let mut streamer = SpawnChunkStreamer::new((0, 0), 1);
    let mut served = 0usize;
    while !streamer.is_complete() {
        let batch = streamer
            .next_batch_with(&mut provider)
            .expect("出生批获取（TCP 来源）");
        for chunk in batch {
            assert!((-1..=1).contains(&chunk.chunk_x) && (-1..=1).contains(&chunk.chunk_z));
            assert!(
                !chunk.data.data.is_empty(),
                "({},{}) section 非空",
                chunk.chunk_x,
                chunk.chunk_z
            );
            served += 1;
        }
    }
    assert_eq!(served, 9, "视野半径 1 = 3×3 区块全部经 TCP 产出");
}

#[tokio::test(flavor = "multi_thread")]
async fn e2e_tcp_batch_incremental_ring() {
    let Some(mut source) = try_connect().await else {
        return;
    };
    // 模拟跑图增量：玩家从 (0,0) 走到 (1,0)，半径 1 的视野圈 gained = x=2 一列。
    // 批量请求 → 流水线回包按请求序对齐 → 逐个两阶段光照。
    use qexed_world::stream::{ViewCircle, CHUNKS_PER_BATCH};
    let old = ViewCircle::new((0, 0), 1);
    let moved = ViewCircle::new((1, 0), 1);
    let gained = ViewCircle::gained(&old, &moved);
    assert_eq!(
        gained,
        vec![(2, 0), (2, -1), (2, 1)],
        "向东一格的增量加载集（由近到远：直格先于角格）"
    );
    let chunks = source
        .request_map_chunks(&gained)
        .await
        .expect("批量增量请求");
    assert_eq!(chunks.len(), gained.len(), "回包数 = 请求数");
    for (chunk, &(x, z)) in chunks.iter().zip(&gained) {
        assert_eq!((chunk.chunk_x, chunk.chunk_z), (x, z), "回包按请求序对齐");
        assert!(!chunk.data.data.is_empty(), "({x},{z}) section 非空");
        assert!(
            chunk.light.sky_light_mask.0.iter().any(|&w| w != 0),
            "({x},{z}) 增量包同样要做 qexed_light 光照重算"
        );
    }
    // 半径 1 一圈最多 9 个：一批装下，CHUNKS_PER_BATCH 是 play 侧分片依据。
    assert!(gained.len() <= CHUNKS_PER_BATCH);
}

/// 金标回归：Java 服务端真实回包字节（chunk-summon 捕获的 chunk_0_0.bin）
/// → 解码 → 光照重算。不依赖 chunk-summon 在线，直接回放捕获字节。
#[test]
fn golden_fixture_light_overwrite() {
    setup_mojang_cache();
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../qexed_chunk_by_tcp/tests/fixtures/chunk_0_0.bin");
    let bytes = std::fs::read(&path).expect("金标 fixture 可读");
    let mut buf = bytes::BytesMut::from(&bytes[..]);
    let mut reader = qexed_packet::PacketReader::new(&mut buf);
    let mut chunk = qexed_protocol::to_client::play::map_chunk::MapChunk::default();
    qexed_packet::Packet::deserialize(&mut chunk, &mut reader).expect("解码金标字节");
    assert_eq!((chunk.chunk_x, chunk.chunk_z), (0, 0));
    // Java 原始形态：光照掩码全零（LevelLightEngine.EMPTY）。
    assert!(
        chunk.light.sky_light_mask.0.iter().all(|&w| w == 0),
        "Java 原始回包天光掩码应为空（这正是要修的 bug）"
    );

    qexed_world::tcp_light::overwrite_chunk_light(&mut chunk);

    let sky_bits: usize = chunk
        .light
        .sky_light_mask
        .0
        .iter()
        .map(|w| w.count_ones() as usize)
        .sum();
    assert!(sky_bits > 0, "重算后天光掩码必须非空");
    assert_eq!(
        chunk.light.sky_light_arrays.len(),
        sky_bits,
        "天光数组数 = 掩码填充数"
    );
    let block_bits: usize = chunk
        .light
        .block_light_mask
        .0
        .iter()
        .map(|w| w.count_ones() as usize)
        .sum();
    assert_eq!(
        chunk.light.block_light_arrays.len(),
        block_bits,
        "方块光数组数 = 掩码填充数"
    );
}
