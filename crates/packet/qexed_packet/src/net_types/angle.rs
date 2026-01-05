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
    
    /// 从度数创建角度
    pub fn from_degrees(degrees: f32) -> Self {
        let units = (degrees * Self::FULL_CIRCLE as f32 / 360.0).round() as u16;
        Angle((units % Self::FULL_CIRCLE) as u8)
    }
    
    /// 从弧度创建角度
    pub fn from_radians(radians: f32) -> Self {
        let degrees = radians * 180.0 / std::f32::consts::PI;
        Self::from_degrees(degrees)
    }
    
    /// 转换为度数
    pub fn to_degrees(&self) -> f32 {
        self.0 as f32 * 360.0 / Self::FULL_CIRCLE as f32
    }
    
    /// 转换为弧度
    pub fn to_radians(&self) -> f32 {
        self.to_degrees() * std::f32::consts::PI / 180.0
    }
    
    /// 获取原始字节值
    pub fn as_byte(&self) -> u8 {
        self.0
    }
    
    /// 计算两个角度之间的最短差异（考虑循环）
    pub fn angle_to(&self, other: &Self) -> Self {
        let diff = other.0 as i16 - self.0 as i16;
        let mut shortest = diff % Self::FULL_CIRCLE as i16;
        
        // 确保结果在 [-128, 127] 范围内，对应最短路径
        if shortest > 128 {
            shortest -= Self::FULL_CIRCLE as i16;
        } else if shortest < -128 {
            shortest += Self::FULL_CIRCLE as i16;
        }
        
        // 转换为无符号字节（自动处理负数）
        Angle(shortest as u8)
    }
    
    /// 线性插值（考虑角度的循环特性）
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let diff = self.angle_to(other);
        *self + Angle((diff.0 as f32 * t).round() as u8)
    }
    
    /// 正弦值（近似计算）
    pub fn sin(&self) -> f32 {
        self.to_radians().sin()
    }
    
    /// 余弦值（近似计算）
    pub fn cos(&self) -> f32 {
        self.to_radians().cos()
    }
    
    /// 规范化角度到 [0, 255] 范围
    pub fn normalize(&self) -> Self {
        // 由于使用u8，值自动在0-255范围内，无需额外规范化
        *self
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