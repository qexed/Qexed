use super::*;

#[derive(Debug, Clone)]
pub(crate) struct BlockStateDefinition {
    pub id: i32,
    pub properties: Vec<(String, String)>,
}

pub(crate) fn default_block_state(block: &str) -> BlockStateDefinition {
    block_state_definition(block, &[]).unwrap_or(BlockStateDefinition {
        id: 0,
        properties: Vec::new(),
    })
}

pub(crate) fn default_block_state_id(block: &str) -> i32 {
    default_block_state(block).id
}

pub(crate) fn default_block_state_id_if_known(block: &str) -> Option<i32> {
    block_state_id(block, &[]).ok()
}

pub(crate) fn block_state(block: &str, properties: &[(String, String)]) -> BlockStateDefinition {
    block_state_definition(block, properties)
        .or_else(|_| block_state_definition(block, &[]))
        .unwrap_or(BlockStateDefinition {
            id: 0,
            properties: Vec::new(),
        })
}

fn block_state_id(block: &str, properties: &[(String, String)]) -> Result<i32> {
    block_state_definition(block, properties).map(|state| state.id)
}

fn block_state_definition(
    block: &str,
    properties: &[(String, String)],
) -> Result<BlockStateDefinition> {
    let block = super::normalize_identifier(block);
    let report = blocks_report()?;
    let states = report
        .get(&block)
        .and_then(|block| block.get("states"))
        .and_then(serde_json::Value::as_array)
        .with_context(|| format!("block states missing in Mojang report: {block}"))?;

    if properties.is_empty() {
        let state = states
            .iter()
            .find(|state| state.get("default").and_then(serde_json::Value::as_bool) == Some(true))
            .or_else(|| states.first())
            .with_context(|| format!("block state id missing in Mojang report: {block}"))?;
        return block_state_definition_from_json(state);
    }

    for state in states {
        let Some(state_properties) = state
            .get("properties")
            .and_then(serde_json::Value::as_object)
        else {
            continue;
        };
        let matches = state_properties.len() == properties.len()
            && properties.iter().all(|(key, value)| {
                state_properties
                    .get(key)
                    .and_then(serde_json::Value::as_str)
                    == Some(value.as_str())
            });
        if matches {
            return block_state_definition_from_json(state);
        }
    }

    anyhow::bail!("block state not found in Mojang report: {block}");
}

fn block_state_definition_from_json(state: &serde_json::Value) -> Result<BlockStateDefinition> {
    let id = state
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .context("block state id missing in Mojang report")?;
    let mut properties = state
        .get("properties")
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flat_map(|properties| properties.iter())
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_string())))
        .collect::<Vec<_>>();
    properties.sort_by(|left, right| left.0.cmp(&right.0));
    Ok(BlockStateDefinition {
        id: i32::try_from(id).context("block state id does not fit i32")?,
        properties,
    })
}

fn blocks_report() -> Result<&'static serde_json::Value> {
    static REPORT: OnceLock<Result<serde_json::Value, String>> = OnceLock::new();
    let result = REPORT.get_or_init(|| {
        let cache = WorldgenCache::default();
        let path = cache.reports_root().join("blocks.json");
        super::read_json(&path).map_err(|err| format!("{err:#}"))
    });
    result.as_ref().map_err(|err| anyhow::anyhow!("{err}"))
}
