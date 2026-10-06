// 26.3 迁移收尾：play 目录去重 + mod.rs 重生成 + 校验
// 在全部 batch 完成后运行：node scripts/finalize-26.3.mjs
import { readdir, readFile, writeFile, unlink } from "node:fs/promises";
import { existsSync } from "node:fs";

const ROOT = "E:/code/qexed-v6/crates/qexed_protocol/src";
const detail = JSON.parse(await readFile("E:/code/qexed-v6/target/mc-26.3/packets-26.3-detail.json", "utf8"));

const javaToOurFile = (s) => s.replace(/^(Clientbound|Serverbound)/, "").replace(/Packet$/, "")
  .replace(/([a-z0-9])([A-Z])/g, "$1_$2").toLowerCase();

for (const dir of ["to_client/play", "to_server/play"]) {
  const files = (await readdir(`${ROOT}/${dir}`)).filter(f => f.endsWith(".rs") && f !== "mod.rs");
  const keep = new Set(
    detail.filter(d => (d.state === "play") &&
      (d.direction === dir.split("/")[0]) &&
      !["Pos", "PosRot", "Rot", "StatusOnly"].includes(d.className))
      .map(d => javaToOurFile(d.className))
  );
  const del = files.filter(f => !keep.has(f.replace(".rs", "")));
  for (const f of del) {
    await unlink(`${ROOT}/${dir}/${f}`);
    console.log("del", dir, f);
  }
  // 重写 mod.rs（按 26.3 清单顺序）
  const mods = [...keep].sort().map(m => `pub mod ${m};`).join("\n") + "\n";
  await writeFile(`${ROOT}/${dir}/mod.rs`, mods);
  console.log(dir, "mod.rs:", keep.size, "modules");
}

// 缺失检查：清单要求但文件不存在
for (const d of detail.filter(x => x.state === "play")) {
  if (["Pos", "PosRot", "Rot", "StatusOnly"].includes(d.className)) continue;
  const f = `${ROOT}/${d.direction}/play/${javaToOurFile(d.className)}.rs`;
  if (!existsSync(f)) console.log("MISSING", d.direction, d.className, "->", f);
}
console.log("done");
