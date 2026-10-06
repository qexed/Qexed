use qexed_doc::DocSchema;
use qexed_language::Translations;
use qexed_packet::PacketDoc;

fn esc(text: impl AsRef<str>) -> String {
    let text = text.as_ref();
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for c in text.chars() {
        match c {
            '`' => { in_code = !in_code; out.push('`'); }
            '{' if !in_code => out.push_str("\\{"),
            '}' if !in_code => out.push_str("\\}"),
            '<' if !in_code => out.push_str("&lt;"),
            _ => out.push(c),
        }
    }
    out
}

fn ui(lang: &str, key: &str) -> &'static str {
    let zh = matches!(lang, "zh-CN" | "zh" | "zh-Hans");
    match (key, zh) {
        ("packet_id", true) => "数据包 ID",
        ("packet_id", false) => "Packet ID",
        ("direction", true) => "方向",
        ("direction", false) => "Direction",
        ("state", true) => "连接状态",
        ("state", false) => "Connection state",
        ("to_server", true) => "客户端 → 服务器",
        ("to_server", false) => "Client → Server",
        ("to_client", true) => "服务器 → 客户端",
        ("to_client", false) => "Server → Client",
        ("fields", true) => "字段",
        ("fields", false) => "Fields",
        ("type", true) => "类型",
        ("type", false) => "Type",
        ("name", true) => "字段",
        ("name", false) => "Field",
        ("empty", true) => "无字段（空包）",
        ("empty", false) => "No fields (empty packet)",
        ("rust", true) => "Rust 类型",
        ("rust", false) => "Rust type",
        ("minecraft", true) => "Minecraft 版本",
        ("minecraft", false) => "Minecraft version",
        ("commit", true) => "Qexed 提交",
        ("commit", false) => "Qexed commit",
        _ => "",
    }
}

pub fn render_packet_mdx(
    doc: &PacketDoc,
    table: &Translations,
    lang: &str,
    mc_version: &str,
    commit: &str,
) -> String {
    let mut schema: DocSchema = doc.schema.clone();
    qexed_language::translate_schema_with(&mut schema, &|key| table.t(key));
    let title = table.get(&schema.key).map(str::to_string).unwrap_or_else(|| doc.rust_name.to_string());
    let mut out = String::new();
    out.push_str("import CommitToGithubHref from \"@components/CommitToGithubHref\";\n\n");
    out.push_str(&format!("# {}\n\n", esc(title)));
    out.push_str(&format!("- {}: `{}`\n", ui(lang, "minecraft"), mc_version));
    out.push_str(&format!("- {}: <CommitToGithubHref commit=\"{commit}\"/>\n", ui(lang, "commit")));
    out.push_str(&format!("- {}: `0x{:02X}` ({})\n", ui(lang, "packet_id"), doc.id as u32, doc.id));
    out.push_str(&format!("- {}: {}\n", ui(lang, "direction"), ui(lang, doc.direction())));
    out.push_str(&format!("- {}: `{}`\n", ui(lang, "state"), doc.state()));
    out.push_str(&format!("- {}: `{}`\n\n", ui(lang, "rust"), doc.rust_name));
    out.push_str(&format!("## {}\n\n", ui(lang, "fields")));
    if schema.fields.is_empty() {
        out.push_str(ui(lang, "empty"));
        out.push('\n');
        return out;
    }
    out.push_str(&format!("| {} | {} |\n| --- | --- |\n", ui(lang, "name"), ui(lang, "type")));
    for field in &schema.fields {
        let label = table.get(&field.key).map(str::to_string).unwrap_or_else(|| field.path.clone());
        // stringify! 可能带换行/多余空格：压成单行再修 token 间距，避免 MDX 把断行的 <...> 当 JSX
        let ty = field
            .value_type
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace(" :: ", "::")
            .replace(" < ", "<")
            .replace(" > ", ">")
            .replace(" >", ">")
            .replace("< ", "<");
        out.push_str(&format!("| `{}` | `{}` |\n", esc(label), ty));
    }
    out
}

pub fn render_packet_index(
    docs: &[PacketDoc],
    table: &Translations,
    lang: &str,
    title: &str,
    mc_version: &str,
    commit: &str,
) -> String {
    let mut out = String::new();
    out.push_str("import CommitToGithubHref from \"@components/CommitToGithubHref\";\n\n");
    out.push_str(&format!("# {}\n\n", esc(title)));
    out.push_str(&format!("- {}: `{}`\n", ui(lang, "minecraft"), mc_version));
    out.push_str(&format!("- {}: <CommitToGithubHref commit=\"{commit}\"/>\n\n", ui(lang, "commit")));
    let mut current = String::new();
    for doc in docs {
        let group = format!("{}/{}", doc.direction(), doc.state());
        if group != current {
            if !current.is_empty() {
                out.push('\n');
            }
            current = group;
            out.push_str(&format!("## {} / {}\n\n", ui(lang, doc.direction()), doc.state()));
        }
        let name = table.get(&doc.schema.key).map(str::to_string).unwrap_or_else(|| doc.rust_name.to_string());
        let href = if matches!(lang, "zh-CN" | "zh" | "zh-Hans") {
            format!("./{}", doc.slug())
        } else {
            format!("../{}/en", doc.slug())
        };
        out.push_str(&format!("- [`0x{:02X}` {}]({})\n", doc.id as u32, esc(name), href));
    }
    out
}
