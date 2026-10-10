pub fn offline_profile(username: &str) -> qexed_packet::net_types::GameProfile {
    qexed_packet::net_types::GameProfile {
        uuid: offline_uuid(username),
        username: username.to_string(),
        properties: Vec::new(),
    }
}

pub(super) fn offline_uuid(username: &str) -> uuid::Uuid {
    let digest = openssl::hash::hash(
        openssl::hash::MessageDigest::md5(),
        format!("OfflinePlayer:{username}").as_bytes(),
    )
    .expect("MD5 digest should be available");

    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(digest.as_ref());
    bytes[6] = (bytes[6] & 0x0f) | 0x30;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    uuid::Uuid::from_bytes(bytes)
}
