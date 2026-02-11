// 变长 VarInt 和VarLong 的处理
use crate::{DecodeError, PacketCodec, net_types::{VarInt, VarLong}};
impl PacketCodec for VarInt {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        let mut val = self.0 as u32;
        loop {
            let mut temp = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                temp |= 0x80;
            }
            temp.serialize(w)?;
            if val == 0 {
                return Ok(());
            }
        }
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        for position in 0..5 {
            let byte = r.buf.get_u8();
            self.0 |= (byte as i32 & 0x7F) << (7 * position);
            if (byte & 0x80) == 0 {
                return Ok(());
            }
        }
        Err(DecodeError::InvalidVarInt.into())
    }
}
impl PacketCodec for VarLong {
    fn serialize(&self, w: &mut crate::PacketWriter) -> anyhow::Result<()> {
        let mut val = ((self.0 << 1) ^ (self.0 >> 63)) as u64;

        loop {
            let mut temp = (val & 0x7F) as u8;
            val >>= 7;
            if val != 0 {
                temp |= 0x80; // 设置续位标志
            }
            temp.serialize(w)?;
            if val == 0 {
                return Ok(());
            }
        }
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> anyhow::Result<()> {
        let mut value: u64 = 0;

        for position in 0..10 {
            // varlong 最多 10 个字节
            let byte = r.buf.get_u8();
            value |= ((byte as u64) & 0x7F) << (7 * position);

            if (byte & 0x80) == 0 {
                // ZigZag 解码
                let decoded_value = ((value >> 1) as i64) ^ (-((value & 1) as i64));
                return Ok(self.0 = decoded_value);
            }

        }

        Err(DecodeError::InvalidVarLong.into())
    }
}