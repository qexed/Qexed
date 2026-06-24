use std::{collections::HashMap, path::PathBuf, sync::Arc};

use anyhow::{Context, Result};
use async_trait::async_trait;
use base64::Engine as _;

use crate::inventory::PlayerInventory;

use super::{
    PlayerDataStore,
    model::{DATA_VERSION, PlayerData, StoredPosition},
};
#[derive(Debug)]
pub(super) struct VanillaPlayerDataStore {
    root: PathBuf,
}

impl VanillaPlayerDataStore {
    pub(super) fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn path(&self, uuid: uuid::Uuid) -> PathBuf {
        self.root.join(format!("{uuid}.dat"))
    }
}

#[async_trait]
impl PlayerDataStore for VanillaPlayerDataStore {
    async fn load(&self, uuid: uuid::Uuid) -> Result<Option<PlayerData>> {
        let path = self.path(uuid);
        if !path.exists() {
            return Ok(None);
        }
        tokio::task::spawn_blocking(move || read_vanilla_player_data(&path, uuid))
            .await
            .context("join vanilla playerdata read")?
    }

    async fn save(&self, data: &PlayerData) -> Result<()> {
        tokio::fs::create_dir_all(&self.root)
            .await
            .with_context(|| {
                format!(
                    "create vanilla playerdata directory: {}",
                    self.root.display()
                )
            })?;
        let path = self.path(data.uuid);
        let data = data.clone();
        tokio::task::spawn_blocking(move || write_vanilla_player_data(&path, &data))
            .await
            .context("join vanilla playerdata write")?
    }
}

pub(super) fn read_vanilla_player_data(
    path: &std::path::Path,
    uuid: uuid::Uuid,
) -> Result<Option<PlayerData>> {
    let (_, tag) = qexed_nbt::from_file(path)
        .with_context(|| format!("read vanilla playerdata: {}", path.display()))?;
    let raw_nbt = serialize_nbt_payload(&tag)?;
    let qexed = compound(&tag)
        .and_then(|root| root.get("qexed"))
        .and_then(compound)
        .and_then(|qexed| qexed.get("payload"))
        .and_then(string_value);
    if let Some(payload) = qexed {
        let mut data: PlayerData =
            serde_json::from_str(payload).context("parse vanilla playerdata qexed payload")?;
        data.raw_nbt = raw_nbt;
        return Ok(Some(data));
    }

    let root = compound(&tag).context("vanilla playerdata root tag is not compound")?;
    let position = position_from_nbt(root)?;
    let dimension = root
        .get("Dimension")
        .and_then(string_value)
        .unwrap_or("minecraft:overworld")
        .to_string();
    let profile_name = root
        .get("bukkit")
        .and_then(compound)
        .and_then(|bukkit| bukkit.get("lastKnownName"))
        .and_then(string_value)
        .unwrap_or_default()
        .to_string();
    Ok(Some(PlayerData {
        data_version: DATA_VERSION,
        uuid,
        profile_name,
        dimension,
        position,
        survival: Default::default(),
        inventory: PlayerInventory::empty().to_stored(),
        raw_nbt,
    }))
}

pub(super) fn write_vanilla_player_data(path: &std::path::Path, data: &PlayerData) -> Result<()> {
    let mut root = data
        .raw_nbt_bytes()?
        .and_then(|bytes| qexed_nbt::from_slice(&bytes).ok().map(|(_, tag)| tag))
        .and_then(|tag| match tag {
            qexed_nbt::Tag::Compound(fields) => Some((*fields).clone()),
            _ => None,
        })
        .unwrap_or_default();
    apply_runtime_fields(&mut root, data)?;
    root.insert(
        "qexed".to_string(),
        qexed_nbt::Tag::Compound(Arc::new(HashMap::from([(
            "payload".to_string(),
            qexed_nbt::Tag::String(Arc::from(serde_json::to_string(&structured_payload(data))?)),
        )]))),
    );

    qexed_nbt::to_file(path, "", &qexed_nbt::Tag::Compound(Arc::new(root)), true)
        .with_context(|| format!("write vanilla playerdata: {}", path.display()))
}

