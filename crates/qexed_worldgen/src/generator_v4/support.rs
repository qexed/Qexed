fn is_stone_ore_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:stone" | "minecraft:granite" | "minecraft:diorite" | "minecraft:andesite"
    )
}

fn is_deepslate_ore_replaceable(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:deepslate" | "minecraft:tuff"
    )
}

fn is_base_stone_overworld(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:stone"
            | "minecraft:granite"
            | "minecraft:diorite"
            | "minecraft:andesite"
            | "minecraft:tuff"
            | "minecraft:deepslate"
    )
}

fn is_substrate_overworld_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:muddy_mangrove_roots"
            | "minecraft:moss_block"
            | "minecraft:pale_moss_block"
            | "minecraft:grass_block"
            | "minecraft:podzol"
            | "minecraft:mycelium"
    )
}

fn is_forest_rock_can_place_on_layer(layer: &BlockLayer) -> bool {
    is_substrate_overworld_layer(layer) || is_base_stone_overworld(layer)
}

fn is_ice_spike_replaceable_layer(layer: &BlockLayer) -> bool {
    is_substrate_overworld_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:snow_block" | "minecraft:ice"
        )
}

fn supports_huge_mushroom_layer(layer: &BlockLayer) -> bool {
    is_substrate_overworld_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:mycelium"
                | "minecraft:podzol"
                | "minecraft:crimson_nylium"
                | "minecraft:warped_nylium"
        )
}

fn is_leaf_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:jungle_leaves"
            | "minecraft:oak_leaves"
            | "minecraft:spruce_leaves"
            | "minecraft:pale_oak_leaves"
            | "minecraft:dark_oak_leaves"
            | "minecraft:acacia_leaves"
            | "minecraft:birch_leaves"
            | "minecraft:azalea_leaves"
            | "minecraft:flowering_azalea_leaves"
            | "minecraft:mangrove_leaves"
            | "minecraft:cherry_leaves"
    )
}

fn is_log_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:oak_log"
            | "minecraft:oak_wood"
            | "minecraft:stripped_oak_log"
            | "minecraft:stripped_oak_wood"
            | "minecraft:birch_log"
            | "minecraft:birch_wood"
            | "minecraft:stripped_birch_log"
            | "minecraft:stripped_birch_wood"
            | "minecraft:spruce_log"
            | "minecraft:spruce_wood"
            | "minecraft:stripped_spruce_log"
            | "minecraft:stripped_spruce_wood"
            | "minecraft:acacia_log"
            | "minecraft:acacia_wood"
            | "minecraft:stripped_acacia_log"
            | "minecraft:stripped_acacia_wood"
            | "minecraft:jungle_log"
            | "minecraft:jungle_wood"
            | "minecraft:stripped_jungle_log"
            | "minecraft:stripped_jungle_wood"
            | "minecraft:dark_oak_log"
            | "minecraft:dark_oak_wood"
            | "minecraft:stripped_dark_oak_log"
            | "minecraft:stripped_dark_oak_wood"
            | "minecraft:pale_oak_log"
            | "minecraft:pale_oak_wood"
            | "minecraft:stripped_pale_oak_log"
            | "minecraft:stripped_pale_oak_wood"
            | "minecraft:cherry_log"
            | "minecraft:cherry_wood"
            | "minecraft:stripped_cherry_log"
            | "minecraft:stripped_cherry_wood"
            | "minecraft:mangrove_log"
            | "minecraft:mangrove_wood"
            | "minecraft:stripped_mangrove_log"
            | "minecraft:stripped_mangrove_wood"
    )
}

fn is_small_flower_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:dandelion"
            | "minecraft:open_eyeblossom"
            | "minecraft:poppy"
            | "minecraft:blue_orchid"
            | "minecraft:allium"
            | "minecraft:azure_bluet"
            | "minecraft:red_tulip"
            | "minecraft:orange_tulip"
            | "minecraft:white_tulip"
            | "minecraft:pink_tulip"
            | "minecraft:oxeye_daisy"
            | "minecraft:cornflower"
            | "minecraft:lily_of_the_valley"
            | "minecraft:wither_rose"
            | "minecraft:torchflower"
            | "minecraft:closed_eyeblossom"
            | "minecraft:golden_dandelion"
    )
}

