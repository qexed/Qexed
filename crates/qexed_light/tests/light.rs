//! qexed_light 集成测试：26.1.2 光照语义 + v4 移植用例。

use qexed_light::config::algorithm::LightAlgorithm;
use qexed_light::config::mode::LightMode;
use qexed_light::data::{BlockLightProps, ChunkLightData, ChunkOpacity, LightNeighborhood, OpacityNeighborhood};
use qexed_light::engine::LightEngine;
use qexed_light::geometry::DimensionGeometry;
use qexed_light::output::LightPayload;

fn air_geometry() -> DimensionGeometry {
    DimensionGeometry::overworld()
}

/// 全空气 chunk 邻域。
fn air_neighborhood(dim: &DimensionGeometry) -> OpacityNeighborhood {
    OpacityNeighborhood::single(ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR))
}

#[test]
fn overworld_sky_light_full_exposure() {
    let dim = air_geometry();
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&air_neighborhood(&dim));
    // 全空气：整列天光 15
    assert_eq!(light.sky_at(0, -64, 0), 15);
    assert_eq!(light.sky_at(8, 0, 8), 15);
    assert_eq!(light.sky_at(15, 319, 15), 15);
    // 方块光全 0
    assert_eq!(light.block_at(8, 0, 8), 0);
}

#[test]
fn solid_block_shades_below() {
    let dim = air_geometry();
    let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    opacity.set(8, 100, 8, BlockLightProps::SOLID).unwrap();
    let neighborhood = OpacityNeighborhood::single(opacity);
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&neighborhood);

    // 方块自身被完全遮挡（opacity=15）
    assert_eq!(light.sky_at(8, 100, 8), 0);
    // 正下方：周围直射 15 水平扩散进来（-1），之后向下无衰减 → 14
    assert_eq!(light.sky_at(8, 99, 8), 14);
    assert_eq!(light.sky_at(8, -64, 8), 14, "向下传播无衰减");
    // 周围一格仍是直射天光
    assert_eq!(light.sky_at(7, 99, 8), 15);
    assert_eq!(light.sky_at(8, 99, 7), 15);
    // 方块上方不受影响
    assert_eq!(light.sky_at(8, 101, 8), 15);
}

#[test]
fn water_dampens_progressively() {
    let dim = air_geometry();
    let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    let water = BlockLightProps { opacity: 1, luminance: 0 };
    // 水柱四周+底部全封闭（隔离侧向/底部空气的补光，纯测列衰减）
    for y in 90..100 {
        for (wx, wz) in [(7, 8), (9, 8), (8, 7), (8, 9)] {
            opacity.set(wx, y, wz, BlockLightProps::SOLID).unwrap();
        }
        opacity.set(8, y, 8, water).unwrap();
    }
    opacity.set(8, 89, 8, BlockLightProps::SOLID).unwrap();
    let neighborhood = OpacityNeighborhood::single(opacity);
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&neighborhood);

    // 10 格水：顶部入口 -1 → 14，每深一格再 -1 → 底部 5
    assert_eq!(light.sky_at(8, 99, 8), 14);
    assert_eq!(light.sky_at(8, 95, 8), 10);
    assert_eq!(light.sky_at(8, 90, 8), 5);
}

#[test]
fn block_light_from_torch_spreads() {
    let dim = air_geometry();
    let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    let torch = BlockLightProps { opacity: 0, luminance: 14 };
    opacity.set(8, 64, 8, torch).unwrap();
    let neighborhood = OpacityNeighborhood::single(opacity);
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_block_light(&neighborhood, None);

    // 光源自身 14，距离 1 → 13，距离 14 → 0
    assert_eq!(light.block_at(8, 64, 8), 14);
    assert_eq!(light.block_at(9, 64, 8), 13);
    assert_eq!(light.block_at(8, 65, 8), 13);
    // 长距离沿 -Y 方向：64-13=51 处 1，再远 0
    assert_eq!(light.block_at(8, 64 - 12, 8), 2);
    assert_eq!(light.block_at(8, 64 - 13, 8), 1);
    assert_eq!(light.block_at(8, 64 - 14, 8), 0);
}

#[test]
fn nether_has_no_sky_light() {
    let dim = DimensionGeometry::the_nether();
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&air_neighborhood(&dim));
    // 下界：天光全 0
    for y in [0, 64, 128, 255] {
        assert_eq!(light.sky_at(8, y, 8), 0, "y={y}");
    }
    // 方块光仍可计算（岩浆）：光源格 15，邻居格进入代价 max(1,15)=15？
    // 不——vanilla 中岩浆邻格 14：亮度源自身不衰减，向外传播第一格按
    // 邻居格 opacity 计代价。此处邻居是空气（opacity 0 → cost 1）→ 14。
    let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    opacity
        .set(4, 40, 4, BlockLightProps { opacity: 15, luminance: 15 })
        .unwrap();
    let neighborhood = OpacityNeighborhood::single(opacity);
    let light = engine.compute_block_light(&neighborhood, None);
    assert_eq!(light.block_at(4, 40, 4), 15);
    assert_eq!(light.block_at(4, 40, 5), 14, "向空气传播 -1");
}

#[test]
fn end_dimension_geometry() {
    let dim = DimensionGeometry::the_end();
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&air_neighborhood(&dim));
    // 末地：无遮挡天光整列 15
    assert_eq!(light.sky_at(8, 0, 8), 15);
    assert_eq!(light.sky_at(8, 255, 8), 15);
}

