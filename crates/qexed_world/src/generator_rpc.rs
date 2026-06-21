use std::path::{Path, PathBuf};

use qexed_save::DimensionId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct VanillaWorldgenClient {
    endpoint: String,
    http: reqwest::Client,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateChunkRequest {
    pub dimension: RpcDimension,
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub save_root: PathBuf,
    pub dimension_root: PathBuf,
    pub region_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RpcDimension {
    pub namespace: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerateChunkResult {
    pub written: bool,
    pub region_path: PathBuf,
}

#[derive(Debug, Serialize)]
struct JsonRpcRequest<'a, T> {
    jsonrpc: &'static str,
    id: u64,
    method: &'static str,
    params: &'a T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

impl VanillaWorldgenClient {
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn generate_chunk(
        &self,
        request: GenerateChunkRequest,
    ) -> anyhow::Result<GenerateChunkResult> {
        let rpc = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "worldgen.generateChunk",
            params: &request,
        };
        let response = self.http.post(&self.endpoint).json(&rpc).send().await?;
        if !response.status().is_success() {
            anyhow::bail!("Java 原版地形生成服务返回异常状态: {}", response.status());
        }

        let response = response
            .json::<JsonRpcResponse<GenerateChunkResult>>()
            .await?;
        if let Some(error) = response.error {
            anyhow::bail!(
                "Java 原版地形生成失败: code={}, message={}",
                error.code,
                error.message
            );
        }
        response
            .result
            .ok_or_else(|| anyhow::anyhow!("Java 原版地形生成服务未返回 result"))
    }
}

impl GenerateChunkRequest {
    pub fn new(
        dimension: &DimensionId,
        chunk_x: i32,
        chunk_z: i32,
        save_root: impl AsRef<Path>,
        dimension_root: impl Into<PathBuf>,
        region_path: impl Into<PathBuf>,
    ) -> Self {
        Self {
            dimension: RpcDimension {
                namespace: dimension.namespace().to_string(),
                value: dimension.value().to_string(),
            },
            chunk_x,
            chunk_z,
            save_root: save_root.as_ref().to_path_buf(),
            dimension_root: dimension_root.into(),
            region_path: region_path.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GenerateChunkRequest;

    #[test]
    fn generate_chunk_request_carries_v5_save_paths() {
        let request = GenerateChunkRequest::new(
            &qexed_save::DimensionId::the_nether(),
            -1,
            32,
            "world",
            "world/dimensions/minecraft/the_nether",
            "world/dimensions/minecraft/the_nether/region/r.-1.1.mca",
        );

        let value = serde_json::to_value(&request).unwrap();
        assert_eq!(value["dimension"]["namespace"], "minecraft");
        assert_eq!(value["dimension"]["value"], "the_nether");
        assert_eq!(value["chunk_x"], -1);
        assert_eq!(
            value["region_path"],
            "world/dimensions/minecraft/the_nether/region/r.-1.1.mca"
        );
    }
}