fn valid_tree_position_layer(layer: &BlockLayer) -> bool {
    layer.is_air
        || is_leaf_layer(layer)
        || is_small_flower_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:pale_moss_carpet"
                | "minecraft:moss_carpet"
                | "minecraft:short_grass"
                | "minecraft:fern"
                | "minecraft:dead_bush"
                | "minecraft:vine"
                | "minecraft:glow_lichen"
                | "minecraft:sunflower"
                | "minecraft:lilac"
                | "minecraft:rose_bush"
                | "minecraft:peony"
                | "minecraft:tall_grass"
                | "minecraft:large_fern"
                | "minecraft:hanging_roots"
                | "minecraft:pitcher_plant"
                | "minecraft:small_dripleaf"
                | "minecraft:big_dripleaf"
                | "minecraft:big_dripleaf_stem"
                | "minecraft:water"
                | "minecraft:seagrass"
                | "minecraft:tall_seagrass"
                | "minecraft:bush"
                | "minecraft:firefly_bush"
                | "minecraft:warped_roots"
                | "minecraft:nether_sprouts"
                | "minecraft:crimson_roots"
                | "minecraft:leaf_litter"
                | "minecraft:short_dry_grass"
                | "minecraft:tall_dry_grass"
        )
}

fn is_replaceable_by_mushrooms_layer(layer: &BlockLayer) -> bool {
    valid_tree_position_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:brown_mushroom"
                | "minecraft:red_mushroom"
                | "minecraft:brown_mushroom_block"
                | "minecraft:red_mushroom_block"
        )
}

fn valid_fallen_log_position_layer(layer: &BlockLayer) -> bool {
    layer.is_air
        || is_small_flower_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:pale_moss_carpet"
                | "minecraft:short_grass"
                | "minecraft:fern"
                | "minecraft:dead_bush"
                | "minecraft:vine"
                | "minecraft:glow_lichen"
                | "minecraft:sunflower"
                | "minecraft:lilac"
                | "minecraft:rose_bush"
                | "minecraft:peony"
                | "minecraft:tall_grass"
                | "minecraft:large_fern"
                | "minecraft:bush"
                | "minecraft:firefly_bush"
                | "minecraft:leaf_litter"
                | "minecraft:short_dry_grass"
                | "minecraft:tall_dry_grass"
        )
}

fn supports_fallen_tree_layer(layer: &BlockLayer) -> bool {
    supports_vegetation_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:snow_block" | "minecraft:sand" | "minecraft:red_sand"
        )
}

fn cannot_replace_below_tree_trunk(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:dirt"
            | "minecraft:coarse_dirt"
            | "minecraft:rooted_dirt"
            | "minecraft:mud"
            | "minecraft:muddy_mangrove_roots"
            | "minecraft:moss_block"
            | "minecraft:pale_moss_block"
            | "minecraft:podzol"
    )
}

fn sapling_would_survive_at(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(valid_tree_position_layer)
        && supports_vegetation_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            world_y - 1,
            world_z,
            min_y,
        )
}

#[allow(clippy::too_many_arguments)]
fn sapling_would_survive_at_with_neighbors(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    context_layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(|layer| valid_tree_position_layer(&layer))
        && supports_vegetation_at_world_with_neighbors(
            chunk,
            chunk_min_x,
            chunk_min_z,
            neighbors,
            world_x,
            world_y - 1,
            world_z,
            min_y,
        )
}

fn is_water_at(
    chunk: &NoiseChunkBlocks,
    local_x: usize,
    world_y: i32,
    local_z: usize,
    min_y: i32,
) -> bool {
    chunk
        .layer(local_x, world_y, local_z, min_y)
        .is_some_and(is_water_layer)
}

fn is_water_layer(layer: &BlockLayer) -> bool {
    layer.is("minecraft:water")
}

fn is_fluid_layer(layer: &BlockLayer) -> bool {
    matches!(layer.block.as_ref(), "minecraft:water" | "minecraft:lava")
}

