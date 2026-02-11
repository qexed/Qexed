use qexed_nbt::{ Tag, from_file};
use std::path::Path;
use std::{env, io::Read};

/// 递归打印NBT标签的树状结构
fn print_tag(tag: &Tag, indent: usize, name: &str) {
    let indent_str = "  ".repeat(indent);

    match tag {
        Tag::Byte(v) => println!("{}{}: Byte({})", indent_str, name, v),
        Tag::Short(v) => println!("{}{}: Short({})", indent_str, name, v),
        Tag::Int(v) => println!("{}{}: Int({})", indent_str, name, v),
        Tag::Long(v) => println!("{}{}: Long({})", indent_str, name, v),
        Tag::Float(v) => println!("{}{}: Float({})", indent_str, name, v),
        Tag::Double(v) => println!("{}{}: Double({})", indent_str, name, v),
        Tag::String(v) => println!("{}{}: String(\"{}\")", indent_str, name, v),
        Tag::ByteArray(v) => println!("{}{}: ByteArray[{} bytes]", indent_str, name, v.len()),
        Tag::IntArray(v) => println!("{}{}: IntArray[{} ints]", indent_str, name, v.len()),
        Tag::LongArray(v) => println!("{}{}: LongArray[{} longs]", indent_str, name, v.len()),
        Tag::List(header, items) => {
            println!(
                "{}{}: List(id={}, len={})",
                indent_str,
                name,
                header.tag_id,
                items.len()
            );
            for (i, item) in items.iter().enumerate() {
                print_tag(item, indent + 1, &format!("[{}]", i));
            }
        }
        Tag::Compound(map) => {
            println!("{}{}: Compound({} entries)", indent_str, name, map.len());
            for (key, value) in map.iter() {
                print_tag(value, indent + 1, key);
            }
        }
        Tag::End => println!("{}{}: End", indent_str, name),
    }
}

/// 统计NBT结构的信息
fn collect_stats(tag: &Tag, stats: &mut NbtStats, depth: usize) {
    stats.total_tags += 1;
    stats.max_depth = stats.max_depth.max(depth);

    match tag {
        Tag::Byte(_v) => stats.byte_count += 1,
        Tag::Short(_v) => stats.short_count += 1,
        Tag::Int(_v) => stats.int_count += 1,
        Tag::Long(_v) => stats.long_count += 1,
        Tag::Float(_v) => stats.float_count += 1,
        Tag::Double(_v) => stats.double_count += 1,
        Tag::String(v) => {
            stats.string_count += 1;
            stats.total_string_bytes += v.len();
        }
        Tag::ByteArray(v) => {
            stats.byte_array_count += 1;
            stats.total_array_elements += v.len();
        }
        Tag::IntArray(v) => {
            stats.int_array_count += 1;
            stats.total_array_elements += v.len();
        }
        Tag::LongArray(v) => {
            stats.long_array_count += 1;
            stats.total_array_elements += v.len();
        }
        Tag::List(_, items) => {
            stats.list_count += 1;
            for item in items.iter() {
                collect_stats(item, stats, depth + 1);
            }
        }
        Tag::Compound(map) => {
            stats.compound_count += 1;
            for value in map.values() {
                collect_stats(value, stats, depth + 1);
            }
        }
        Tag::End => stats.end_count += 1,
    }
}

#[derive(Debug, Default)]
struct NbtStats {
    total_tags: usize,
    max_depth: usize,
    byte_count: usize,
    short_count: usize,
    int_count: usize,
    long_count: usize,
    float_count: usize,
    double_count: usize,
    string_count: usize,
    byte_array_count: usize,
    int_array_count: usize,
    long_array_count: usize,
    list_count: usize,
    compound_count: usize,
    end_count: usize,
    total_string_bytes: usize,
    total_array_elements: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Qexed NBT .dat 文件读取测试 ===\n");

    // 获取命令行参数
    let args: Vec<String> = env::args().collect();
    let file_path = if args.len() > 1 {
        &args[1]
    } else {
        println!("用法: cargo run --example read_dat -- <路径到.dat文件>");
        println!("示例: cargo run --example read_dat -- ../test_data/level.dat");
        println!("       cargo run --example read_dat -- world/level.dat");
        println!("\n提示: 你可以从Minecraft世界存档中复制 level.dat 文件进行测试");
        return Ok(());
    };

    println!("读取文件: {}", file_path);

