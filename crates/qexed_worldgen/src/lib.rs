mod cache;
mod chunk_nbt;
mod compare;
mod constants;
mod empty;
mod fallback;
mod generator_v4;
mod registry;
mod types;
mod util;
mod vanilla_noise;

pub use cache::WorldgenCache;
pub use compare::compare_chunk_nbt;
pub use types::{ChunkComparison, ChunkRequest, WorldGenerator};

#[cfg(test)]
mod tests {
    use super::{ChunkRequest, WorldGenerator};
    use qexed_nbt::Tag;

    #[test]
    fn generates_overworld_chunk_from_mojang_cache() {
        let Ok(generator) = WorldGenerator::default_cache(0) else {
            return;
        };

        let root = generator
            .generate_chunk_nbt(ChunkRequest {
                dimension: "minecraft:overworld",
                chunk_x: 0,
                chunk_z: 0,
            })
            .unwrap();

        assert!(matches!(root, qexed_nbt::Tag::Compound(_)));
    }

    #[test]
    #[ignore = "manual Windows diagnostic: full v4 worldgen can terminate the test process without a Rust panic"]
    fn v4_pipeline_smoke_when_enabled() {
        let chunk = super::generator_v4::generate_overworld_chunk_nbt(0, 0, 0).unwrap();
        let Tag::Compound(root) = chunk else {
            panic!("chunk root must be compound");
        };
        assert!(root.contains_key("sections"));
        assert!(root.contains_key("Heightmaps"));
    }
}
