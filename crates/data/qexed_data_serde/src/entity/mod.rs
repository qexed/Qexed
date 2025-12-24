pub mod living_entity;
pub mod memories;
pub mod types;
pub mod projectile;
pub mod any;
use std::fmt;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    /// （命名空间ID）实体类型。
    pub id: Option<String>,

    /// （-20≤值≤实体最大空气值）当前实体所剩的空气值。
    #[serde(rename = "Air")]
    pub air: Option<i16>, // NBT 中使用 i16

    /// （文本组件）当前实体的自定义名称。
    #[serde(rename = "CustomName")]
    pub custom_name: Option<String>,

    /// 表示实体是否一直渲染名称。
    #[serde(rename = "CustomNameVisible")]
    #[serde(default)]
    pub custom_name_visible: bool,

    /// 任意NBT数据。
    pub data: Option<EntityData>,
    #[serde(rename = "fall_distance")]
    /// 当前实体已经摔落的距离。
    pub fall_distance: f64,

    /// 正值代表距离火熄灭剩余的时间，负值表示当前实体能够在火中站立而不着火的时间。
    #[serde(rename = "Fire")]
    pub fire: i16, // NBT 中使用 i16

    /// 实体是否有发光的轮廓线。
    #[serde(rename = "Glowing")]
    #[serde(default)]
    pub glowing: bool,

    /// 表示实体是否视觉上正在着火。
    #[serde(rename = "HasVisualFire")]
    #[serde(default)]
    
    pub has_visual_fire: bool,

    /// 实体是否能抵抗绝大多数伤害。
    #[serde(rename = "Invulnerable")]
    #[serde(default)]
    
    pub invulnerable: bool,

    /// 当前实体的速度，代表了下一游戏刻实体将要移动的距离向量。
    #[serde(rename = "Motion")]
    pub motion: Motion,

    /// 实体是否不会受到重力的影响。
    #[serde(rename = "NoGravity")]
    #[serde(default)]
    
    pub no_gravity: bool,

    /// 实体是否正在接触地面。
    #[serde(rename = "OnGround")]
    #[serde(default)]
    
    pub on_ground: bool,

    /// 正在骑乘当前实体的实体的数据，递归标签。
    #[serde(rename = "Passengers")]
    pub passengers: Option<Vec<Entity>>,

    /// 距离当前实体下一次可以穿过下界传送门传送的时间。
    #[serde(rename = "PortalCooldown")]
    pub portal_cooldown: i32,

    /// 当前实体的坐标。
    #[serde(rename = "Pos")]
    pub pos: Pos,

    /// 实体的旋转角度，使用角度制。
    #[serde(rename = "Rotation")]
    pub rotation: Rotation,

    /// 实体是否不会发出任何声音。
    #[serde(rename = "Silent")]
    #[serde(default)]
    
    pub silent: bool,

    /// 实体的自定义记分板标签。
    #[serde(rename = "Tags")]
    pub tags: Option<Vec<String>>,

    /// 实体的冷冻时间。
    #[serde(rename = "TicksFrozen")]
    pub ticks_frozen: Option<i32>,

    /// （UUID）实体的UUID。
    #[serde(rename = "UUID")]
    pub uuid: types::NbtUuid,
}

/// 自定义数据字段
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityData {
    // 这里可以根据需要添加具体的字段
    // 目前作为一个通用的容器
}

/// 当前实体的速度，代表了下一游戏刻实体将要移动的距离向量。
#[derive(Debug, Clone)]
pub struct Motion {
    /// （-10≤值≤10）X轴速度分量。
    pub x: f64,
    /// （-10≤值≤10）Y轴速度分量。
    pub y: f64,
    /// （-10≤值≤10）Z轴速度分量。
    pub z: f64,
}
// 手动实现 Serialize：将 Pos 转为 List<Double>
impl Serialize for Motion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(3))?; // 固定长度3
        seq.serialize_element(&self.x)?;
        seq.serialize_element(&self.y)?;
        seq.serialize_element(&self.z)?;
        seq.end()
    }
}

// 手动实现 Deserialize：从 List<Double> 解析为 Pos
impl<'de> Deserialize<'de> for Motion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 反序列化为 Vec<f64>
        let values = <Vec<f64>>::deserialize(deserializer)?;
        
        // 验证列表长度必须为3（X/Y/Z）
        if values.len() != 3 {
            return Err(serde::de::Error::invalid_length(
                values.len(),
                &"a sequence of exactly 3 elements (x, y, z)",
            ));
        }

        Ok(Motion {
            x: values[0],
            y: values[1],
            z: values[2],
        })
    }
}

