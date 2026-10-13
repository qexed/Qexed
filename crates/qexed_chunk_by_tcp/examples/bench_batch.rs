
use qexed_chunk_by_tcp::{ChunkTcpClient, Dimension};

#[tokio::main(flavor = "multi_thread", worker_threads = 2)]
async fn main() {
    let mut client = ChunkTcpClient::connect_local().await.expect("connect");
    let mut coords = Vec::new();
    for x in 0..9 { for z in 0..9 { coords.push((x, z)); } }

    let t0 = std::time::Instant::now();
    let datas = client.request_chunks(&coords, Dimension::Overworld).await.expect("batch");
    let t1 = t0.elapsed();
    println!("batch of {} chunks: {:?}", datas.len(), t1);

    let t2 = std::time::Instant::now();
    let datas2 = client.request_chunks(&coords, Dimension::Overworld).await.expect("batch2");
    println!("batch2 of {} chunks: {:?}", datas2.len(), t2.elapsed());
    let _ = datas2;
}
