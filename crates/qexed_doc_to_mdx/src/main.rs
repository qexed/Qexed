mod export_item;
mod render;
mod render_lang_index;
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
    /// Next.js app 目录根。
    #[arg(long, default_value = "./app")]
    out: PathBuf,
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

    println!("done: {generated} files across {} languages", langs.len());
    Ok(())
}
