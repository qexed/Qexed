use std::path::Path;

use anyhow::Result;

pub fn install(archive: impl AsRef<Path>, target: impl AsRef<Path>) -> Result<()> {
    crate::archive::unpack(archive, target)
}

pub fn create_package(source: impl AsRef<Path>, output: impl AsRef<Path>) -> Result<()> {
    crate::archive::pack_dir(source, output)
}
