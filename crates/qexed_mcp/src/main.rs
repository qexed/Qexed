mod chunk;
mod protocol;
mod region;
mod registry;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Context, Result};
use protocol::{
    ContentItem, Request, Response, ServerCapabilities, Tool, ToolCallResult, ToolsCapability,
};
use serde_json::Value;

struct ServerState {
    save_path: Option<PathBuf>,
    dimension: String,
}

fn main() -> Result<()> {
    let mut save_path: Option<PathBuf> = None;
    let mut dimension = "minecraft:overworld".to_string();

    // Auto-load from env vars
    if let Ok(path) = std::env::var("QEXED_SAVE_PATH") {
        let p = PathBuf::from(&path);
        if p.is_dir() {
            save_path = Some(p);
            eprintln!("[qexed-mcp] auto-loaded save_path from env: {path}");
        } else {
            eprintln!("[qexed-mcp] WARNING: QEXED_SAVE_PATH is not a directory: {path}");
        }
    }
    if let Ok(dim) = std::env::var("QEXED_DIMENSION") {
        dimension = dim;
        eprintln!("[qexed-mcp] auto-loaded dimension from env: {dimension}");
    }
    if let Ok(reg_path) = std::env::var("QEXED_REGISTRY_PATH") {
        match registry::load_registry(std::path::Path::new(&reg_path)) {
            Ok(count) => eprintln!("[qexed-mcp] auto-loaded block registry: {count} blocks"),
            Err(e) => eprintln!("[qexed-mcp] WARNING: failed to load registry: {e}"),
        }
    }

    let state = Mutex::new(ServerState {
        save_path,
        dimension,
    });

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();

    for line in stdin.lock().lines() {
        let line = line.context("failed to read stdin")?;
        if line.trim().is_empty() {
            continue;
        }

        let request: Request = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let err_response = Response::error(None, -32700, format!("Parse error: {e}"));
                let mut out = stdout.lock();
                let _ = serde_json::to_writer(&mut out, &err_response);
                let _ = writeln!(out);
                let _ = out.flush();
                continue;
            }
        };

        let response = handle_request(&request, &state);

        if let Some(resp) = response {
            let mut out = stdout.lock();
            if serde_json::to_writer(&mut out, &resp).is_err() {
                break;
            }
            let _ = writeln!(out);
            let _ = out.flush();
        }
    }

    Ok(())
}

fn handle_request(request: &Request, state: &Mutex<ServerState>) -> Option<Response> {
    match request.method.as_str() {
        "initialize" => handle_initialize(request),
        "initialized" => {
            // Client sends initialized notification, no response needed
            None
        }
        "tools/list" => handle_tools_list(request),
        "tools/call" => handle_tools_call(request, state),
        "notifications/initialized" => None,
        _ => Some(Response::error(
            request.id.clone(),
            -32601,
            format!("Method not found: {}", request.method),
        )),
    }
}

fn handle_initialize(request: &Request) -> Option<Response> {
    let capabilities = ServerCapabilities {
        tools: Some(ToolsCapability {
            list_changed: Some(false),
        }),
    };

    let result = serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": capabilities,
        "serverInfo": {
            "name": "qexed-mcp",
            "version": "0.1.0"
        }
    });

    Some(Response::success(request.id.clone(), result))
}

