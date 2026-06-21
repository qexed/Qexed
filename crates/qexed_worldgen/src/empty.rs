use std::{collections::HashMap, sync::Arc};

use qexed_nbt::{Tag, tag_id};

use crate::{
    chunk_nbt::{heightmaps_tag, list_tag},
    constants::{DATA_VERSION, SECTION_HEIGHT, WORLD_MIN_Y},
};
pub(crate) fn empty_chunk_root(chunk_x: i32, chunk_z: i32) -> Tag {
    Tag::Compound(Arc::new(HashMap::from([
        ("DataVersion".to_string(), Tag::Int(DATA_VERSION)),
        ("xPos".to_string(), Tag::Int(chunk_x)),
        ("yPos".to_string(), Tag::Int(WORLD_MIN_Y / SECTION_HEIGHT)),
        ("zPos".to_string(), Tag::Int(chunk_z)),
        (
            "Status".to_string(),
            Tag::String(Arc::from("minecraft:full")),
        ),
        (
            "sections".to_string(),
            list_tag(tag_id::COMPOUND, Vec::new()),
        ),
        (
            "Heightmaps".to_string(),
            heightmaps_tag(
                WORLD_MIN_Y,
                &[WORLD_MIN_Y; 16 * 16],
                &[WORLD_MIN_Y; 16 * 16],
            ),
        ),
        (
            "block_entities".to_string(),
            list_tag(tag_id::COMPOUND, Vec::new()),
        ),
    ])))
}
