
#[derive(Debug,PartialEq,Clone)]
// VarInt 结构体定义
pub struct VarLong(pub i64);
impl Default for VarLong{
    fn default() -> Self {
        Self(Default::default())
    }
}
impl std::fmt::Display for VarLong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

#[cfg(test)]
mod test{
    use crate::net_types::var_long::VarLong;

    #[test]
    fn test_print(){
        println!("{}",VarLong(123));
        println!("{:?}",VarLong(123));
        println!("{}",123);
    }
}