fn handle_tools_list(request: &Request) -> Option<Response> {
    let tools = vec![
        Tool {
            name: "set_save_path".to_string(),
            description: "Set the path to the Minecraft world save directory. Must be called before any other tools."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "save_path": {
                        "type": "string",
                        "description": "Absolute path to the Minecraft world save directory (the folder containing region/, DIM-1/, DIM1/, level.dat)"
                    },
                    "dimension": {
                        "type": "string",
                        "description": "Dimension to operate on: 'minecraft:overworld', 'minecraft:the_nether', or 'minecraft:the_end'",
                        "default": "minecraft:overworld"
                    }
                },
                "required": ["save_path"]
            }),
        },
        Tool {
            name: "load_block_registry".to_string(),
            description: "Load the block state registry from a blocks.json report file. This enables name-based block lookups. The blocks.json file is typically at assets/reports/blocks.json in the Qexed project.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "registry_path": {
                        "type": "string",
                        "description": "Path to blocks.json file (e.g., 'assets/reports/blocks.json')"
                    }
                },
                "required": ["registry_path"]
            }),
        },
        Tool {
            name: "lookup_block_state".to_string(),
            description: "Look up a block state ID by block name. Returns the numeric ID needed for place_blocks and fill_region. Use properties for blocks like stairs, slabs, fences, etc.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "block": {
                        "type": "string",
                        "description": "Block name, e.g. 'minecraft:stone', 'stone', 'minecraft:oak_stairs'"
                    },
                    "properties": {
                        "type": "object",
                        "description": "Optional block state properties, e.g. {\"facing\": \"north\", \"half\": \"bottom\"}"
                    }
                },
                "required": ["block"]
            }),
        },
        Tool {
            name: "list_blocks".to_string(),
            description: "List all known block names and their default block state IDs. Requires that load_block_registry has been called first.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
        Tool {
            name: "read_region".to_string(),
            description: "Read block states from a rectangular region of the world. Returns an array of block_state_ids. The region is defined by min/max coordinates (inclusive). Large regions may be truncated in the response - use the summary format for large areas.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "min": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "integer"},
                            "y": {"type": "integer"},
                            "z": {"type": "integer"}
                        },
                        "required": ["x", "y", "z"]
                    },
                    "max": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "integer"},
                            "y": {"type": "integer"},
                            "z": {"type": "integer"}
                        },
                        "required": ["x", "y", "z"]
                    },
                    "summary_only": {
                        "type": "boolean",
                        "description": "If true, only return a summary with unique block states and their counts instead of the full array",
                        "default": false
                    }
                },
                "required": ["min", "max"]
            }),
        },
        Tool {
            name: "place_blocks".to_string(),
            description: "Place individual blocks in the world. Each block is specified by its x,y,z coordinates and block_state_id. Blocks are grouped by chunk and written to the Anvil region files directly. Use lookup_block_state first to get the numeric IDs.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "blocks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "x": {"type": "integer"},
                                "y": {"type": "integer"},
                                "z": {"type": "integer"},
                                "block_state_id": {"type": "integer"}
                            },
                            "required": ["x", "y", "z", "block_state_id"]
                        }
                    }
                },
                "required": ["blocks"]
            }),
        },
        Tool {
            name: "fill_region".to_string(),
            description: "Fill a rectangular region with a single block type. Much faster than place_blocks for large areas. Good for flattening terrain, creating platforms, or clearing space.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "min": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "integer"},
                            "y": {"type": "integer"},
                            "z": {"type": "integer"}
                        },
                        "required": ["x", "y", "z"]
                    },
                    "max": {
                        "type": "object",
                        "properties": {
                            "x": {"type": "integer"},
                            "y": {"type": "integer"},
                            "z": {"type": "integer"}
                        },
                        "required": ["x", "y", "z"]
                    },
                    "block_state_id": {
                        "type": "integer",
                        "description": "The block state ID to fill with (0 = air)"
                    }
                },
                "required": ["min", "max", "block_state_id"]
            }),
        },
    ];

    let tools_value = serde_json::to_value(&tools).unwrap_or_default();
    Some(Response::success(request.id.clone(), tools_value))
}

fn handle_tools_call(request: &Request, state_mutex: &Mutex<ServerState>) -> Option<Response> {
    let params = match &request.params {
        Some(Value::Object(params)) => params.clone(),
        _ => {
            return Some(Response::error(
                request.id.clone(),
                -32602,
                "Invalid params".to_string(),
            ));
        }
    };

    let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));

    let result = match tool_name {
        "set_save_path" => tool_set_save_path(&arguments, state_mutex),
        "load_block_registry" => tool_load_block_registry(&arguments),
        "lookup_block_state" => tool_lookup_block_state(&arguments),
        "list_blocks" => tool_list_blocks(&arguments),
        "read_region" => tool_read_region(&arguments, state_mutex),
        "place_blocks" => tool_place_blocks(&arguments, state_mutex),
        "fill_region" => tool_fill_region(&arguments, state_mutex),
        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    };

    let is_error = result.is_err();

    let content = match result {
        Ok(text) => vec![ContentItem {
            content_type: "text".to_string(),
            text,
        }],
        Err(err) => vec![ContentItem {
            content_type: "text".to_string(),
            text: format!("Error: {err}"),
        }],
    };

    let call_result = ToolCallResult {
        content,
        is_error: if is_error { Some(true) } else { None },
    };

    Some(Response::success(
        request.id.clone(),
        serde_json::to_value(&call_result).unwrap_or_default(),
    ))
}

