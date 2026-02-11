
#[derive(Debug,PartialEq,Clone)]
// VarInt 结构体定义
pub struct VarInt(pub i32);
impl Default for VarInt{
    fn default() -> Self {
        Self(Default::default())
    }
}
impl std::fmt::Display for VarInt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

#[cfg(test)]
mod test{
    use crate::net_types::var_int::VarInt;
    #[test]
    fn test_print(){
        println!("{}",VarInt(123));
        println!("{:?}",VarInt(123));
        println!("{}",123);
    }
}