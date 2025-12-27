use std::f32::consts::E;

use async_trait::async_trait;
use dashmap::DashMap;
use qexed_packet::net_types::{Bitset, VarInt};
use qexed_task::{
    event::task::TaskEvent,
    message::{MessageSender, return_message::ReturnMessage, unreturn_message::UnReturnMessage},
};

use crate::{
    engine::mini_lobby::event::chunk::ChunkTask,
    message::{
        chunk::{ChunkCommand, ChunkData},
        region::RegionCommand,
    },
};
use qexed_tcp_connect::PacketSend;

#[async_trait]
impl TaskEvent<UnReturnMessage<ChunkCommand>, UnReturnMessage<RegionCommand>> for ChunkTask {
    async fn event(
        &mut self,
        api: &MessageSender<UnReturnMessage<ChunkCommand>>,
        manage_api: &MessageSender<UnReturnMessage<RegionCommand>>,
        data: UnReturnMessage<ChunkCommand>,
    ) -> anyhow::Result<bool> {
        match data.data {
            ChunkCommand::Init => {
                // 初始化函数暂时没写
            }
            ChunkCommand::PlayerJoin {
                uuid,
                pos,
                packet_send,
            } => {
               
                // 暂时旧版空区块
                let p_q = qexed_protocol::to_client::play::map_chunk::MapChunk {
                    chunk_x: self.pos[0] as i32,
                    chunk_z: self.pos[1] as i32,
                    data: qexed_protocol::to_client::play::map_chunk::Chunk {
                        // 高度图 - 使用修复后的高度图
                        heightmaps: create_heightmaps(),
                        // 空的区块数据 - 使用修复后的编码函数
                        data: encode_empty_chunk_data_1_21(),
                        // 无方块实体
                        block_entities: vec![],
                    },
                    light: create_light_data_for_all_sections(),
                };
                packet_send.send(PacketSend::build_send_packet(p_q).await?)?;
            }
            ChunkCommand::CloseCommand { result } => {
                // 暂时没写数据读写
                result.send(ChunkData::default());
            }
        }
        Ok(false)
    }
}

fn encode_empty_chunk_data_1_21() -> Vec<u8> {
    let mut data = Vec::new();

    // 1.21.8 使用 24 个区块段落 (从 y=-64 到 y=319)
    for d in 0..24 {
        // 段落非空气方块数量为 0
        // 段落非空气方块数量为 0
        if d == 0 {
            data.extend_from_slice(&256i16.to_be_bytes());
        } else {
            data.extend_from_slice(&0i16.to_be_bytes());
        }       

        // 方块状态
        if d == 0 {
            // 第一个段落有基岩和空气两种方块
            let bits_per_block = 4; // 需要至少4位来表示0-15的索引
            data.push(bits_per_block as u8);

            // 调色板长度 - 使用 VarInt 编码
            data.extend(encode_var_int(16)); // 需要定义16个调色板条目

            // 定义所有可能的调色板条目（0-15）
            for i in 0..16 {
                if i == 1 {
                    // 索引1对应基岩
                    data.extend(encode_var_int(1));
                } else {
                    // 其他索引对应空气
                    data.extend(encode_var_int(0));
                }
            }
        } else {
            // 其他段落只有空气方块
            let bits_per_block = 1; // 只需要1位，因为只有空气
            data.push(bits_per_block as u8);

            // 调色板长度 - 使用 VarInt 编码
            data.extend(encode_var_int(1));

            // 空气方块的 ID
            data.extend(encode_var_int(0));
        }       

        // 计算需要多少个 long 来存储 4096 个方块
        let bits_per_block = if d == 0 { 4 } else { 1 };
        let blocks_per_long = 64 / bits_per_block;
        let num_longs = (4096 + blocks_per_long - 1) / blocks_per_long;     

        // 设置方块数据
        if d == 0 {
            // 第一个段落: 最底层是基岩 (索引1)，其余是空气 (索引0)
            for i in 0..num_longs {
                let mut long_value = 0i64;

                // 每个long包含多个方块
                for j in 0..blocks_per_long {
                    let block_index = i * blocks_per_long + j;

                    // 检查这个方块是否在最底层 (y=-64)
                    if block_index < 256 {
                        // 最底层方块是基岩 (调色板索引1)
                        long_value |= 1 << (j * bits_per_block);
                    }
                    // 其他方块保持为0 (空气，调色板索引0)
                }

                data.extend_from_slice(&long_value.to_be_bytes());
            }
        } else {
            // 其他段落: 所有方块都是空气 (调色板索引 0)
            for _ in 0..num_longs {
                data.extend_from_slice(&0i64.to_be_bytes());
            }
        }


        // 生物群系数据
        // 使用调色板模式，只有一个生物群系
        let bits_per_biome = 1; // 只需要 1 位，因为只有一种生物群系
        data.push(bits_per_biome as u8);

        // 生物群系调色板长度 - 使用 VarInt 编码
        data.extend(encode_var_int(1));

        // 平原生物群系的 ID
        data.extend(encode_var_int(1));

        // 计算需要多少个 long 来存储 64 个生物群系 (4x4x4)
        let biomes_per_long = 64 / bits_per_biome;
        let num_biome_longs = (64 + biomes_per_long - 1) / biomes_per_long;

        // 所有生物群系都是平原 (调色板索引 0)
        for _ in 0..num_biome_longs {
            data.extend_from_slice(&0i64.to_be_bytes());
        }
    }
    data
}

