use bytes::{Buf, BufMut as _};
use qexed_packet::{
    PacketCodec, PacketReader, PacketWriter,
    net_types::VarInt,
};

// TODO: Vec3 movement 字段在 26.3 使用新的 LpVec3 压缩格式（变长整数打包），
// 这里用 LpVec3 包装类型承载；xRot/yRot/yHeadRot 是 256 级角度字节。

/// LpVec3：26.3 新增的压缩向量编码（替代旧的 3x short 速度字段）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct LpVec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl PacketCodec for LpVec3 {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        write_lp_vec3(w, self.x, self.y, self.z)
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        let (x, y, z) = read_lp_vec3(r)?;
        self.x = x;
        self.y = y;
        self.z = z;
        Ok(())
    }
}

#[qexed_packet_macros::packet(id = 0x1)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct AddEntity {
    pub id: VarInt,
    pub uuid: uuid::Uuid,
    // TODO: net.minecraft.world.entity.EntityType<?> — 注册表 id，用 VarInt 占位
    pub entity_type: VarInt,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub movement: LpVec3,
    pub x_rot: i8,
    pub y_rot: i8,
    pub y_head_rot: i8,
    pub data: VarInt,
}

pub(crate) fn write_lp_vec3(w: &mut PacketWriter, x: f64, y: f64, z: f64) -> qexed_packet::Result<()> {
    const ABS_MIN_VALUE: f64 = 3.051944088384301E-5;
    const ABS_MAX_VALUE: f64 = 1.7179869183E10;
    const DATA_MASK: f64 = 32766.0;

    let x = sanitize_lp_value(x, ABS_MAX_VALUE);
    let y = sanitize_lp_value(y, ABS_MAX_VALUE);
    let z = sanitize_lp_value(z, ABS_MAX_VALUE);
    let chessboard_length = x.abs().max(y.abs()).max(z.abs());
    if chessboard_length < ABS_MIN_VALUE {
        w.buf.put_u8(0);
        return Ok(());
    }

    let scale = chessboard_length.ceil() as u64;
    let is_partial = (scale & 3) != scale;
    let markers = if is_partial { (scale & 3) | 4 } else { scale };
    let buffer = markers
        | (pack_lp_value(x / scale as f64, DATA_MASK) << 3)
        | (pack_lp_value(y / scale as f64, DATA_MASK) << 18)
        | (pack_lp_value(z / scale as f64, DATA_MASK) << 33);

    w.buf.put_u8(buffer as u8);
    w.buf.put_u8((buffer >> 8) as u8);
    w.buf.put_i32((buffer >> 16) as i32);
    if is_partial {
        VarInt((scale >> 2) as i32).serialize(w)?;
    }
    Ok(())
}

pub(crate) fn read_lp_vec3(r: &mut PacketReader) -> qexed_packet::Result<(f64, f64, f64)> {
    let lowest = r.buf.get_u8();
    if lowest == 0 {
        return Ok((0.0, 0.0, 0.0));
    }

    let middle = r.buf.get_u8();
    let highest = r.buf.get_u32();
    let buffer = ((highest as u64) << 16) | ((middle as u64) << 8) | lowest as u64;
    let mut scale = (lowest & 3) as u64;
    if lowest & 4 == 4 {
        let mut continuation = VarInt::default();
        continuation.deserialize(r)?;
        scale |= (continuation.0 as u32 as u64) << 2;
    }

    Ok((
        unpack_lp_value(buffer >> 3) * scale as f64,
        unpack_lp_value(buffer >> 18) * scale as f64,
        unpack_lp_value(buffer >> 33) * scale as f64,
    ))
}

fn sanitize_lp_value(value: f64, abs_max: f64) -> f64 {
    if value.is_nan() {
        0.0
    } else {
        value.clamp(-abs_max, abs_max)
    }
}

fn pack_lp_value(value: f64, data_mask: f64) -> u64 {
    ((value * 0.5 + 0.5) * data_mask).round() as u64
}

fn unpack_lp_value(value: u64) -> f64 {
    const DATA_BITS_MASK: u64 = 32767;
    const MAX_QUANTIZED_VALUE: f64 = 32766.0;

    ((value & DATA_BITS_MASK).min(MAX_QUANTIZED_VALUE as u64) as f64) * 2.0 / MAX_QUANTIZED_VALUE
        - 1.0
}
