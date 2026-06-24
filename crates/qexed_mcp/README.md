# Qexed MCP Server

离线 Minecraft 世界编辑器 —— 通过 MCP (Model Context Protocol) 让 AI 直接读写 Anvil 格式的存档文件。

**核心原理：不启动 Minecraft 服务器，直接操作 `.mca` 区域文件。** AI 生成完建筑后，玩家打开游戏即可看到。

## 快速开始

```bash
# 编译
cargo build -p qexed_mcp --release

# 验证 (MCP 通过 stdio 通信，发送 initialize 请求)
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | cargo run -p qexed_mcp
```

## Tools (7个)

### 1. set_save_path — 设置存档路径
必须先调用，指定世界存档目录和维度。

| 参数 | 类型 | 说明 |
|------|------|------|
| `save_path` | string | 世界存档根目录 (包含 `region/`, `level.dat` 的目录) |
| `dimension` | string | 默认 `"minecraft:overworld"`，可选 `"minecraft:the_nether"`, `"minecraft:the_end"` |

### 2. load_block_registry — 加载方块注册表
从 `blocks.json` 加载方块名→ID映射，启用名称查询。

| 参数 | 类型 | 说明 |
|------|------|------|
| `registry_path` | string | 路径，如 `"assets/reports/blocks.json"` |

### 3. lookup_block_state — 查方块ID
将方块名转换为 `block_state_id` (数字)。

| 参数 | 类型 | 说明 |
|------|------|------|
| `block` | string | 如 `"minecraft:stone"` 或 `"stone"` |
| `properties` | object? | 如 `{"facing":"north","half":"bottom"}` |

### 4. list_blocks — 列出所有方块
列出已加载的所有方块名及其默认 ID。

### 5. read_region — 读取区域
读取矩形区域内的所有方块状态。

| 参数 | 类型 | 说明 |
|------|------|------|
| `min` | `{x,y,z}` | 区域起点 (包含) |
| `max` | `{x,y,z}` | 区域终点 (包含) |
| `summary_only` | bool? | `true` 时只返回方块统计，不返回完整数组 |

### 6. place_blocks — 放置方块
批量放置方块，按 chunk 分组合并写入 `.mca`。

| 参数 | 类型 | 说明 |
|------|------|------|
| `blocks` | array | `[{x, y, z, block_state_id}, ...]` |

### 7. fill_region — 填充区域
用单一方块填充矩形区域。比 place_blocks 快，适合平整地形。

| 参数 | 类型 | 说明 |
|------|------|------|
| `min` | `{x,y,z}` | 区域起点 |
| `max` | `{x,y,z}` | 区域终点 |
| `block_state_id` | int | 填充方块的ID (0=空气) |

## 典型工作流

```
AI 生成建筑的标准调用顺序：

1. set_save_path({save_path: "/path/to/world"})
2. load_block_registry({registry_path: "assets/reports/blocks.json"})
3. lookup_block_state({block: "stone"})          → 得到 ID=...
4. lookup_block_state({block: "oak_stairs", properties: {facing: "north"}})
5. read_region({min:{x:0,y:60,z:0}, max:{x:100,y:80,z:100}, summary_only:true})
     → AI 了解现有地形
6. fill_region({min:{x:0,y:60,z:0}, max:{x:100,y:63,z:100}, block_state_id: 0})
     → 平整场地
7. place_blocks({blocks: [{x:10,y:64,z:10,block_state_id: 123}, ...]})
     → 逐块放置建筑
8. 玩家打开 Minecraft → 进入世界 → 建筑已就位！
```

## 项目结构

```
crates/qexed_mcp/
├── Cargo.toml       # 依赖: qexed_nbt, serde_json, flate2, bytes
└── src/
    ├── main.rs      # MCP stdio 服务器 + 工具路由
    ├── region.rs    # AnvilRegion (.mca 文件读写)
    ├── chunk.rs     # Chunk NBT 操作 (方块读写、填充、高度图)
    ├── registry.rs  # 方块注册表 (blocks.json 加载)
    └── protocol.rs  # MCP JSON-RPC 2.0 类型
```

## 方块 ID 对照表 (常用)

| 方块名 | 默认 ID (1.21) |
|--------|---------------|
| `minecraft:air` | 0 |
| `minecraft:stone` | 见 blocks.json |
| `minecraft:grass_block` | 见 blocks.json |
| `minecraft:dirt` | 见 blocks.json |
| `minecraft:cobblestone` | 见 blocks.json |
| `minecraft:oak_planks` | 见 blocks.json |
| `minecraft:oak_stairs` | 见 blocks.json |
| `minecraft:glass` | 见 blocks.json |
| `minecraft:red_concrete` | 见 blocks.json |

> 使用 `lookup_block_state` 获取精确 ID，或 `list_blocks` 查看全部。

## 坐标系统

- **Y轴**: -64 (基岩) 到 319 (建筑上限)
- **chunk**: 16×384×16 的柱状区域，文件存储在 `region/r.{rx}.{rz}.mca`
- **region**: 32×32 个 chunk 的集合 (512×512 方块)

## 限制 & 注意事项

1. **无光照计算** — 写入的方块没有光照数据，Minecraft 客户端会在加载时重新计算
2. **无方块实体** — 不支持箱子、告示牌等需要额外数据的方块
3. **Y 轴范围** — 必须在 `[-64, 319]` 内
4. **存档独占** — 写入时确保没有其他程序（服务器/客户端）打开该存档
5. **大区域填充** — `fill_region` 按 chunk 逐个处理，非常大的区域可能需要几秒
