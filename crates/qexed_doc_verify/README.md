# qexed_doc_verify — 文档示例校验器

把 MDX 页面 frontmatter 里的 `verify:` 清单交给 docker 复现「构建 + 运行 + 比对」，
入口就是 `cargo test`，机制详见仓库根的 `verify-manifest.example.mdx`。

## 两层校验

| 层 | 命令 | 需要 docker | 校验内容 |
| --- | --- | --- | --- |
| 轻量 | `cargo test -p qexed_doc_verify` | 否（毫秒级） | verify_hash / api.hash / result.hash / snippet_hash 四层哈希自洽 |
| 完整 | `cargo test -p qexed_doc_verify -- --ignored` | 是 | rust:1.98 容器内真实构建 + 运行 snippet，比对退出码 |

## 工作方式

1. **物化**：正文第一个 rust 围栏写到 `target/doc-verify/<页名>/src/main.rs`，
   清单里 `cargo_toml` 的 `{{WORKSPACE}}` 占位符替换为 `/workspace`。
2. **挂载**：宿主工作区只读挂到 `/workspace`，物化目录可写挂到 `/doc-verify`，
   snippet 的 `qexed_log = { path = "/workspace/crates/qexed_log" }` 直接命中真实 crate ——
   上游 API 变化时容器内编译必然失败，这正是「定义漂移」的裁判。
3. **判定**：容器退出码 == 清单 `exit_code` 即通过。

## 台账规则（重要）

- `result.hash` 变化 → 测试失败 → **人工确认新结果后**由 CI 回填哈希，
  严禁机器自动静默回填，否则清单会随代码腐烂。
- `verified_at` / `commit` / `ci_run` 由 CI 校验通过后回填（`verify-refresh`，待实现）。
- 哈希规范化规则由 `spec: qexed.doc.verify/1` 固定；规则变化必须升 spec 版本。

## 测试

```bash
cargo test -p qexed_doc_verify             # 轻量层
cargo test -p qexed_doc_verify -- --ignored --nocapture  # 完整层（需 docker）
```
