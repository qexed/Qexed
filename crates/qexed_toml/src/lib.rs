
mod merge;
mod document;
mod file;
mod error;
mod secrets;

pub use error::TomlError;
pub use document::from_document;
pub use document::to_document;
pub use file::create_file;
pub use file::has_file;
pub use file::load_file;
pub use file::save_file;
pub use merge::merge;
pub use secrets::merge_secrets;
pub use secrets::split_secrets;