    // 检查文件是否存在
    if !Path::new(file_path).exists() {
        eprintln!("错误: 文件 '{}' 不存在", file_path);
        eprintln!("请提供有效的.dat文件路径");
        eprintln!("常见的Minecraft .dat文件位置：");
        eprintln!("  - 世界存档: <世界目录>/level.dat");
        eprintln!("  - 玩家数据: <世界目录>/playerdata/<UUID>.dat");
        eprintln!("  - 实体数据: <世界目录>/entities/*.dat");
        return Ok(());
    }

    // 使用库函数读取文件
    match from_file(file_path) {
        Ok((root_name, root_tag)) => {
            println!("\n=== 解析成功 ===");
            println!(
                "根标签名: '{}'",
                if root_name.is_empty() {
                    "(空)"
                } else {
                    &root_name
                }
            );
            println!("根标签类型: Compound (0x0A)");

            // 收集统计信息
            let mut stats = NbtStats::default();
            collect_stats(&root_tag, &mut stats, 1);

            println!("\n内容结构:");
            print_tag(
                &root_tag,
                0,
                if root_name.is_empty() {
                    "(root)"
                } else {
                    &root_name
                },
            );

            // 显示统计信息
            println!("\n=== 统计信息 ===");
            println!("总标签数: {}", stats.total_tags);
            println!("最大深度: {}", stats.max_depth);
            println!("\n按类型统计:");
            if stats.byte_count > 0 {
                println!("  Byte: {}", stats.byte_count);
            }
            if stats.short_count > 0 {
                println!("  Short: {}", stats.short_count);
            }
            if stats.int_count > 0 {
                println!("  Int: {}", stats.int_count);
            }
            if stats.long_count > 0 {
                println!("  Long: {}", stats.long_count);
            }
            if stats.float_count > 0 {
                println!("  Float: {}", stats.float_count);
            }
            if stats.double_count > 0 {
                println!("  Double: {}", stats.double_count);
            }
            if stats.string_count > 0 {
                println!(
                    "  String: {} (共{}字节)",
                    stats.string_count, stats.total_string_bytes
                );
            }
            if stats.byte_array_count > 0 {
                println!("  ByteArray: {}", stats.byte_array_count);
            }
            if stats.int_array_count > 0 {
                println!("  IntArray: {}", stats.int_array_count);
            }
            if stats.long_array_count > 0 {
                println!("  LongArray: {}", stats.long_array_count);
            }
            if stats.list_count > 0 {
                println!("  List: {}", stats.list_count);
            }
            if stats.compound_count > 0 {
                println!("  Compound: {}", stats.compound_count);
            }
            if stats.end_count > 0 {
                println!("  End: {}", stats.end_count);
            }

            if stats.total_array_elements > 0 {
                println!("\n数组元素总数: {}", stats.total_array_elements);
            }

            // 显示文件大小信息
            if let Ok(metadata) = std::fs::metadata(file_path) {
                let file_size = metadata.len();
                let compression_ratio = if file_size > 0 {
                    // 更合理的"数据密度"计算：平均每个字节存储的标签数
                    let density = stats.total_tags as f64 / file_size as f64;
                    format!(" ({:.2} 标签/字节)", density)
                } else {
                    String::new()
                };
                println!("\n文件大小: {} 字节{}", file_size, compression_ratio);
            }
        }
        Err(e) => {
            eprintln!("\n=== 解析失败 ===");
            eprintln!("错误: {}", e);
            eprintln!("\n可能的原因:");
            eprintln!("1. 文件不是有效的NBT格式");
            eprintln!("2. 文件损坏或不完整");
            eprintln!("3. 使用了不支持的NBT变体（如基岩版小端序）");
            eprintln!("4. Gzip压缩损坏（如果是压缩文件）");

            // 尝试显示文件基本信息
            if let Ok(metadata) = std::fs::metadata(file_path) {
                println!("\n文件基本信息:");
                println!("  大小: {} 字节", metadata.len());

                // 检查是否为Gzip压缩
                let mut buffer = [0u8; 2];
                if let Ok(mut file) = std::fs::File::open(file_path) {
                    if file.read_exact(&mut buffer).is_ok() {
                        let is_gzip = buffer == [0x1F, 0x8B];
                        println!("  Gzip压缩: {}", if is_gzip { "是" } else { "否" });
                    }
                }
            }

            // 建议的调试步骤
            eprintln!("\n调试建议:");
            eprintln!("1. 使用 hexdump 或类似工具检查文件头");
            eprintln!("2. 确保文件是Java版Minecraft生成的NBT文件");
            eprintln!("3. 尝试使用其他NBT工具（如NBTExplorer）验证文件");
        }
    }

    Ok(())
}
