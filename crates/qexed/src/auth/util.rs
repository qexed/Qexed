use sha1::{Digest, Sha1};

pub(super) fn parse_mojang_uuid(value: &str) -> anyhow::Result<uuid::Uuid> {
    if value.len() == 32 {
        let hyphenated = format!(
            "{}-{}-{}-{}-{}",
            &value[0..8],
            &value[8..12],
            &value[12..16],
            &value[16..20],
            &value[20..32]
        );
        return Ok(uuid::Uuid::parse_str(&hyphenated)?);
    }

    Ok(uuid::Uuid::parse_str(value)?)
}

pub(super) fn minecraft_server_hash(
    server_id: &str,
    shared_secret: &[u8],
    public_key_der: &[u8],
) -> String {
    let mut hasher = Sha1::new();
    hasher.update(server_id.as_bytes());
    hasher.update(shared_secret);
    hasher.update(public_key_der);

    java_signed_hex(&hasher.finalize())
}

pub(super) fn current_epoch_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or_default()
}

pub(super) fn java_signed_hex(bytes: &[u8]) -> String {
    let negative = bytes.first().is_some_and(|byte| byte & 0x80 != 0);
    let mut value = bytes.to_vec();

    if negative {
        for byte in &mut value {
            *byte = !*byte;
        }

        for byte in value.iter_mut().rev() {
            let (next, overflow) = byte.overflowing_add(1);
            *byte = next;
            if !overflow {
                break;
            }
        }
    }

    let first_non_zero = value
        .iter()
        .position(|byte| *byte != 0)
        .unwrap_or(value.len() - 1);
    let mut hex = hex::encode(&value[first_non_zero..]);
    while hex.starts_with('0') && hex.len() > 1 {
        hex.remove(0);
    }

    if negative { format!("-{hex}") } else { hex }
}
