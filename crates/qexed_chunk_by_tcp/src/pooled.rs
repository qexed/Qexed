//! 多连接池化客户端：chunk-summon 每连接一线程逐个生成区块，
//! 单连接流水线只是省 RTT、生成仍然串行。开 N 条连接把一批请求分片并行，
//! Java 侧生成即并行化（N 连接 ≈ N 路生成，本机实测 165ms/块）。

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::client::{ChunkClientError, ChunkData, ChunkTcpClient, Dimension};

/// 池化客户端：N 条独立连接，请求按连接数分片并行发送。
pub struct PooledChunkClient {
    conns: Vec<Arc<Mutex<ChunkTcpClient>>>,
    dimension: Dimension,
}

impl PooledChunkClient {
    /// 连接 count 条（至少 1）。
    pub async fn connect(
        addr: impl tokio::net::ToSocketAddrs + Clone,
        count: usize,
    ) -> Result<Self, ChunkClientError> {
        let mut conns = Vec::with_capacity(count.max(1));
        for _ in 0..count.max(1) {
            conns.push(Arc::new(Mutex::new(ChunkTcpClient::connect(addr.clone()).await?)));
        }
        Ok(Self {
            conns,
            dimension: Dimension::Overworld,
        })
    }

    pub fn with_dimension(mut self, dimension: Dimension) -> Self {
        self.dimension = dimension;
        self
    }

    /// 并行请求一批：坐标按连接数轮转分片，每片单连接流水线（先全发后全收）。
    /// 返回顺序与 coords 一致。
    pub async fn request_chunks(
        &self,
        coords: &[(i32, i32)],
    ) -> Result<Vec<ChunkData>, ChunkClientError> {
        if coords.is_empty() {
            return Ok(Vec::new());
        }
        let n = self.conns.len();
        let mut shards: Vec<Vec<(i32, i32)>> = vec![Vec::new(); n];
        for (i, &coord) in coords.iter().enumerate() {
            shards[i % n].push(coord);
        }
        let mut handles = Vec::with_capacity(n);
        let dimension = self.dimension;
        for (conn, shard) in self.conns.iter().cloned().zip(shards.into_iter()) {
            handles.push(tokio::spawn(async move {
                let mut client = conn.lock().await;
                client.request_chunks(&shard, dimension).await
            }));
        }
        let mut out = Vec::with_capacity(coords.len());
        let mut results = Vec::with_capacity(n);
        for handle in handles {
            results.push(handle.await.map_err(|e| ChunkClientError::Decode(format!("join: {e}")))?);
        }
        // 分片结果按坐标索引归位：request_chunks 保序,但跨分片要按轮转位置重排。
        let mut by_index: Vec<Option<ChunkData>> = (0..coords.len()).map(|_| None).collect();
        for (shard_i, datas) in results.into_iter().enumerate() {
            let datas = datas?;
            for (k, data) in datas.into_iter().enumerate() {
                let coord_index = shard_i + k * n;
                by_index[coord_index] = Some(data);
            }
        }
        for (i, slot) in by_index.into_iter().enumerate() {
            match slot {
                Some(data) => out.push(data),
                None => {
                    return Err(ChunkClientError::Decode(format!(
                        "分片归位缺失: index {i}"
                    )))
                }
            }
        }
        Ok(out)
    }

    /// keep-alive 探测（轮询连接）。
    pub async fn ping(&self, id: i64) -> Result<i64, ChunkClientError> {
        let conn = self.conns[0].clone();
        let mut client = conn.lock().await;
        client.ping(id).await
    }

    /// 连接数。
    pub fn len(&self) -> usize {
        self.conns.len()
    }

    /// 是否无连接。
    pub fn is_empty(&self) -> bool {
        self.conns.is_empty()
    }
}
