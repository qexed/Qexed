use bytes::{Bytes, BytesMut};
use qexed_protocol::to_client::status::ping::Ping;
use qexed_tcp_connect::{FramePart, PacketReadError, PacketSend, PacketSink, PacketStream, bind};
use tokio::io::duplex;
use tokio::net::TcpStream;
use tokio_stream::StreamExt;

async fn roundtrip(
    payload: &[u8],
    compression_threshold: Option<i32>,
    encryption_key: Option<&[u8]>,
) -> Result<BytesMut, PacketReadError> {
    let (client, server) = duplex(64 * 1024);
    let mut writer = PacketSink::new(client);
    let mut reader = PacketStream::new(server);

    if let Some(threshold) = compression_threshold {
        writer.set_compression_threshold(threshold);
        reader.set_compression_threshold(threshold);
    }

    if let Some(key) = encryption_key {
        writer.enable_encryption(key).unwrap();
        reader.enable_encryption(key)?;
    }

    writer.send_raw(payload).await.unwrap();
    writer.shutdown().await.unwrap();

    reader.next().await.unwrap()
}

#[tokio::test]
async fn reads_plain_packet() {
    let payload = b"\x01hello";
    let packet = roundtrip(payload, None, None).await.unwrap();
    assert_eq!(&packet[..], payload);
}

#[tokio::test]
async fn sends_packet_by_reference() {
    let (client, server) = duplex(64 * 1024);
    let mut writer = PacketSink::new(client);
    let mut reader = PacketStream::new(server);

    writer.send_ref(&Ping { time: 42 }).await.unwrap();
    writer.shutdown().await.unwrap();

    let packet = reader.next().await.unwrap().unwrap();
    assert_eq!(&packet[..], &[1, 0, 0, 0, 0, 0, 0, 0, 42]);
}

#[tokio::test]
async fn appends_multiple_packet_frames() {
    let (client, server) = duplex(64 * 1024);
    let mut writer = PacketSink::new(client);
    let mut reader = PacketStream::new(server);
    let mut frames = BytesMut::new();

    writer
        .append_packet_frame_ref(&Ping { time: 1 }, &mut frames)
        .unwrap();
    writer
        .append_packet_frame_ref(&Ping { time: 2 }, &mut frames)
        .unwrap();
    writer.send_encoded_frame(frames).await.unwrap();
    writer.shutdown().await.unwrap();

    let first = reader.next().await.unwrap().unwrap();
    let second = reader.next().await.unwrap().unwrap();
    assert_eq!(&first[..], &[1, 0, 0, 0, 0, 0, 0, 0, 1]);
    assert_eq!(&second[..], &[1, 0, 0, 0, 0, 0, 0, 0, 2]);
}

#[tokio::test]
async fn sends_encoded_frame_parts_vectored() {
    static BODY: std::sync::OnceLock<Bytes> = std::sync::OnceLock::new();

    let (client, server) = duplex(64 * 1024);
    let mut writer = PacketSink::new(client);
    let mut reader = PacketStream::new(server);
    let body = BODY.get_or_init(|| Bytes::from_static(b"hello"));

    writer
        .send_encoded_frames_vectored(&[
            FramePart::Bytes(Bytes::from_static(b"\x06\x01")),
            FramePart::Shared(body),
        ])
        .await
        .unwrap();
    writer.shutdown().await.unwrap();

    let packet = reader.next().await.unwrap().unwrap();
    assert_eq!(&packet[..], b"\x01hello");
}

#[tokio::test]
async fn reads_compression_enabled_uncompressed_packet() {
    let payload = b"\x02small";
    let packet = roundtrip(payload, Some(128), None).await.unwrap();
    assert_eq!(&packet[..], payload);
}

#[tokio::test]
async fn reads_compressed_packet() {
    let payload = vec![0x03; 512];
    let packet = roundtrip(&payload, Some(32), None).await.unwrap();
    assert_eq!(&packet[..], payload.as_slice());
}

#[tokio::test]
async fn reads_encrypted_packet() {
    let payload = b"\x04encrypted";
    let key = b"0123456789abcdef";
    let packet = roundtrip(payload, None, Some(key)).await.unwrap();
    assert_eq!(&packet[..], payload);
}

#[tokio::test]
async fn reads_encrypted_compressed_packet() {
    let payload = vec![0x05; 1024];
    let key = b"0123456789abcdef";
    let packet = roundtrip(&payload, Some(16), Some(key)).await.unwrap();
    assert_eq!(&packet[..], payload.as_slice());
}

#[tokio::test]
async fn reports_incomplete_packet_on_eof() {
    let (mut client, server) = duplex(1024);
    let mut reader = PacketStream::new(server);

    tokio::io::AsyncWriteExt::write_all(&mut client, &[3, 1])
        .await
        .unwrap();
    drop(client);

    let err = reader.next().await.unwrap().unwrap_err();
    assert!(matches!(
        err,
        PacketReadError::ConnectionClosedWithIncompletePacket
    ));
}

#[tokio::test]
async fn legacy_packet_send_constructor_still_writes_packets() {
    let listener = bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let client = TcpStream::connect(addr);
    let server = listener.accept();
    let (client, server) = tokio::join!(client, server);
    let client = client.unwrap();
    let (server, _addr) = server.unwrap();

    let (_client_read, client_write) = tokio::io::split(client);
    let (server_read, _server_write) = tokio::io::split(server);
    let mut writer = PacketSend::new(client_write, 16);
    let mut reader = PacketStream::new(server_read);
    reader.set_compression_threshold(16);

    let payload = vec![0x06; 256];
    writer.set_compression(true);
    writer.send_raw(&payload).await.unwrap();
    writer.shutdown().await.unwrap();

    let packet = reader.next().await.unwrap().unwrap();
    assert_eq!(&packet[..], payload.as_slice());
}