fn apply_runtime_fields(
    root: &mut HashMap<String, qexed_nbt::Tag>,
    data: &PlayerData,
) -> Result<()> {
    root.insert("DataVersion".to_string(), qexed_nbt::Tag::Int(DATA_VERSION));
    root.insert(
        "Dimension".to_string(),
        qexed_nbt::Tag::String(Arc::from(data.dimension.clone())),
    );
    root.insert(
        "Pos".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::DOUBLE,
            vec![
                qexed_nbt::Tag::Double(data.position.x),
                qexed_nbt::Tag::Double(data.position.y),
                qexed_nbt::Tag::Double(data.position.z),
            ],
        )?,
    );
    root.insert(
        "Rotation".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::FLOAT,
            vec![
                qexed_nbt::Tag::Float(data.position.yaw),
                qexed_nbt::Tag::Float(data.position.pitch),
            ],
        )?,
    );
    root.insert(
        "OnGround".to_string(),
        qexed_nbt::Tag::Byte(i8::from(data.position.on_ground)),
    );
    Ok(())
}

pub(super) fn serialize_nbt_payload(tag: &qexed_nbt::Tag) -> Result<String> {
    let tag = raw_playerdata_tag_without_qexed(tag);
    let bytes = qexed_nbt::to_vec("", &tag).context("serialize raw playerdata nbt")?;
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

fn structured_payload(data: &PlayerData) -> PlayerData {
    let mut payload = data.clone();
    payload.raw_nbt.clear();
    payload
}

fn raw_playerdata_tag_without_qexed(tag: &qexed_nbt::Tag) -> qexed_nbt::Tag {
    let qexed_nbt::Tag::Compound(root) = tag else {
        return tag.clone();
    };
    let mut root = (**root).clone();
    root.remove("qexed");
    qexed_nbt::Tag::Compound(Arc::new(root))
}

fn position_from_nbt(root: &HashMap<String, qexed_nbt::Tag>) -> Result<StoredPosition> {
    let pos = root
        .get("Pos")
        .and_then(list)
        .context("vanilla playerdata missing Pos")?;
    let rotation = root
        .get("Rotation")
        .and_then(list)
        .context("vanilla playerdata missing Rotation")?;
    Ok(StoredPosition {
        x: double_at(pos, 0)?,
        y: double_at(pos, 1)?,
        z: double_at(pos, 2)?,
        yaw: float_at(rotation, 0)?,
        pitch: float_at(rotation, 1)?,
        on_ground: root
            .get("OnGround")
            .and_then(byte_value)
            .is_none_or(|value| value != 0),
    })
}

fn compound(tag: &qexed_nbt::Tag) -> Option<&HashMap<String, qexed_nbt::Tag>> {
    match tag {
        qexed_nbt::Tag::Compound(value) => Some(value),
        _ => None,
    }
}

fn list(tag: &qexed_nbt::Tag) -> Option<&[qexed_nbt::Tag]> {
    match tag {
        qexed_nbt::Tag::List(_, values) => Some(values),
        _ => None,
    }
}

fn string_value(tag: &qexed_nbt::Tag) -> Option<&str> {
    match tag {
        qexed_nbt::Tag::String(value) => Some(value),
        _ => None,
    }
}

fn byte_value(tag: &qexed_nbt::Tag) -> Option<i8> {
    match tag {
        qexed_nbt::Tag::Byte(value) => Some(*value),
        _ => None,
    }
}

fn double_at(values: &[qexed_nbt::Tag], index: usize) -> Result<f64> {
    match values.get(index) {
        Some(qexed_nbt::Tag::Double(value)) => Ok(*value),
        Some(other) => anyhow::bail!("Pos[{index}] has wrong type: {:?}", other),
        None => anyhow::bail!("Pos missing index {index}"),
    }
}

fn float_at(values: &[qexed_nbt::Tag], index: usize) -> Result<f32> {
    match values.get(index) {
        Some(qexed_nbt::Tag::Float(value)) => Ok(*value),
        Some(other) => anyhow::bail!("Rotation[{index}] has wrong type: {:?}", other),
        None => anyhow::bail!("Rotation missing index {index}"),
    }
}
