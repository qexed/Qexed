//! Codec roundtrip tests for the consolidated implementation.
use qexed_nbt::net::NetNbtIo;
use qexed_nbt::named::{from_slice, to_vec};
use qexed_nbt::{Tag, tag_id};
use std::collections::HashMap;
use std::sync::Arc;

fn sample() -> Tag {
    let mut root = HashMap::new();
    root.insert("byte".into(), Tag::Byte(-1));
    root.insert("short".into(), Tag::Short(300));
    root.insert("int".into(), Tag::Int(-70000));
    root.insert("long".into(), Tag::Long(i64::MIN));
    root.insert("float".into(), Tag::Float(1.5));
    root.insert("double".into(), Tag::Double(-2.25));
    root.insert("string".into(), Tag::String(Arc::from("hello nbt")));
    root.insert("bytes".into(), Tag::byte_array_from_u8_slice(&[0, 1, 2, 255]));
    root.insert(
        "list".into(),
        Tag::string_list(vec!["a".into(), "b".into(), "c".into()]).unwrap(),
    );
    let mut nested = HashMap::new();
    nested.insert("inner".into(), Tag::Int(42));
    root.insert("nested".into(), Tag::Compound(Arc::new(nested)));
    Tag::Compound(Arc::new(root))
}

#[test]
fn named_roundtrip() {
    let tag = sample();
    let bytes = to_vec("root", &tag).unwrap();
    let (name, back) = from_slice(&bytes).unwrap();
    assert_eq!(name, "root");
    assert_eq!(back, tag);
}

#[test]
fn network_roundtrip() {
    let tag = sample();
    let mut buf = Vec::new();
    NetNbtIo::to_writer(&mut buf, &tag, false).unwrap();
    let named = to_vec(
        "root", &tag,
    ).unwrap();
    assert_eq!(buf.len() + 2 + 4, named.len());
    let mut cur = std::io::Cursor::new(buf);
    let back = NetNbtIo::from_reader(&mut cur, false).unwrap();
    assert_eq!(back, tag);
}

#[test]
fn network_null_is_end() {
    let mut buf = Vec::new();
    NetNbtIo::to_writer(&mut buf, &Tag::End, false).unwrap();
    assert_eq!(buf, vec![tag_id::END]);
}
