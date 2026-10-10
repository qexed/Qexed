//! 通用调色板容器（paletted container）编解码。
//!
//! Anvil 区块里 block_states 与 biomes 都用这种结构存储：
//! `palette`（NBT 列表）+ `data`（64 位长整型紧凑位打包的索引数组）。

use qexed_nbt::Tag;

use crate::error::AnvilError;

/// 每个方块 section 的条目数：16*16*16。
pub const BLOCK_ENTRY_COUNT: usize = 16 * 16 * 16;
/// 每个 biome section 的条目数：4*4*4。
pub const BIOME_ENTRY_COUNT: usize = 4 * 4 * 4;

/// 容器种类（决定最小位宽约定）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PaletteKind {
    Block,
    Biome,
}

/// 向上取整 log2；count <= 1 时为 0。
pub fn ceil_log2(count: usize) -> usize {
    if count <= 1 {
        0
    } else {
        usize::BITS as usize - (count - 1).leading_zeros() as usize
    }
}

/// 磁盘格式（Anvil NBT）侧的索引位宽。
pub fn storage_bits(kind: PaletteKind, palette_len: usize) -> usize {
    let entry_bits = ceil_log2(palette_len);
    match kind {
        PaletteKind::Block => match entry_bits {
            0 => 0,
            1..=4 => 4,
            _ => entry_bits,
        },
        PaletteKind::Biome => entry_bits,
    }
}

/// 从长整型数组解包出 `entry_count` 个调色板索引。
///
/// `data` 为 `None` 且调色板非空时按"全部指向第 0 项"处理（单元素调色板
/// 在 Anvil 里省略 data 字段）。
pub fn unpack_indices(
    data: Option<&[i64]>,
    palette_len: usize,
    entry_count: usize,
    kind: PaletteKind,
) -> Result<Vec<usize>, AnvilError> {
    unpack_indices_spanning(data, palette_len, entry_count, kind, false)
}

/// 解包（可选拆分模式）。
///
/// `spanning = true`：1.13–1.15 的跨 long 位流（DataVersion < 2529）；
/// `false`：现代的每 long 独立打包。
pub fn unpack_indices_spanning(
    data: Option<&[i64]>,
    palette_len: usize,
    entry_count: usize,
    kind: PaletteKind,
    spanning: bool,
) -> Result<Vec<usize>, AnvilError> {
    if palette_len == 0 {
        return Ok(vec![0; entry_count]);
    }

    let bits = storage_bits(kind, palette_len);
    if bits == 0 {
        return Ok(vec![0; entry_count]);
    }

    let data = data.ok_or(AnvilError::MissingPaletteData(palette_len))?;
    let mask = (1_u64 << bits) - 1;

    if spanning {
        // 位流跨界：整体是 ceil(bits * count / 64) 个 long 的连续位流
        let required_len = (bits * entry_count).div_ceil(64);
        if data.len() != required_len {
            return Err(AnvilError::PaletteDataLength { got: data.len(), expected: required_len });
        }
        let mut values = Vec::with_capacity(entry_count);
        for index in 0..entry_count {
            let bit_offset = index * bits;
            let cell = bit_offset / 64;
            let shift = bit_offset % 64;
            let word = data[cell] as u64;
            // 跨界时拼接下一个 long（最后一个不会越界：required_len 已含它）
            let mut value = word >> shift;
            if shift + bits > 64 {
                let next = if cell + 1 < data.len() { data[cell + 1] as u64 } else { 0 };
                value |= next << (64 - shift);
            }
            let value = (value & mask) as usize;
            if value >= palette_len {
                return Err(AnvilError::PaletteIndexOutOfRange { index: value, palette_len });
            }
            values.push(value);
        }
        return Ok(values);
    }

    let values_per_long = 64 / bits;
    let required_len = entry_count.div_ceil(values_per_long);
    if data.len() != required_len {
        return Err(AnvilError::PaletteDataLength { got: data.len(), expected: required_len });
    }

    let mut values = Vec::with_capacity(entry_count);
    for index in 0..entry_count {
        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        let value = ((data[cell] as u64) >> bit_offset) & mask;
        let value = usize::try_from(value).expect("masked value fits usize");
        if value >= palette_len {
            return Err(AnvilError::PaletteIndexOutOfRange { index: value, palette_len });
        }
        values.push(value);
    }

    Ok(values)
}

/// 把调色板索引打包成长整型数组（与 unpack_indices 互逆）。
pub fn pack_values(values: &[usize], bits: usize) -> Result<Vec<u64>, AnvilError> {
    if bits == 0 {
        return Ok(Vec::new());
    }

    let values_per_long = 64 / bits;
    let mut packed = vec![0_u64; values.len().div_ceil(values_per_long)];
    let mask = (1_u64 << bits) - 1;

    for (index, value) in values.iter().enumerate() {
        let value = *value as u64;
        if value > mask {
            return Err(AnvilError::PaletteValueOverflow { value, bits });
        }

        let cell = index / values_per_long;
        let bit_offset = (index % values_per_long) * bits;
        packed[cell] |= value << bit_offset;
    }

    Ok(packed)
}

/// 值序列去重出本地调色板：返回 (palette, 每个值在 palette 里的索引)。
pub fn local_palette<T: PartialEq + Copy>(values: &[T]) -> (Vec<T>, Vec<usize>) {
    let mut palette = Vec::new();
    let mut local_values = Vec::with_capacity(values.len());

    for value in values {
        let index = palette
            .iter()
            .position(|existing| existing == value)
            .unwrap_or_else(|| {
                palette.push(*value);
                palette.len() - 1
            });
        local_values.push(index);
    }

    (palette, local_values)
}

/// 取 compound 字段（block_states / biomes 这类容器标签）。
pub fn container<'a>(tag: Option<&'a Tag>) -> Option<&'a std::collections::HashMap<String, Tag>> {
    match tag {
        Some(Tag::Compound(map)) => Some(map),
        _ => None,
    }
}

/// 取 NBT 里的长整型数组。
pub fn long_array(tag: Option<&Tag>) -> Option<&[i64]> {
    match tag {
        Some(Tag::LongArray(values)) => Some(values),
        _ => None,
    }
}
