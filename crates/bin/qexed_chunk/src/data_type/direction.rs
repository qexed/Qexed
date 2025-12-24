//! 8方向存储容器
//! 支持8个方向的数据存储和查询，使用Option<T>表示空状态

/// 8个方向枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    NorthWest,  // 西北/左上 (-1, 1)
    North,      // 北/上     (0, 1)
    NorthEast,  // 东北/右上 (1, 1)
    West,       // 西/左     (-1, 0)
    East,       // 东/右     (1, 0)
    SouthWest,  // 西南/左下 (-1, -1)
    South,      // 南/下     (0, -1)
    SouthEast,  // 东南/右下 (1, -1)
}

impl Direction {
    /// 所有8个方向
    pub const ALL: [Self; 8] = [
        Self::NorthWest,
        Self::North,
        Self::NorthEast,
        Self::West,
        Self::East,
        Self::SouthWest,
        Self::South,
        Self::SouthEast,
    ];
    
    /// 基本方向（上下左右）
    pub const CARDINAL: [Self; 4] = [
        Self::North,
        Self::South,
        Self::West,
        Self::East,
    ];
    
    /// 对角线方向
    pub const DIAGONAL: [Self; 4] = [
        Self::NorthWest,
        Self::NorthEast,
        Self::SouthWest,
        Self::SouthEast,
    ];
    
    /// 获取坐标偏移量 (dx, dy) 或 (dx, dz)
    pub const fn offset(&self) -> (i8, i8) {
        match self {
            Self::NorthWest => (-1, 1),
            Self::North => (0, 1),
            Self::NorthEast => (1, 1),
            Self::West => (-1, 0),
            Self::East => (1, 0),
            Self::SouthWest => (-1, -1),
            Self::South => (0, -1),
            Self::SouthEast => (1, -1),
        }
    }
    
    /// 获取索引 (0-7)
    pub const fn index(&self) -> usize {
        match self {
            Self::NorthWest => 0,
            Self::North => 1,
            Self::NorthEast => 2,
            Self::West => 3,
            Self::East => 4,
            Self::SouthWest => 5,
            Self::South => 6,
            Self::SouthEast => 7,
        }
    }
    
    /// 获取相反方向
    pub const fn opposite(&self) -> Self {
        match self {
            Self::NorthWest => Self::SouthEast,
            Self::North => Self::South,
            Self::NorthEast => Self::SouthWest,
            Self::West => Self::East,
            Self::East => Self::West,
            Self::SouthWest => Self::NorthEast,
            Self::South => Self::North,
            Self::SouthEast => Self::NorthWest,
        }
    }
    
    /// 从相对坐标获取方向
    pub const fn from_offset(dx: i8, dz: i8) -> Option<Self> {
        match (dx, dz) {
            (-1, 1) => Some(Self::NorthWest),
            (0, 1) => Some(Self::North),
            (1, 1) => Some(Self::NorthEast),
            (-1, 0) => Some(Self::West),
            (1, 0) => Some(Self::East),
            (-1, -1) => Some(Self::SouthWest),
            (0, -1) => Some(Self::South),
            (1, -1) => Some(Self::SouthEast),
            _ => None, // 包括 (0,0) 中心位置
        }
    }
    
    /// 从中心坐标和目标坐标获取方向
    pub fn from_coords(center: [i64; 2], target: [i64; 2]) -> Option<Self> {
        let dx = target[0] - center[0];
        let dz = target[1] - center[1];
        
        let dx_i8 = match dx {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None,
        };
        
        let dz_i8 = match dz {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None,
        };
        
        Self::from_offset(dx_i8, dz_i8)
    }
    
    /// 获取从中心到目标的坐标
    pub fn target_coord(center: [i64; 2], dir: Direction) -> [i64; 2] {
        let (dx, dz) = dir.offset();
        [center[0] + dx as i64, center[1] + dz as i64]
    }
}

/// 8方向存储容器，专为存储Option<T>设计
/// 这个设计更简单、更直观，避免了复杂的泛型处理
#[derive(Debug, Clone)]
pub struct DirectionMap<T> {
    /// 按方向索引存储数据
    data: [Option<T>; 8],
}

impl<T> DirectionMap<T> {
    /// 创建一个所有方向都为None的DirectionMap
    pub fn new_none() -> Self {
        Self { data: [None, None, None, None, None, None, None, None] }
    }
    
    /// 从迭代器创建DirectionMap
    pub fn from_iter<I>(iter: I) -> Option<Self>
    where
        I: IntoIterator<Item = Option<T>>,
    {
        let mut iter = iter.into_iter();
        Some(Self {
            data: [
                iter.next()?,
                iter.next()?,
                iter.next()?,
                iter.next()?,
                iter.next()?,
                iter.next()?,
                iter.next()?,
                iter.next()?,
            ],
        })
    }
    
    /// 使用函数创建 DirectionMap
    pub fn from_fn<F>(mut f: F) -> Self
    where
        F: FnMut(Direction) -> Option<T>,
    {
        Self {
            data: [
                f(Direction::NorthWest),
                f(Direction::North),
                f(Direction::NorthEast),
                f(Direction::West),
                f(Direction::East),
                f(Direction::SouthWest),
                f(Direction::South),
                f(Direction::SouthEast),
            ],
        }
    }
    
