use std::io::Write as _;
use std::time::Instant;

use qexed_worldgen::{ChunkRequest, WorldGenerator};

fn main() -> anyhow::Result<()> {
    let seed = env_i64("QEXED_DIAG_SEED", 0);
    let chunk_x = env_i32("QEXED_DIAG_CHUNK_X", 0);
    let chunk_z = env_i32("QEXED_DIAG_CHUNK_Z", 0);
    let dimension =
        std::env::var("QEXED_DIAG_DIMENSION").unwrap_or_else(|_| "minecraft:overworld".to_string());

    let total_started = Instant::now();
    println!(
        "direct_worldgen_diag_stage=total start seed={seed} chunk=({chunk_x},{chunk_z}) dimension={dimension}"
    );
    flush_stdout();

    let generator_started = Instant::now();
    println!("direct_worldgen_diag_stage=generator_init start");
    flush_stdout();
    let generator = WorldGenerator::default_cache(seed)?;
    print_done("generator_init", generator_started);

    let generate_started = Instant::now();
    println!("direct_worldgen_diag_stage=generate_chunk_nbt start");
    flush_stdout();
    let root = generator.generate_chunk_nbt(ChunkRequest {
        dimension: &dimension,
        chunk_x,
        chunk_z,
    })?;
    print_done("generate_chunk_nbt", generate_started);

    let encode_started = Instant::now();
    println!("direct_worldgen_diag_stage=nbt_encode start");
    flush_stdout();
    let raw = qexed_nbt::to_vec("", &root)?;
    let chunk = qexed_world::region::ChunkData::zlib(&raw)?;
    println!(
        "direct_worldgen_diag_stage=nbt_encode bytes={} compressed_bytes={}",
        raw.len(),
        chunk.data.len()
    );
    print_done("nbt_encode", encode_started);

    let save_started = Instant::now();
    println!("direct_worldgen_diag_stage=region_save start");
    flush_stdout();
    let temp = tempfile::tempdir()?;
    let path = temp.path().join("region").join("r.0.0.mca");
    let mut region = qexed_world::region::AnvilRegion::new(&path);
    region.write_chunk(chunk_x, chunk_z, chunk)?;
    region.save()?;
    print_done("region_save", save_started);

    println!(
        "direct_worldgen_diag_stage=total done elapsed_ms={:.2} region_path={}",
        total_started.elapsed().as_secs_f64() * 1000.0,
        path.display()
    );
    flush_stdout();
    Ok(())
}

fn env_i64(name: &str, default: i64) -> i64 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn env_i32(name: &str, default: i32) -> i32 {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn print_done(stage: &str, started: Instant) {
    println!(
        "direct_worldgen_diag_stage={stage} done elapsed_ms={:.2}",
        started.elapsed().as_secs_f64() * 1000.0
    );
    flush_stdout();
}

fn flush_stdout() {
    let _ = std::io::stdout().flush();
}