fn require_save_path(state: &Mutex<ServerState>) -> Result<(PathBuf, String)> {
    let state = state.lock().unwrap();
    match &state.save_path {
        Some(path) => Ok((path.clone(), state.dimension.clone())),
        None => anyhow::bail!("save_path not set. Call set_save_path first."),
    }
}

fn tool_set_save_path(args: &Value, state: &Mutex<ServerState>) -> Result<String> {
    let save_path = args
        .get("save_path")
        .and_then(|v| v.as_str())
        .context("missing 'save_path' argument")?;
    let dimension = args
        .get("dimension")
        .and_then(|v| v.as_str())
        .unwrap_or("minecraft:overworld");

    let path = PathBuf::from(save_path);
    if !path.exists() {
        anyhow::bail!("save_path does not exist: {save_path}");
    }
    if !path.is_dir() {
        anyhow::bail!("save_path is not a directory: {save_path}");
    }

    let mut state = state.lock().unwrap();
    state.save_path = Some(path.clone());
    state.dimension = dimension.to_string();

    Ok(format!(
        "Save path set to: {}\nDimension: {}",
        path.display(),
        dimension
    ))
}

fn tool_load_block_registry(args: &Value) -> Result<String> {
    let registry_path = args
        .get("registry_path")
        .and_then(|v| v.as_str())
        .context("missing 'registry_path' argument")?;

    let count = registry::load_registry(std::path::Path::new(registry_path))?;
    Ok(format!("Loaded {count} block types from registry"))
}

fn tool_lookup_block_state(args: &Value) -> Result<String> {
    let block = args
        .get("block")
        .and_then(|v| v.as_str())
        .context("missing 'block' argument")?;

    let properties: Vec<(String, String)> = match args.get("properties") {
        Some(Value::Object(props)) => props
            .iter()
            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
            .collect(),
        _ => Vec::new(),
    };

    match registry::block_state_id(block, &properties) {
        Some(id) => Ok(format!(
            "Block: {block}\nProperties: {properties:?}\nBlock state ID: {id}"
        )),
        None => {
            // Try without properties
            let default_props: Vec<(String, String)> = Vec::new();
            match registry::block_state_id(block, &default_props) {
                Some(id) => Ok(format!(
                    "Block: {block}\nDefault block state ID: {id}\nNote: specified properties {properties:?} not found, using default"
                )),
                None => {
                    anyhow::bail!(
                        "Block not found in registry: {block}. Make sure load_block_registry was called and the block name is correct."
                    )
                }
            }
        }
    }
}

fn tool_list_blocks(_args: &Value) -> Result<String> {
    let blocks = registry::list_block_names();
    if blocks.is_empty() {
        return Err(anyhow::anyhow!(
            "Registry not loaded. Call load_block_registry first."
        ));
    }
    let mut out = format!("{} known blocks:\n", blocks.len());
    for (name, id) in blocks.iter().take(200) {
        out.push_str(&format!("  {name} -> {id}\n"));
    }
    if blocks.len() > 200 {
        out.push_str(&format!("  ... and {} more\n", blocks.len() - 200));
    }
    Ok(out)
}

