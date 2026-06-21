# Windows worldgen v4 烟测诊断

`tests::v4_pipeline_smoke_when_enabled` 曾直接执行完整 v4 overworld chunk 生成。Windows 上该路径会在 feature placement 阶段偶发以 `0xffffffff` 结束测试进程，Cargo 看不到 Rust panic，因此不会打印断言或回溯。

现在默认测试只保留轻量、稳定的 base+carvers 覆盖：

```powershell
cargo test -p qexed_worldgen generator_v4::tests::v4_pipeline_base_and_carvers_smoke_is_bounded -- --test-threads=1
```

完整管线仍作为手动诊断保留：

```powershell
cargo test -p qexed_worldgen generator_v4::tests::v4_pipeline_full_chunk_manual_diagnostic -- --ignored --test-threads=1 --nocapture
```

如需复现旧入口：

```powershell
cargo test -p qexed_worldgen tests::v4_pipeline_smoke_when_enabled -- --ignored --test-threads=1 --nocapture
```

如果随后出现 `link.exe : fatal error LNK1104` 且目标是 `target\debug\deps\qexed_worldgen-*.exe`，通常是前一次异常退出后测试进程或杀软扫描仍短暂持有 exe。先确认没有残留测试进程，再重试构建。
