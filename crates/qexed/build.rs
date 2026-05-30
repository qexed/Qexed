use shadow_rs::ShadowBuilder;

fn main() {
    ShadowBuilder::builder().build().unwrap();
    if std::env::var("TARGET").unwrap().contains("windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("logo.ico");
        res.set("FileDescription", "Qexed");
        res.set("ProductName", "Qexed 服务端");

        res.compile().expect("无法编译 Windows 资源文件");
    }
}
