//! FML 握手编解码往返与状态机测试。

use qexed_forge::handshake::{
    self, HandshakeMessage, HandshakeState, ModEntry, ModList, ServerHandshake,
};

fn sample_list() -> ModList {
    ModList::new(vec![
        ModEntry { mod_id: "minecraft".into(), version: "1.20.1".into() },
        ModEntry { mod_id: "forge".into(), version: "47.2.0".into() },
        ModEntry { mod_id: "jei".into(), version: "15.2.0.27".into() },
    ])
}

#[test]
fn mod_list_roundtrip() {
    let list = sample_list();
    let bytes = handshake::encode_server_mod_list(&list);
    let decoded = handshake::decode_handshake(&bytes).unwrap();
    match decoded {
        HandshakeMessage::ServerModList(back) => assert_eq!(back, list),
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn server_hello_roundtrip() {
    let bytes = handshake::encode_server_hello(handshake::FML_NETWORK_VERSION);
    match handshake::decode_handshake(&bytes).unwrap() {
        HandshakeMessage::ServerHello { fml_version } => {
            assert_eq!(fml_version, handshake::FML_NETWORK_VERSION);
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn client_mod_list_decodes() {
    // 手工构造客户端方向（discriminator 0x01）
    use bytes::BufMut as _;
    let mut buf = bytes::BytesMut::new();
    buf.put_u8(0x01);
    let list = sample_list();
    let encoded = handshake::encode_server_mod_list(&list);
    // 借用 server 编码再改 discriminator
    let mut client_bytes = Vec::with_capacity(encoded.len());
    client_bytes.push(0x01);
    client_bytes.extend_from_slice(&encoded[1..]);
    match handshake::decode_handshake(&client_bytes).unwrap() {
        HandshakeMessage::ClientModList(back) => assert_eq!(back, list),
        other => panic!("wrong variant: {other:?}"),
    }
    let _ = buf;
}

#[test]
fn ack_roundtrip() {
    let bytes = handshake::encode_ack();
    assert_eq!(handshake::decode_handshake(&bytes).unwrap(), HandshakeMessage::Ack);
}

#[test]
fn unknown_discriminator_is_forward_compatible() {
    let bytes = vec![0x7F, 0xDE, 0xAD];
    match handshake::decode_handshake(&bytes).unwrap() {
        HandshakeMessage::Unknown { discriminator, payload } => {
            assert_eq!(discriminator, 0x7F);
            assert_eq!(payload, vec![0xDE, 0xAD]);
        }
        other => panic!("wrong variant: {other:?}"),
    }
}

#[test]
fn compatibility_check() {
    let client = sample_list();
    let server_needs = ModList::new(vec![
        ModEntry { mod_id: "minecraft".into(), version: "1.20.1".into() },
    ]);
    assert!(client.check_compatibility(&server_needs).is_ok());

    let strict_server = ModList::new(vec![
        ModEntry { mod_id: "somemod".into(), version: "1.0".into() },
    ]);
    let missing = client.check_compatibility(&strict_server).unwrap_err();
    assert_eq!(missing, vec!["somemod".to_string()]);
}

#[test]
fn server_handshake_state_machine() {
    let server_mods = sample_list();
    let mut hs = ServerHandshake::new(server_mods.clone());
    assert_eq!(hs.state(), HandshakeState::AwaitingModList);

    // 客户端回 mod 列表 -> 服务器应答 ServerModList
    let mut client_bytes = Vec::new();
    client_bytes.push(0x01);
    let enc = handshake::encode_server_mod_list(&sample_list());
    client_bytes.extend_from_slice(&enc[1..]);
    let reply = hs.on_client_response(&client_bytes).unwrap().expect("reply");
    match handshake::decode_handshake(&reply).unwrap() {
        HandshakeMessage::ServerModList(list) => assert_eq!(list, server_mods),
        other => panic!("wrong variant: {other:?}"),
    }
    assert_eq!(hs.state(), HandshakeState::AwaitingClientAck);
    assert_eq!(hs.client_mods().unwrap().entries.len(), 3);

    // 客户端 Ack -> 完成
    let done = hs.on_client_response(&handshake::encode_ack()).unwrap();
    assert!(done.is_none());
    assert_eq!(hs.state(), HandshakeState::Complete);
}

#[test]
fn forge_login_channel_roundtrip() {
    use qexed_forge::channels::ForgeLoginMessage;
    let ready = ForgeLoginMessage::Ready {
        channels: vec!["forge:handshake".into(), "mymod:mainscreen".into()],
    };
    let bytes = ready.encode();
    let back = ForgeLoginMessage::decode(&bytes).unwrap();
    assert_eq!(back, ready);

    let ack = ForgeLoginMessage::Ack;
    assert_eq!(ForgeLoginMessage::decode(&ack.encode()).unwrap(), ack);
}
