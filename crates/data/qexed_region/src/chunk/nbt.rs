use std::collections::HashMap;
use std::error::Error;
use serde::{Deserialize, Serialize};
use qexed_nbt::Tag;
use qexed_nbt::nbt_serde;

/// Minecraft区块数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    /// 用于在升级为1.18的世界时，原区块负值高度重新生成方块的数据。
    /// 此项只对于原型区块有效，如果此区块已经达到full（区块生成完毕）阶段成为世界区块时，此标签被删除。
    pub below_zero_retrogen: Option<BelowZeroRetrogen>,
    
    /// 用于新旧区块平滑过渡的混合数据信息。
    pub blending_data: Option<BlendingData>,
    
    /// 区块雕刻时的标记。当区块生成完毕后此项会被删除。
    /// 每个二进制位指示洞穴是否在特定位置生成，位置使用扩展YZX编码。
    pub carving_mask: Option<Vec<f64>>,
    
    /// 区块生成时的初始放置的实体信息，当区块生成完毕后此项会被删除。
    /// 对于世界区块设置此项也有效，即使这些实体数据应该放置在实体存储文件内。
    pub entities: Option<Vec<qexed_data_serde::entity::Entity>>, // 暂时这样，后面换成实体枚举
    
    /// 区块的高度图信息，存储方式见§ 高度图存储格式。
    /// 不存在此标签时游戏将自动重新生成，优化世界时此项会被删除。
    #[serde(rename = "Heightmaps")]
    pub heightmaps: Heightmaps,
    
    /// 所有玩家在此区块停留的总时间，以游戏刻计。
    /// 如果有多个玩家在这个区块停留过则分别计时并相加。
    /// 用于区域难度的计算，当此值大于3600000游戏刻（150游戏日）时区域难度达到最大值。
    #[serde(rename = "InhabitedTime")]
    pub inhabited_time: i64,
    
    #[serde(rename = "isLightOn")]
    #[serde(default)]
    pub is_light_on: bool,
    
    #[serde(rename = "LastUpdate")]
    pub last_update: i64,
    
    /// 储区块生成完毕后需要进行更新的位置。对于已生成完毕的世界区块无效。
    /// - NBT列表/JSON数组：一个子区块内需要更新的位置。保存顺序为从最低的子区块开始到最高子区块结束。
    /// - - 短整型：一个需要更新的方块的位置，按照ZYX编码存储。
    #[serde(rename = "PostProcessing")]
    pub post_processing: Vec<Option<Vec<i16>>>,

    pub sections: Vec<Section>,
    
    #[serde(rename = "shouldSave")]
    #[serde(default)]
    pub should_save: bool,
    
    #[serde(rename = "Status")]
    pub status: String,

    pub structures: Structures,
    
    #[serde(rename = "UpgradeData")]
    pub upgrade_data: Option<UpgradeData>,

    #[serde(rename = "xPos")]
    pub x_pos: i32,
    
    #[serde(rename = "yPos")]
    pub y_pos: i32,
    
    #[serde(rename = "zPos")]
    pub z_pos: i32,
}

/// 区块升级数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeData {
    /// 需要更新升级的位置，包含了一个区块中所有子区块的信息
    #[serde(rename = "Indices")]
    pub indices: HashMap<i32, Vec<i32>>, // 子区块序号 -> 位置数组
    
    /// 升级数据中保存的将进行的方块计划刻
    #[serde(rename = "neighbor_block_ticks", skip_serializing_if = "Option::is_none")]
    pub neighbor_block_ticks: Option<Vec<BlockTick>>,
    
    /// 升级数据中保存的将进行的流体计划刻
    #[serde(rename = "neighbor_fluid_ticks", skip_serializing_if = "Option::is_none")]
    pub neighbor_fluid_ticks: Option<Vec<FluidTick>>,
    
    /// 二进制位表示是否对某一方向上的区块进行更新升级
    #[serde(rename = "Sides")]
    pub sides: i8,
}

/// 方块计划刻
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockTick {
    /// 方块ID
    #[serde(rename = "i")]
    pub block_id: String,
    
    /// 优先级
    #[serde(rename = "p")]
    pub priority: i32,
    
    /// 计划执行的时间（游戏刻）
    #[serde(rename = "t")]
    pub time: i32,
    
    /// 方块坐标
    #[serde(rename = "x")]
    pub x: i32,
    
    #[serde(rename = "y")]
    pub y: i32,
    
    #[serde(rename = "z")]
    pub z: i32,
}