    /// 获取指定方向的值，如果为None则返回None
    #[inline]
    pub fn get(&self, dir: Direction) -> Option<&T> {
        self.data[dir.index()].as_ref()
    }
    
    /// 获取指定方向的可变值，如果为None则返回None
    #[inline]
    pub fn get_mut(&mut self, dir: Direction) -> Option<&mut T> {
        self.data[dir.index()].as_mut()
    }
    
    /// 设置指定方向的值，返回旧值
    #[inline]
    pub fn set(&mut self, dir: Direction, value: Option<T>) -> Option<T> {
        std::mem::replace(&mut self.data[dir.index()], value)
    }
    
    /// 插入值，返回旧值
    pub fn insert(&mut self, dir: Direction, value: T) -> Option<T> {
        self.data[dir.index()].replace(value)
    }
    
    /// 移除值
    pub fn remove(&mut self, dir: Direction) -> Option<T> {
        self.data[dir.index()].take()
    }
    
    /// 检查方向是否有值
    pub fn has_value(&self, dir: Direction) -> bool {
        self.data[dir.index()].is_some()
    }
    
    /// 检查方向是否为空
    pub fn is_empty(&self, dir: Direction) -> bool {
        self.data[dir.index()].is_none()
    }
    
    /// 获取所有有值的方向
    pub fn filled_directions(&self) -> Vec<Direction> {
        Direction::ALL.iter()
            .copied()
            .filter(|&dir| self.has_value(dir))
            .collect()
    }
    
    /// 获取所有空的方向
    pub fn empty_directions(&self) -> Vec<Direction> {
        Direction::ALL.iter()
            .copied()
            .filter(|&dir| self.is_empty(dir))
            .collect()
    }
    
    /// 清空所有方向
    pub fn clear_all(&mut self) {
        for item in &mut self.data {
            *item = None;
        }
    }
    
    /// 从另一个DirectionMap合并有值的项
    pub fn merge_from(&mut self, other: &DirectionMap<T>)
    where
        T: Clone,
    {
        for dir in Direction::ALL.iter().copied() {
            if let Some(value) = other.get(dir) {
                self.insert(dir, value.clone());
            }
        }
    }
    
    /// 根据中心坐标和目标坐标获取目标方向的值
    /// 如果目标在中心8方向相邻位置，返回对应方向的值
    /// 否则返回 None
    pub fn get_at_target(&self, center: [i64; 2], target: [i64; 2]) -> Option<&T> {
        let dx = target[0] - center[0];
        let dz = target[1] - center[1];
        
        let dx_i8 = match dx {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None, // 偏移超出1格范围
        };
        
        let dz_i8 = match dz {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None, // 偏移超出1格范围
        };
        
        if let Some(dir) = Direction::from_offset(dx_i8, dz_i8) {
            self.get(dir)
        } else {
            None
        }
    }
    
    /// 根据中心坐标和目标坐标获取目标方向的可变值
    pub fn get_mut_at_target(&mut self, center: [i64; 2], target: [i64; 2]) -> Option<&mut T> {
        let dx = target[0] - center[0];
        let dz = target[1] - center[1];
        
        let dx_i8 = match dx {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None,
        };
        
        let dz_i8 = match dz {
            -1 => -1,
            0 => 0,
            1 => 1,
            _ => return None,
        };
        
        if let Some(dir) = Direction::from_offset(dx_i8, dz_i8) {
            self.get_mut(dir)
        } else {
            None
        }
    }
    
    /// 检查目标坐标是否在中心坐标的8方向邻接范围内
    pub fn is_adjacent_to_center(center: [i64; 2], target: [i64; 2]) -> bool {
        let dx = (target[0] - center[0]).abs();
        let dz = (target[1] - center[1]).abs();
        
        dx <= 1 && dz <= 1 && (dx != 0 || dz != 0)
    }
    
    /// 获取内部数据的数组引用
    pub fn as_array(&self) -> &[Option<T>; 8] {
        &self.data
    }
    
    /// 获取内部数据的可变数组引用
    pub fn as_array_mut(&mut self) -> &mut [Option<T>; 8] {
        &mut self.data
    }
    
    /// 转换为数组
    pub fn into_array(self) -> [Option<T>; 8] {
        self.data
    }
    
    /// 对每个方向应用函数
    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(Direction, Option<&T>),
    {
        for dir in Direction::ALL.iter().copied() {
            f(dir, self.get(dir));
        }
    }
    
    /// 对每个方向应用函数（可变版本）
    pub fn for_each_mut<F>(&mut self, mut f: F)
    where
        F: FnMut(Direction, Option<&mut T>),
    {
        for dir in Direction::ALL.iter().copied() {
            f(dir, self.get_mut(dir));
        }
    }
    
