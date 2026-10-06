use super::{
    offline::offline_uuid,
    util::{java_signed_hex, parse_mojang_uuid},
};

#[test]
fn parses_compact_mojang_uuid() {
    let uuid = parse_mojang_uuid("00112233445566778899aabbccddeeff").unwrap();
    assert_eq!(uuid.to_string(), "00112233-4455-6677-8899-aabbccddeeff");
}

#[test]
fn formats_java_signed_sha1_hash() {
    assert_eq!(java_signed_hex(&[0, 0, 0x0f]), "f");
    assert_eq!(java_signed_hex(&[0xff; 20]), "-1");
    assert_eq!(java_signed_hex(&[0x80, 0, 0]), "-800000");
}

#[test]
fn generates_java_compatible_offline_uuid() {
    assert_eq!(
        offline_uuid("Steve").to_string(),
        "5627dd98-e6be-3c21-b8a8-e92344183641"
    );
}