fn tool_read_region(args: &Value, state: &Mutex<ServerState>) -> Result<String> {
    let (save_path, dimension) = require_save_path(state)?;

    let min = args.get("min").context("missing 'min'")?;
    let max = args.get("max").context("missing 'max'")?;
    let min_x = min.get("x").and_then(|v| v.as_i64()).context("min.x")? as i32;
    let min_y = min.get("y").and_then(|v| v.as_i64()).context("min.y")? as i32;
    let min_z = min.get("z").and_then(|v| v.as_i64()).context("min.z")? as i32;
    let max_x = max.get("x").and_then(|v| v.as_i64()).context("max.x")? as i32;
    let max_y = max.get("y").and_then(|v| v.as_i64()).context("max.y")? as i32;
    let max_z = max.get("z").and_then(|v| v.as_i64()).context("max.z")? as i32;
    let summary_only = args
        .get("summary_only")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let region = chunk::read_blocks_region(
        &save_path, &dimension, min_x, min_y, min_z, max_x, max_y, max_z,
    )?;

    let total = region.blocks.len();

    if total == 0 {
        return Ok("Region is empty (0 blocks requested)".to_string());
    }

    if summary_only || total > 10000 {
        // Summary mode: count unique block states
        let mut counts: HashMap<i32, usize> = HashMap::new();
        for &id in &region.blocks {
            *counts.entry(id).or_default() += 1;
        }
        let mut sorted: Vec<_> = counts.into_iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

        let mut out = format!(
            "Region ({min_x},{min_y},{min_z}) -> ({max_x},{max_y},{max_z}): {total} blocks\n\nUnique block states:\n"
        );
        for (id, count) in sorted.iter().take(50) {
            let entry = registry::block_state_entry(*id);
            let name = entry.name.clone();
            out.push_str(&format!("  {id}: {name} (x{count})\n"));
        }
        if sorted.len() > 50 {
            out.push_str(&format!("  ... and {} more types\n", sorted.len() - 50));
        }
        Ok(out)
    } else {
        // Full mode: return the full array
        let data = serde_json::json!({
            "min": {"x": min_x, "y": min_y, "z": min_z},
            "max": {"x": max_x, "y": max_y, "z": max_z},
            "size": {"x": region.size_x, "y": region.size_y, "z": region.size_z},
            "blocks": &region.blocks
        });
        Ok(serde_json::to_string_pretty(&data).unwrap_or_default())
    }
}

fn tool_place_blocks(args: &Value, state: &Mutex<ServerState>) -> Result<String> {
    let (save_path, dimension) = require_save_path(state)?;

    let blocks = args
        .get("blocks")
        .and_then(|v| v.as_array())
        .context("missing 'blocks' array")?;

    if blocks.is_empty() {
        return Ok("No blocks to place.".to_string());
    }

    // Group blocks by chunk
    let mut chunks: HashMap<(i32, i32), Vec<(i32, i32, i32, i32)>> = HashMap::new();
    for block in blocks {
        let x = block.get("x").and_then(|v| v.as_i64()).context("block.x")? as i32;
        let y = block.get("y").and_then(|v| v.as_i64()).context("block.y")? as i32;
        let z = block.get("z").and_then(|v| v.as_i64()).context("block.z")? as i32;
        let state_id = block
            .get("block_state_id")
            .and_then(|v| v.as_i64())
            .context("block.block_state_id")? as i32;

        if y < chunk::WORLD_MIN_Y || y > chunk::WORLD_MAX_Y {
            anyhow::bail!(
                "Y coordinate {y} is out of range [{}, {}]",
                chunk::WORLD_MIN_Y,
                chunk::WORLD_MAX_Y
            );
        }

        let cx = x.div_euclid(16);
        let cz = z.div_euclid(16);
        chunks
            .entry((cx, cz))
            .or_default()
            .push((x, y, z, state_id));
    }

    let region_dir = chunk::dimension_region_path(&save_path, &dimension);
    std::fs::create_dir_all(&region_dir)?;

    let mut placed = 0usize;
    let mut errors = Vec::new();

    for ((cx, cz), chunk_blocks) in &chunks {
        let region_path = region_dir.join(crate::region::region_file_name(*cx, *cz));

        // Read existing region file if it exists
        let existing = if region_path.exists() {
            let region = region::AnvilRegion::from_file(&region_path)?;
            region.read_chunk(*cx, *cz)?
        } else {
            None
        };

        match chunk::set_block_states_in_chunk(existing.as_ref(), *cx, *cz, chunk_blocks) {
            Ok(new_chunk) => {
                let mut region = if region_path.exists() {
                    region::AnvilRegion::from_file(&region_path)?
                } else {
                    region::AnvilRegion::new(&region_path)
                };
                region.write_chunk(*cx, *cz, new_chunk)?;
                region.save()?;
                placed += chunk_blocks.len();
            }
            Err(e) => {
                errors.push(format!("chunk ({cx}, {cz}): {e}"));
            }
        }
    }

    let mut msg = format!("Placed {placed} blocks in {} chunks.", chunks.len());
    if !errors.is_empty() {
        msg.push_str(&format!("\nErrors:\n{}", errors.join("\n")));
    }
    Ok(msg)
}

