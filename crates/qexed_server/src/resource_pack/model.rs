#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourcePackOffer {
    pub url: String,
    pub hash: String,
}

#[derive(Clone)]
pub(super) enum ResourcePackState {
    Disabled,
    Url,
    ObjectStorage,
    Local(super::local::LocalResourcePack),
}
