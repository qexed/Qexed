//! 实体位置类型。
//!
//! v4 的 `EntityPosition` 定义在 `qexed_protocol::to_client::play::add_entity`，
//! v6 协议 crate 不再提供该类型（26.3 的 AddEntity 是扁平字段），因此在实体 crate
//! 本地定义，供 manager / packets / 外部调用方共用。

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
}

impl Default for EntityPosition {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: true,
        }
    }
}

impl EntityPosition {
    /// 两点水平（XZ）距离平方。
    pub fn horizontal_distance_sq(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dz = self.z - other.z;
        dx * dx + dz * dz
    }

    /// 两点欧氏距离平方。
    pub fn distance_sq(self, other: Self) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        dx * dx + dy * dy + dz * dz
    }

    /// 所在方块坐标。
    pub fn block(self) -> qexed_packet::net_types::Position {
        qexed_packet::net_types::Position {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }
}
