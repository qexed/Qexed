use std::path::PathBuf;

use clap::Parser as _;
use qexed_language_server::{export, router, LanguageRoot};

/// 语言文件服务器：提供翻译文件的 HTTP 服务，或静态导出为可托管目录。
#[derive(Debug, clap::Parser)]
struct Args {
    /// 语言文件树根目录
    #[arg(long, default_value = "./language-data")]
    root: PathBuf,
    /// 监听地址
    #[arg(long, default_value = "127.0.0.1:8090")]
    addr: String,
    /// 静态导出目标目录；提供时执行导出并退出，不启动 HTTP 服务
    #[arg(long)]
    export_to: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async_main(args))
}

async fn async_main(args: Args) -> anyhow::Result<()> {
    let root = LanguageRoot::new(&args.root);
    if let Some(out) = args.export_to {
        // 静态导出：与服务器同构的目录树，可直接交给 nginx / 对象存储。
        export(&root, &out).await?;
        println!("exported {} -> {}", args.root.display(), out.display());
        return Ok(());
    }
    let app = router(root);
    let listener = tokio::net::TcpListener::bind(&args.addr).await?;
    println!("language server listening on http://{}", args.addr);
    axum::serve(listener, app).await?;
    Ok(())
}
