#[derive(Debug, Default, PartialEq,Clone)]
pub struct RestBuffer(pub Vec<u8>);
#[cfg(test)]
mod test{
    use crate::net_types::rest_buffer::RestBuffer;

    
    #[test]
    fn test_print(){
        println!("{:?}",RestBuffer(vec![1,2,3]));
        println!("{:?}",vec![1,2,3]);
    }
}