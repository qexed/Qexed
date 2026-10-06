use qexed_doc::DocSchema;

/// 一个网络包的文档描述：id、方向、状态、字段结构。
#[derive(Debug, Clone)]
pub struct PacketDoc {
    pub id: i32,
    pub rust_name: &'static str,
    pub module_path: &'static str,
    pub schema: DocSchema,
}

impl PacketDoc {
    pub fn direction(&self) -> &'static str {
        if self.module_path.contains("::to_server::") {
            "to_server"
        } else if self.module_path.contains("::to_client::") {
            "to_client"
        } else {
            "unknown"
        }
    }

    pub fn state(&self) -> &'static str {
        for state in [
            "handshaking",
            "status",
            "login",
            "configuration",
            "play",
        ] {
            if self.module_path.contains(&format!("::{state}::")) {
                return state;
            }
        }
        "unknown"
    }

    pub fn slug(&self) -> String {
        let mut name = String::new();
        for (i, ch) in self.rust_name.chars().enumerate() {
            if ch.is_uppercase() {
                if i > 0 {
                    name.push('-');
                }
                name.extend(ch.to_lowercase());
            } else {
                name.push(ch);
            }
        }
        format!("{}/{}/{}", self.direction().replace('_', "-"), self.state(), name)
    }
}

pub struct PacketDocSubmit {
    pub build: fn() -> PacketDoc,
}

inventory::collect!(PacketDocSubmit);

pub fn collect_packet_docs() -> Vec<PacketDoc> {
    let mut docs: Vec<PacketDoc> = inventory::iter::<PacketDocSubmit>
        .into_iter()
        .map(|item| (item.build)())
        .collect();
    docs.sort_by(|a, b| {
        (a.direction(), a.state(), a.id, a.rust_name).cmp(&(
            b.direction(),
            b.state(),
            b.id,
            b.rust_name,
        ))
    });
    docs
}