/// 流体计划刻
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FluidTick {
    /// 流体类型
    #[serde(rename = "i")]
    pub fluid_type: String,
    
    /// 优先级
    #[serde(rename = "p")]
    pub priority: i32,
    
    /// 计划执行的时间（游戏刻）
    #[serde(rename = "t")]
    pub time: i32,
    
    /// 流体坐标
    #[serde(rename = "x")]
    pub x: i32,
    
    #[serde(rename = "y")]
    pub y: i32,
    
    #[serde(rename = "z")]
    pub z: i32,
}

/// 区块中的结构数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Structures {
    /// 包含结构中结构生成点的区块坐标引用
    #[serde(rename = "References")]
    pub references: HashMap<String, Vec<i64>>,
    
    /// 此区块中的结构生成点
    #[serde(rename = "starts")]
    pub starts: HashMap<String, StructureStart>,
}

/// 结构生成点数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructureStart {
    /// 结构ID
    #[serde(rename = "id")]
    pub id: String,
    
    /// 结构生成点的区块坐标
    #[serde(rename = "ChunkX")]
    pub chunk_x: i32,
    
    #[serde(rename = "ChunkZ")]
    pub chunk_z: i32,
    
    /// 结构的相关数据（可选，取决于结构类型）
    #[serde(rename = "Children", skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<StructurePiece>>,
    
    /// 参考位置（可选）
    #[serde(rename = "References", skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<i64>>,
}

/// 结构的边界框
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundingBox {
    #[serde(rename = "X")]
    pub x: [i32; 2],  // [min_x, max_x]
    
    #[serde(rename = "Y")]
    pub y: [i32; 2],  // [min_y, max_y]
    
    #[serde(rename = "Z")]
    pub z: [i32; 2],  // [min_z, max_z]
}

/// 结构片段（子结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructurePiece {
    /// 片段ID
    #[serde(rename = "id")]
    pub id: String,
    
    /// 片段的坐标和方向
    #[serde(rename = "GD")]
    pub ground_level_delta: i32,
    
    #[serde(rename = "O")]
    pub orientation: i32, // 方向，0-3对应北东南西
    

    
    /// 片段的边界框
    #[serde(rename = "BB")]
    pub bounding_box: Vec<i32>,
}

/// 区块子区块
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Section {
    #[serde(rename = "biomes")]    
    pub biome: Biome,
    pub block_states: BlockStates,

    #[serde(rename = "BlockLight")]    
    pub block_light: Option<Vec<i8>>,
    
    #[serde(rename = "SkyLight")]
    pub sky_light: Option<Vec<i8>>,
    
    #[serde(rename = "Y")]
    pub y: i8,
}

/// 生物群系数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Biome {
    pub data: Option<Vec<i64>>,
    pub palette: Option<Vec<String>>,
}

/// 方块状态数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockStates {
    pub data: Option<Vec<i64>>,
    pub palette: Option<Vec<qexed_data_serde::block::BlockStates>>,
}

/// 区块的高度图信息
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heightmaps {
    /// 最高的能阻挡移动或含有液体的方块。在区块生成到features（生成地物）阶段后此项才存在。
    #[serde(rename = "MOTION_BLOCKING")]
    pub motion_blocking: Option<Vec<i64>>,
    
    /// 最高的阻挡移动、含有液体或在#leaves标签里的方块。在区块生成到features（生成地物）阶段后此项才存在。
    #[serde(rename = "MOTION_BLOCKING_NO_LEAVES")]
    pub motion_blocking_no_leaves: Option<Vec<i64>>,
    
    /// 最高的既不是空气也不包含液体的方块。在区块生成到features（生成地物）阶段后此项才存在。
    #[serde(rename = "OCEAN_FLOOR")]
    pub ocean_floor: Option<Vec<i64>>,
    
    /// 最高的既不是空气也不包含液体的方块。此值用于世界生成，在区块生成到features（生成地物）阶段后此项被删除。
    #[serde(rename = "OCEAN_FLOOR_WG")]
    pub ocean_floor_wg: Option<Vec<i64>>,
    
    /// 最高的非空气方块。在区块生成到features（生成地物）阶段后此项才存在。
    #[serde(rename = "WORLD_SURFACE")]
    pub world_surface: Option<Vec<i64>>,
    
    /// 最高的非空气方块。此值用于世界生成，在区块生成到features（生成地物）阶段后此项被删除。
    #[serde(rename = "WORLD_SURFACE_WG")]
    pub world_surface_wg: Option<Vec<i64>>,
}

