use bytes::BufMut as _;
use qexed_packet::{
    net_types::VarInt,
    PacketCodec, PacketReader, PacketWriter,
};
use crate::types::{ComponentsToAdd, Slot};

/// net.minecraft.world.item.trading.ItemCost：
/// Item.STREAM_CODEC（Holder<Item> → VarInt 注册表 id）+ count(VarInt)
/// + DataComponentExactPredicate.STREAM_CODEC（VarInt 计数前缀 + TypedDataComponent 列表）。
/// 依据 server-26.3.jar ItemCost.STREAM_CODEC（StreamCodec.composite 三段）与
/// DataComponentExactPredicate.STREAM_CODEC（ByteBufCodecs.list() 包装 TypedDataComponent）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ItemCost {
    /// net.minecraft.core.Holder<net.minecraft.world.item.Item> 注册表 id（VarInt）。
    pub item: VarInt,
    pub count: VarInt,
    /// DataComponentExactPredicate：线上为 VarInt 计数前缀 + TypedDataComponent 列表。
    /// 本仓库用已建模的组件枚举近似（组件 id 分发），空谓词编码为计数 0。
    pub components: Vec<ComponentsToAdd>,
}

impl PacketCodec for ItemCost {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.item.serialize(w)?;
        self.count.serialize(w)?;
        VarInt(self.components.len() as i32).serialize(w)?;
        for component in &self.components {
            component.serialize(w)?;
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.item.deserialize(r)?;
        self.count.deserialize(r)?;
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(qexed_packet::PacketError::msg(format!(
                "negative ItemCost components length: {}",
                len.0
            )));
        }
        self.components.clear();
        self.components.reserve(len.0 as usize);
        for _ in 0..len.0 {
            let mut component = ComponentsToAdd::default();
            component.deserialize(r)?;
            self.components.push(component);
        }
        Ok(())
    }
}

/// net.minecraft.world.item.trading.MerchantOffer：
/// 字段顺序依据 server-26.3.jar MerchantOffer.writeToStream / createFromStream——
/// costA、result、costB(Optional)、outOfStock(bool)、uses(i32)、maxUses(i32)、xp(i32)、
/// specialPriceDiff(i32)、priceMultiplier(f32)、demand(i32)。
/// 注意：26.3 中 uses/maxUses/xp/specialPriceDiff/demand 为固定 4 字节 writeInt，非 VarInt。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MerchantOffer {
    pub base_cost_a: ItemCost,
    /// result 为 ItemStack（ItemStack.STREAM_CODEC，即完整槽位格式，空栈非法）。
    pub result: Slot,
    /// costB 为 ItemCost.OPTIONAL_STREAM_CODEC：bool 存在标记 + 值。
    pub cost_b: Option<ItemCost>,
    pub out_of_stock: bool,
    pub uses: i32,
    pub max_uses: i32,
    pub villager_xp: i32,
    pub special_price_diff: i32,
    pub price_multiplier: f32,
    pub demand: i32,
}

impl PacketCodec for MerchantOffer {
    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.base_cost_a.serialize(w)?;
        self.result.serialize(w)?;
        self.cost_b.serialize(w)?;
        self.out_of_stock.serialize(w)?;
        w.buf.put_i32(self.uses);
        w.buf.put_i32(self.max_uses);
        w.buf.put_i32(self.villager_xp);
        w.buf.put_i32(self.special_price_diff);
        w.buf.put_f32(self.price_multiplier);
        w.buf.put_i32(self.demand);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        use bytes::Buf as _;
        self.base_cost_a.deserialize(r)?;
        self.result.deserialize(r)?;
        self.cost_b.deserialize(r)?;
        self.out_of_stock.deserialize(r)?;
        self.uses = r.buf.get_i32();
        self.max_uses = r.buf.get_i32();
        self.villager_xp = r.buf.get_i32();
        self.special_price_diff = r.buf.get_i32();
        self.price_multiplier = r.buf.get_f32();
        self.demand = r.buf.get_i32();
        Ok(())
    }
}

/// 26.3 ClientboundMerchantOffers（minecraft:merchant_offers，id 0x35 = 53）。
/// offers 容器编码依据 server-26.3.jar
/// MerchantOffers.STREAM_CODEC = MerchantOffer.STREAM_CODEC
///     .apply(ByteBufCodecs.collection(MerchantOffers::new))：
/// collection 即 ByteBufCodecs$26——VarInt 计数前缀 + 逐个元素，
/// 而非 present 字节 + 0 终止符（javap 反汇编已核实，见迁移报告）。
#[derive(Debug, Default, PartialEq, Clone)]
pub struct MerchantOffers {
    pub container_id: VarInt,
    pub offers: Vec<MerchantOffer>,
    pub villager_level: VarInt,
    pub villager_xp: VarInt,
    pub show_progress: bool,
    pub can_restock: bool,
}

impl qexed_packet::Packet for MerchantOffers {
    const ID: i32 = 0x35;

    fn serialize(&self, w: &mut PacketWriter) -> qexed_packet::Result<()> {
        self.container_id.serialize(w)?;
        VarInt(self.offers.len() as i32).serialize(w)?;
        for offer in &self.offers {
            offer.serialize(w)?;
        }
        self.villager_level.serialize(w)?;
        self.villager_xp.serialize(w)?;
        self.show_progress.serialize(w)?;
        self.can_restock.serialize(w)?;
        Ok(())
    }

    fn deserialize(&mut self, r: &mut PacketReader) -> qexed_packet::Result<()> {
        self.container_id.deserialize(r)?;
        let mut len = VarInt::default();
        len.deserialize(r)?;
        if len.0 < 0 {
            return Err(qexed_packet::PacketError::msg(format!(
                "negative merchant offers length: {}",
                len.0
            )));
        }
        self.offers.clear();
        self.offers.reserve(len.0.min(65536) as usize);
        for _ in 0..len.0 {
            let mut offer = MerchantOffer::default();
            offer.deserialize(r)?;
            self.offers.push(offer);
        }
        self.villager_level.deserialize(r)?;
        self.villager_xp.deserialize(r)?;
        self.show_progress.deserialize(r)?;
        self.can_restock.deserialize(r)?;
        Ok(())
    }
}
