use crate::{PacketCodec, net_types::Position};

impl PacketCodec for Position {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        let x_part = (self.x as i64) & 0x3FFFFFF;
        let z_part = (self.z as i64) & 0x3FFFFFF;
        let y_part = (self.y as i64) & 0xFFF;
        let encoded = (x_part << 38) | (z_part << 12) | y_part;
        encoded.serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut val :i64= 0;
        val.deserialize(r)?;
        self.x = (val >> 38) as i32;
        self.y = ((val << 52) >> 52) as i32;
        self.z = ((val << 26) >> 38) as i32;
        Ok(())
    }
}
// impl Subdata for Position {
//     fn new() -> Self {
//         Position { x: 0, y: 0, z: 0 }
//     }
//     fn serialize(&self, w: &mut PacketWriter) {
//         let x_part = (self.x as i64) & 0x3FFFFFF;
//         let z_part = (self.z as i64) & 0x3FFFFFF;
//         let y_part = (self.y as i64) & 0xFFF;
//         let encoded = (x_part << 38) | (z_part << 12) | y_part;
//         w.i64(encoded);
//     }
// 
//     fn deserialize(&mut self, r: &mut PacketReader) {
//         let val = r.i64();
//         self.x = (val >> 38) as i32;
//         self.y = ((val << 52) >> 52) as i32;
//         self.z = ((val << 26) >> 38) as i32;
//     }
// }
// 