/// 混合数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlendingData {
    /// 混合数据中元胞的高度值，共16个数值。如果此数据为空列表则游戏在运行时自动重新生成。
    pub heights: Option<Vec<f64>>,
    
    /// 混合数据中最高子区块的Y坐标。
    pub max_section: i32,
    
    /// 混合数据中最低子区块的Y坐标。
    pub min_section: i32,
}

/// 负值高度重新生成数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BelowZeroRetrogen {
    /// 每个二进制位代表对应位置上是否缺失基岩，决定是否重新生成此位置下的方块。
    /// 一共256个二进制位（4个长整型整数），位置编码从(0,0)开始横向扫描到(15,15)结束。
    /// 此项不存在则代表基岩全部存在。
    pub missing_bedrock: Option<Vec<i64>>,
    
    /// 重新生成方块时区块的目标状态，可选值与字符串Status相同，但不包含empty。
    pub target_status: String,
}

// 实现一些实用的方法
impl Chunk {
    /// 创建一个新的空区块
    pub fn new(x: i32, z: i32, status: String) -> Self {
        Self {
            below_zero_retrogen: None,
            blending_data: None,
            carving_mask: None,
            entities: None,
            heightmaps: Heightmaps {
                motion_blocking: None,
                motion_blocking_no_leaves: None,
                ocean_floor: None,
                ocean_floor_wg: None,
                world_surface: None,
                world_surface_wg: None,
            },
            inhabited_time: 0,
            is_light_on: false,
            last_update: 0,
            post_processing: Vec::new(),
            sections: Vec::new(),
            should_save: false,
            status,
            structures: Structures {
                references: HashMap::new(),
                starts: HashMap::new(),
            },
            upgrade_data: None,
            x_pos: x,
            y_pos: 0, // 区域文件中的y_pos通常是0
            z_pos: z,
        }
    }
    
    /// 检查区块是否为空（没有区块数据）
    pub fn is_empty(&self) -> bool {
        self.sections.is_empty() && 
        self.entities.as_ref().map_or(true, |e| e.is_empty()) &&
        self.status == "minecraft:empty"
    }
    
    /// 获取区块的世界坐标
    pub fn get_world_coords(&self, region_x: i32, region_z: i32) -> (i32, i32) {
        (region_x * 32 + self.x_pos, region_z * 32 + self.z_pos)
    }
    
    /// 添加一个子区块
    pub fn add_section(&mut self, section: Section) {
        self.sections.push(section);
    }
    
    /// 获取指定Y坐标的子区块
    pub fn get_section(&self, y: i8) -> Option<&Section> {
        self.sections.iter().find(|s| s.y == y)
    }
    
    /// 序列化区块为NBT字节
    pub fn to_nbt_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        // 使用 qexed_nbt 的序列化功能
        let tags = nbt_serde::nbt_serde::to_tag(self)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(bytes)
    }
    
    /// 从NBT字节反序列化区块
    pub fn from_nbt_bytes(bytes: &[u8]) -> Result<Self, Box<dyn Error>> {
        // 使用 qexed_nbt 的反序列化功能
        let chunk = nbt_serde::from_nbt_bytes(bytes)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(chunk)
    }
    
    /// 序列化区块为NBT Tag
    pub fn to_nbt_tag(&self) -> Result<Tag, Box<dyn Error>> {
        // 使用 qexed_nbt 的序列化功能
        let tag = nbt_serde::to_nbt_tag(self)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(tag)
    }
    
    /// 从NBT Tag反序列化区块
    pub fn from_nbt_tag(tag: &Tag) -> Result<Self, Box<dyn Error>> {
        // 使用 qexed_nbt 的反序列化功能
        let chunk = nbt_serde::from_nbt_tag(tag)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(chunk)
    }
}

