// crates/data/qexed_nbt/examples/basic.rs
use qexed_nbt::{Tag, tag_id};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建基础数据
    let health = Tag::Float(20.5);
    let name = Tag::String("Steve".into()); // `"str".into()` 可以转为 Arc<str>

    // 2. 创建数组
    let inventory_bytes = vec![0u8, 1, 2, 3];
    let byte_array = Tag::byte_array_from_u8_slice(&inventory_bytes);

    // 3. 创建 Compound (类似JSON对象)
    use std::collections::HashMap;
    let mut player_data = HashMap::new();
    player_data.insert("Health".to_string(), health);
    player_data.insert("Name".to_string(), name);
    player_data.insert("Inventory".to_string(), byte_array);
    let compound = Tag::Compound(std::sync::Arc::new(player_data));

    // 4. 使用 serde_json 来观察序列化结果（仅用于调试，实际使用二进制格式）
    println!("Compound as JSON (debug view):");
    println!("{}", serde_json::to_string_pretty(&compound)?);

    // 5. 创建同质List
    let pos_list = Tag::new_list(
        tag_id::DOUBLE,
        vec![Tag::Double(100.5), Tag::Double(64.0), Tag::Double(300.0)],
    )?;
    println!("\nPosition list created successfully: {:?}", pos_list.tag_id());

    Ok(())
}