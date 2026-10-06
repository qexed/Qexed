use std::{collections::HashMap, path::PathBuf, sync::Arc};

use base64::Engine as _;

use super::{
    PlayerDataStore,
    model::{DATA_VERSION, PlayerData, StoredPosition},
};
use crate::error::{PlayerError, Result};

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

impl PlayerDataStore for VanillaPlayerDataStore {
    fn load(
        &self,
        uuid: uuid::Uuid,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Option<PlayerData>>> + Send + '_>> {
        let path = self.path(uuid);
        Box::pin(async move {
            if !path.exists() {
                return Ok(None);
            }
            tokio::task::spawn_blocking(move || read_vanilla_player_data(&path, uuid))
                .await
                .map_err(|err| PlayerError::msg(format!("join vanilla playerdata read: {err}")))?
        })
    }

    fn save(
        &self,
        data: &PlayerData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send + '_>> {
        let root = self.root.clone();
        let path = self.path(data.uuid);
        let data = data.clone();
        Box::pin(async move {
            tokio::fs::create_dir_all(&root).await.map_err(|err| {
                PlayerError::msg(format!(
                    "create vanilla playerdata directory: {} ({err})",
                    root.display()
                ))
            })?;
            tokio::task::spawn_blocking(move || write_vanilla_player_data(&path, &data))
                .await
                .map_err(|err| PlayerError::msg(format!("join vanilla playerdata write: {err}")))?
        })
    }
}

pub(super) fn read_vanilla_player_data(
    path: &std::path::Path,
    uuid: uuid::Uuid,
) -> Result<Option<PlayerData>> {
    let (_, tag) = qexed_nbt::from_file(path)
        .map_err(|err| PlayerError::msg(format!("read vanilla playerdata: {} ({err})", path.display())))?;
    let raw_nbt = serialize_nbt_payload(&tag)?;
    let qexed = compound(&tag)
        .and_then(|root| root.get("qexed"))
        .and_then(compound)
        .and_then(|qexed| qexed.get("payload"))
        .and_then(string_value);
    if let Some(payload) = qexed {
        let mut data: PlayerData = serde_json::from_str(payload)
            .map_err(|err| PlayerError::msg(format!("parse vanilla playerdata qexed payload: {err}")))?;
        data.raw_nbt = raw_nbt;
        return Ok(Some(data));
    }

    let root = compound(&tag)
        .ok_or_else(|| PlayerError::msg("vanilla playerdata root tag is not compound"))?;
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
        inventory: Default::default(),
        raw_nbt,
    }))
}

pub(super) fn write_vanilla_player_data(path: &std::path::Path, data: &PlayerData) -> Result<()> {
    let mut root = data
        .raw_nbt_bytes()
        .map_err(|err| PlayerError::msg(format!("decode raw playerdata nbt: {err}")))?
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
            qexed_nbt::Tag::String(Arc::from(
                serde_json::to_string(&structured_payload(data))
                    .map_err(PlayerError::Json)?,
            )),
        )]))),
    );

    qexed_nbt::to_file(path, "", &qexed_nbt::Tag::Compound(Arc::new(root)), true)
        .map_err(|err| PlayerError::msg(format!("write vanilla playerdata: {} ({err})", path.display())))
}

fn apply_runtime_fields(root: &mut HashMap<String, qexed_nbt::Tag>, data: &PlayerData) -> Result<()> {
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
        )
        .map_err(PlayerError::Nbt)?,
    );
    root.insert(
        "Rotation".to_string(),
        qexed_nbt::Tag::new_list(
            qexed_nbt::tag_id::FLOAT,
            vec![
                qexed_nbt::Tag::Float(data.position.yaw),
                qexed_nbt::Tag::Float(data.position.pitch),
            ],
        )
        .map_err(PlayerError::Nbt)?,
    );
    root.insert(
        "OnGround".to_string(),
        qexed_nbt::Tag::Byte(i8::from(data.position.on_ground)),
    );
    Ok(())
}

pub(super) fn serialize_nbt_payload(tag: &qexed_nbt::Tag) -> Result<String> {
    let tag = raw_playerdata_tag_without_qexed(tag);
    let bytes = qexed_nbt::to_vec("", &tag).map_err(PlayerError::Nbt)?;
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
        .ok_or_else(|| PlayerError::msg("vanilla playerdata missing Pos"))?;
    let rotation = root
        .get("Rotation")
        .and_then(list)
        .ok_or_else(|| PlayerError::msg("vanilla playerdata missing Rotation"))?;
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
        Some(other) => Err(PlayerError::msg(format!(
            "Pos[{index}] has wrong type: {other:?}"
        ))),
        None => Err(PlayerError::msg(format!("Pos missing index {index}"))),
    }
}

fn float_at(values: &[qexed_nbt::Tag], index: usize) -> Result<f32> {
    match values.get(index) {
        Some(qexed_nbt::Tag::Float(value)) => Ok(*value),
        Some(other) => Err(PlayerError::msg(format!(
            "Rotation[{index}] has wrong type: {other:?}"
        ))),
        None => Err(PlayerError::msg(format!("Rotation missing index {index}"))),
    }
}