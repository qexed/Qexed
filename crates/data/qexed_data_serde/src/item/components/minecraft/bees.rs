use serde::{Serialize, Deserialize};
use qexed_nbt::Tag;
use std::collections::HashMap;
use std::sync::Arc;
use crate::entity::living_entity::mob::pathfinder_mob::ageable_mob::animal::bee::Bee;

// 使用方案1
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bees {
    #[serde(serialize_with = "serialize_filtered_bee", deserialize_with = "deserialize_filtered_bee")]
    pub entity_data: Option<Bee>,
    pub min_ticks_in_hive: i32,
    pub ticks_in_hive: i32,
}

fn serialize_filtered_bee<S>(bee: &Option<Bee>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let filtered = FilteredBee(bee.clone());
    filtered.serialize(serializer)
}

fn deserialize_filtered_bee<'de, D>(deserializer: D) -> Result<Option<Bee>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let filtered = FilteredBee::deserialize(deserializer)?;
    Ok(filtered.0)
}

#[derive(Debug, Clone)]
struct FilteredBee(Option<Bee>);

impl Serialize for FilteredBee {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match &self.0 {
            Some(bee) => {
                // 使用正确的序列化函数 - 假设 qexed_nbt 提供了 to_nbt_tag
                // 如果实际函数名不同，请根据你的 crate 文档调整
                let tag = qexed_nbt::nbt_serde::nbt_serde::to_tag(&bee)
                    .map_err(serde::ser::Error::custom)?;
                
                // 过滤
                let filtered_tag = filter_bee_tag(tag);
                
                // 将 Tag 转换为可序列化的形式
                // 如果 Tag 实现了 Serialize，直接序列化
                // 否则需要手动处理
                filtered_tag.serialize(serializer)
            }
            None => serializer.serialize_none(),
        }
    }
}

impl<'de> Deserialize<'de> for FilteredBee {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // 直接反序列化为 Bee，而不是 Tag
        let bee: Option<Bee> = Option::deserialize(deserializer)?;
        Ok(FilteredBee(bee))
    }
}

fn filter_bee_tag(tag: Tag) -> Tag {
    if let Tag::Compound(arc_map) = tag {
        // 获取可变的 HashMap
        let map = Arc::try_unwrap(arc_map)
            .unwrap_or_else(|arc| (*arc).clone());
        
        let mut filtered = HashMap::new();
        let remove_fields = [
            "Air", "drop_chances", "equipment", "Brain", "CanPickUpLoot",
            "DeathTime", "fall_distance", "FallFlying", "Fire", "HurtByTimestamp",
            "HurtTime", "LeftHanded", "Motion", "NoGravity", "OnGround",
            "PortalCooldown", "Pos", "Rotation", "sleeping_pos",
            "CannotEnterHiveTicks", "TicksSincePollination", "CropsGrownSincePollination",
            "hive_pos", "Passengers", "leash", "UUID"
        ];
        
        for (key, value) in map.into_iter() {
            if !remove_fields.contains(&key.as_str()) {
                filtered.insert(key, value);
            }
        }
        
        Tag::Compound(Arc::new(filtered))
    } else {
        tag
    }
}