#[test]
fn neighbor_seeding_pulls_sky_light_across_chunk_border() {
    let dim = air_geometry();
    // 西邻 (15,y,8) 列全透明（独立预计算：整列直射 15）
    let mut west = ChunkOpacity::filled(&dim.geometry, BlockLightProps::SOLID);
    for y in dim.geometry.world_min_y..=dim.geometry.world_max_y {
        west.set(15, y, 8, BlockLightProps::AIR).unwrap();
    }
    // 中心 chunk：x=0 列在 y=51 以上放实心（挡 (0,50,8) 的直射，格本身保持空气）
    let mut center = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    for y in 51..=dim.geometry.world_max_y {
        center.set(0, y, 8, BlockLightProps::SOLID).unwrap();
    }
    let mut neighborhood = OpacityNeighborhood::single(center);
    neighborhood.set(-1, 0, west);

    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&neighborhood);
    // (0,50,8)：自身列被挡（列扫描 0），靠西邻直射 15 水平进入 → 15-1=14
    assert_eq!(light.sky_at(0, 50, 8), 14);
    // 邻居方向墙住的对照列（z=9 位置西邻是墙）没有播种源
    // 没有西邻数据的方向（如东侧 x=15）不受影响
    assert_eq!(light.sky_at(15, 50, 8), 15, "东边界无邻居 → 自身直射");
}

#[test]
fn ray_trace_brightens_cliff_bottom() {
    let dim = air_geometry();
    let mut opacity = ChunkOpacity::filled(&dim.geometry, BlockLightProps::AIR);
    // 悬崖：x=8 列从 y=100 到顶全实心，其余全空气。
    // (7,99,8) 在悬崖侧面脚下：自身列透明 → 直射 15（不是被挡的列！）
    // 真正被挡的是 (8,99,8)：头顶整列实心 → Fast 下只能靠侧面扩散 14。
    for y in 100..=dim.geometry.world_max_y {
        opacity.set(8, y, 8, BlockLightProps::SOLID).unwrap();
    }
    let neighborhood = OpacityNeighborhood::single(opacity);

    let fast = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let fast_light = fast.compute_sky_light(&neighborhood);
    // 悬崖遮挡列正下方：侧面 (7,99,8) 直射 15 水平扩散进来 → 14
    assert_eq!(fast_light.sky_at(8, 99, 8), 14);

    let rt = LightEngine::new(dim, LightAlgorithm::RayTrace.into());
    let rt_light = rt.compute_sky_light(&neighborhood);
    // RayTrace 不劣于 Fast
    assert!(rt_light.sky_at(8, 99, 8) >= fast_light.sky_at(8, 99, 8));
    // 侧面脚下直射位两种算法都满
    assert_eq!(fast_light.sky_at(7, 99, 8), 15);
    assert_eq!(rt_light.sky_at(7, 99, 8), 15);
}

#[test]
fn payload_masks_and_section_arrays() {
    let dim = air_geometry();
    let engine = LightEngine::new(dim, LightAlgorithm::Fast.into());
    let light = engine.compute_sky_light(&air_neighborhood(&dim));
    let payload = LightPayload::from_chunk_light(&light);

    let g = &dim.geometry;
    // 全空气：非空天光 section = 所有实 section（24）+ 顶部虚拟层（1）
    assert_eq!(
        payload.sky_light_mask[0].count_ones() as usize,
        g.world_section_count + 1,
        "24 实层 + 顶虚拟层"
    );
    assert_eq!(payload.sky_light_arrays.len(), g.world_section_count + 1);
    // 每个 2048 字节且全 ff（15）
    assert!(payload.sky_light_arrays.iter().all(|a| a.iter().all(|&b| b == 0xff)));
    // 方块光全 0 → 空
    assert!(payload.block_light_arrays.is_empty());
    assert_eq!(
        payload.empty_block_light_mask[0].count_ones() as usize,
        g.light_section_count
    );
}

#[test]
fn config_defaults_and_conversion() {
    use qexed_light::config::LightConfig;
    use qexed_light::SkyLightAlgorithm;

    let config = LightConfig::default();
    assert!(config.enable);
    assert_eq!(config.mode, LightMode::Static);
    assert_eq!(config.algorithm, LightAlgorithm::Fast);
    assert_eq!(config.fixed_level, 15);

    assert_eq!(SkyLightAlgorithm::from(LightAlgorithm::Fast), SkyLightAlgorithm::Fast);
    assert_eq!(SkyLightAlgorithm::from(LightAlgorithm::RayTrace), SkyLightAlgorithm::RayTrace);
}

#[test]
fn light_neighborhood_round_trip() {
    let dim = air_geometry();
    let mut light = ChunkLightData::empty(&dim.geometry);
    light.set_sky(3, 64, 3, 12);
    light.set_block(3, 64, 3, 9);
    let mut neighborhood = LightNeighborhood::single(light);
    // 从中心查
    assert_eq!(neighborhood.sky_at(3, 64, 3), Some(12));
    assert_eq!(neighborhood.block_at(3, 64, 3), Some(9));
    // 放东邻
    let mut east = ChunkLightData::empty(&dim.geometry);
    east.set_sky(0, 64, 3, 7);
    neighborhood.set(1, 0, east);
    // 东邻坐标（中心 x=16）在邻域坐标下查得到
    assert_eq!(neighborhood.sky_at(16, 64, 3), Some(7));
    // 超出 3x3：None
    assert_eq!(neighborhood.sky_at(32, 64, 3), None);
}
