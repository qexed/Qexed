# qexed_anvil

[![Crates.io](https://img.shields.io/crates/v/qexed_anvil.svg)](https://crates.io/crates/qexed_anvil)
[![Documentation](https://docs.rs/qexed_anvil/badge.svg)](https://docs.rs/qexed_anvil)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](./LICENSE)

Minecraft Java 版 Anvil 世界存档读写工具：`.mca` 区域文件容器 + 区块 NBT 的 section/调色板操作。基于 [`qexed_nbt`](https://crates.io/crates/qexed_nbt)，面向通用场景（存档编辑器、地图工具、统计分析），不依赖任何服务端实现。

## 设计要点

- **方块状态用名称引用**（`BlockStateRef` = 名称 + 属性），不依赖方块注册表——任何版本、任何模组的存档都能正确读写
- **区域文件读写经过实测**：扇区分配、原位复用、空洞回收与 vanilla `.mca` 布局一致
- **调色板编解码独立成模块**：位打包/解包可单独用于网络协议或其他容器
- 纯同步、无 `unsafe`、无全局状态

## 安装

```toml
[dependencies]
qexed_anvil = "0.1"
```

## 快速上手

### 读取方块

```rust,no_run
use qexed_anvil::{chunk, region::AnvilRegion};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let region = AnvilRegion::from_file("world/region/r.0.0.mca")?;
if let Some(data) = region.read_chunk(0, 4)? {          // 区块 (0, 4)
    let raw = data.decompress()?;                        // zlib -> NBT 字节
    let (_, root) = qexed_nbt::from_slice(&raw)?;        // 解析 NBT
    let state = chunk::block_state_at(&root, 5, 70, 9)?; // 世界坐标 (5, 70, 9)
    if let Some(state) = state {
        println!("{} {:?}", state.name, state.properties);
    }
}
# Ok(()) }
```

### 修改并写回

```rust,no_run
use qexed_anvil::{chunk::{self, BlockStateRef}, region::{AnvilRegion, ChunkData}};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let path = "world/region/r.0.0.mca";
let mut region = AnvilRegion::from_file(path)?;
let existing = region.read_chunk(0, 0)?;

let root = match &existing {
    Some(data) => {
        let (_, root) = qexed_nbt::from_slice(&data.decompress()?)?;
        root
    }
    None => /* 新区块：构造最小根或用空 section 填充 */ return Ok(()),
};

let updated = chunk::set_block_state(
    &root, 8, -60, 8,
    &BlockStateRef::new("minecraft:diamond_ore")
        .with_properties(vec![("lit".into(), "false".into())]),
    &BlockStateRef::new("minecraft:stone"),  // 新建 section 的填充方块
    "minecraft:plains",                       // 新建 section 的默认生物群系
)?;

let bytes = qexed_nbt::to_vec("", &updated)?;
region.write_chunk(0, 0, ChunkData::zlib(&bytes)?)?;
region.save()?;
# Ok(()) }
```

### 遍历区块 / 定位区域文件

```rust
use qexed_anvil::paths;

# fn main() {
// 维度 -> region 目录（含 DIM-1/DIM1/dimensions 布局）
let dir = paths::dimension_region_dir("world".as_ref(), "minecraft:the_nether");

// 区块坐标 -> 区域文件（floor 语义，负坐标正确）
assert_eq!(paths::region_file_name(-1, -33), "r.-1.-2.mca");

// 列出主世界全部区域文件
let files = paths::list_region_files("world".as_ref(), "minecraft:overworld").unwrap();
println!("{} region files", files.len());
# }
```

## 模块一览

| 模块 | 内容 |
| --- | --- |
| [`region`] | `AnvilRegion`/`ChunkData`：.mca 扇区表、区块定位、zlib/gzip/未压缩、原位复用与空洞回收 |
| [`chunk`] | section 索引、`block_state_at`/`set_block_state`（新旧布局读写保持原格式）、`SectionBlocks`/`SectionBiomes`、空 section 构造 |
| [`migrate`] | 存档迁移：`migrate_chunk`（自动衔接 1.12→1.13 flattening 与 1.13→1.18 布局转换）、`migrate_region_file`、`migrate_dimension`、`migrate_world` |
| [`upgrade`] | 1.12 扁平 ID 升级：`upgrade_chunk_1_12`（Blocks/Data → 调色板）、`flatten_state`（数字 ID+meta → 现代方块状态表） |
| [`world`] | 完整存档目录：`probe` 布局探测（维度/entities/poi/mcr）、`migrate_world` 一键迁移（含 level.dat） |
| [`world`] | 完整存档目录：`probe` 布局探测（维度/entities/poi/mcr）、`migrate_world` 一键迁移（含 level.dat） |
| [`palette`] | 调色板容器位打包（`pack_values`/`unpack_indices`）、`local_palette`、位宽约定 |
| [`paths`] | 维度 region 目录、区域文件名、`floor_div` |

## 新旧格式兼容

同时支持两种区块布局，**读写都保持原布局**（在 1.17 存档上修改不会把它变成 1.18 格式）：

| | 1.13–1.17（旧版） | 1.18+（新版） |
| --- | --- | --- |
| 区块根 | `{DataVersion, Level: {...}}` | 扁平根 |
| Section 列表 | `Level.Sections` | `sections` |
| 方块存储 | section 内平铺 `Palette` + `BlockStates` | `block_states: {palette, data}` 容器 |
| 生物群系 | 根级 `Biomes` 字节数组（不改动） | section 级 `biomes` 容器 |

修改旧版区块时：被改 section 的 `Palette`/`BlockStates` 原地更新（位打包方案与原格式一致，已在真实 1.17 存档上验证字节级往返）；未触碰的 section 与 `Level` 内其他字段原样保留。

1.12 及更早的扁平 ID 存储（`Blocks`/`Data` 字节数组）：**读取**时跳过（报 0 个 section 而非崩溃）；**迁移**时通过 [`upgrade`] 模块自动完成 flattening（数字 ID+meta → 现代方块状态，vanilla 常用方块已收录，未收录组合回退 air 并在统计中报告）。

## 存档迁移

需要把旧版存档升级到现代布局时（例如给只认 1.18+ 的工具用）：

```rust,no_run
use qexed_anvil::migrate::{self, MigrateOptions};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
// 单区块：旧版 -> 现代（方块数据语义不变，已是现代则原样返回）
let (modern, outcome) = migrate::migrate_chunk(&chunk_root, &MigrateOptions::default())?;

// 整个区域文件就地迁移（.mca 被重写；解析失败的区块保持原样不丢数据）
let stats = migrate::migrate_region_file("world/region/r.0.0.mca", &MigrateOptions::default())?;
println!("migrated {}, already modern {}, unsupported {}, failed {}",
    stats.migrated, stats.modern, stats.unsupported, stats.failed);

// 整个维度
let (files, total) = migrate::migrate_dimension(
    "world".as_ref(), "minecraft:overworld", &MigrateOptions::default())?;

// 完整存档目录（识别全部维度 + level.dat，一键迁移）
use qexed_anvil::world;
let layout = world::probe("world")?;                    // 布局探测
println!("{} dimensions, DataVersion {:?}", layout.dimensions.len(), layout.data_version);
let report = world::migrate_world("world", &MigrateOptions::default())?;
for (dim, stats) in &report.dimensions {
    println!("{dim}: migrated={} modern={} failed={}", stats.migrated, stats.modern, stats.failed);
}
# Ok(()) }
```

迁移做的事：解包 `Level`、section 存储容器化、`Biomes` int[] 按各 section 主导值转成名称调色板（内置 1.13–1.17 数字 ID → 现代标识符表）、`Status` 补命名空间、新增 `yPos`、更新 `DataVersion`（默认 3337，可配）。光照等其他字段保留。

安全保证：已在真实 1.17 存档上验证迁移前后**全部方块数据语义等价**；幂等（迁移过的再迁移是 no-op）；建议迁移前备份存档（就地重写）。

### 存档目录的版本差异（`world` 模块处理）

| | 旧版 | 现代版 |
| --- | --- | --- |
| 区域文件扩展名 | `.mcr`（MCRegion，1.2 之前） | `.mca` |
| 下界/末地 | `DIM-1/`、`DIM1/` | 同左（历史目录保留至今） |
| 自定义维度 | 无 | `dimensions/<namespace>/<path>/`（1.16+） |
| 实体 | 区块 NBT 内 `Entities` 字段 | `entities/*.mca` 独立区域文件（1.17+） |
| 兴趣点 | 无 | `poi/*.mca`（1.14+） |
| `level.dat` | 无 DataVersion（1.9 之前） | `Data.DataVersion` |

`world::migrate_world` 会：探测全部维度（含 `dimensions` 树与 DIM 目录）→ 逐文件迁移区块 → 更新 `level.dat` 的 DataVersion（仅当更旧）→ 报告但跳过 `.mcr`（容器格式不同，需先用旧版游戏升级到 1.2+）。`entities/`/`poi/` 容器相同，不做改动。

## 约定与限制

- 读写覆盖 1.13+；`MIN_SECTION_Y = -4` 按 1.18+ 语义（旧版存档高度由其自身字段决定，不受影响）
- LZ4 与自定义压缩、外部 `.mcc` 流会返回明确错误（vanilla 存档不使用它们）
- 生物群系按名称字符串处理；写回时不改动既有 palette 之外的任何字段（Heightmaps、方块实体等保持原样）
- NBT 字符串按 UTF-8 解析；含非法序列的存档可用 `qexed_nbt::from_slice_lossy` 宽松读取（真实世界存档存在这种情况）

## Qexed 内部集成（可选）

开启 `qexed` 特征可得到接入 qexed_config 体系的 `AnvilConfig`。通用场景不需要：

```toml
qexed_anvil = {  features = ["qexed"] }
```

## MSRV

Rust 1.85+（edition 2024）。

## License

MIT — 见 [LICENSE](./LICENSE)。