fn is_water_or_air_layer(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn is_air_or_water_layer(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn is_full_solid_layer(layer: &BlockLayer) -> bool {
    !layer.is_air && !is_fluid_layer(layer)
}

fn supports_vegetation_layer(layer: &BlockLayer) -> bool {
    SUPPORTS_VEGETATION_BLOCKS.contains(&layer.block.as_ref())
}

fn supports_dead_bush_layer(layer: &BlockLayer) -> bool {
    supports_dry_vegetation_layer(layer)
}

fn supports_dry_vegetation_layer(layer: &BlockLayer) -> bool {
    supports_vegetation_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:sand"
                | "minecraft:red_sand"
                | "minecraft:suspicious_sand"
                | "minecraft:terracotta"
                | "minecraft:white_terracotta"
                | "minecraft:orange_terracotta"
                | "minecraft:magenta_terracotta"
                | "minecraft:light_blue_terracotta"
                | "minecraft:yellow_terracotta"
                | "minecraft:lime_terracotta"
                | "minecraft:pink_terracotta"
                | "minecraft:gray_terracotta"
                | "minecraft:light_gray_terracotta"
                | "minecraft:cyan_terracotta"
                | "minecraft:purple_terracotta"
                | "minecraft:blue_terracotta"
                | "minecraft:brown_terracotta"
                | "minecraft:green_terracotta"
                | "minecraft:red_terracotta"
                | "minecraft:black_terracotta"
        )
}

fn supports_sugar_cane_layer(layer: &BlockLayer) -> bool {
    is_substrate_overworld_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:sand" | "minecraft:red_sand" | "minecraft:suspicious_sand"
        )
}

fn supports_sugar_cane_adjacently_layer(layer: &BlockLayer) -> bool {
    is_water_layer(layer) || layer.is("minecraft:frosted_ice")
}

fn supports_cactus_layer(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:sand" | "minecraft:red_sand" | "minecraft:suspicious_sand" | "minecraft:cactus"
    )
}

fn cactus_side_is_clear(layer: &BlockLayer) -> bool {
    layer.is_air || is_water_layer(layer)
}

fn supports_bamboo_layer(layer: &BlockLayer) -> bool {
    is_substrate_overworld_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:sand"
                | "minecraft:red_sand"
                | "minecraft:suspicious_sand"
                | "minecraft:gravel"
                | "minecraft:suspicious_gravel"
                | "minecraft:bamboo"
                | "minecraft:bamboo_sapling"
        )
}

fn supports_lily_pad_layer(layer: &BlockLayer) -> bool {
    is_water_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:ice" | "minecraft:frosted_ice"
        )
}

fn supports_underwater_vegetation_layer(layer: &BlockLayer) -> bool {
    is_base_stone_overworld(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:dirt"
                | "minecraft:coarse_dirt"
                | "minecraft:gravel"
                | "minecraft:sand"
                | "minecraft:red_sand"
                | "minecraft:clay"
                | "minecraft:mud"
                | "minecraft:moss_block"
        )
}

fn is_moss_replaceable_layer(layer: &BlockLayer) -> bool {
    is_base_stone_overworld(layer)
        || supports_vegetation_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:mud"
                | "minecraft:muddy_mangrove_roots"
                | "minecraft:cave_vines"
                | "minecraft:cave_vines_plant"
        )
}

fn is_lush_ground_replaceable_layer(layer: &BlockLayer) -> bool {
    is_moss_replaceable_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:clay" | "minecraft:gravel" | "minecraft:sand"
        )
}

fn supports_azalea_layer(layer: &BlockLayer) -> bool {
    supports_vegetation_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:sand"
                | "minecraft:red_sand"
                | "minecraft:terracotta"
                | "minecraft:white_terracotta"
                | "minecraft:orange_terracotta"
                | "minecraft:magenta_terracotta"
                | "minecraft:light_blue_terracotta"
                | "minecraft:yellow_terracotta"
                | "minecraft:lime_terracotta"
                | "minecraft:pink_terracotta"
                | "minecraft:gray_terracotta"
                | "minecraft:light_gray_terracotta"
                | "minecraft:cyan_terracotta"
                | "minecraft:purple_terracotta"
                | "minecraft:blue_terracotta"
                | "minecraft:brown_terracotta"
                | "minecraft:green_terracotta"
                | "minecraft:red_terracotta"
                | "minecraft:black_terracotta"
                | "minecraft:snow_block"
                | "minecraft:powder_snow"
        )
}