fn create_heightmaps() -> Vec<qexed_protocol::to_client::play::map_chunk::Heightmaps> {
    vec![
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(0), // MOTION_BLOCKING
            // 高度图应该包含 256 个值（16x16），每个值是一个 VarLong
            // 对于空区块，所有高度都是世界底部（-64）
            data: vec![0; 37], // 这个大小可能需要调整
        },
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(1), // WORLD_SURFACE
            data: vec![0; 37],  // 这个大小可能需要调整
        },
    ]
}
fn encode_var_int(value: i32) -> Vec<u8> {
    let mut value = value as u32;
    let mut buf = Vec::new();
    loop {
        if value & !0x7F == 0 {
            buf.push(value as u8);
            break;
        } else {
            buf.push((value as u8 & 0x7F) | 0x80);
            value >>= 7;
        }
    }
    buf
}
fn create_light_data_for_all_sections() -> qexed_protocol::to_client::play::map_chunk::Light {
    let total_sections = 24; // 从 y=-64 到 y=319

    // 1. 设置光照掩码 - 所有段落都需要更新光照
    let mut sky_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);
    let mut block_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);

    // 设置所有段落
    for i in 0..total_sections {
        let index = i / 64;
        let bit = i % 64;
        sky_light_mask.0[index] |= 1 << bit;
        block_light_mask.0[index] |= 1 << bit;
    }

    // 2. 空光照掩码设置为空（没有段落被标记为空）
    let empty_sky_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);
    let empty_block_light_mask = Bitset(vec![0; (total_sections + 63) / 64]);

    // 3. 创建光照数据 - 为每个段落创建光照数据
    let mut sky_light_arrays = Vec::new();
    let mut block_light_arrays = Vec::new();

    for _ in 0..total_sections {
        let mut sky_light_data = vec![0u8; 2048];
        let mut block_light_data = vec![0u8; 2048];

        // 设置全部方块为最大方块光照 (15)
        for i in 0..2048 {
            block_light_data[i] = 0xFF; // 每个字节存储两个15值 (0xF = 15)
            // 同时设置天空光照为最大值
            sky_light_data[i] = 0xFF;
        }

        sky_light_arrays.push(sky_light_data);
        block_light_arrays.push(block_light_data);
    }

    // 4. 返回Light结构体
    qexed_protocol::to_client::play::map_chunk::Light {
        sky_light_mask,
        block_light_mask,
        empty_sky_light_mask,
        empty_block_light_mask,
        sky_light_arrays,
        block_light_arrays,
    }
}
// 创建一个简单的 /qexed 命令