    /// 映射到另一种类型
    pub fn map<U, F>(self, mut f: F) -> DirectionMap<U>
    where
        F: FnMut(Direction, Option<T>) -> Option<U>,
    {
        let [nw, n, ne, w, e, sw, s, se] = self.data;
        
        DirectionMap {
            data: [
                f(Direction::NorthWest, nw),
                f(Direction::North, n),
                f(Direction::NorthEast, ne),
                f(Direction::West, w),
                f(Direction::East, e),
                f(Direction::SouthWest, sw),
                f(Direction::South, s),
                f(Direction::SouthEast, se),
            ],
        }
    }
}

// 为DirectionMap<T>实现Index和IndexMut以便使用[]语法
impl<T> std::ops::Index<Direction> for DirectionMap<T> {
    type Output = Option<T>;
    
    fn index(&self, dir: Direction) -> &Self::Output {
        &self.data[dir.index()]
    }
}

impl<T> std::ops::IndexMut<Direction> for DirectionMap<T> {
    fn index_mut(&mut self, dir: Direction) -> &mut Self::Output {
        &mut self.data[dir.index()]
    }
}

// 实现Default
impl<T> Default for DirectionMap<T> {
    fn default() -> Self {
        Self::new_none()
    }
}

// 实用的构造函数
impl<T: Clone> DirectionMap<T> {
    /// 创建所有方向使用相同值的DirectionMap
    pub fn new_with(value: T) -> Self {
        Self {
            data: [
                Some(value.clone()), // NorthWest
                Some(value.clone()), // North
                Some(value.clone()), // NorthEast
                Some(value.clone()), // West
                Some(value.clone()), // East
                Some(value.clone()), // SouthWest
                Some(value.clone()), // South
                Some(value.clone()), // SouthEast
            ],
        }
    }
}

// 测试用例
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_direction_map_basic() {
        let mut map = DirectionMap::new_none();
        
        // 设置值
        map.insert(Direction::North, 100);
        map.insert(Direction::South, 200);
        
        // 获取值
        assert_eq!(map.get(Direction::North), Some(&100));
        assert_eq!(map.get(Direction::South), Some(&200));
        assert_eq!(map.get(Direction::East), None);
        
        // 检查状态
        assert!(map.has_value(Direction::North));
        assert!(map.is_empty(Direction::East));
        
        // 获取有值的方向
        let filled = map.filled_directions();
        assert_eq!(filled.len(), 2);
        assert!(filled.contains(&Direction::North));
        assert!(filled.contains(&Direction::South));
        
        // 移除值
        let removed = map.remove(Direction::North);
        assert_eq!(removed, Some(100));
        assert_eq!(map.get(Direction::North), None);
        
        // 清空
        map.clear_all();
        assert_eq!(map.filled_directions().len(), 0);
    }
    
    #[test]
    fn test_get_at_target() {
        let mut map = DirectionMap::new_none();
        
        // 插入一些值
        map.insert(Direction::North, 100);
        map.insert(Direction::South, 200);
        map.insert(Direction::East, 300);
        
        let center = [10, 10];
        
        // 测试相邻位置
        assert_eq!(map.get_at_target(center, [10, 11]), Some(&100));  // 北
        assert_eq!(map.get_at_target(center, [10, 9]), Some(&200));   // 南
        assert_eq!(map.get_at_target(center, [11, 10]), Some(&300));  // 东
        assert_eq!(map.get_at_target(center, [9, 10]), None);         // 西（无值）
        
        // 测试非相邻位置
        assert_eq!(map.get_at_target(center, [10, 12]), None);  // 太远
        assert_eq!(map.get_at_target(center, [10, 10]), None);  // 自身位置
    }
    
    #[test]
    fn test_mut_methods() {
        let mut map = DirectionMap::new_none();
        map.insert(Direction::North, 100);
        
        let center = [0, 0];
        
        // 修改值
        if let Some(value) = map.get_mut_at_target(center, [0, 1]) {
            *value = 999;
        }
        
        assert_eq!(map.get(Direction::North), Some(&999));
    }
    
    #[test]
    fn test_is_adjacent_to_center() {
        let center = [5, 5];
        
        // 相邻
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [5, 6]));
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [5, 4]));
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [6, 5]));
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [4, 5]));
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [6, 6]));
        assert!(DirectionMap::<i32>::is_adjacent_to_center(center, [4, 4]));
        
        // 非相邻
        assert!(!DirectionMap::<i32>::is_adjacent_to_center(center, [5, 5]));  // 自身
        assert!(!DirectionMap::<i32>::is_adjacent_to_center(center, [5, 7]));  // 太远
        assert!(!DirectionMap::<i32>::is_adjacent_to_center(center, [7, 5]));  // 太远
    }
    
    // #[test]
    // fn test_map_function() {
    //     let map = DirectionMap::new_none();
        
    //     // 测试从None映射
    //     let mapped = map.map(|dir, value| {
    //         match dir {
    //             Direction::North => Some(100),
    //             Direction::South => Some(200),
    //             _ => None,
    //         }
    //     });
        
    //     assert_eq!(mapped.get(Direction::North), Some(&100));
    //     assert_eq!(mapped.get(Direction::South), Some(&200));
    //     assert_eq!(mapped.get(Direction::East), None);
    // }
}