fn is_azalea_root_replaceable_layer(layer: &BlockLayer) -> bool {
    is_base_stone_overworld(layer)
        || supports_azalea_layer(layer)
        || matches!(
            layer.block.as_ref(),
            "minecraft:clay" | "minecraft:gravel" | "minecraft:sand" | "minecraft:red_sand"
        )
}

fn can_lake_replace_block(layer: &BlockLayer) -> bool {
    can_feature_replace_block(layer)
}

fn local_coord(world: i32, origin: i32) -> Option<usize> {
    let local = world - origin;
    (0..16).contains(&local).then_some(local as usize)
}

fn local_coords(
    world_x: i32,
    world_z: i32,
    origin_x: i32,
    origin_z: i32,
) -> Option<(usize, usize)> {
    Some((
        local_coord(world_x, origin_x)?,
        local_coord(world_z, origin_z)?,
    ))
}

fn overlaps_chunk(world_x: i32, world_z: i32, origin_x: i32, origin_z: i32) -> bool {
    local_coords(world_x, world_z, origin_x, origin_z).is_some()
}

fn horizontal_box_overlaps_chunk(
    min_x: i32,
    max_x: i32,
    min_z: i32,
    max_z: i32,
    origin_x: i32,
    origin_z: i32,
) -> bool {
    max_x >= origin_x && min_x < origin_x + 16 && max_z >= origin_z && min_z < origin_z + 16
}

fn can_feature_replace_block(layer: &BlockLayer) -> bool {
    !matches!(
        layer.block.as_ref(),
        "minecraft:bedrock"
            | "minecraft:spawner"
            | "minecraft:chest"
            | "minecraft:end_portal_frame"
            | "minecraft:reinforced_deepslate"
            | "minecraft:trial_spawner"
            | "minecraft:vault"
    )
}

fn layer_at_world<'a>(
    chunk: &'a NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<&'a BlockLayer> {
    let (local_x, local_z) = local_coords(world_x, world_z, chunk_min_x, chunk_min_z)?;
    chunk.layer(local_x, world_y, local_z, min_y)
}

#[allow(clippy::too_many_arguments)]
fn context_layer_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> Option<BlockLayer> {
    if let Some(layer) = layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    ) {
        return Some(layer.clone());
    }

    neighbors
        .iter()
        .find_map(|(neighbor_min_x, neighbor_min_z, neighbor_chunk)| {
            layer_at_world(
                neighbor_chunk,
                *neighbor_min_x,
                *neighbor_min_z,
                world_x,
                world_y,
                world_z,
                min_y,
            )
            .cloned()
        })
}

fn is_air_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(|layer| layer.is_air)
}

fn is_water_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_water_layer)
}

fn is_air_or_water_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_air_or_water_layer)
}

fn waterlogged_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> &'static str {
    if is_water_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    ) {
        "true"
    } else {
        "false"
    }
}

fn is_solid_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(is_full_solid_layer)
}

fn supports_vegetation_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(supports_vegetation_layer)
}

#[allow(clippy::too_many_arguments)]
fn supports_vegetation_at_world_with_neighbors(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    neighbors: &[(i32, i32, &NoiseChunkBlocks)],
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    context_layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        neighbors,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(|layer| supports_vegetation_layer(&layer))
}

fn supports_azalea_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(supports_azalea_layer)
}

fn supports_underwater_vegetation_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(supports_underwater_vegetation_layer)
}

fn is_fallen_tree_support_at_world(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> bool {
    layer_at_world(
        chunk,
        chunk_min_x,
        chunk_min_z,
        world_x,
        world_y,
        world_z,
        min_y,
    )
    .is_some_and(supports_fallen_tree_layer)
}

fn horizontal_directions() -> &'static [(i32, i32)] {
    &[(0, -1), (1, 0), (0, 1), (-1, 0)]
}

fn all_directions() -> [(i32, i32, i32, &'static str); 6] {
    [
        (0, -1, 0, "down"),
        (0, 1, 0, "up"),
        (0, 0, -1, "north"),
        (0, 0, 1, "south"),
        (-1, 0, 0, "west"),
        (1, 0, 0, "east"),
    ]
}

fn shuffled_all_directions(random: &mut FeatureRandom) -> Vec<(i32, i32, i32, &'static str)> {
    let mut directions = all_directions().to_vec();
    shuffle_directions(&mut directions, random);
    directions
}

fn shuffled_horizontal_vine_faces(random: &mut FeatureRandom) -> Vec<&'static str> {
    let mut faces = vec!["north", "south", "west", "east"];
    for index in (1..faces.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        faces.swap(index, swap);
    }
    faces
}