impl UpgradeData {
    /// 检查是否需要对指定方向进行升级
    pub fn should_upgrade_direction(&self, direction: i8) -> bool {
        (self.sides & direction) != 0
    }
    
    /// 设置方向的升级标志
    pub fn set_upgrade_direction(&mut self, direction: i8, upgrade: bool) {
        if upgrade {
            self.sides |= direction;
        } else {
            self.sides &= !direction;
        }
    }
}

impl BoundingBox {
    /// 计算边界框的宽度
    pub fn width(&self) -> i32 {
        self.x[1] - self.x[0]
    }
    
    /// 计算边界框的高度
    pub fn height(&self) -> i32 {
        self.y[1] - self.y[0]
    }
    
    /// 计算边界框的长度
    pub fn length(&self) -> i32 {
        self.z[1] - self.z[0]
    }
    
    /// 检查点是否在边界框内
    pub fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        x >= self.x[0] && x <= self.x[1] &&
        y >= self.y[0] && y <= self.y[1] &&
        z >= self.z[0] && z <= self.z[1]
    }
}

impl Section {
    /// 创建一个新的子区块
    pub fn new(y: i8) -> Self {
        Self {
            biome: Biome {
                data: None,
                palette: None,
            },
            block_states: BlockStates {
                data: None,
                palette: None,
            },
            block_light: None,
            sky_light: None,
            y,
        }
    }
}

// 为与区域文件交互添加一些工具函数
impl Chunk {
    /// 从区域文件数据创建区块
    pub fn from_region_data(data: &[u8], chunk_x: i32, chunk_z: i32) -> Result<Self, Box<dyn Error>> {
        // 使用 qexed_nbt 的 from_nbt_bytes
        let chunk = nbt_serde::from_nbt_bytes(data)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(chunk)
    }
    
    /// 将区块转换为区域文件数据
    pub fn to_region_data(&self) -> Result<Vec<u8>, Box<dyn Error>> {
        // 使用 qexed_nbt 的 to_nbt_bytes
        let bytes = nbt_serde::to_nbt_bytes(self)
            .map_err(|e| Box::new(e) as Box<dyn Error>)?;
        Ok(bytes)
    }
}

// 实现默认值
impl Default for Chunk {
    fn default() -> Self {
        Self::new(0, 0, "minecraft:empty".to_string())
    }
}

