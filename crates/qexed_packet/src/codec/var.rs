use crate::{
    DecodeError, PacketCodec,
    net_types::{VarInt, VarLong},
};

impl PacketCodec for VarInt {
    fn serialize(&self, w: &mut crate::PacketWriter) -> crate::Result<()> {
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

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> crate::Result<()> {
        let mut value = 0_i32;
        for position in 0..5 {
            if !r.buf.has_remaining() {
                return Err(DecodeError::IncompletePacket.into());
            }
            let byte = r.buf.get_u8();
            value |= ((byte & 0x7F) as i32) << (7 * position);
            if (byte & 0x80) == 0 {
                self.0 = value;
                return Ok(());
            }
        }
        Err(DecodeError::InvalidVarInt.into())
    }
}

impl PacketCodec for VarLong {
    fn serialize(&self, w: &mut crate::PacketWriter) -> crate::Result<()> {
        let mut val = self.0 as u64;
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

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> crate::Result<()> {
        let mut value = 0_i64;
        for position in 0..10 {
            if !r.buf.has_remaining() {
                return Err(DecodeError::IncompletePacket.into());
            }
            let byte = r.buf.get_u8();
            value |= ((byte & 0x7F) as i64) << (7 * position);
            if (byte & 0x80) == 0 {
                self.0 = value;
                return Ok(());
            }
        }

        Err(DecodeError::InvalidVarLong.into())
    }
}
