use bytes::{Buf as _, BufMut as _};
use qexed_packet::{Packet, PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct DebugSample {
    pub sample: Vec<i64>,
    // TODO: RemoteDebugSampleType — readEnum VarInt 序数（目前仅 TICK_TIME=0），用 VarInt 占位
    pub debug_sample_type: VarInt,
}

impl Packet for DebugSample {
    const ID: i32 = 0x1D;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        VarInt(self.sample.len() as i32).serialize(w)?;
        for value in &self.sample {
            value.serialize(w)?;
        }
        self.debug_sample_type.serialize(w)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(qexed_packet::PacketError::msg(format!(
                "negative debug sample length: {}",
                len.0
            )));
        }
        self.sample.clear();
        self.sample.reserve(len.0 as usize);
        for _ in 0..len.0 {
            let mut value: i64 = 0;
            value.deserialize(r)?;
            self.sample.push(value);
        }
        self.debug_sample_type.deserialize(r)
    }
}