fn shuffle_directions(
    directions: &mut [(i32, i32, i32, &'static str)],
    random: &mut FeatureRandom,
) {
    for index in (1..directions.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        directions.swap(index, swap);
    }
}

fn shuffle_positions(positions: &mut [(i32, i32, i32)], random: &mut FeatureRandom) {
    for index in (1..positions.len()).rev() {
        let swap = random.next_int(index as i32 + 1) as usize;
        positions.swap(index, swap);
    }
}

fn direction_opposite(direction: (i32, i32, i32, &'static str)) -> (i32, i32, i32, &'static str) {
    let name = direction_opposite_name(direction.3);
    direction_by_name(name)
}

fn direction_opposite_name(name: &str) -> &'static str {
    match name {
        "down" => "up",
        "up" => "down",
        "north" => "south",
        "south" => "north",
        "west" => "east",
        "east" => "west",
        _ => "north",
    }
}

fn direction_by_name(name: &str) -> (i32, i32, i32, &'static str) {
    match name {
        "down" => (0, -1, 0, "down"),
        "up" => (0, 1, 0, "up"),
        "north" => (0, 0, -1, "north"),
        "south" => (0, 0, 1, "south"),
        "west" => (-1, 0, 0, "west"),
        "east" => (1, 0, 0, "east"),
        _ => (0, 0, -1, "north"),
    }
}

fn spread_direction_axis(direction: (i32, i32, i32, &'static str)) -> u8 {
    if direction.0 != 0 {
        0
    } else if direction.1 != 0 {
        1
    } else {
        2
    }
}

fn chest_facing(
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    world_y: i32,
    world_z: i32,
    min_y: i32,
) -> &'static str {
    for (dx, dz, facing) in [
        (0, -1, "south"),
        (1, 0, "west"),
        (0, 1, "north"),
        (-1, 0, "east"),
    ] {
        if is_solid_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x + dx,
            world_y,
            world_z + dz,
            min_y,
        ) {
            return facing;
        }
    }
    "north"
}

fn random_monster_room_entity(random: &mut FeatureRandom) -> &'static str {
    match random.next_int(4) {
        0 => "minecraft:skeleton",
        1 | 2 => "minecraft:zombie",
        _ => "minecraft:spider",
    }
}

fn chest_block_entity_nbt(loot_table_seed: i64) -> Tag {
    let mut fields = HashMap::new();
    fields.insert(
        "LootTable".to_string(),
        Tag::String(Arc::from("minecraft:chests/simple_dungeon")),
    );
    if loot_table_seed != 0 {
        fields.insert("LootTableSeed".to_string(), Tag::Long(loot_table_seed));
    }
    Tag::Compound(Arc::new(fields))
}

fn spawner_block_entity_nbt(entity_id: &str) -> Tag {
    compound_tag([
        ("Delay", Tag::Short(20)),
        ("MinSpawnDelay", Tag::Short(200)),
        ("MaxSpawnDelay", Tag::Short(800)),
        ("SpawnCount", Tag::Short(4)),
        ("MaxNearbyEntities", Tag::Short(6)),
        ("RequiredPlayerRange", Tag::Short(16)),
        ("SpawnRange", Tag::Short(4)),
        (
            "SpawnData",
            compound_tag([(
                "entity",
                compound_tag([("id", Tag::String(Arc::from(entity_id.to_string())))]),
            )]),
        ),
    ])
}

fn beehive_block_entity_nbt(random: &mut FeatureRandom) -> Tag {
    let bee_count = 2 + random.next_int(2);
    let bees = (0..bee_count)
        .map(|_| {
            compound_tag([
                (
                    "entity_data",
                    compound_tag([("id", Tag::String(Arc::from("minecraft:bee")))]),
                ),
                ("ticks_in_hive", Tag::Int(random.next_int(599))),
                ("min_ticks_in_hive", Tag::Int(600)),
            ])
        })
        .collect::<Vec<_>>();

    let mut fields = HashMap::new();
    fields.insert(
        "bees".to_string(),
        Tag::List(
            ListHeader {
                tag_id: tag_id::COMPOUND,
                length: bees.len() as i32,
            },
            Arc::from(bees),
        ),
    );
    Tag::Compound(Arc::new(fields))
}

