use std::{collections::HashMap, sync::Arc};

use bytes::BytesMut;
use qexed_nbt::{Tag, tag_id};
use qexed_packet::{
    PacketCodec, PacketReader, PacketWriter,
    net_types::{
        ByteArray, GameProfile, JsonValue, OptionalVarInt, ProfileProperty, VarInt, VarLong,
    },
};

fn encode<T: PacketCodec>(value: &T) -> Vec<u8> {
    let mut buf = BytesMut::new();
    let mut writer = PacketWriter::new(&mut buf);
    value.serialize(&mut writer).unwrap();
    buf.to_vec()
}

fn decode<T: PacketCodec>(bytes: &[u8]) -> T {
    let mut bytes = bytes::Bytes::copy_from_slice(bytes);
    let mut reader = PacketReader::new(&mut bytes);
    reader.deserialize().unwrap()
}

fn decode_result<T: PacketCodec>(bytes: &[u8]) -> anyhow::Result<T> {
    let mut bytes = bytes::Bytes::copy_from_slice(bytes);
    let mut reader = PacketReader::new(&mut bytes);
    reader.deserialize()
}

#[test]
fn json_value_decodes_as_json_instead_of_string_literal() {
    let value: serde_json::Value = decode(&[7, b'{', b'"', b'a', b'"', b':', b'1', b'}']);

    assert_eq!(value["a"], 1);
}

#[test]
fn var_long_uses_minecraft_two_complement_varint_encoding() {
    assert_eq!(encode(&VarLong(0)), vec![0x00]);
    assert_eq!(encode(&VarLong(1)), vec![0x01]);
    assert_eq!(
        encode(&VarLong(-1)),
        vec![0xff; 9].into_iter().chain([0x01]).collect::<Vec<_>>()
    );

    let decoded: VarLong = decode(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]);
    assert_eq!(decoded, VarLong(-1));
}

#[test]
fn byte_array_uses_varint_length_prefix() {
    let value = ByteArray(vec![1, 2, 3]);

    assert_eq!(encode(&value), vec![3, 1, 2, 3]);
    assert_eq!(decode::<ByteArray>(&[3, 1, 2, 3]), value);
}

#[test]
fn optional_varint_offsets_present_values_by_one() {
    assert_eq!(encode(&OptionalVarInt(None)), vec![0]);
    assert_eq!(encode(&OptionalVarInt(Some(VarInt(36)))), vec![37]);
    assert_eq!(
        decode::<OptionalVarInt>(&[37]),
        OptionalVarInt(Some(VarInt(36)))
    );
}

#[test]
fn game_profile_round_trips_properties() {
    let uuid = uuid::Uuid::from_u128(0x00112233445566778899aabbccddeeff);
    let profile = GameProfile {
        uuid,
        username: "Steve".to_string(),
        properties: vec![ProfileProperty {
            name: "textures".to_string(),
            value: "value".to_string(),
            signature: Some("signature".to_string()),
        }],
    };

    let decoded: GameProfile = decode(&encode(&profile));

    assert_eq!(decoded, profile);
}

#[test]
fn nbt_codec_accepts_non_compound_root_tags() {
    let value = Tag::String(Arc::from("hello"));

    assert_eq!(
        encode(&value),
        vec![tag_id::STRING, 0, 5, b'h', b'e', b'l', b'l', b'o']
    );
    assert_eq!(decode::<Tag>(&encode(&value)), value);
}

#[test]
fn nbt_codec_round_trips_compound_tags() {
    let value = Tag::Compound(Arc::new(HashMap::from([
        ("name".to_string(), Tag::String(Arc::from("Alex"))),
        ("level".to_string(), Tag::Int(42)),
    ])));

    assert_eq!(decode::<Tag>(&encode(&value)), value);
}

#[test]
fn nbt_byte_array_round_trips_signed_bytes() {
    let value = Tag::ByteArray(Arc::from([0_i8, 1, -1, i8::MIN, i8::MAX]));

    assert_eq!(decode::<Tag>(&encode(&value)), value);
}

#[test]
fn nbt_array_lengths_must_fit_remaining_payload() {
    assert!(decode_result::<Tag>(&[tag_id::INT_ARRAY, 0, 0, 0, 1]).is_err());
    assert!(decode_result::<Tag>(&[tag_id::LONG_ARRAY, 0, 0, 0, 1]).is_err());
}

#[test]
fn json_value_wrapper_uses_same_codec() {
    let value = JsonValue(serde_json::json!({ "text": "ok" }));

    assert_eq!(decode::<JsonValue>(&encode(&value)), value);
}

#[test]
fn fixed_byte_array_round_trips_as_raw_bytes() {
    let value = [1u8, 2, 3, 4];

    assert_eq!(encode(&value), vec![1, 2, 3, 4]);
    assert_eq!(decode::<[u8; 4]>(&[1, 2, 3, 4]), value);
}
