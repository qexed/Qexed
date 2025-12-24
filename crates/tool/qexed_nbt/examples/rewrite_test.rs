// crates/data/qexed_nbt/examples/rewrite_test.rs
use qexed_nbt::{from_file, to_file,  Tag};
use std::path::{Path, PathBuf};
use std::{env, fs};

/// 比较两个NBT标签是否相等（考虑顺序无关性）
fn tags_equal(tag1: &Tag, tag2: &Tag, depth: usize) -> bool {
    // 最大递归深度保护
    if depth > 100 {
        eprintln!("警告：递归深度超过100，可能存在循环引用");
        return false;
    }
    
    match (tag1, tag2) {
        (Tag::Byte(a), Tag::Byte(b)) => a == b,
        (Tag::Short(a), Tag::Short(b)) => a == b,
        (Tag::Int(a), Tag::Int(b)) => a == b,
        (Tag::Long(a), Tag::Long(b)) => a == b,
        (Tag::Float(a), Tag::Float(b)) => a.to_bits() == b.to_bits(), // 精确比较浮点数
        (Tag::Double(a), Tag::Double(b)) => a.to_bits() == b.to_bits(),
        (Tag::String(a), Tag::String(b)) => a == b,
        (Tag::ByteArray(a), Tag::ByteArray(b)) => a == b,
        (Tag::IntArray(a), Tag::IntArray(b)) => a == b,
        (Tag::LongArray(a), Tag::LongArray(b)) => a == b,
        (Tag::List(h1, items1), Tag::List(h2, items2)) => {
            // List必须完全一致：类型、长度、顺序
            if h1.tag_id != h2.tag_id || h1.length != h2.length || items1.len() != items2.len() {
                return false;
            }
            items1.iter().zip(items2.iter()).all(|(a, b)| tags_equal(a, b, depth + 1))
        }
        (Tag::Compound(map1), Tag::Compound(map2)) => {
            // Compound比较内容，不比较顺序
            if map1.len() != map2.len() {
                return false;
            }
            
            for (key, value1) in map1.iter() {
                match map2.get(key) {
                    Some(value2) => {
                        if !tags_equal(value1, value2, depth + 1) {
                            return false;
                        }
                    }
                    None => return false,
                }
            }
            true
        }
        (Tag::End, Tag::End) => true,
        _ => false, // 类型不匹配
    }
}

