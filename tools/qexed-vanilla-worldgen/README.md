# qexed-vanilla-worldgen

这是 qexed 的外置原版地形生成服务。Rust 侧通过 JSON-RPC 调用本服务，服务负责调用 Minecraft 原版 jar 的 worldgen 逻辑，并把生成后的区块写入 Rust 指定的 v5 存档目录。

服务会把生成后的区块写入请求中的 `region_path`。`VanillaChunkWriter` 是地形生成实现边界，Rust world 模块只依赖 JSON-RPC 协议和落盘结果。

## JSON-RPC

方法：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "worldgen.generateChunk",
  "params": {
    "dimension": { "namespace": "minecraft", "value": "overworld" },
    "chunk_x": 0,
    "chunk_z": 0,
    "save_root": "world",
    "dimension_root": "world/dimensions/minecraft/overworld",
    "region_path": "world/dimensions/minecraft/overworld/region/r.0.0.mca"
  }
}
```

结果：

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": {
    "written": true,
    "region_path": "world/dimensions/minecraft/overworld/region/r.0.0.mca"
  }
}
```
