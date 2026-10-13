//! qexed_command 集成测试：树构建、协议扁平化、解析分发、错误路径。

use bytes::BytesMut;
use qexed_command::{argument, literal, tokenize, ArgValue, CommandError, CommandTree};
use qexed_packet::{net_types::VarInt, PacketCodec, PacketReader, PacketWriter};
use qexed_protocol::to_client::play::commands::{
    BrigadierString, CommandArgumentType as Parser, MinecraftEntity, Varies,
};

/// 搭一棵覆盖全部特性的测试树：
/// - announce（可执行 literal，无参）
/// - msg <target:entity> <message:greedy>（msg 自身可执行做 usage 兜底）
/// - warp <name:word> [confirm:bool]
/// - heal <amount:int>
fn demo_tree() -> CommandTree {
    CommandTree::new()
        .command(literal("announce").executes(|_| Ok(())))
        .command(
            literal("msg")
                .child(
                    argument("target", Parser::MinecraftEntity(MinecraftEntity { flags: 1 }), false)
                        .child(
                            argument("message", Parser::BrigadierString(BrigadierString { behavior: VarInt(0) }), true)
                                .executes(|_| Ok(())),
                        ),
                )
                .executes(|_| Ok(())),
        )
        .command(
            literal("warp").child(
                argument("name", Parser::BrigadierString(BrigadierString { behavior: VarInt(0) }), false)
                    .child(argument("confirm", Parser::BrigadierBool, false).executes(|_| Ok(())))
                    .executes(|_| Ok(())),
            ),
        )
        .command(
            literal("heal").child(
                argument("amount", Parser::BrigadierInteger(Default::default()), false)
                    .executes(|_| Ok(())),
            ),
        )
}

#[test]
fn packet_shape_postorder_and_root_index() {
    let packet = demo_tree().to_packet();
    assert_eq!(packet.root_index, VarInt(0));

    // 节点 0 恒为根：flags=0，children 指向 4 条顶层 literal
    let root = &packet.nodes[0];
    assert_eq!(root.flags, 0);
    assert_eq!(root.children.len(), 4);
    assert!(root.name.is_none());

    // root 在 0；其余节点后序排列：父索引大于全部子孙索引（root 允许前向引用）
    for (idx, node) in packet.nodes.iter().enumerate().skip(1) {
        for child in &node.children {
            assert!((child.0 as usize) < idx, "child {child:?} of {idx} must precede");
        }
    }
    // root children 指向 4 条顶层 literal：announce(1) msg(4) warp(7) heal(9)
    assert_eq!(root.children, vec![VarInt(1), VarInt(4), VarInt(7), VarInt(9)]);

    // msg：自身可执行 literal -> flags 0x05
    let msg_idx = root.children[1].0 as usize;
    let msg = &packet.nodes[msg_idx];
    assert_eq!(msg.flags, 0x05);
    assert_eq!(msg.name.as_deref(), Some("msg"));
}

#[test]
fn packet_wire_roundtrip() {
    let packet = demo_tree().to_packet();
    let mut buf = BytesMut::new();
    {
        let mut w = PacketWriter::new(&mut buf);
        // Commands 无整体 PacketCodec，按包体顺序逐字段写：nodes -> root_index
        packet.nodes.serialize(&mut w).expect("serialize nodes");
        packet.root_index.serialize(&mut w).expect("serialize root_index");
    }
    let original = buf.clone();
    let mut reader = PacketReader::new(&mut buf);
    let nodes: Vec<qexed_protocol::to_client::play::commands::Node> =
        reader.deserialize().expect("deserialize nodes");
    let root_index: VarInt = reader.deserialize().expect("deserialize root_index");
    assert_eq!(root_index, packet.root_index);
    assert_eq!(nodes.len(), packet.nodes.len());
    // 反序列化会为省略的默认属性填回协议默认值（如 entity flags=0），逐字段核对：
    for (raw, orig) in nodes.iter().zip(&packet.nodes) {
        assert_eq!(raw.flags, orig.flags);
        assert_eq!(raw.children, orig.children);
        assert_eq!(raw.name, orig.name);
        assert_eq!(raw.parser_id, orig.parser_id);
        assert_eq!(raw.suggestions_type, orig.suggestions_type);
        if orig.properties.is_some() {
            assert_eq!(raw.properties, orig.properties);
        }
    }
    // 重新序列化必须逐字节一致
    let mut buf2 = BytesMut::new();
    {
        let mut w = PacketWriter::new(&mut buf2);
        nodes.serialize(&mut w).expect("re-serialize nodes");
        root_index.serialize(&mut w).expect("re-serialize root_index");
    }
    assert_eq!(buf2, original, "重序列化必须与原始字节一致");
}

#[test]
fn executable_flags() {
    // announce：literal + executor -> 0x05
    let tree = CommandTree::new().command(literal("announce").executes(|_| Ok(())));
    let packet = tree.to_packet();
    assert_eq!(packet.nodes[1].flags, 0x05);

    // heal <amount:int>：argument + executor -> 0x06
    let packet = demo_tree().to_packet();
    let heal_idx = packet.nodes[0].children[3].0 as usize; // heal literal
    let heal_amount_idx = packet.nodes[heal_idx].children[0].0 as usize;
    let amount = &packet.nodes[heal_amount_idx];
    assert_eq!(amount.flags, 0x06);
    assert_eq!(amount.parser_id, Some(VarInt(3)), "brigadier:integer");
}

