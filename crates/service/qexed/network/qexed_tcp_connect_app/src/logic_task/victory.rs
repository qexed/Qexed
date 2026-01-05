use hmac::Mac;
use qexed_packet::{PacketCodec, PacketReader};
use thiserror::Error;

// 代理来源:pumpkin，MIT许可证
#[must_use]
pub fn check_integrity(data: (&[u8], &[u8]), secret: &str) -> bool {
    let (signature, data_without_signature) = data;
    let mut mac = hmac::Hmac::<sha2::Sha256>::new_from_slice(secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data_without_signature);
    mac.verify_slice(signature).is_ok()
}
#[derive(Error, Debug)]
pub enum VelocityError {
    #[error("No response data received")]
    NoData,
    #[error("Unable to verify player details")]
    FailedVerifyIntegrity,
    #[error("Failed to read forward version")]
    FailedReadForwardVersion,
    #[error("Unsupported forwarding version {0}. Maximum supported version is {1}")]
    UnsupportedForwardVersion(u8, u8),
    #[error("Failed to read address")]
    FailedReadAddress,
    #[error("Failed to parse address")]
    FailedParseAddress,
    #[error("Failed to read game profile name")]
    FailedReadProfileName,
    #[error("Failed to read game profile UUID")]
    FailedReadProfileUUID,
    #[error("Failed to read game profile properties")]
    FailedReadProfileProperties,
}
