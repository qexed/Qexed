use crate::{PacketCodec, error::PacketError, net_types::VarInt};

impl<T> PacketCodec for Vec<T>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        if self.len() > i32::MAX as usize {
            return Err(PacketError::LengthExceedsVarIntMax { name: "vec", len: self.len() });
        }

        VarInt(self.len() as i32).serialize(w)?;
        for prop in self {
            prop.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(PacketError::NegativeLength {
                name: "vec",
                value: len.0,
            });
        }

        self.clear();
        self.reserve(len.0 as usize);
        for _ in 0..len.0 {
            let mut value = T::default();
            value.deserialize(r)?;
            self.push(value);
        }
        Ok(())
    }
}

impl<const N: usize> PacketCodec for [u8; N]
where
    [u8; N]: Default,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        for value in self {
            value.serialize(w)?
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        for value in self {
            value.deserialize(r)?;
        }
        Ok(())
    }
}

impl<T> PacketCodec for Box<T>
where
    T: PacketCodec + Default,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        self.as_ref().serialize(w)
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        let mut value = T::default();
        value.deserialize(r)?;
        *self = Box::new(value);
        Ok(())
    }
}