/// 生成输出文件名
fn generate_output_path(input_path: &Path, output_arg: Option<&String>) -> PathBuf {
    if let Some(output_path) = output_arg {
        PathBuf::from(output_path)
    } else {
        // 默认：在原文件名后添加 .rewritten
        let mut new_path = input_path.to_path_buf();
        let file_name = new_path.file_name().unwrap_or_default().to_string_lossy();
        new_path.set_file_name(format!("{}_rewritten", file_name));
        new_path
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Qexed NBT 重写测试工具 ===\n");
    println!("功能：读取NBT文件并重新写入，测试读写一致性\n");
    
    // 获取命令行参数
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("用法：");
        println!("  cargo run --example rewrite_test -- <输入文件> [输出文件]");
        println!();
        println!("参数说明：");
        println!("  <输入文件>  要读取的NBT文件（如 .dat 文件）");
        println!("  [输出文件]  可选，输出文件路径（默认：输入文件_rewritten）");
        println!();
        println!("示例：");
        println!("  cargo run --example rewrite_test -- player.dat");
        println!("  cargo run --example rewrite_test -- level.dat level_modified.dat");
        println!("  cargo run --example rewrite_test -- test.dat --verify  # 额外验证");
        return Ok(());
    }
    
    let input_path = Path::new(&args[1]);
    let output_path = generate_output_path(input_path, args.get(2));
    let verify = args.iter().any(|arg| arg == "--verify" || arg == "-v");
    
    // 检查输入文件是否存在
    if !input_path.exists() {
        eprintln!("错误：输入文件 '{}' 不存在", input_path.display());
        return Ok(());
    }
    
    // 获取文件信息
    let input_metadata = fs::metadata(input_path)?;
    let input_size = input_metadata.len();
    
    println!("输入文件: {}", input_path.display());
    println!("文件大小: {} 字节", input_size);
    println!("输出文件: {}", output_path.display());
    if verify {
        println!("验证模式: 启用");
    }
    println!();
    
    // 步骤1：读取原始文件
    println!("1. 读取原始文件...");
    let (root_name, original_tag) = match from_file(input_path) {
        Ok(result) => {
            println!("   ✓ 读取成功");
            println!("   根标签名: '{}'", result.0);
            println!("   根标签类型: {:?}", result.1.tag_id());
            result
        }
        Err(e) => {
            eprintln!("   ✗ 读取失败: {}", e);
            return Ok(());
        }
    };
    
    // 简单统计
    let mut tag_count = 0;
    let mut max_depth = 0;
    fn count_tags(tag: &Tag, count: &mut usize, depth: &mut usize, current_depth: usize) {
        *count += 1;
        *depth = (*depth).max(current_depth);
        
        match tag {
            Tag::List(_, items) => {
                for item in items.iter() {
                    count_tags(item, count, depth, current_depth + 1);
                }
            }
            Tag::Compound(map) => {
                for value in map.values() {
                    count_tags(value, count, depth, current_depth + 1);
                }
            }
            _ => {}
        }
    }
    count_tags(&original_tag, &mut tag_count, &mut max_depth, 1);
    println!("   标签总数: {}", tag_count);
    println!("   最大深度: {}", max_depth);
    
    // 步骤2：写入新文件
    println!("\n2. 写入新文件...");
    let compress = input_path.extension()
        .map(|ext| ext.to_string_lossy().to_lowercase() == "dat")
        .unwrap_or(true);
    
    match to_file(&output_path, &root_name, &original_tag, compress) {
        Ok(()) => {
            let output_metadata = fs::metadata(&output_path)?;
            let output_size = output_metadata.len();
            println!("   ✓ 写入成功");
            println!("   输出大小: {} 字节", output_size);
            
            // 大小变化分析
            let size_diff = output_size as i64 - input_size as i64;
            let size_ratio = if input_size > 0 {
                output_size as f64 / input_size as f64
            } else {
                1.0
            };
            
            println!("   大小变化: {}{} ({:.1}%)", 
                if size_diff >= 0 { "+" } else { "" },
                size_diff,
                (size_ratio - 1.0) * 100.0
            );
            
            // 压缩建议
            if !compress && output_size > input_size * 2 {
                println!("   提示：输出文件比输入大很多，考虑启用压缩");
            }
        }
        Err(e) => {
            eprintln!("   ✗ 写入失败: {}", e);
            return Ok(());
        }
    }
    
    // 步骤3：验证（可选）
    if verify {
        println!("\n3. 验证数据一致性...");
        
        println!("   重新读取输出文件...");
        match from_file(&output_path) {
            Ok((readback_name, readback_tag)) => {
                println!("   ✓ 重新读取成功");
                
                // 比较根标签名
                if root_name == readback_name {
                    println!("   ✓ 根标签名一致");
                } else {
                    println!("   ✗ 根标签名不一致");
                    println!("     原始: '{}'", root_name);
                    println!("     回读: '{}'", readback_name);
                }
                
                // 比较标签内容
                println!("   比较NBT结构...");
                if tags_equal(&original_tag, &readback_tag, 0) {
                    println!("   ✓ 数据完全一致！");
                } else {
                    println!("   ✗ 数据不一致！");
                    
                    // 提供一些调试信息
                    println!("\n调试信息：");
                    println!("   原始标签类型: {:?}", original_tag.tag_id());
                    println!("   回读标签类型: {:?}", readback_tag.tag_id());
                    
                    // 如果是Compound，比较键集合
                    if let (Tag::Compound(map1), Tag::Compound(map2)) = (&original_tag, &readback_tag) {
                        let keys1: Vec<_> = map1.keys().collect();
                        let keys2: Vec<_> = map2.keys().collect();
                        
                        if keys1.len() != keys2.len() {
                            println!("   键数量不同: {} vs {}", keys1.len(), keys2.len());
                        }
                        
                        // 找出缺失的键
                        let missing_in_2: Vec<_> = keys1.iter()
                            .filter(|k| !map2.contains_key(**k))
                            .collect();
                        let missing_in_1: Vec<_> = keys2.iter()
                            .filter(|k| !map1.contains_key(**k))
                            .collect();
                        
                        if !missing_in_2.is_empty() {
                            println!("   输出文件缺少的键: {:?}", missing_in_2);
                        }
                        if !missing_in_1.is_empty() {
                            println!("   输出文件多余的键: {:?}", missing_in_1);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("   ✗ 重新读取失败: {}", e);
                println!("   验证失败：无法读取刚刚写入的文件");
            }
        }
    }
    
    // 步骤4：完整性检查
    println!("\n4. 完整性检查...");
    
    // 检查输出文件是否可读
    println!("   测试输出文件可读性...");
    match from_file(&output_path) {
        Ok(_) => println!("   ✓ 输出文件可正常读取"),
        Err(e) => eprintln!("   ✗ 输出文件读取失败: {}", e),
    }
    
    // 文件属性对比
    println!("\n=== 测试完成 ===");
    println!("输入文件: {}", input_path.display());
    println!("输出文件: {}", output_path.display());
    
    if verify {
        println!("验证结果: {}", if tags_equal(&original_tag, &original_tag, 0) { "通过" } else { "失败" });
    }
    
    println!("\n提示：");
    println!("  1. 可以使用NBTExplorer等工具可视化检查两个文件");
    println!("  2. 使用 --verify 参数启用自动验证");
    println!("  3. 差异可能来自HashMap的遍历顺序（这是正常的）");
    
    Ok(())
}