#[test]
fn greedy_argument_wire_format() {
    // greedy 参数必须强制 brigadier:string + GREEDY_PHRASE(2)
    let tree = CommandTree::new().command(
        literal("say").child(
            argument("text", Parser::BrigadierString(Default::default()), true)
                .executes(|_| Ok(())),
        ),
    );
    let packet = tree.to_packet();
    let arg = &packet.nodes[1]; // 后序：参数(1) -> say(2) -> root(0)
    assert_eq!(arg.parser_id, Some(VarInt(5)));
    match &arg.properties {
        Some(Varies::BrigadierString(s)) => assert_eq!(s.behavior, VarInt(2)),
        other => panic!("greedy 属性错误: {other:?}"),
    }
    assert_eq!(arg.flags, 0x06, "argument + executable");
}

#[test]
fn dispatch_deep_executor_hits_inner_node() {
    // 回归：带参数链的指令命中深层节点时必须执行深层回调
    let mut tree = demo_tree();
    // warp home（停在 name 节点自身 executor）
    tree.dispatch("warp home").expect("warp <name>");
    // warp home true（停在 confirm 子节点）
    tree.dispatch("warp home true").expect("warp <name> <confirm>");
}

#[test]
fn dispatch_args_parsed() {
    use std::sync::{Arc, Mutex};
    let hits: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let h1 = hits.clone();
    let h2 = hits.clone();
    let mut tree = CommandTree::new()
        .command(literal("announce").executes(move |ctx| {
            h1.lock().unwrap().push(ctx.name.clone());
            Ok(())
        }))
        .command(
            literal("heal").child(
                argument("amount", Parser::BrigadierInteger(Default::default()), false)
                    .executes(move |ctx| {
                        h2.lock().unwrap()
                            .push(format!("heal:{}", ctx.args.int("amount").unwrap()));
                        Ok(())
                    }),
            ),
        );
    tree.dispatch("announce").expect("exact literal");
    tree.dispatch("heal 42").expect("int arg");
    assert_eq!(
        *hits.lock().unwrap(),
        vec!["announce".to_string(), "heal:42".to_string()]
    );
}

#[test]
fn dispatch_greedy_keeps_spaces() {
    use std::sync::{Arc, Mutex};
    let captured: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    let mut tree = CommandTree::new().command(
        literal("say").child(
            argument("text", Parser::BrigadierString(Default::default()), true)
                .executes(move |ctx| {
                    *cap.lock().unwrap() = Some(ctx.args.text("text").unwrap().to_string());
                    Ok(())
                }),
        ),
    );
    // 引号包裹 + 转义引号：还原后应含字面引号与连续空格
    tree.dispatch(r#"say "hello  world \"quote\"""#).expect("greedy");
    let got = captured.lock().unwrap().clone().unwrap();
    assert_eq!(got, r#"hello  world "quote""#);
}

#[test]
fn dispatch_bad_bool_is_bad_argument() {
    let mut tree = demo_tree();
    let err = tree.dispatch("warp home yes").unwrap_err();
    match err {
        CommandError::BadArgument { name, expected, .. } => {
            assert_eq!(name, "confirm");
            assert_eq!(expected, "true|false");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn unknown_command_and_usage() {
    let mut tree = demo_tree();
    let err = tree.dispatch("nosuch").unwrap_err();
    assert_eq!(err, CommandError::UnknownCommand("nosuch".to_string()));
    assert_eq!(err.to_string(), "Unknown command: /nosuch");

    // heal 缺参数 -> InvalidUsage
    let err = tree.dispatch("heal").unwrap_err();
    assert!(matches!(err, CommandError::InvalidUsage { .. }), "got {err:?}");

    // msg 只带 target（target 自身不可执行）-> InvalidUsage
    let err = tree.dispatch("msg Steve").unwrap_err();
    assert!(matches!(err, CommandError::InvalidUsage { .. }), "got {err:?}");

    // msg 带全部参数 -> 命中 greedy message
    tree.dispatch("msg Steve hi there").expect("full msg");

    // 空输入
    let err = tree.dispatch("").unwrap_err();
    assert_eq!(err, CommandError::UnknownCommand(String::new()));
}

#[test]
fn int_overflow_is_bad_argument() {
    let mut tree = demo_tree();
    let err = tree.dispatch("heal 99999999999999").unwrap_err();
    match err {
        CommandError::BadArgument { name, expected, .. } => {
            assert_eq!(name, "amount");
            assert_eq!(expected, "i32");
        }
        other => panic!("unexpected: {other:?}"),
    }
}

#[test]
fn usage_lines() {
    let tree = demo_tree();
    let warp = tree.lookup("warp").expect("warp registered");
    let usage = CommandTree::usage_of(warp);
    assert!(
        usage.contains("warp <name>") && usage.contains("warp <name> <confirm>"),
        "usage: {usage}"
    );

    let heal = tree.lookup("heal").unwrap();
    assert_eq!(CommandTree::usage_of(heal), "heal <amount>");
}

#[test]
fn tokenize_basic_and_quotes() {
    assert_eq!(tokenize("heal  42  "), vec![
        "heal".to_string(),
        "42".to_string()
    ]);
    assert_eq!(tokenize(r#"say "a b" c"#), vec![
        "say".to_string(),
        "a b".to_string(),
        "c".to_string()
    ]);
    assert!(tokenize("   ").is_empty());
}

#[test]
fn arg_value_views() {
    assert_eq!(ArgValue::Int(7).as_i64(), Some(7));
    assert_eq!(ArgValue::Long(7).as_i64(), Some(7));
    assert_eq!(ArgValue::Float(1.5).as_f64(), Some(1.5));
    assert_eq!(ArgValue::Bool(true).as_bool(), Some(true));
    assert_eq!(ArgValue::Text("x".into()).as_text(), Some("x"));
    assert_eq!(ArgValue::Bool(true).as_i64(), None);
}