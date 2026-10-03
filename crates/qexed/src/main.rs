use std::time;

use clap::Parser;
use qexed_club::{arg_fields, ServerArgs};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DocApiVersion {
    hash: &'static str,
    version: &'static str,
    build_date: &'static str,
    args: Vec<qexed_club::ArgField>,
}

fn main() {
    let args = ServerArgs::parse();
    if args.doc_api_version {
        let payload = DocApiVersion {
            hash: env!("QEXED_GIT_HASH"),
            version: env!("CARGO_PKG_VERSION"),
            build_date: env!("QEXED_BUILD_DATE"),
            args: arg_fields(),
        };
        println!("{}", serde_json::to_string(&payload).expect("doc api version json"));
        return;
    }

    println!("Qexed {}", env!("CARGO_PKG_VERSION"));
    loop{
        std::thread::sleep(time::Duration::from_secs(5)); // 等待 5 秒
    }
}