fn tool_fill_region(args: &Value, state: &Mutex<ServerState>) -> Result<String> {
    let (save_path, dimension) = require_save_path(state)?;

    let min = args.get("min").context("missing 'min'")?;
    let max = args.get("max").context("missing 'max'")?;
    let min_x = min.get("x").and_then(|v| v.as_i64()).context("min.x")? as i32;
    let min_y = min.get("y").and_then(|v| v.as_i64()).context("min.y")? as i32;
    let min_z = min.get("z").and_then(|v| v.as_i64()).context("min.z")? as i32;
    let max_x = max.get("x").and_then(|v| v.as_i64()).context("max.x")? as i32;
    let max_y = max.get("y").and_then(|v| v.as_i64()).context("max.y")? as i32;
    let max_z = max.get("z").and_then(|v| v.as_i64()).context("max.z")? as i32;
    let block_state_id = args
        .get("block_state_id")
        .and_then(|v| v.as_i64())
        .context("missing 'block_state_id'")? as i32;

    if min_y < chunk::WORLD_MIN_Y || max_y > chunk::WORLD_MAX_Y {
        anyhow::bail!(
            "Y range [{min_y}, {max_y}] is out of world bounds [{}, {}]",
            chunk::WORLD_MIN_Y,
            chunk::WORLD_MAX_Y
        );
    }

    if min_x > max_x || min_y > max_y || min_z > max_z {
        anyhow::bail!("invalid region: min must be <= max in all dimensions");
    }

    let region_dir = chunk::dimension_region_path(&save_path, &dimension);
    std::fs::create_dir_all(&region_dir)?;

    let min_cx = min_x.div_euclid(16);
    let max_cx = max_x.div_euclid(16);
    let min_cz = min_z.div_euclid(16);
    let max_cz = max_z.div_euclid(16);

    let total_chunks = ((max_cx - min_cx + 1) * (max_cz - min_cz + 1)) as usize;
    let volume =
        (max_x - min_x + 1) as u64 * (max_y - min_y + 1) as u64 * (max_z - min_z + 1) as u64;
    let mut errors = Vec::new();

    for cx in min_cx..=max_cx {
        for cz in min_cz..=max_cz {
            let region_path = region_dir.join(crate::region::region_file_name(cx, cz));

            let existing = if region_path.exists() {
                let region = region::AnvilRegion::from_file(&region_path)?;
                region.read_chunk(cx, cz)?
            } else {
                None
            };

            match chunk::fill_region_chunk(
                existing.as_ref(),
                cx,
                cz,
                min_x,
                min_y,
                min_z,
                max_x,
                max_y,
                max_z,
                block_state_id,
            ) {
                Ok(new_chunk) => {
                    let mut region = if region_path.exists() {
                        region::AnvilRegion::from_file(&region_path)?
                    } else {
                        region::AnvilRegion::new(&region_path)
                    };
                    region.write_chunk(cx, cz, new_chunk)?;
                    region.save()?;
                }
                Err(e) => {
                    errors.push(format!("chunk ({cx}, {cz}): {e}"));
                }
            }
        }
    }

    let mut msg = format!("Fill completed across {total_chunks} chunks (~{volume} blocks).");
    if !errors.is_empty() {
        msg.push_str(&format!("\nErrors:\n{}", errors.join("\n")));
    }
    Ok(msg)
}