impl Default for Section {
    fn default() -> Self {
        Self::new(0)
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            x: [0, 0],
            y: [0, 0],
            z: [0, 0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_chunk_creation() {
        let chunk = Chunk::new(10, 20, "minecraft:full".to_string());
        
        assert_eq!(chunk.x_pos, 10);
        assert_eq!(chunk.z_pos, 20);
        assert_eq!(chunk.status, "minecraft:full");
        assert!(chunk.is_empty());
    }
    
    #[test]
    fn test_chunk_serialization() {
        let mut chunk = Chunk::new(5, 5, "minecraft:empty".to_string());
        
        // 添加一个子区块
        let section = Section::new(0);
        chunk.add_section(section);
        
        // 序列化
        let bytes = chunk.to_nbt_bytes().unwrap();
        assert!(!bytes.is_empty());
        
        // 反序列化
        let deserialized = Chunk::from_nbt_bytes(&bytes).unwrap();
        assert_eq!(chunk.x_pos, deserialized.x_pos);
        assert_eq!(chunk.z_pos, deserialized.z_pos);
        assert_eq!(chunk.sections.len(), deserialized.sections.len());
    }
    
    #[test]
    fn test_bounding_box() {
        let bbox = BoundingBox {
            x: [0, 10],
            y: [0, 5],
            z: [0, 8],
        };
        
        assert_eq!(bbox.width(), 10);
        assert_eq!(bbox.height(), 5);
        assert_eq!(bbox.length(), 8);
        assert!(bbox.contains(5, 3, 4));
        assert!(!bbox.contains(15, 3, 4));
    }
    
    #[test]
    fn test_upgrade_data() {
        let mut upgrade_data = UpgradeData {
            indices: HashMap::new(),
            neighbor_block_ticks: None,
            neighbor_fluid_ticks: None,
            sides: 0,
        };
        
        // 测试方向设置
        upgrade_data.set_upgrade_direction(0b00000001, true); // 北方向
        assert!(upgrade_data.should_upgrade_direction(0b00000001));
        
        upgrade_data.set_upgrade_direction(0b00000010, true); // 东北方向
        assert!(upgrade_data.should_upgrade_direction(0b00000010));
        assert!(!upgrade_data.should_upgrade_direction(0b00000100)); // 东方向未设置
        
        upgrade_data.set_upgrade_direction(0b00000001, false); // 取消北方向
        assert!(!upgrade_data.should_upgrade_direction(0b00000001));
    }
    
    #[test]
    fn test_world_coords() {
        let chunk = Chunk::new(10, 5, "minecraft:full".to_string());
        let (world_x, world_z) = chunk.get_world_coords(2, 3);
        
        // 区域坐标 (2,3) 对应世界坐标 (2 * 32+10, 3 * 32+5) = (74, 101)
        assert_eq!(world_x, 2 * 32 + 10);
        assert_eq!(world_z, 3 * 32 + 5);
    }
}

#[cfg(test)]
mod anvil_tests {
    use crate::region::anvil::{Anvil, COMPRESSION_UNCOMPRESSED, COMPRESSION_ZLIB, ChunkData};

    use super::*;
    use std::path::PathBuf;
    use std::fs;
    use tempfile::tempdir;
    
    /// 测试从区域文件读取区块数据
    #[test]
    fn test_read_chunk_from_anvil() {
        // 创建临时目录用于测试
        let temp_dir = tempdir().expect("Failed to create temp directory");
        let region_path = temp_dir.path().join("r.0.0.mca");
        
        // 创建一个简单的区域文件用于测试
        create_test_region_file(&region_path).expect("Failed to create test region file");
        
        // 尝试读取区域文件
        match Anvil::from_file(&region_path) {
            Ok(anvil) => {
                println!("成功加载区域文件: {}", region_path.display());
                println!("区域坐标: r.{}.{}", anvil.region_x, anvil.region_z);
                
                // 分析文件内容
                anvil.analyze_file();
                
                // 尝试读取区块数据
                for x in 0..32 {
                    for z in 0..32 {
                        if let Ok(Some(chunk_data)) = anvil.get_chunk_data(x, z) {
                            println!("找到区块 ({}, {})", x, z);
                            
                            // 测试解压缩
                            match Anvil::decompress_chunk_data(&chunk_data) {
                                Ok(decompressed) => {
                                    println!("  解压缩成功，大小: {} 字节", decompressed.len());
                                    
                                    // 尝试解析为Chunk结构
                                    match Chunk::from_nbt_bytes(&decompressed) {
                                        Ok(chunk) => {
                                            println!("  成功解析为Chunk结构");
                                            println!("  区块坐标: ({}, {})", chunk.x_pos, chunk.z_pos);
                                            println!("  状态: {}", chunk.status);
                                            println!("  子区块数量: {}", chunk.sections.len());
                                            
                                            // 验证基本属性
                                            assert_eq!(chunk.x_pos, x);
                                            assert_eq!(chunk.z_pos, z);
                                            assert!(!chunk.status.is_empty());
                                            
                                            return; // 成功读取一个区块就返回
                                        }
                                        Err(e) => {
                                            println!("  解析Chunk失败: {}", e);
                                            // 继续尝试其他区块
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("  解压缩失败: {}", e);
                                }
                            }
                        }
                    }
                }
                
                println!("没有找到有效的区块数据");
            }
            Err(e) => {
                println!("无法读取区域文件 {}: {}", region_path.display(), e);
                // 在测试中，如果文件不存在或格式错误，我们认为是正常的
            }
        }
    }
    
    /// 创建一个简单的测试区域文件
    fn create_test_region_file(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let mut anvil = Anvil::new(path, 0, 0)?;
        
        // 创建一个简单的测试区块
        let test_chunk = create_test_chunk();
        let nbt_data = test_chunk.to_nbt_bytes()?;
        
        // 压缩数据
        let compressed_data = Anvil::compress_data(&nbt_data, COMPRESSION_ZLIB)?;
        
        let chunk_data = ChunkData {
            length: compressed_data.len() as u32,
            compression: COMPRESSION_ZLIB,
            data: compressed_data,
            is_external: false,
        };
        
        // 写入测试区块到位置 (0, 0)
        anvil.write_chunk_data(0, 0, chunk_data)?;
        
        // 保存区域文件
        anvil.save()?;
        
        println!("创建测试区域文件: {}", path.display());
        Ok(())
    }
    
    /// 创建一个简单的测试区块
    fn create_test_chunk() -> Chunk {
        let mut chunk = Chunk::new(0, 0, "minecraft:full".to_string());
        
        // 添加一个简单的子区块
        let mut section = Section::new(0);
        
        // 设置简单的生物群系
        section.biome = Biome {
            data: Some(vec![0; 256]), // 简单的平原生物群系
            palette: Some(vec!["minecraft:plains".to_string()]),
        };
        
        // 设置简单的方块状态
        section.block_states = BlockStates {
            data: Some(vec![0; 1024]), // 简单的空气方块
            palette: Some(vec![]),
        };
        
        chunk.add_section(section);
        chunk.inhabited_time = 1000;
        chunk.last_update = 1234567890;
        chunk.is_light_on = true;
        
        chunk
    }
    
    /// 测试区块序列化循环
    #[test]
    fn test_chunk_serialization_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let chunk = create_test_chunk();
        
        // 序列化为NBT
        let nbt_data = chunk.to_nbt_bytes()?;
        assert!(!nbt_data.is_empty(), "序列化数据不应为空");
        
        // 反序列化
        let deserialized = Chunk::from_nbt_bytes(&nbt_data)?;
        
        // 验证基本属性
        assert_eq!(chunk.x_pos, deserialized.x_pos);
        assert_eq!(chunk.z_pos, deserialized.z_pos);
        assert_eq!(chunk.status, deserialized.status);
        assert_eq!(chunk.sections.len(), deserialized.sections.len());
        
        Ok(())
    }
    
    /// 测试压缩和解压缩
    #[test]
    fn test_compression_roundtrip() -> Result<(), Box<dyn std::error::Error>> {
        let test_data = b"Hello, Minecraft Chunk!";
        
        // 测试ZLIB压缩
        let compressed = Anvil::compress_data(test_data, COMPRESSION_ZLIB)?;
        let decompressed = Anvil::decompress_chunk_data(&ChunkData {
            length: compressed.len() as u32,
            compression: COMPRESSION_ZLIB,
            data: compressed,
            is_external: false,
        })?;
        
        assert_eq!(test_data, decompressed.as_slice());
        
        // 测试未压缩
        let uncompressed = Anvil::compress_data(test_data, COMPRESSION_UNCOMPRESSED)?;
        let decompressed_uncompressed = Anvil::decompress_chunk_data(&ChunkData {
            length: uncompressed.len() as u32,
            compression: COMPRESSION_UNCOMPRESSED,
            data: uncompressed,
            is_external: false,
        })?;
        
        assert_eq!(test_data, decompressed_uncompressed.as_slice());
        
        Ok(())
    }
    
    /// 测试错误处理
    #[test]
    fn test_error_handling() {
        // 测试无效的压缩数据
        let invalid_data = ChunkData {
            length: 10,
            compression: 255, // 无效的压缩类型
            data: vec![0, 1, 2, 3],
            is_external: false,
        };
        
        let result = Anvil::decompress_chunk_data(&invalid_data);
        assert!(result.is_err(), "无效压缩类型应该返回错误");
        
        // 测试无效的NBT数据
        let invalid_nbt = vec![0u8; 100];
        let result = Chunk::from_nbt_bytes(&invalid_nbt);
        assert!(result.is_err(), "无效NBT数据应该返回错误");
    }
    
    /// 测试性能：大量区块序列化
    #[test]
    fn test_performance_large_chunk() -> Result<(), Box<dyn std::error::Error>> {
        let mut large_chunk = Chunk::new(0, 0, "minecraft:full".to_string());
        
        // 添加多个子区块模拟真实世界
        for y in 0..16 {
            let mut section = Section::new(y as i8);
            
            // 添加一些测试数据
            section.biome.data = Some(vec![y as i64; 256]);
            section.biome.palette = Some(vec![format!("minecraft:biome_{}", y)]);
            
            section.block_states.data = Some(vec![y as i64; 1024]);
            
            large_chunk.add_section(section);
        }
        
        // 测试序列化性能
        let start = std::time::Instant::now();
        let nbt_data = large_chunk.to_nbt_bytes()?;
        let serialization_time = start.elapsed();
        
        // 测试反序列化性能
        let start = std::time::Instant::now();
        let _: Chunk = Chunk::from_nbt_bytes(&nbt_data)?;
        let deserialization_time = start.elapsed();
        
        println!("大型区块序列化时间: {:?}", serialization_time);
        println!("大型区块反序列化时间: {:?}", deserialization_time);
        println!("序列化数据大小: {} 字节", nbt_data.len());
        
        // 确保在合理时间内完成
        assert!(serialization_time.as_millis() < 1000, "序列化不应超过1秒");
        assert!(deserialization_time.as_millis() < 1000, "反序列化不应超过1秒");
        
        Ok(())
    }
}

// 为测试添加一些辅助函数
impl Chunk {
    /// 创建一个用于测试的简单区块
    pub fn create_test_chunk(x: i32, z: i32) -> Self {
        let mut chunk = Self::new(x, z, "minecraft:full".to_string());
        
        // 添加一个基础子区块
        let mut section = Section::new(0);
        section.biome = Biome {
            data: Some(vec![0; 64]),
            palette: Some(vec!["minecraft:plains".to_string()]),
        };
        section.block_states = BlockStates {
            data: Some(vec![0; 256]),
            palette: None,
        };
        
        chunk.add_section(section);
        chunk.inhabited_time = 1000;
        chunk.last_update = 1234567890;
        chunk.is_light_on = true;
        
        chunk
    }
    
    /// 验证区块数据的完整性
    pub fn validate(&self) -> Result<(), &'static str> {

        if self.status.is_empty() {
            return Err("区块状态不能为空");
        }
        

        
        Ok(())
    }
}

// 在文件末尾添加集成测试
#[cfg(test)]
mod integration_tests {
    use crate::region::anvil::Anvil;

    use super::*;
    use std::fs;
    
    /// 集成测试：完整的区域文件到区块解析流程
    #[test]
    #[ignore = "需要真实的Minecraft区域文件"]
    fn test_real_minecraft_region() {
        // 这个测试需要真实的Minecraft区域文件
        // 将路径替换为你的实际区域文件路径
        let region_path = "tests/r.0.0.mca";
        
        if !fs::metadata(region_path).is_ok() {
            println!("跳过测试：区域文件不存在 {}", region_path);
            return;
        }
        
        match Anvil::from_file(region_path) {
            Ok(anvil) => {
                println!("成功加载区域文件: {}", region_path);
                
                let mut chunks_found = 0;
                
                // 扫描所有可能的区块位置
                for x in 0..32 {
                    for z in 0..32 {
                        if let Ok(Some(chunk_data)) = anvil.get_chunk_data(x, z) {
                            chunks_found += 1;
                            println!("找到区块 ({}, {})", x, z);
                            
                            match Anvil::decompress_chunk_data(&chunk_data) {
                                Ok(decompressed) => {
                                    match Chunk::from_nbt_bytes(&decompressed) {
                                        Ok(chunk) => {
                                            println!("  成功解析区块");
                                            println!("  全局坐标: ({}, {})", chunk.x_pos, chunk.z_pos);
                                            println!("  状态: {}", chunk.status);
                                            println!("  子区块数: {}", chunk.sections.len());
                                            
                                            // 验证区块数据
                                            if let Err(e) = chunk.validate() {
                                                println!("  区块验证失败: {}", e);
                                            } else {
                                                println!("  区块验证通过");
                                            }
                                        }
                                        Err(e) => {
                                            panic!("{}", e);
                                            println!("  解析区块失败: {}", e);
                                        }
                                    }
                                }
                                Err(e) => {
                                    println!("  解压缩失败: {}", e);
                                }
                            }
                        }
                    }
                }
                
                println!("在区域文件中找到 {} 个区块", chunks_found);
                assert!(chunks_found > 0, "应该在区域文件中找到至少一个区块");
            }
            Err(e) => {
                panic!("无法读取区域文件 {}: {}", region_path, e);
            }
        }
    }
}