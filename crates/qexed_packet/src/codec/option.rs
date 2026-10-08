use crate::{PacketCodec, error::PacketError};

impl<T> PacketCodec for Option<T>
where
    T: PacketCodec,
{
    fn serialize(&self, w: &mut crate::PacketWriter) -> Result<(), PacketError> {
        if let Some(t) = self {
            true.serialize(w)?;
            t.serialize(w)?;
        } else {
            false.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut crate::PacketReader) -> Result<(), PacketError> {
        let mut is_present = false;
        is_present.deserialize(r)?;
        if is_present {
            let mut value = T::default();
            value.deserialize(r)?;
            *self = Some(value);
        } else {
            *self = None;
        }
        Ok(())
    }
}
