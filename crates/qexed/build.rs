use shadow_rs::{BuildPattern, ShadowBuilder};

fn main() {
    ShadowBuilder::builder()
        .build_pattern(BuildPattern::RealTime)
        .build()
        .unwrap();
    if std::env::var("TARGET").unwrap().contains("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("logo.ico");
        res.set("FileDescription", "基于Rust的我的世界Java版服务端");
        res.set("ProductName", "Qexed 服务端");

        res.compile().expect("无法编译 Windows 资源文件");
    }
}
