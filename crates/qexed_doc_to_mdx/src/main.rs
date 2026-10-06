mod export_item;
mod render;
mod render_lang_index;
mod render_packet;
mod render_root;
mod route;

use shadow_rs::shadow;
shadow!(shadow);

use render_lang_index::render_lang_index;
use render_root::render_root_page;
use route::{PageKind, collect_pages};
use std::path::PathBuf;

/// 当前参与导出的配置 schema 清单：新增配置 crate 时在此登记。
///
/// 顺序即文档站上"应用列表"的展示顺序（见 `render_lang_index`），
/// 不是字典序，增删配置时注意保持期望的顺序。
fn export_items() -> anyhow::Result<Vec<export_item::ExportItem>> {
    Ok(vec![
        export_item::ExportItem::of::<qexed_log::config::LogConfig>()?,
        export_item::ExportItem::of::<qexed_language::config::LanguageConfig>()?,
        export_item::ExportItem::of::<qexed_mojang_data::config::MojangDataConfig>()?,
        
        // 新增配置 crate 时一行搞定：
        // ExportItem::of::<qexed_xxx::config::XxxConfig>(),
    ])
}

/// 多语言文档生成器：按 Next.js app 路由生成页面文件骨架。
#[derive(Debug, clap::Parser)]
struct Args {
    /// 目标语言列表，逗号分隔。
    #[arg(long, default_value = "zh-CN,en")]
    langs: String,
    /// Next.js app 目录根（配置文档）。
    #[arg(long, default_value = "./app")]
    out: PathBuf,
    /// 网络数据包文档输出根（文档站 `app/docs/protocol`）。空则跳过。
    #[arg(long, default_value = "")]
    packets_out: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Args = clap::Parser::parse();
    qexed_config::init_config_path("./config".into())?;
    let langs: Vec<&str> = args
        .langs
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();
    let items = export_items()?;

    // 阶段 1：一次性算出所有路由页及其类型（与 lang 无关）。
    let pages = collect_pages(&items)?;
    let config_count = pages
        .iter()
        .filter(|p| p.kind() == PageKind::Config)
        .count();
    let index_count = pages.len() - config_count;
    println!(
        "routes: {} total ({} config, {} index)",
        pages.len(),
        config_count,
        index_count
    );

    // 阶段 2：按 lang × 页面生成。
    let mut generated = 0;
    for lang in &langs {
        let table = qexed_language::load_translations(shadow::SHORT_COMMIT, lang).await?;
        for page in &pages {
            let dir = args.out.join(lang).join(&page.rel_dir);
            tokio::fs::create_dir_all(&dir).await?;
            let file = dir.join("page.mdx");

            match page.kind() {
                PageKind::Config => {
                    let item = &items[page.items[0]];
                    let mdx = render::render_mdx(item.name, &item.schema, &table, lang, &item.data);
                    tokio::fs::write(&file, mdx).await?;
                }
                PageKind::Index => {
                    // TODO: 路由列表页内容待接入：
                    //   列出 page.child_segments 里每个子路由的链接。
                    tokio::fs::write(&file, "").await?;
                }
            }
            generated += 1;
            println!("generated {} [{}]", file.display(), page.kind().label());
        }

        // 该语言目录的入口页：app/<lang>/page.mdx
        let lang_dir = args.out.join(lang);
        tokio::fs::create_dir_all(&lang_dir).await?;
        let lang_page = lang_dir.join("page.mdx");
        tokio::fs::write(&lang_page, render_lang_index(&items, &table)?).await?;
        generated += 1;
        println!("generated {} [lang]", lang_page.display());
    }

    // 站点根入口：app/page.mdx（Next.js 根路由 /）
    tokio::fs::create_dir_all(&args.out).await?;
    let root_file = args.out.join("page.mdx");
    tokio::fs::write(&root_file, render_root_page(&langs)).await?;
    generated += 1;
    println!("generated {} [root]", root_file.display());

    if !args.packets_out.is_empty() {
        generated += write_packet_docs(PathBuf::from(&args.packets_out), &langs).await?;
    }

    println!("done: {generated} files across {} languages", langs.len());
    Ok(())
}

async fn write_packet_docs(out: PathBuf, langs: &[&str]) -> anyhow::Result<usize> {
    let _linked = qexed_protocol::link_packet_docs();
    let docs = qexed_packet::collect_packet_docs();
    println!("packets: {} (linked {})", docs.len(), _linked);
    let mut n = 0usize;
    for lang in langs {
        let table = qexed_language::load_translations(shadow::SHORT_COMMIT, lang).await?;
        let zh = matches!(*lang, "zh-CN" | "zh" | "zh-Hans");
        let index_title = if zh { "网络数据包" } else { "Network packets" };
        let index = crate::render_packet::render_packet_index(
            &docs,
            &table,
            lang,
            index_title,
            qexed_config::MC_VERSION,
            shadow::COMMIT_HASH,
        );
        let index_file = if zh {
            out.join("page.mdx")
        } else {
            out.join("en").join("page.mdx")
        };
        if let Some(parent) = index_file.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(&index_file, index).await?;
        n += 1;
        println!("generated {} [packet-index]", index_file.display());

        for doc in &docs {
            let mdx = crate::render_packet::render_packet_mdx(
                doc,
                &table,
                lang,
                qexed_config::MC_VERSION,
                shadow::COMMIT_HASH,
            );
            let rel = doc.slug();
            let file = if zh {
                out.join(&rel).join("page.mdx")
            } else {
                out.join(&rel).join("en").join("page.mdx")
            };
            if let Some(parent) = file.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&file, mdx).await?;
            n += 1;
        }
    }
    Ok(n)
}
