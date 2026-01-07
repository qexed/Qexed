use std::ops::{Add, Sub, Mul, Div, Neg, AddAssign, SubAssign, Rem};

#[derive(Debug, PartialEq, Clone, Default, Copy)]
pub struct Angle(pub u8);

impl Angle {
    /// 完整圆周的分数（256表示一个完整圆周）
    pub const FULL_CIRCLE: u16 = 256;
    
    /// 零角度
    pub const ZERO: Angle = Angle(0);
    
    /// 直角（90度）
    pub const RIGHT_ANGLE: Angle = Angle(64);  // 256/4 = 64
    
    /// 平角（180度）
    pub const STRAIGHT_ANGLE: Angle = Angle(128);  // 256/2 = 128
    
    /// 从度数创建角度（修复版）
    pub fn from_degrees(degrees: f32) -> Self {
        // 将度数归一化到 [0, 360) 范围
        let normalized_degrees = degrees.rem_euclid(360.0);
        // 转换为256分度
        let units = (normalized_degrees * Self::FULL_CIRCLE as f32 / 360.0).round() as u16;
        Angle((units % Self::FULL_CIRCLE) as u8)
    }
    
    /// 从Minecraft协议的标准偏航角创建（处理-180°到180°范围）
    pub fn from_minecraft_yaw(yaw: f32) -> Self {
        // Minecraft偏航角：-180°（北）到180°（北）[1](@ref)
        let normalized_degrees = if yaw < 0.0 { yaw + 360.0 } else { yaw };
        Self::from_degrees(normalized_degrees)
    }
    
    /// 从Minecraft协议的标准俯仰角创建（处理-90°到90°范围）
    pub fn from_minecraft_pitch(pitch: f32) -> Self {
        // Minecraft俯仰角：-90°（上看）到90°（下看）[1](@ref)
        let normalized_pitch = pitch.clamp(-90.0, 90.0);
        // 转换为0°到360°等效表示
        let normalized_degrees = if normalized_pitch < 0.0 { 
            normalized_pitch + 360.0 
        } else { 
            normalized_pitch 
        };
        Self::from_degrees(normalized_degrees)
    }
    
    /// 转换为Minecraft标准偏航角（-180°到180°）
    pub fn to_minecraft_yaw(&self) -> f32 {
        let degrees = self.to_degrees();
        if degrees > 180.0 { degrees - 360.0 } else { degrees }
    }
    
    /// 转换为Minecraft标准俯仰角（-90°到90°）
    pub fn to_minecraft_pitch(&self) -> f32 {
        let degrees = self.to_degrees();
        // 将360°表示映射回-90°到90°
        if degrees > 270.0 { degrees - 360.0 } 
        else if degrees > 90.0 { 180.0 - degrees } 
        else { degrees }
    }
    
    /// 转换为度数（0°到360°范围）
    pub fn to_degrees(&self) -> f32 {
        self.0 as f32 * 360.0 / Self::FULL_CIRCLE as f32
    }
    
    /// 转换为弧度
    pub fn to_radians(&self) -> f32 {
        self.to_degrees().to_radians()
    }
    
    /// 获取原始字节值
    pub fn as_byte(&self) -> u8 {
        self.0
    }
}

// 基本算术运算实现
impl Add for Angle {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Angle(self.0.wrapping_add(other.0))
    }
}

impl Sub for Angle {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Angle(self.0.wrapping_sub(other.0))
    }
}

impl Mul<u8> for Angle {
    type Output = Self;

    fn mul(self, scalar: u8) -> Self {
        Angle(self.0.wrapping_mul(scalar))
    }
}

impl Div<u8> for Angle {
    type Output = Self;

    fn div(self, scalar: u8) -> Self {
        if scalar == 0 {
            Self::ZERO // 避免除零，返回零角度
        } else {
            Angle(self.0 / scalar)
        }
    }
}

impl Neg for Angle {
    type Output = Self;

    fn neg(self) -> Self {
        Angle(self.0.wrapping_neg())
    }
}

// 复合赋值运算
impl AddAssign for Angle {
    fn add_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_add(other.0);
    }
}

impl SubAssign for Angle {
    fn sub_assign(&mut self, other: Self) {
        self.0 = self.0.wrapping_sub(other.0);
    }
}
// 从原始值转换
impl From<u8> for Angle {
    fn from(value: u8) -> Self {
        Angle(value)
    }
}

impl From<Angle> for u8 {
    fn from(angle: Angle) -> u8 {
        angle.0
    }
}