fn can_geode_replace_block(layer: &BlockLayer) -> bool {
    can_feature_replace_block(layer)
}

fn is_geode_invalid_block(layer: &BlockLayer) -> bool {
    matches!(
        layer.block.as_ref(),
        "minecraft:bedrock"
            | "minecraft:water"
            | "minecraft:lava"
            | "minecraft:ice"
            | "minecraft:packed_ice"
            | "minecraft:blue_ice"
    )
}

fn can_amethyst_cluster_grow_at(layer: &BlockLayer) -> bool {
    layer.is_air || layer.is("minecraft:water")
}

fn is_freezing_biome(biome: &str) -> bool {
    matches!(
        biome,
        "minecraft:frozen_ocean"
            | "minecraft:deep_frozen_ocean"
            | "minecraft:frozen_river"
            | "minecraft:snowy_beach"
            | "minecraft:snowy_plains"
            | "minecraft:ice_spikes"
            | "minecraft:snowy_taiga"
            | "minecraft:grove"
            | "minecraft:snowy_slopes"
            | "minecraft:jagged_peaks"
            | "minecraft:frozen_peaks"
    )
}

fn amethyst_cluster_block(block: &str) -> BlockLayer {
    BlockLayer::with_properties(block, &[("facing", "up"), ("waterlogged", "false")])
}

fn inv_sqrt_distance(x0: i32, y0: i32, z0: i32, x1: i32, y1: i32, z1: i32, offset: i32) -> f64 {
    let dx = x0 - x1;
    let dy = y0 - y1;
    let dz = z0 - z1;
    1.0 / ((dx * dx + dy * dy + dz * dz + offset) as f64).sqrt()
}

fn geode_noise(x: i32, y: i32, z: i32) -> f64 {
    let seed = (x as i64).wrapping_mul(3_129_871)
        ^ (z as i64).wrapping_mul(116_129_781)
        ^ (y as i64).wrapping_mul(42_317_861);
    let mut random = FeatureRandom::new(seed);
    random.next_double() * 2.0 - 1.0
}

fn scan_down_to_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    local_x: usize,
    origin_y: i32,
    local_z: usize,
    max_steps: i32,
) -> Option<i32> {
    for step in 0..=max_steps {
        let y = origin_y - step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y)
            || y - 5 < settings.min_y
        {
            return None;
        }
        if chunk
            .layer(local_x, y, local_z, settings.min_y)
            .is_some_and(|layer| !layer.is_air)
        {
            return Some(y);
        }
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn scan_up_to_solid(
    settings: &NoiseSettings,
    chunk: &NoiseChunkBlocks,
    chunk_min_x: i32,
    chunk_min_z: i32,
    world_x: i32,
    origin_y: i32,
    world_z: i32,
    max_steps: i32,
) -> Option<i32> {
    for step in 0..=max_steps {
        let y = origin_y + step;
        if !(settings.min_y..settings.min_y + settings.height).contains(&y)
            || y + 1 >= settings.min_y + settings.height
        {
            return None;
        }
        let current = layer_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            y,
            world_z,
            settings.min_y,
        )?;
        if !current.is_air {
            return None;
        }
        if is_solid_at_world(
            chunk,
            chunk_min_x,
            chunk_min_z,
            world_x,
            y + 1,
            world_z,
            settings.min_y,
        ) {
            return Some(y + 1);
        }
    }

    None
}

fn lake_index(x: usize, y: usize, z: usize) -> usize {
    (x * 16 + z) * 8 + y
}

fn is_lake_boundary(grid: &[bool], x: usize, y: usize, z: usize) -> bool {
    (x < 15 && grid[lake_index(x + 1, y, z)])
        || (x > 0 && grid[lake_index(x - 1, y, z)])
        || (z < 15 && grid[lake_index(x, y, z + 1)])
        || (z > 0 && grid[lake_index(x, y, z - 1)])
        || (y < 7 && grid[lake_index(x, y + 1, z)])
        || (y > 0 && grid[lake_index(x, y - 1, z)])
}

fn lerp_f64(delta: f64, start: f64, end: f64) -> f64 {
    start + delta * (end - start)
}
