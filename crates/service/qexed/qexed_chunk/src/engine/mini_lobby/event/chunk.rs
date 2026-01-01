use std::{collections::HashMap, path::PathBuf};

use qexed_packet::net_types::{Bitset, VarInt};
use qexed_task::message::{MessageSender, unreturn_message::UnReturnMessage};
use qexed_tcp_connect::PacketSend;
use uuid::Uuid;

use crate::{data_type::direction::DirectionMap, message::chunk::ChunkCommand};
#[derive(Debug)]
pub struct ChunkTask {
    // 世界配置文件
    pub config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
    // 世界目录
    world_root: PathBuf,
    // 世界uuid
    world_uuid: uuid::Uuid,
    // 区块数据
    pub chunk: Option<qexed_region::chunk::nbt::Chunk>,
    // 区块坐标 pos
    pub pos: [i64; 2],
    // 相邻区块
    direction_chunk: DirectionMap<MessageSender<UnReturnMessage<ChunkCommand>>>,
    // 跨维度对应区块API
    cross_dimension_counterpart_apis: HashMap<Uuid, MessageSender<UnReturnMessage<ChunkCommand>>>,
    // 当前区块直属玩家API管道

    // 是否属于地图，否则空区块
    pub map_chunk: bool,
    // 当前区块信息
    pub chunk_packet: Option<bytes::Bytes>,
}
impl ChunkTask {
    pub fn new(
        // 世界配置文件
        config: qexed_config::app::qexed_chunk::engine::mini_lobby::MiniLobbyConfig,
        world_root: PathBuf,
        world_uuid: uuid::Uuid,
        pos: [i64; 2],
        chunk: qexed_region::chunk::nbt::Chunk,
        map_chunk: bool,
    ) -> Self {
        Self {
            config,
            world_root,
            world_uuid,
            pos,
            chunk: Some(chunk),
            direction_chunk: Default::default(),
            cross_dimension_counterpart_apis: Default::default(),
            map_chunk,
            chunk_packet: None,
        }
    }
    pub async fn init(&mut self) -> anyhow::Result<()> {
        let chunk = match self.chunk.take() {
            Some(v) => v,
            None => return Ok(()),
        };
        let p_q: qexed_protocol::to_client::play::map_chunk::MapChunk = if self.map_chunk {
            let mut chunk_bytes = vec![];
            // let mut chunk_date = vec![];
            for i in &chunk.sections {
                if (i.y < -4) || (i.y > 19) {
                    // 主世界的区块的高度为-4~19(加起来24)
                    continue;
                }
                let mut block_status_id_vec = vec![];
                for block in &i.block_states.palette {
                    let a = block.get_state_id();
                    if let Some(t) = a {
                        if !block_status_id_vec.contains(&t) {
                            block_status_id_vec.push(t);
                        };
                    }
                }
                // 绘画板长度,不是PCL启动器！！！
                let pcl = block_status_id_vec.len();
                // 只有一种方块，看都不看
                // 直接0
                if pcl == 1 {
                    let block_id = block_status_id_vec[0];
                    if block_id == 0 {
                        chunk_bytes.extend_from_slice(&0i16.to_be_bytes());
                    } else {
                        chunk_bytes.extend_from_slice(&4096i16.to_be_bytes());
                    }

                    let bits_per_block = 0; // 只需要1位，因为只有空气
                    chunk_bytes.push(bits_per_block as u8);
                    // 将屏障方块的ID放入调色板，其索引为0
                    chunk_bytes.extend(encode_var_int(block_id as i32));
                    // 生物群系数据
                    // 使用调色板模式，只有一个生物群系
                    let bits_per_biome = 1; // 只需要 1 位，因为只有一种生物群系
                    chunk_bytes.push(bits_per_biome as u8);

                    // 生物群系调色板长度 - 使用 VarInt 编码
                    chunk_bytes.extend(encode_var_int(1));

                    // 平原生物群系的 ID
                    chunk_bytes.extend(encode_var_int(1));

                    // 计算需要多少个 long 来存储 64 个生物群系 (4x4x4)
                    let biomes_per_long = 64 / bits_per_biome;
                    let num_biome_longs = (64 + biomes_per_long - 1) / biomes_per_long;

                    // 所有生物群系都是平原 (调色板索引 0)
                    for _ in 0..num_biome_longs {
                        chunk_bytes.extend_from_slice(&0i64.to_be_bytes());
                    }
                } else if pcl <= 16 {
                    let bits_per_block = 4;
                    // 统计空气方块数
                    let block_data: &Vec<i64> = match &i.block_states.data {
                        Some(v) => v,
                        None => {
                            return Err(anyhow::anyhow!("区块数据损坏"));
                        }
                    };
                    // 注: block_status_id_vec: Vec<u32>
                    // 空气的方块状态为0
                    let mut block_date_byte = vec![];
                    block_date_byte.push(4u8);
                    block_date_byte.extend(encode_var_int(pcl as i32));
                    for id in &block_status_id_vec {
                        block_date_byte.extend(encode_var_int(*id as i32));
                    }
                    // 一次性处理所有方块
                    let total_blocks = 4096;
                    let u = (64 / bits_per_block) as usize;
                    let expected_data_len = (total_blocks + u - 1) / u;
                    if block_data.len() < expected_data_len {
                        return Err(anyhow::anyhow!(
                            "区块数据损坏：预期{}个长整数，实际{}个",
                            expected_data_len,
                            block_data.len()
                        ));
                    }
                    let block_status_id_vers = block_status_id_vec.clone();
                    let (air_count, palette_indices): (usize, Vec<u32>) = (0..total_blocks)
                        .map(|i| get_palette_index(i, block_data, bits_per_block))
                        .fold(
                            (0, Vec::with_capacity(total_blocks)),
                            |(air_count, mut indices), palette_index| {
                                // 检查索引有效性并统计空气方块
                                let is_air = if (palette_index as usize) < pcl {
                                    block_status_id_vers[palette_index as usize] == 0
                                } else {
                                    false // 无效索引不视为空气
                                };

                                indices.push(palette_index);
                                (air_count + if is_air { 1 } else { 0 }, indices)
                            },
                        );
                    chunk_bytes.extend_from_slice(&(air_count as i16).to_be_bytes());
                    chunk_bytes.extend_from_slice(&block_date_byte);

                    // 写入方块数据 - 完善的部分开始
                    let mut block_data_bytes = Vec::new();
                    let mut current_long: i64 = 0;
                    let mut current_bit_offset = 0;

                    for (i, &palette_index) in palette_indices.iter().enumerate() {
                        current_long |= (palette_index as i64) << current_bit_offset;
                        current_bit_offset += bits_per_block;

                        if current_bit_offset >= 64 || i == total_blocks - 1 {
                            block_data_bytes.extend_from_slice(&current_long.to_be_bytes());
                            current_long = 0;
                            current_bit_offset = 0;
                        }
                    }

                    chunk_bytes.extend_from_slice(&block_data_bytes);
                    // 生物群系数据
                    // 使用调色板模式，只有一个生物群系
                    let bits_per_biome = 0; // 只需要 1 位，因为只有一种生物群系
                    chunk_bytes.push(bits_per_biome as u8);
                    // 平原生物群系的 ID
                    chunk_bytes.extend(encode_var_int(1));
                } else {
                    let mut bits_per_block: u32 = (pcl as f64).log2().ceil() as u32;
                    if bits_per_block > 8 {
                        bits_per_block = 15
                    }
                    let block_data: &Vec<i64> = match &i.block_states.data {
                        Some(v) => v,
                        None => {
                            return Err(anyhow::anyhow!("区块数据损坏"));
                        }
                    };
                    let total_blocks = 4096;
                    let block_status_id_vers = block_status_id_vec.clone();
                    let (air_count, palette_indices): (usize, Vec<u32>) = (0..total_blocks)
                        .map(|i| get_palette_index(i, block_data, bits_per_block))
                        .fold(
                            (0, Vec::with_capacity(total_blocks)),
                            |(air_count, mut indices), palette_index| {
                                // 检查索引有效性并统计空气方块
                                let is_air = if (palette_index as usize) < pcl {
                                    block_status_id_vers[palette_index as usize] == 0
                                } else {
                                    false // 无效索引不视为空气
                                };

                                indices.push(palette_index);
                                (air_count + if is_air { 1 } else { 0 }, indices)
                            },
                        );
                    let i64_array = if bits_per_block == 15 {
                        // 2. 统计空气方块并打包到i64数组
                        let mut i64_array = vec![0u64; 1024]; // 1024个i64
                        for (index, &block_index) in palette_indices.iter().enumerate() {
                            let block_id: u16 = match block_status_id_vec.get(block_index as usize)
                            {
                                Some(&v) => v as u16,
                                None => 0,
                            };
                            let block_id_masked = block_id & 0x7FFF;
                            let i64_index = index / 4;
                            let position_in_i64 = index % 4;
                            let shift = position_in_i64 * 15;

                            i64_array[i64_index] |= (block_id_masked as u64) << shift;
                        }
                        i64_array
                    } else {
                        // bits_per_block 保证在5-8之间
                        let blocks_per_long = 64 / bits_per_block as usize; // 每个u64可以存储的方块数
                        let num_longs = (4096 + blocks_per_long - 1) / blocks_per_long; // 需要的u64数量

                        let mut i64_array = vec![0u64; num_longs];

                        for (index, &palette_index) in palette_indices.iter().enumerate() {
                            // 计算当前方块在哪个u64中
                            let long_index = index / blocks_per_long;

                            // 计算在当前u64中的位偏移
                            let position_in_long = index % blocks_per_long;
                            let shift = position_in_long * bits_per_block as usize;

                            // 确保索引值在有效范围内
                            let masked_index = palette_index & ((1 << bits_per_block) - 1);

                            // 将索引值设置到对应的位位置
                            i64_array[long_index] |= (masked_index as u64) << shift;
                        }

                        i64_array
                    };

                    // 3. 写入非空气方块数
                    let non_air_count = 4096 - air_count;
                    chunk_bytes.extend_from_slice(&(non_air_count as i16).to_be_bytes());

                    // 4. 写入 bits_per_block
                    chunk_bytes.push(bits_per_block as u8);
                    if bits_per_block != 15 {
                        chunk_bytes.extend(encode_var_int(pcl as i32));
                        for id in &block_status_id_vec {
                            chunk_bytes.extend(encode_var_int(*id as i32));
                        }
                    }
                    // 5. 写入方块数据 (8192字节)
                    for &value in &i64_array {
                        chunk_bytes.extend_from_slice(&value.to_be_bytes());
                    }

                    // 生物群系数据
                    // 使用调色板模式，只有一个生物群系
                    let bits_per_biome = 0; // 只需要 1 位，因为只有一种生物群系
                    chunk_bytes.push(bits_per_biome as u8);
                    // 平原生物群系的 ID
                    chunk_bytes.extend(encode_var_int(1));
                }
            }

            // 暂时旧版空区块
            qexed_protocol::to_client::play::map_chunk::MapChunk {
                chunk_x: self.pos[0] as i32,
                chunk_z: self.pos[1] as i32,
                data: qexed_protocol::to_client::play::map_chunk::Chunk {
                    // 高度图 - 使用修复后的高度图
                    heightmaps: create_heightmaps(),
                    // 空的区块数据 - 使用修复后的编码函数
                    data: chunk_bytes,
                    // 无方块实体
                    block_entities: vec![],
                },
                light: create_light_data_for_all_sections(),
            }
        } else {
            // 全屏障区块
            qexed_protocol::to_client::play::map_chunk::MapChunk {
                chunk_x: self.pos[0] as i32,
                chunk_z: self.pos[1] as i32,
                data: qexed_protocol::to_client::play::map_chunk::Chunk {
                    heightmaps: vec![], //create_heightmaps(),
                    data: encode_barrier_chunk_data_1_21(),
                    block_entities: vec![],
                },
                light: create_light_data_for_all_sections(),
            }
        };
        self.chunk_packet = Some(PacketSend::build_send_packet(p_q).await?);
        drop(chunk);
        Ok(())
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
// qexed_block::BlockId::Barrier
/// 为 Minecraft 1.21.8 编码全屏障方块的区块数据（修复版）
/// 整个区块段落内只有屏障一种方块，没有空气。
fn encode_barrier_chunk_data_1_21() -> Vec<u8> {
    let mut data = Vec::new();

    // 关键：必须通过注册表或协议库动态获取屏障方块的正确ID，切勿硬编码。
    let barrier_block_state_id = 11255; // 示例：返回 1234

    // 1.21.8 版本中，一个完整的区块有 24 个子区块（从 y=-64 到 y=319）
    for _chunk_section_index in 0..24 {
        // 1. 非空气方块数量：整个子区块都是屏障，所以是4096个方块均为“非空气”
        data.extend_from_slice(&4096i16.to_be_bytes());
        data.push(0u8);
        // 将屏障方块的ID放入调色板，其索引为0
        data.extend(encode_var_int(barrier_block_state_id));
        // 生物群系数据
        data.push(0u8);
        // 平原生物群系的 ID
        data.extend(encode_var_int(1));
    }
    data
}
/// 创建适用于全屏障方块区块的高度图数据
/// 屏障是固体方块，因此高度图应设置为世界顶部（Y=319）
/// 对于主世界（最小Y=-64，最大Y=319），高度图值存储的是相对于世界底部的差值（无负值）
/// 高度图包含256个值（16x16），使用9比特编码每个高度值，打包在37个u64中
pub fn create_barrier_heightmaps() -> Vec<qexed_protocol::to_client::play::map_chunk::Heightmaps> {
    // 主世界参数配置
    const WORLD_MIN_Y: i32 = -64; // 世界最低点
    const WORLD_MAX_Y: i32 = 319; // 世界最高点
    const WORLD_HEIGHT: i32 = WORLD_MAX_Y - WORLD_MIN_Y + 1; // 总高度384
    const BITS_PER_VALUE: usize = 9; // 需要9比特存储0-383的值
    const VALUES_PER_LONG: usize = 64 / BITS_PER_VALUE; // 每个u64存储7个值
    const NUM_LONGS: usize = (256 + VALUES_PER_LONG - 1) / VALUES_PER_LONG; // 需要37个u64

    // 计算高度图值：屏障在Y=319，对应高度图值 = 319 - (-64) = 383
    let barrier_height_value = (WORLD_MAX_Y - WORLD_MIN_Y) as u64;

    // 创建所有位置高度均为383的数组（256个值）
    let mut height_values = [barrier_height_value; 256];

    // 确保值在有效范围内（0-383）
    for value in height_values.iter_mut() {
        *value = (*value).min((1 << BITS_PER_VALUE) - 1);
    }

    // 打包高度图数据
    let packed_data =
        pack_heightmap_values(&height_values, BITS_PER_VALUE, VALUES_PER_LONG, NUM_LONGS);

    // 返回两种主要高度图类型
    vec![
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(0), // MOTION_BLOCKING
            data: packed_data.clone(),
        },
        qexed_protocol::to_client::play::map_chunk::Heightmaps {
            type_id: VarInt(1), // WORLD_SURFACE
            data: packed_data,
        },
    ]
}

/// 将高度值数组打包成u64数组
/// 按照Minecraft协议要求：行主序排列，每个值用9比特存储
fn pack_heightmap_values(
    height_values: &[u64; 256],
    bits_per_value: usize,
    values_per_long: usize,
    num_longs: usize,
) -> Vec<u64> {
    let mut packed = Vec::with_capacity(num_longs);
    let value_mask = (1u64 << bits_per_value) - 1; // 用于掩码操作

    for long_index in 0..num_longs {
        let mut long_value: u64 = 0;

        for value_offset in 0..values_per_long {
            let value_index = long_index * values_per_long + value_offset;

            // 处理最后一个long可能不满的情况
            if value_index >= 256 {
                break;
            }

            // 获取高度值并应用掩码
            let height_val = height_values[value_index] & value_mask;

            // 将值移动到正确比特位并合并到long中
            long_value |= height_val << (value_offset * bits_per_value);
        }

        packed.push(long_value);
    }

    packed
}

/// 遍历所有方块并处理（高效迭代器版本）
fn process_blocks_efficiently(
    block_data: &[i64],
    bits_per_block: u32,
    block_status_id_vec: &[u32],
) -> anyhow::Result<(usize, Vec<u8>)> {
    let total_blocks = 4096; // 16x16x16

    // 验证数据完整性
    let u = (64 / bits_per_block) as usize;
    let expected_data_len = (total_blocks + u - 1) / u;
    if block_data.len() < expected_data_len {
        return Err(anyhow::anyhow!(
            "区块数据损坏：预期{}个长整数，实际{}个",
            expected_data_len,
            block_data.len()
        ));
    }

    // 🎯 核心：使用迭代器一次性完成所有操作
    let (air_count, palette_indices): (usize, Vec<u32>) = (0..total_blocks)
        .map(|i| get_palette_index(i, block_data, bits_per_block))
        .fold(
            (0, Vec::with_capacity(total_blocks)),
            |(air_count, mut indices), palette_index| {
                // 检查索引有效性并统计空气方块
                let is_air = if (palette_index as usize) < block_status_id_vec.len() {
                    block_status_id_vec[palette_index as usize] == 0
                } else {
                    false // 无效索引不视为空气
                };

                indices.push(palette_index);
                (air_count + if is_air { 1 } else { 0 }, indices)
            },
        );

    // 构建最终的字节数据（基于您现有的代码逻辑）
    let mut block_date_byte = Vec::new();
    block_date_byte.push(bits_per_block as u8);
    block_date_byte.extend(encode_var_int(block_status_id_vec.len() as i32)); // 假设pcl是调色板长度

    // 添加调色板内容
    for &id in block_status_id_vec {
        block_date_byte.extend(encode_var_int(id as i32));
    }

    // 添加方块数据（如果需要）
    // 这里可以根据palette_indices继续构建数据...

    Ok((air_count, block_date_byte))
}
/// 从block_data中提取指定位置的调色板索引
fn get_palette_index(i: usize, block_data: &[i64], bits_per_block: u32) -> u32 {
    let u = (64 / bits_per_block) as usize; // 每个i64存储的方块数
    let array_index = i / u;
    let bit_offset = (i % u) * bits_per_block as usize;
    let mask = (1u64 << bits_per_block) - 1;

    let data_val = block_data[array_index] as u64;
    let value = (data_val >> bit_offset) & mask;
    value as u32
}
fn write_generic_packed(output: &mut Vec<u8>, block_ids: &[u32], bits_per_block: usize) {
    let mut bit_buffer: u64 = 0;
    let mut bits_in_buffer = 0;
    let mask = (1u64 << bits_per_block) - 1;

    for &block_id in block_ids {
        bit_buffer = (bit_buffer << bits_per_block) | (block_id as u64 & mask);
        bits_in_buffer += bits_per_block;

        while bits_in_buffer >= 8 {
            let byte = (bit_buffer >> (bits_in_buffer - 8)) as u8;
            output.push(byte);
            bits_in_buffer -= 8;
        }
    }

    if bits_in_buffer > 0 {
        let byte = (bit_buffer << (8 - bits_in_buffer)) as u8;
        output.push(byte);
    }
}