// 可选：添加 Debug 格式化，方便打印
impl fmt::Display for Motion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({}, {}, {})", self.x, self.y, self.z)
    }
}
/// 坐标。
#[derive(Debug, Clone)]
pub struct Pos {
    /// （-30000512≤值≤30000512）X轴坐标。
    pub x: f64,
    /// （-20000000≤值≤20000000）Y轴坐标。
    pub y: f64,
    /// （-30000512≤值≤30000512）Z轴坐标。
    pub z: f64,
}
// 手动实现 Serialize：将 Pos 转为 List<Double>
impl Serialize for Pos {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(3))?; // 固定长度3
        seq.serialize_element(&self.x)?;
        seq.serialize_element(&self.y)?;
        seq.serialize_element(&self.z)?;
        seq.end()
    }
}

// 手动实现 Deserialize：从 List<Double> 解析为 Pos
impl<'de> Deserialize<'de> for Pos {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 反序列化为 Vec<f64>
        let values = <Vec<f64>>::deserialize(deserializer)?;
        
        // 验证列表长度必须为3（X/Y/Z）
        if values.len() != 3 {
            return Err(serde::de::Error::invalid_length(
                values.len(),
                &"a sequence of exactly 3 elements (x, y, z)",
            ));
        }

        Ok(Pos {
            x: values[0],
            y: values[1],
            z: values[2],
        })
    }
}



/// 旋转角度。
#[derive(Debug, Clone)]
pub struct Rotation {
    /// 当前实体以Y轴为中心，与正南方以顺时针方向旋转的视角角度。
    pub yaw: f32,
    /// （-90≤值≤90）当前实体与视角与水平面之间的倾斜角。
    pub pitch: f32,
}
impl Serialize for Rotation {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(Some(2))?; // 固定长度3
        seq.serialize_element(&self.yaw)?;
        seq.serialize_element(&self.pitch)?;
        seq.end()
    }
}

// 手动实现 Deserialize：从 List<Double> 解析为 Rotation
impl<'de> Deserialize<'de> for Rotation {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // 反序列化为 Vec<f64>
        let values = <Vec<f32>>::deserialize(deserializer)?;
        
        // 验证列表长度必须为3（X/Y/Z）
        if values.len() != 2 {
            return Err(serde::de::Error::invalid_length(
                values.len(),
                &"a sequence of exactly 2 elements (yaw,pitch)",
            ));
        }

        Ok(Rotation {
            yaw: values[0],
            pitch: values[1],
        })
    }
}

// 为各个结构体实现默认值
impl Default for Entity {
    fn default() -> Self {
        Self {
            id: Some("minecraft:pig".to_owned()),
            air: None,
            custom_name: None,
            custom_name_visible: false,
            data: None,
            fall_distance: 0.0,
            fire: 0,
            glowing: false,
            has_visual_fire: false,
            invulnerable: false,
            motion: Motion::default(),
            no_gravity: false,
            on_ground: true,
            passengers: None,
            portal_cooldown: 0,
            pos: Pos::default(),
            rotation: Rotation::default(),
            silent: false,
            tags: None,
            ticks_frozen: None, 
            uuid: types::NbtUuid::from(uuid::Uuid::new_v4()),
        }
    }
}

impl Default for Motion {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

impl Default for Pos {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 64.0,
            z: 0.0,
        }
    }
}

impl Default for Rotation {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

impl Default for EntityData {
    fn default() -> Self {
        Self {}
    }
}

// 序列化和反序列化实现
impl Entity {
    /// 创建一个简单的实体
    pub fn simple(id: Option<String>, pos: Pos) -> Self {
        Self {
            id,
            pos,
            ..Default::default()
        }
    }

    /// 设置实体的自定义名称
    pub fn with_custom_name(mut self, name: String) -> Self {
        self.custom_name = Some(name);
        self.custom_name_visible = true;
        self
    }

    /// 设置实体的运动速度
    pub fn with_motion(mut self, motion: Motion) -> Self {
        self.motion = motion;
        self
    }

    /// 设置实体的旋转角度
    pub fn with_rotation(mut self, rotation: Rotation) -> Self {
        self.rotation = rotation;
        self
    }

    /// 添加乘客
    pub fn add_passenger(mut self, passenger: Entity) -> Self {
        if let Some(ref mut passengers) = self.passengers {
            passengers.push(passenger);
        } else {
            self.passengers = Some(vec![passenger]);
        }
        self
    }

    /// 设置 UUID
    pub fn with_uuid(mut self, uuid:  types::NbtUuid) -> Self {
        self.uuid = uuid;
        self
    }
}

// 为 Motion, Pos, Rotation 实现便利方法
impl Motion {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl Pos {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
}

impl Rotation {
    pub fn new(yaw: f32, pitch: f32) -> Self {
        Self { yaw, pitch }
    }
}

