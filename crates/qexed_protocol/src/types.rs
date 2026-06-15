use anyhow::Ok;
use bytes::Buf as _;
use qexed_packet::{
    PacketCodec,
    net_types::{Position, VarInt, VarLong},
};
use uuid::Uuid;
pub type TextComponent = qexed_packet::net_types::AnyNbt;

#[derive(Debug, Default, PartialEq, Eq, Clone)]
pub enum NumberFormat {
    #[default]
    Blank,
}

impl PacketCodec for NumberFormat {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        match self {
            Self::Blank => VarInt(0).serialize(w),
        }
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let mut type_id = VarInt::default();
        type_id.deserialize(r)?;
        match type_id.0 {
            0 => {
                *self = Self::Blank;
                Ok(())
            }
            other => anyhow::bail!("unsupported number format type id: {other}"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct MessageSignature(pub Vec<u8>);

impl MessageSignature {
    pub const BYTE_LEN: usize = 256;

    pub fn new(bytes: Vec<u8>) -> anyhow::Result<Self> {
        if bytes.len() != Self::BYTE_LEN {
            anyhow::bail!(
                "message signature length must be {}, got {}",
                Self::BYTE_LEN,
                bytes.len()
            );
        }
        Ok(Self(bytes))
    }
}

impl PacketCodec for MessageSignature {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        if self.0.len() != Self::BYTE_LEN {
            anyhow::bail!(
                "message signature length must be {}, got {}",
                Self::BYTE_LEN,
                self.0.len()
            );
        }
        w.buf.extend_from_slice(&self.0);
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        if r.buf.remaining() < Self::BYTE_LEN {
            anyhow::bail!(
                "message signature length {} exceeds remaining {}",
                Self::BYTE_LEN,
                r.buf.remaining()
            );
        }
        self.0 = r.buf.copy_to_bytes(Self::BYTE_LEN).to_vec();
        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct ChatSessionData {
    pub session_id: Uuid,
    pub expires_at_epoch_millis: i64,
    pub public_key_der: Vec<u8>,
    pub key_signature: Vec<u8>,
}

impl PacketCodec for ChatSessionData {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.session_id.serialize(w)?;
        self.expires_at_epoch_millis.serialize(w)?;
        write_byte_array(&self.public_key_der, 512, "chat public key", w)?;
        write_byte_array(&self.key_signature, 4096, "chat key signature", w)
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.session_id.deserialize(r)?;
        self.expires_at_epoch_millis.deserialize(r)?;
        self.public_key_der = read_byte_array(512, "chat public key", r)?;
        self.key_signature = read_byte_array(4096, "chat key signature", r)?;
        Ok(())
    }
}

fn write_byte_array(
    value: &[u8],
    max_size: usize,
    name: &str,
    w: &mut qexed_packet::PacketWriter,
) -> anyhow::Result<()> {
    if value.len() > max_size {
        anyhow::bail!("{name} length {} exceeds max {max_size}", value.len());
    }
    VarInt(value.len() as i32).serialize(w)?;
    w.buf.extend_from_slice(value);
    Ok(())
}

fn read_byte_array(
    max_size: usize,
    name: &str,
    r: &mut qexed_packet::PacketReader,
) -> anyhow::Result<Vec<u8>> {
    let mut len = VarInt::default();
    len.deserialize(r)?;
    if len.0 < 0 {
        anyhow::bail!("negative {name} length: {}", len.0);
    }
    let len = len.0 as usize;
    if len > max_size {
        anyhow::bail!("{name} length {len} exceeds max {max_size}");
    }
    if len > r.buf.remaining() {
        anyhow::bail!(
            "{name} length {len} exceeds remaining {}",
            r.buf.remaining()
        );
    }
    Ok(r.buf.copy_to_bytes(len).to_vec())
}

#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct KnownPacks {
    pub namespace: String,
    pub id: String,
    pub version: String,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct Slot {
    pub item_count: VarInt,
    pub item_id: Option<VarInt>,
    pub number_of_components_to_add: Option<VarInt>,
    pub number_of_components_to_remove: Option<VarInt>,
    pub components_to_add: Option<Vec<ComponentsToAdd>>,
    pub components_to_remove: Option<Vec<VarInt>>,
}

impl PacketCodec for Slot {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.item_count.serialize(w)?;

        // 如果 item_count 为 0，则后面没有数据
        if self.item_count.0 == 0 {
            return Ok(());
        }

        // 序列化 item_id (必须存在，因为 item_count > 0)
        if let Some(item_id) = &self.item_id {
            item_id.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!("item_id is required when item_count > 0"));
        }

        // 序列化 number_of_components_to_add
        if let Some(number_of_components_to_add) = &self.number_of_components_to_add {
            number_of_components_to_add.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!(
                "number_of_components_to_add is required when item_count > 0"
            ));
        }

        // 序列化 number_of_components_to_remove
        if let Some(number_of_components_to_remove) = &self.number_of_components_to_remove {
            number_of_components_to_remove.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!(
                "number_of_components_to_remove is required when item_count > 0"
            ));
        }

        // 序列化 components_to_add
        if let Some(components_to_add) = &self.components_to_add {
            // 注意：长度已经在 number_of_components_to_add 中指定
            for prop in components_to_add {
                prop.serialize(w)?;
            }
        }

        // 序列化 components_to_remove
        if let Some(components_to_remove) = &self.components_to_remove {
            // 注意：长度已经在 number_of_components_to_remove 中指定
            for prop in components_to_remove {
                prop.serialize(w)?;
            }
        }

        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        // 读取 item_count
        self.item_count.deserialize(r)?;

        // 如果 item_count 为 0，则后面没有数据
        if self.item_count.0 == 0 {
            // 清空其他字段
            self.item_id = None;
            self.number_of_components_to_add = None;
            self.number_of_components_to_remove = None;
            self.components_to_add = None;
            self.components_to_remove = None;
            return Ok(());
        }

        // 读取 item_id
        let mut item_id = VarInt(0);
        item_id.deserialize(r)?;
        self.item_id = Some(item_id);

        // 读取 number_of_components_to_add
        let mut num_to_add = VarInt(0);
        num_to_add.deserialize(r)?;
        self.number_of_components_to_add = Some(num_to_add);

        // 读取 number_of_components_to_remove
        let mut num_to_remove = VarInt(0);
        num_to_remove.deserialize(r)?;
        self.number_of_components_to_remove = Some(num_to_remove);

        // 读取 components_to_add
        if self.number_of_components_to_add.as_ref().unwrap().0 > 0 {
            let mut components =
                Vec::with_capacity(self.number_of_components_to_add.as_ref().unwrap().0 as usize);
            for _ in 0..self.number_of_components_to_add.as_ref().unwrap().0 {
                let mut component = ComponentsToAdd::default();
                component.deserialize(r)?;
                components.push(component);
            }
            self.components_to_add = Some(components);
        } else {
            self.components_to_add = None;
        }

        // 读取 components_to_remove
        if self.number_of_components_to_remove.as_ref().unwrap().0 > 0 {
            let mut components = Vec::with_capacity(
                self.number_of_components_to_remove.as_ref().unwrap().0 as usize,
            );
            for _ in 0..self.number_of_components_to_remove.as_ref().unwrap().0 {
                let mut component = VarInt(0);
                component.deserialize(r)?;
                components.push(component);
            }
            self.components_to_remove = Some(components);
        } else {
            self.components_to_remove = None;
        }

        Ok(())
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct SlotHash {
    pub item_count: VarInt,
    pub item_id: Option<VarInt>,
    pub number_of_components_to_add: Option<VarInt>,
    pub number_of_components_to_remove: Option<VarInt>,
    pub components_to_add: Option<Vec<ComponentsToAddHash>>,
    pub components_to_remove: Option<Vec<VarInt>>,
}
impl PacketCodec for SlotHash {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.item_count.serialize(w)?;

        // 如果 item_count 为 0，则后面没有数据
        if self.item_count.0 == 0 {
            return Ok(());
        }

        // 序列化 item_id (必须存在，因为 item_count > 0)
        if let Some(item_id) = &self.item_id {
            item_id.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!("item_id is required when item_count > 0"));
        }

        // 序列化 number_of_components_to_add
        if let Some(number_of_components_to_add) = &self.number_of_components_to_add {
            number_of_components_to_add.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!(
                "number_of_components_to_add is required when item_count > 0"
            ));
        }

        // 序列化 number_of_components_to_remove
        if let Some(number_of_components_to_remove) = &self.number_of_components_to_remove {
            number_of_components_to_remove.serialize(w)?;
        } else {
            return Err(anyhow::anyhow!(
                "number_of_components_to_remove is required when item_count > 0"
            ));
        }

        // 序列化 components_to_add
        if let Some(components_to_add) = &self.components_to_add {
            // 注意：长度已经在 number_of_components_to_add 中指定
            for prop in components_to_add {
                prop.serialize(w)?;
            }
        }

        // 序列化 components_to_remove
        if let Some(components_to_remove) = &self.components_to_remove {
            // 注意：长度已经在 number_of_components_to_remove 中指定
            for prop in components_to_remove {
                prop.serialize(w)?;
            }
        }

        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        // 读取 item_count
        self.item_count.deserialize(r)?;

        // 如果 item_count 为 0，则后面没有数据
        if self.item_count.0 == 0 {
            // 清空其他字段
            self.item_id = None;
            self.number_of_components_to_add = None;
            self.number_of_components_to_remove = None;
            self.components_to_add = None;
            self.components_to_remove = None;
            return Ok(());
        }

        // 读取 item_id
        let mut item_id = VarInt(0);
        item_id.deserialize(r)?;
        self.item_id = Some(item_id);

        // 读取 number_of_components_to_add
        let mut num_to_add = VarInt(0);
        num_to_add.deserialize(r)?;
        self.number_of_components_to_add = Some(num_to_add);

        // 读取 number_of_components_to_remove
        let mut num_to_remove = VarInt(0);
        num_to_remove.deserialize(r)?;
        self.number_of_components_to_remove = Some(num_to_remove);

        // 读取 components_to_add
        if self.number_of_components_to_add.as_ref().unwrap().0 > 0 {
            let mut components =
                Vec::with_capacity(self.number_of_components_to_add.as_ref().unwrap().0 as usize);
            for _ in 0..self.number_of_components_to_add.as_ref().unwrap().0 {
                let mut component = ComponentsToAddHash::default();
                component.deserialize(r)?;
                components.push(component);
            }
            self.components_to_add = Some(components);
        } else {
            self.components_to_add = None;
        }

        // 读取 components_to_remove
        if self.number_of_components_to_remove.as_ref().unwrap().0 > 0 {
            let mut components = Vec::with_capacity(
                self.number_of_components_to_remove.as_ref().unwrap().0 as usize,
            );
            for _ in 0..self.number_of_components_to_remove.as_ref().unwrap().0 {
                let mut component = VarInt(0);
                component.deserialize(r)?;
                components.push(component);
            }
            self.components_to_remove = Some(components);
        } else {
            self.components_to_remove = None;
        }

        Ok(())
    }
}
#[derive(Debug, PartialEq, Clone)]
pub enum ComponentsToAdd {
    MinecraftCustomData(minecraft::CustomData),
    MinecraftMaxStackSize(minecraft::MaxStackSize),
    MinecraftMaxDamage(minecraft::MaxDamage),
    MinecraftDamage(minecraft::Damage),
    MinecraftUnbreakable(minecraft::Unbreakable),
    MinecraftCustomName(minecraft::CustomName),
    MinecraftItemName(minecraft::ItemName),
    MinecraftItemModel(minecraft::ItemModel),
    MinecraftLore(minecraft::Lore),
    MinecraftRarity(minecraft::Rarity),
    MinecraftEnchantments(minecraft::Enchantments),
    MinecraftCanPlaceOn(minecraft::CanPlaceOn),
    MinecraftCanBreak(minecraft::CanBreak),
    MinecraftAttributeModifiers(minecraft::AttributeModifiers),
    MinecraftCustomModelData(minecraft::CustomModelData),
    MinecraftTooltipDisplay(minecraft::TooltipDisplay),
    MinecraftRepairCost(minecraft::RepairCost),
    MinecraftCreativeSlotLock(minecraft::CreativeSlotLock),
    MinecraftEnchantmentGlintOverride(minecraft::EnchantmentGlintOverride),
    MinecraftIntangibleProjectile(minecraft::IntangibleProjectile),
    MinecraftFood(minecraft::Food),
    MinecraftConsumable(minecraft::Consumable),
    MinecraftUseRemainder(minecraft::UseRemainder),
    MinecraftUseCooldown(minecraft::UseCooldown),
    MinecraftDamageResistant(minecraft::DamageResistant),
    MinecraftTool(minecraft::Tool),
    MinecraftWeapon(minecraft::Weapon),
    MinecraftEnchantable(minecraft::Enchantable),
    MinecraftEquippable(minecraft::Equippable),
    MinecraftRepairable(minecraft::Repairable),
    MinecraftGlider(minecraft::Glider),
    MinecraftTooltipStyle(minecraft::TooltipStyle),
    MinecraftDeathProtection(minecraft::DeathProtection),
    MinecraftBlocksAttacks(minecraft::BlocksAttacks),
    MinecraftStoredEnchantments(minecraft::StoredEnchantments),
    MinecraftDyedColor(minecraft::DyedColor),
    MinecraftMapColor(minecraft::MapColor),
    MinecraftMapId(minecraft::MapId),
    MinecraftMapDecorations(minecraft::MapDecorations),
    MinecraftMapPostProcessing(minecraft::MapPostProcessing),
    MinecraftChargedProjectiles(minecraft::ChargedProjectiles),
    MinecraftBundleContents(minecraft::BundleContents),
    MinecraftPotionContents(minecraft::PotionContents),
    MinecraftPotionDurationScale(minecraft::PotionDurationScale),
    MinecraftSuspiciousStewEffects(minecraft::SuspiciousStewEffects),
    MinecraftWritableBookContent(minecraft::WritableBookContent),
    MinecraftWrittenBookContent(minecraft::WrittenBookContent),
    MinecraftTrim(minecraft::Trim),
    MinecraftDebugStickState(minecraft::DebugStickState),
    MinecraftEntityData(minecraft::EntityData),
    MinecraftBucketEntityData(minecraft::BucketEntityData),
    MinecraftBlockEntityData(minecraft::BlockEntityData),
    MinecraftInstrument(minecraft::Instrument),
    MinecraftProvidesTrimMaterial(minecraft::ProvidesTrimMaterial),
    MinecraftOminousBottleAmplifier(minecraft::OminousBottleAmplifier),
    MinecraftJukeboxPlayable(minecraft::JukeboxPlayable),
    MinecraftProvidesBannerPatterns(minecraft::ProvidesBannerPatterns),
    MinecraftRecipes(minecraft::Recipes),
    MinecraftLodestoneTracker(minecraft::LodestoneTracker),
    MinecraftFireworkExplosion(minecraft::FireworkExplosion),
    MinecraftFireworks(minecraft::Fireworks),
    MinecraftProfile(minecraft::Profile),
    MinecraftNoteBlockSound(minecraft::NoteBlockSound),
    MinecraftBannerPatterns(minecraft::BannerPatterns),
    MinecraftBaseColor(minecraft::BaseColor),
    MinecraftPotDecorations(minecraft::PotDecorations),
    MinecraftContainer(minecraft::Container),
    MinecraftBlockState(minecraft::BlockState),
    MinecraftBees(minecraft::Bees),
    MinecraftLock(minecraft::Lock),
    MinecraftContainerLoot(minecraft::ContainerLoot),
    MinecraftBreakSound(minecraft::BreakSound),
    MinecraftVillagerVariant(minecraft::VillagerVariant),
    MinecraftWolfVariant(minecraft::WolfVariant),
    MinecraftWolfSoundVariant(minecraft::WolfSoundVariant),
    MinecraftWolfCollar(minecraft::WolfCollar),
    MinecraftFoxVariant(minecraft::FoxVariant),
    MinecraftSalmonSize(minecraft::SalmonSize),
    MinecraftParrotVariant(minecraft::ParrotVariant),
    MinecraftTropicalFishPattern(minecraft::TropicalFishPattern),
    MinecraftTropicalFishBaseColor(minecraft::TropicalFishBaseColor),
    MinecraftTropicalFishPatternColor(minecraft::TropicalFishPatternColor),
    MinecraftMooshroomVariant(minecraft::MooshroomVariant),
    MinecraftRabbitVariant(minecraft::RabbitVariant),
    MinecraftPigVariant(minecraft::PigVariant),
    MinecraftCowVariant(minecraft::CowVariant),
    MinecraftChickenVariant(minecraft::ChickenVariant),
    MinecraftFrogVariant(minecraft::FrogVariant),
    MinecraftHorseVariant(minecraft::HorseVariant),
    MinecraftPaintingVariant(minecraft::PaintingVariant),
    MinecraftLlamaVariant(minecraft::LlamaVariant),
    MinecraftAxolotlVariant(minecraft::AxolotlVariant),
    MinecraftCatVariant(minecraft::CatVariant),
    MinecraftCatCollar(minecraft::CatCollar),
    MinecraftSheepColor(minecraft::SheepColor),
    MinecraftShulkerColor(minecraft::ShulkerColor),
    Unknown,
}

impl Default for ComponentsToAdd {
    fn default() -> Self {
        Self::Unknown
    }
}

impl PacketCodec for ComponentsToAdd {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        match self {
            Self::MinecraftCustomData(data) => serialize_data_component(0, data, w),
            Self::MinecraftMaxStackSize(data) => serialize_data_component(1, data, w),
            Self::MinecraftMaxDamage(data) => serialize_data_component(2, data, w),
            Self::MinecraftDamage(data) => serialize_data_component(3, data, w),
            Self::MinecraftUnbreakable(data) => serialize_data_component(4, data, w),
            Self::MinecraftCustomName(data) => serialize_data_component(6, data, w),
            Self::MinecraftItemName(data) => serialize_data_component(9, data, w),
            Self::MinecraftItemModel(data) => serialize_data_component(10, data, w),
            Self::MinecraftLore(data) => serialize_data_component(11, data, w),
            Self::MinecraftRarity(data) => serialize_data_component(12, data, w),
            Self::MinecraftEnchantments(data) => serialize_data_component(13, data, w),
            Self::MinecraftCanPlaceOn(data) => serialize_data_component(14, data, w),
            Self::MinecraftCanBreak(data) => serialize_data_component(15, data, w),
            Self::MinecraftAttributeModifiers(data) => serialize_data_component(16, data, w),
            Self::MinecraftCustomModelData(data) => serialize_data_component(17, data, w),
            Self::MinecraftTooltipDisplay(data) => serialize_data_component(18, data, w),
            Self::MinecraftRepairCost(data) => serialize_data_component(19, data, w),
            Self::MinecraftCreativeSlotLock(data) => serialize_data_component(20, data, w),
            Self::MinecraftEnchantmentGlintOverride(data) => serialize_data_component(21, data, w),
            Self::MinecraftIntangibleProjectile(data) => serialize_data_component(22, data, w),
            Self::MinecraftFood(data) => serialize_data_component(23, data, w),
            Self::MinecraftConsumable(data) => serialize_data_component(24, data, w),
            Self::MinecraftUseRemainder(data) => serialize_data_component(25, data, w),
            Self::MinecraftUseCooldown(data) => serialize_data_component(26, data, w),
            Self::MinecraftDamageResistant(data) => serialize_data_component(27, data, w),
            Self::MinecraftTool(data) => serialize_data_component(28, data, w),
            Self::MinecraftWeapon(data) => serialize_data_component(29, data, w),
            Self::MinecraftEnchantable(data) => serialize_data_component(31, data, w),
            Self::MinecraftEquippable(data) => serialize_data_component(32, data, w),
            Self::MinecraftRepairable(data) => serialize_data_component(33, data, w),
            Self::MinecraftGlider(data) => serialize_data_component(34, data, w),
            Self::MinecraftTooltipStyle(data) => serialize_data_component(35, data, w),
            Self::MinecraftDeathProtection(data) => serialize_data_component(36, data, w),
            Self::MinecraftBlocksAttacks(data) => serialize_data_component(37, data, w),
            Self::MinecraftStoredEnchantments(data) => serialize_data_component(42, data, w),
            Self::MinecraftDyedColor(data) => serialize_data_component(44, data, w),
            Self::MinecraftMapColor(data) => serialize_data_component(45, data, w),
            Self::MinecraftMapId(data) => serialize_data_component(46, data, w),
            Self::MinecraftMapDecorations(data) => serialize_data_component(47, data, w),
            Self::MinecraftMapPostProcessing(data) => serialize_data_component(48, data, w),
            Self::MinecraftChargedProjectiles(data) => serialize_data_component(49, data, w),
            Self::MinecraftBundleContents(data) => serialize_data_component(50, data, w),
            Self::MinecraftPotionContents(data) => serialize_data_component(51, data, w),
            Self::MinecraftPotionDurationScale(data) => serialize_data_component(52, data, w),
            Self::MinecraftSuspiciousStewEffects(data) => serialize_data_component(53, data, w),
            Self::MinecraftWritableBookContent(data) => serialize_data_component(54, data, w),
            Self::MinecraftWrittenBookContent(data) => serialize_data_component(55, data, w),
            Self::MinecraftTrim(data) => serialize_data_component(56, data, w),
            Self::MinecraftDebugStickState(data) => serialize_data_component(57, data, w),
            Self::MinecraftEntityData(data) => serialize_data_component(58, data, w),
            Self::MinecraftBucketEntityData(data) => serialize_data_component(59, data, w),
            Self::MinecraftBlockEntityData(data) => serialize_data_component(60, data, w),
            Self::MinecraftInstrument(data) => serialize_data_component(61, data, w),
            Self::MinecraftProvidesTrimMaterial(data) => serialize_data_component(62, data, w),
            Self::MinecraftOminousBottleAmplifier(data) => serialize_data_component(63, data, w),
            Self::MinecraftJukeboxPlayable(data) => serialize_data_component(64, data, w),
            Self::MinecraftProvidesBannerPatterns(data) => serialize_data_component(65, data, w),
            Self::MinecraftRecipes(data) => serialize_data_component(66, data, w),
            Self::MinecraftLodestoneTracker(data) => serialize_data_component(67, data, w),
            Self::MinecraftFireworkExplosion(data) => serialize_data_component(68, data, w),
            Self::MinecraftFireworks(data) => serialize_data_component(69, data, w),
            Self::MinecraftProfile(data) => serialize_data_component(70, data, w),
            Self::MinecraftNoteBlockSound(data) => serialize_data_component(71, data, w),
            Self::MinecraftBannerPatterns(data) => serialize_data_component(72, data, w),
            Self::MinecraftBaseColor(data) => serialize_data_component(73, data, w),
            Self::MinecraftPotDecorations(data) => serialize_data_component(74, data, w),
            Self::MinecraftContainer(data) => serialize_data_component(75, data, w),
            Self::MinecraftBlockState(data) => serialize_data_component(76, data, w),
            Self::MinecraftBees(data) => serialize_data_component(77, data, w),
            Self::MinecraftLock(data) => serialize_data_component(78, data, w),
            Self::MinecraftContainerLoot(data) => serialize_data_component(79, data, w),
            Self::MinecraftBreakSound(data) => serialize_data_component(80, data, w),
            Self::MinecraftVillagerVariant(data) => serialize_data_component(81, data, w),
            Self::MinecraftWolfVariant(data) => serialize_data_component(82, data, w),
            Self::MinecraftWolfSoundVariant(data) => serialize_data_component(83, data, w),
            Self::MinecraftWolfCollar(data) => serialize_data_component(84, data, w),
            Self::MinecraftFoxVariant(data) => serialize_data_component(85, data, w),
            Self::MinecraftSalmonSize(data) => serialize_data_component(86, data, w),
            Self::MinecraftParrotVariant(data) => serialize_data_component(87, data, w),
            Self::MinecraftTropicalFishPattern(data) => serialize_data_component(88, data, w),
            Self::MinecraftTropicalFishBaseColor(data) => serialize_data_component(89, data, w),
            Self::MinecraftTropicalFishPatternColor(data) => serialize_data_component(90, data, w),
            Self::MinecraftMooshroomVariant(data) => serialize_data_component(91, data, w),
            Self::MinecraftRabbitVariant(data) => serialize_data_component(92, data, w),
            Self::MinecraftPigVariant(data) => serialize_data_component(93, data, w),
            Self::MinecraftCowVariant(data) => serialize_data_component(95, data, w),
            Self::MinecraftChickenVariant(data) => serialize_data_component(97, data, w),
            Self::MinecraftFrogVariant(data) => serialize_data_component(100, data, w),
            Self::MinecraftHorseVariant(data) => serialize_data_component(101, data, w),
            Self::MinecraftPaintingVariant(data) => serialize_data_component(102, data, w),
            Self::MinecraftLlamaVariant(data) => serialize_data_component(103, data, w),
            Self::MinecraftAxolotlVariant(data) => serialize_data_component(104, data, w),
            Self::MinecraftCatVariant(data) => serialize_data_component(105, data, w),
            Self::MinecraftCatCollar(data) => serialize_data_component(107, data, w),
            Self::MinecraftSheepColor(data) => serialize_data_component(108, data, w),
            Self::MinecraftShulkerColor(data) => serialize_data_component(109, data, w),
            Self::Unknown => Err(anyhow::anyhow!("Cannot serialize unknown data component")),
        }
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        let mut id = VarInt(-1);
        id.deserialize(r)?;
        *self = match id.0 {
            0 => Self::MinecraftCustomData(deserialize_data_component(r)?),
            1 => Self::MinecraftMaxStackSize(deserialize_data_component(r)?),
            2 => Self::MinecraftMaxDamage(deserialize_data_component(r)?),
            3 => Self::MinecraftDamage(deserialize_data_component(r)?),
            4 => Self::MinecraftUnbreakable(deserialize_data_component(r)?),
            6 => Self::MinecraftCustomName(deserialize_data_component(r)?),
            9 => Self::MinecraftItemName(deserialize_data_component(r)?),
            10 => Self::MinecraftItemModel(deserialize_data_component(r)?),
            11 => Self::MinecraftLore(deserialize_data_component(r)?),
            12 => Self::MinecraftRarity(deserialize_data_component(r)?),
            13 => Self::MinecraftEnchantments(deserialize_data_component(r)?),
            14 => Self::MinecraftCanPlaceOn(deserialize_data_component(r)?),
            15 => Self::MinecraftCanBreak(deserialize_data_component(r)?),
            16 => Self::MinecraftAttributeModifiers(deserialize_data_component(r)?),
            17 => Self::MinecraftCustomModelData(deserialize_data_component(r)?),
            18 => Self::MinecraftTooltipDisplay(deserialize_data_component(r)?),
            19 => Self::MinecraftRepairCost(deserialize_data_component(r)?),
            20 => Self::MinecraftCreativeSlotLock(deserialize_data_component(r)?),
            21 => Self::MinecraftEnchantmentGlintOverride(deserialize_data_component(r)?),
            22 => Self::MinecraftIntangibleProjectile(deserialize_data_component(r)?),
            23 => Self::MinecraftFood(deserialize_data_component(r)?),
            24 => Self::MinecraftConsumable(deserialize_data_component(r)?),
            25 => Self::MinecraftUseRemainder(deserialize_data_component(r)?),
            26 => Self::MinecraftUseCooldown(deserialize_data_component(r)?),
            27 => Self::MinecraftDamageResistant(deserialize_data_component(r)?),
            28 => Self::MinecraftTool(deserialize_data_component(r)?),
            29 => Self::MinecraftWeapon(deserialize_data_component(r)?),
            31 => Self::MinecraftEnchantable(deserialize_data_component(r)?),
            32 => Self::MinecraftEquippable(deserialize_data_component(r)?),
            33 => Self::MinecraftRepairable(deserialize_data_component(r)?),
            34 => Self::MinecraftGlider(deserialize_data_component(r)?),
            35 => Self::MinecraftTooltipStyle(deserialize_data_component(r)?),
            36 => Self::MinecraftDeathProtection(deserialize_data_component(r)?),
            37 => Self::MinecraftBlocksAttacks(deserialize_data_component(r)?),
            42 => Self::MinecraftStoredEnchantments(deserialize_data_component(r)?),
            44 => Self::MinecraftDyedColor(deserialize_data_component(r)?),
            45 => Self::MinecraftMapColor(deserialize_data_component(r)?),
            46 => Self::MinecraftMapId(deserialize_data_component(r)?),
            47 => Self::MinecraftMapDecorations(deserialize_data_component(r)?),
            48 => Self::MinecraftMapPostProcessing(deserialize_data_component(r)?),
            49 => Self::MinecraftChargedProjectiles(deserialize_data_component(r)?),
            50 => Self::MinecraftBundleContents(deserialize_data_component(r)?),
            51 => Self::MinecraftPotionContents(deserialize_data_component(r)?),
            52 => Self::MinecraftPotionDurationScale(deserialize_data_component(r)?),
            53 => Self::MinecraftSuspiciousStewEffects(deserialize_data_component(r)?),
            54 => Self::MinecraftWritableBookContent(deserialize_data_component(r)?),
            55 => Self::MinecraftWrittenBookContent(deserialize_data_component(r)?),
            56 => Self::MinecraftTrim(deserialize_data_component(r)?),
            57 => Self::MinecraftDebugStickState(deserialize_data_component(r)?),
            58 => Self::MinecraftEntityData(deserialize_data_component(r)?),
            59 => Self::MinecraftBucketEntityData(deserialize_data_component(r)?),
            60 => Self::MinecraftBlockEntityData(deserialize_data_component(r)?),
            61 => Self::MinecraftInstrument(deserialize_data_component(r)?),
            62 => Self::MinecraftProvidesTrimMaterial(deserialize_data_component(r)?),
            63 => Self::MinecraftOminousBottleAmplifier(deserialize_data_component(r)?),
            64 => Self::MinecraftJukeboxPlayable(deserialize_data_component(r)?),
            65 => Self::MinecraftProvidesBannerPatterns(deserialize_data_component(r)?),
            66 => Self::MinecraftRecipes(deserialize_data_component(r)?),
            67 => Self::MinecraftLodestoneTracker(deserialize_data_component(r)?),
            68 => Self::MinecraftFireworkExplosion(deserialize_data_component(r)?),
            69 => Self::MinecraftFireworks(deserialize_data_component(r)?),
            70 => Self::MinecraftProfile(deserialize_data_component(r)?),
            71 => Self::MinecraftNoteBlockSound(deserialize_data_component(r)?),
            72 => Self::MinecraftBannerPatterns(deserialize_data_component(r)?),
            73 => Self::MinecraftBaseColor(deserialize_data_component(r)?),
            74 => Self::MinecraftPotDecorations(deserialize_data_component(r)?),
            75 => Self::MinecraftContainer(deserialize_data_component(r)?),
            76 => Self::MinecraftBlockState(deserialize_data_component(r)?),
            77 => Self::MinecraftBees(deserialize_data_component(r)?),
            78 => Self::MinecraftLock(deserialize_data_component(r)?),
            79 => Self::MinecraftContainerLoot(deserialize_data_component(r)?),
            80 => Self::MinecraftBreakSound(deserialize_data_component(r)?),
            81 => Self::MinecraftVillagerVariant(deserialize_data_component(r)?),
            82 => Self::MinecraftWolfVariant(deserialize_data_component(r)?),
            83 => Self::MinecraftWolfSoundVariant(deserialize_data_component(r)?),
            84 => Self::MinecraftWolfCollar(deserialize_data_component(r)?),
            85 => Self::MinecraftFoxVariant(deserialize_data_component(r)?),
            86 => Self::MinecraftSalmonSize(deserialize_data_component(r)?),
            87 => Self::MinecraftParrotVariant(deserialize_data_component(r)?),
            88 => Self::MinecraftTropicalFishPattern(deserialize_data_component(r)?),
            89 => Self::MinecraftTropicalFishBaseColor(deserialize_data_component(r)?),
            90 => Self::MinecraftTropicalFishPatternColor(deserialize_data_component(r)?),
            91 => Self::MinecraftMooshroomVariant(deserialize_data_component(r)?),
            92 => Self::MinecraftRabbitVariant(deserialize_data_component(r)?),
            93 => Self::MinecraftPigVariant(deserialize_data_component(r)?),
            95 => Self::MinecraftCowVariant(deserialize_data_component(r)?),
            97 => Self::MinecraftChickenVariant(deserialize_data_component(r)?),
            100 => Self::MinecraftFrogVariant(deserialize_data_component(r)?),
            101 => Self::MinecraftHorseVariant(deserialize_data_component(r)?),
            102 => Self::MinecraftPaintingVariant(deserialize_data_component(r)?),
            103 => Self::MinecraftLlamaVariant(deserialize_data_component(r)?),
            104 => Self::MinecraftAxolotlVariant(deserialize_data_component(r)?),
            105 => Self::MinecraftCatVariant(deserialize_data_component(r)?),
            107 => Self::MinecraftCatCollar(deserialize_data_component(r)?),
            108 => Self::MinecraftSheepColor(deserialize_data_component(r)?),
            109 => Self::MinecraftShulkerColor(deserialize_data_component(r)?),
            other => anyhow::bail!("unsupported data component type id: {other}"),
        };
        Ok(())
    }
}

fn serialize_data_component<T: PacketCodec>(
    id: i32,
    data: &T,
    w: &mut qexed_packet::PacketWriter,
) -> anyhow::Result<()> {
    VarInt(id).serialize(w)?;
    data.serialize(w)
}

fn deserialize_data_component<T: PacketCodec + Default>(
    r: &mut qexed_packet::PacketReader,
) -> anyhow::Result<T> {
    let mut data = T::default();
    data.deserialize(r)?;
    Ok(data)
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ComponentsToAddHash {
    pub component_type: VarInt,
    pub component_data: i32,
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct IDSet {
    pub r#type: VarInt,
    pub tag_name: Option<String>,
    pub ids: Option<Vec<VarInt>>,
}
impl PacketCodec for IDSet {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.r#type.serialize(w)?;
        if self.r#type.0 == 0 {
            if let Some(tag_name) = &self.tag_name {
                tag_name.serialize(w)?;
            } else {
                return Err(anyhow::anyhow!("未定义tag_name"));
            }
        } else {
            if let Some(ids) = &self.ids {
                for prop in ids {
                    prop.serialize(w)?;
                }
            } else {
                return Err(anyhow::anyhow!("未定义ids"));
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.r#type.deserialize(r)?;
        if self.r#type.0 == 0 {
            let mut tag_name: String = Default::default();
            tag_name.deserialize(r)?;
            self.tag_name = Some(tag_name);
        } else {
            let mut ids: Vec<VarInt> = Default::default();
            for _ in 0..self.r#type.0 - 1 {
                let mut id = VarInt::default();
                id.deserialize(r)?;
                ids.push(id);
            }
            self.ids = Some(ids);
        }

        Ok(())
    }
}

#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum RecipeDisplay {
    MinecraftCraftingShapeless(minecraft::CraftingShapeless),
    MinecraftCraftingShaped(minecraft::CraftingShaped),
    MinecraftFurnace(minecraft::Furnace),
    MinecraftStonecutter(minecraft::Stonecutter),
    MinecraftSmithing(minecraft::Smithing),
    Unknown,
}
pub mod minecraft {
    use qexed_packet::{
        PacketCodec,
        net_types::{Position, VarInt},
    };
    use uuid::Uuid;

    use crate::types::{Slot, SlotDisplay, TextComponent};

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CustomData {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MaxStackSize {
        pub max_stack_size: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MaxDamage {
        pub max_damage: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Damage {
        pub damage: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Unbreakable;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CustomName {
        pub name: TextComponent,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ItemName {
        pub name: TextComponent,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ItemModel {
        pub model: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Lore {
        pub lines: Vec<TextComponent>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Rarity {
        pub rarity: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Enchantment {
        pub enchantment: VarInt,
        pub level: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Enchantments {
        pub enchantments: Vec<Enchantment>,
    }

    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BlockPredicate {
        pub blocks: Option<super::IDSet>,
        pub properties: Option<StatePropertiesPredicate>,
        pub nbt: Option<qexed_nbt::Tag>,
    }

    impl PacketCodec for BlockPredicate {
        fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
            self.blocks.serialize(w)?;
            self.properties.serialize(w)?;
            self.nbt.serialize(w)
        }

        fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
            self.blocks.deserialize(r)?;
            self.properties.deserialize(r)?;
            self.nbt.deserialize(r)
        }
    }

    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct StatePropertiesPredicate {
        pub properties: Vec<StatePropertyMatcher>,
    }

    impl PacketCodec for StatePropertiesPredicate {
        fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
            self.properties.serialize(w)
        }

        fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
            self.properties.deserialize(r)
        }
    }

    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct StatePropertyMatcher {
        pub name: String,
        pub matcher: StatePropertyMatcherValue,
    }

    impl PacketCodec for StatePropertyMatcher {
        fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
            self.name.serialize(w)?;
            self.matcher.serialize(w)
        }

        fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
            self.name.deserialize(r)?;
            self.matcher.deserialize(r)
        }
    }

    #[derive(Debug, PartialEq, Clone)]
    pub enum StatePropertyMatcherValue {
        Exact(String),
        Range {
            min: Option<String>,
            max: Option<String>,
        },
    }

    impl Default for StatePropertyMatcherValue {
        fn default() -> Self {
            Self::Exact(String::new())
        }
    }

    impl PacketCodec for StatePropertyMatcherValue {
        fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
            match self {
                Self::Exact(value) => {
                    true.serialize(w)?;
                    value.serialize(w)
                }
                Self::Range { min, max } => {
                    false.serialize(w)?;
                    min.serialize(w)?;
                    max.serialize(w)
                }
            }
        }

        fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
            let mut is_exact = false;
            is_exact.deserialize(r)?;
            if is_exact {
                let mut value = String::new();
                value.deserialize(r)?;
                *self = Self::Exact(value);
            } else {
                let mut min = Option::<String>::default();
                let mut max = Option::<String>::default();
                min.deserialize(r)?;
                max.deserialize(r)?;
                *self = Self::Range { min, max };
            }
            Ok(())
        }
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CanPlaceOn {
        pub block_predicates: Vec<BlockPredicate>,
        pub show_in_tooltip: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CanBreak {
        pub block_predicates: Vec<BlockPredicate>,
        pub show_in_tooltip: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct AttributeModifier {
        pub attribute: VarInt,
        pub modifier_id: String,
        pub value: f64,
        pub operation: VarInt,
        pub slot: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct AttributeModifiers {
        pub modifiers: Vec<AttributeModifier>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CustomModelData {
        pub f32s: Vec<f32>,
        pub flags: Vec<bool>,
        pub strings: Vec<String>,
        pub colors: Vec<i32>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct TooltipDisplay {
        pub hide_tooltip: bool,
        pub hidden_components: Vec<VarInt>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct RepairCost {
        pub cost: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CreativeSlotLock;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct EnchantmentGlintOverride {
        pub has_glint: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct IntangibleProjectile {
        pub empty: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Food {
        pub nutrition: VarInt,
        pub saturation_modifier: f32,
        pub can_always_eat: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ConsumeEffect;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Consumable {
        pub consume_seconds: f32,
        pub animation: VarInt,
        pub sound: String,
        pub has_consume_particles: bool,
        pub effects: Vec<ConsumeEffect>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct UseRemainder {
        pub remainder: Slot,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct UseCooldown {
        pub seconds: f32,
        pub cooldown_group: Option<String>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct DamageResistant {
        pub types: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ToolRule {
        pub blocks: Vec<String>,
        pub speed: Option<f32>,
        pub correct_drop_for_blocks: Option<bool>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Tool {
        pub rules: Vec<ToolRule>,
        pub default_mining_speed: f32,
        pub damage_per_block: VarInt,
        pub can_destroy_blocks_in_creative: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Weapon {
        pub damage_per_attack: VarInt,
        pub disable_blocking_for: f32,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Enchantable {
        pub value: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Equippable {
        pub slot: VarInt,
        pub equip_sound: String,
        pub model: Option<String>,
        pub camera_overlay: Option<String>,
        pub allowed_entities: Option<Vec<String>>,
        pub dispensable: bool,
        pub swappable: bool,
        pub damage_on_hurt: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Repairable {
        pub items: Vec<String>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Glider;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct TooltipStyle {
        pub style: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct DeathProtection {
        pub effects: Vec<ConsumeEffect>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BlocksAttacks {
        pub block_delay_seconds: f32,
        pub disable_cooldown_scale: f32,
        pub damage_reductions: Vec<f32>,
        pub horizontal_blocking_angle: Vec<f32>,
        pub r#type: Option<Vec<String>>,
        pub base: f32,
        pub factor: f32,
        pub item_damage_threshold: f32,
        pub item_damage_base: f32,
        pub item_damage_factor: f32,
        pub bypassed_by: Option<String>,
        pub block_sound: Option<String>,
        pub disable_sound: Option<String>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct StoredEnchantments {
        pub enchantments: Vec<Enchantment>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct DyedColor {
        pub color: i32,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MapColor {
        pub color: i32,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MapId {
        pub id: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MapDecorations {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MapPostProcessing {
        pub r#type: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ChargedProjectiles {
        pub projectiles: Vec<Slot>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BundleContents {
        pub items: Vec<Slot>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PotionEffect;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PotionContents {
        pub potion_id: Option<VarInt>,
        pub custom_color: Option<i32>,
        pub custom_effects: Vec<PotionEffect>,
        pub custom_name: Option<String>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PotionDurationScale {
        pub effect_multiplier: f32,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct SuspiciousStewEffects {
        pub effects: Vec<Effect>,
    }
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Effect {
        pub type_id: VarInt,
        pub duration: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct WritableBookContent {
        pub pages: Vec<String>,
        pub filtered_content: Option<Vec<String>>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct WrittenBookContent {
        pub raw_title: String,
        pub filtered_title: Option<String>,
        pub author: String,
        pub generation: VarInt,
        pub pages: Vec<TextComponent>,
        pub filtered_content: Option<Vec<TextComponent>>,
        pub resolved: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Trim {
        pub trim_material: String,
        pub trim_pattern: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct DebugStickState {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct EntityData {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BucketEntityData {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BlockEntityData {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Instrument {
        pub instrument: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ProvidesTrimMaterial {
        pub mode: u8,
        pub material: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct OminousBottleAmplifier {
        pub amplifier: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct JukeboxPlayable {
        pub mode: u8,
        pub jukebox_song: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ProvidesBannerPatterns {
        pub key: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Recipes {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct LodestoneTracker {
        pub has_global_position: bool,
        pub dimension: Option<String>,
        pub position: Option<Position>,
        pub tracked: bool,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct FireworkExplosion;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Fireworks {
        pub flight_duration: VarInt,
        pub explosions: Vec<FireworkExplosion>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ProfileProperty {
        pub name: String,
        pub value: String,
        pub has_signature: bool,
        pub signature: Option<String>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Profile {
        pub name: Option<String>,
        pub unique_id: Option<Uuid>,
        pub properties: Vec<ProfileProperty>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct NoteBlockSound {
        pub sound: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct DyeColor;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BannerPattern {
        pub pattern_type: String,
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BannerPatterns {
        pub layers: Vec<BannerPattern>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BaseColor {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PotDecorations {
        pub decorations: Vec<VarInt>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Container {
        pub items: Vec<Slot>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BlockState {
        pub properties: Vec<Property>,
    }
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Property {
        pub name: String,
        pub value: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Bee {
        pub entity_data: qexed_nbt::Tag,
        pub ticks_in_hive: VarInt,
        pub min_ticks_in_hive: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Bees {
        pub bees: Vec<Bee>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Lock {
        pub key: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ContainerLoot {
        pub data: qexed_nbt::Tag,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BreakSound {
        pub sound_event: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct VillagerVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct WolfVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct WolfSoundVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct WolfCollar {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct FoxVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct SalmonSize {
        pub r#type: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ParrotVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct TropicalFishPattern {
        pub pattern: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct TropicalFishBaseColor {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct TropicalFishPatternColor {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct MooshroomVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct RabbitVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PigVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CowVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ChickenVariant {
        pub mode: u8,
        pub variant: String,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct FrogVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct HorseVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct PaintingVariant;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct LlamaVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct AxolotlVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CatVariant {
        pub variant: VarInt,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CatCollar {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct SheepColor {
        pub color: DyeColor,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct ShulkerColor {
        pub color: DyeColor,
    }
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CraftingShapeless {
        pub ingredients: Vec<SlotDisplay>,
        pub result: SlotDisplay,
        pub crafting_station: SlotDisplay,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CraftingShaped {
        pub width: VarInt,
        pub height: VarInt,
        pub ingredients: Vec<SlotDisplay>,
        pub result: SlotDisplay,
        pub crafting_station: SlotDisplay,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Furnace {
        pub ingredient: SlotDisplay,
        pub fuel: SlotDisplay,
        pub result: SlotDisplay,
        pub crafting_station: SlotDisplay,
        pub cooking_time: VarInt,
        pub experience: f32,
    }
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Stonecutter {
        pub ingredient: SlotDisplay,
        pub result: SlotDisplay,
        pub crafting_station: SlotDisplay,
    }
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Smithing {
        pub template: SlotDisplay,
        pub base: SlotDisplay,
        pub addition: SlotDisplay,
        pub result: SlotDisplay,
        pub crafting_station: SlotDisplay,
    }

    pub mod particle {
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Dust {
            color: i32,
            scale: f32,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct DustColorTransition {
            form_color: i32,
            to_color: i32,
            scale: f32,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Effect {
            color: i32,
            power: f32,
        }
    }
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum SlotDisplay {
    Empty,
    AnyFuel,
    WithAnyPotion(slot_display_types::minecraft::WithAnyPotion),
    OnlyWithComponent(slot_display_types::minecraft::OnlyWithComponent),
    Item(slot_display_types::minecraft::Item),
    ItemStack(slot_display_types::minecraft::ItemStack),
    Tag(slot_display_types::minecraft::Tag),
    Dyed(slot_display_types::minecraft::Dyed),
    SmithingTrim(Box<slot_display_types::minecraft::SmithingTrim>),
    WithRemainder(Box<slot_display_types::minecraft::WithRemainder>),
    Composite(slot_display_types::minecraft::Composite),
    Unknown,
}

#[cfg(test)]
mod tests {
    use qexed_packet::{PacketCodec, PacketReader, PacketWriter, net_types::VarInt};

    use super::{ComponentsToAdd, IDSet, Slot, minecraft};

    #[test]
    fn slot_item_name_and_lore_use_data_component_registry_ids() {
        let slot = Slot {
            item_count: VarInt(1),
            item_id: Some(VarInt(1)),
            number_of_components_to_add: Some(VarInt(2)),
            number_of_components_to_remove: Some(VarInt(0)),
            components_to_add: Some(vec![
                ComponentsToAdd::MinecraftItemName(minecraft::ItemName {
                    name: text_component("Menu"),
                }),
                ComponentsToAdd::MinecraftLore(minecraft::Lore {
                    lines: vec![text_component("Open")],
                }),
            ]),
            components_to_remove: None,
        };

        let mut buf = bytes::BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        slot.serialize(&mut writer).unwrap();
        let mut bytes = buf.freeze();
        let mut reader = PacketReader::new(&mut bytes);

        let mut count = VarInt::default();
        count.deserialize(&mut reader).unwrap();
        let mut item_id = VarInt::default();
        item_id.deserialize(&mut reader).unwrap();
        let mut add_count = VarInt::default();
        add_count.deserialize(&mut reader).unwrap();
        let mut remove_count = VarInt::default();
        remove_count.deserialize(&mut reader).unwrap();
        let mut first_component = VarInt::default();
        first_component.deserialize(&mut reader).unwrap();
        let mut item_name = minecraft::ItemName::default();
        item_name.deserialize(&mut reader).unwrap();
        let mut second_component = VarInt::default();
        second_component.deserialize(&mut reader).unwrap();

        assert_eq!(count.0, 1);
        assert_eq!(item_id.0, 1);
        assert_eq!(add_count.0, 2);
        assert_eq!(remove_count.0, 0);
        assert_eq!(first_component.0, 9);
        assert_eq!(second_component.0, 11);
    }

    #[test]
    fn slot_can_break_round_trips_block_predicate() {
        let slot = Slot {
            item_count: VarInt(1),
            item_id: Some(VarInt(1)),
            number_of_components_to_add: Some(VarInt(1)),
            number_of_components_to_remove: Some(VarInt(0)),
            components_to_add: Some(vec![ComponentsToAdd::MinecraftCanBreak(
                minecraft::CanBreak {
                    block_predicates: vec![minecraft::BlockPredicate {
                        blocks: Some(IDSet {
                            r#type: VarInt(2),
                            tag_name: None,
                            ids: Some(vec![VarInt(1)]),
                        }),
                        properties: Some(minecraft::StatePropertiesPredicate {
                            properties: vec![minecraft::StatePropertyMatcher {
                                name: "axis".to_string(),
                                matcher: minecraft::StatePropertyMatcherValue::Exact(
                                    "y".to_string(),
                                ),
                            }],
                        }),
                        nbt: None,
                    }],
                    show_in_tooltip: true,
                },
            )]),
            components_to_remove: None,
        };

        let mut buf = bytes::BytesMut::new();
        let mut writer = PacketWriter::new(&mut buf);
        slot.serialize(&mut writer).unwrap();
        let mut bytes = buf.freeze();
        let mut reader = PacketReader::new(&mut bytes);
        let mut decoded = Slot::default();
        decoded.deserialize(&mut reader).unwrap();

        assert_eq!(decoded, slot);
    }

    fn text_component(text: &str) -> super::TextComponent {
        let mut map = std::collections::HashMap::new();
        map.insert(
            "text".to_string(),
            qexed_nbt::Tag::String(std::sync::Arc::from(text.to_string())),
        );
        qexed_nbt::Tag::Compound(std::sync::Arc::new(map))
    }
}
pub mod slot_display_types {

    pub mod minecraft {
        use qexed_packet::net_types::VarInt;

        use crate::types::{Slot, SlotDisplay};

        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Item {
            pub item_type: VarInt,
        }
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct ItemStack {
            pub item_stack: Slot,
        }
        impl qexed_packet::PacketCodec for ItemStack {
            fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
                let item_id =
                    self.item_stack.item_id.as_ref().ok_or_else(|| {
                        anyhow::anyhow!("item_id is required for item stack display")
                    })?;
                item_id.serialize(w)?;
                self.item_stack.item_count.serialize(w)?;

                let add_count = self
                    .item_stack
                    .number_of_components_to_add
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                let remove_count = self
                    .item_stack
                    .number_of_components_to_remove
                    .as_ref()
                    .cloned()
                    .unwrap_or_default();
                add_count.serialize(w)?;
                remove_count.serialize(w)?;

                if let Some(components_to_add) = &self.item_stack.components_to_add {
                    for component in components_to_add {
                        component.serialize(w)?;
                    }
                }
                if let Some(components_to_remove) = &self.item_stack.components_to_remove {
                    for component in components_to_remove {
                        component.serialize(w)?;
                    }
                }
                Ok(())
            }

            fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
                let mut item_id = VarInt::default();
                item_id.deserialize(r)?;
                let mut item_count = VarInt::default();
                item_count.deserialize(r)?;
                let mut add_count = VarInt::default();
                add_count.deserialize(r)?;
                let mut remove_count = VarInt::default();
                remove_count.deserialize(r)?;

                let mut components_to_add = Vec::with_capacity(add_count.0.max(0) as usize);
                for _ in 0..add_count.0 {
                    let mut component = crate::types::ComponentsToAdd::default();
                    component.deserialize(r)?;
                    components_to_add.push(component);
                }
                let mut components_to_remove = Vec::with_capacity(remove_count.0.max(0) as usize);
                for _ in 0..remove_count.0 {
                    let mut component = VarInt::default();
                    component.deserialize(r)?;
                    components_to_remove.push(component);
                }

                self.item_stack = Slot {
                    item_count,
                    item_id: Some(item_id),
                    number_of_components_to_add: Some(add_count),
                    number_of_components_to_remove: Some(remove_count),
                    components_to_add: (!components_to_add.is_empty()).then_some(components_to_add),
                    components_to_remove: (!components_to_remove.is_empty())
                        .then_some(components_to_remove),
                };
                Ok(())
            }
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Tag {
            pub tag: String,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct WithAnyPotion {
            pub display: Box<SlotDisplay>,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct OnlyWithComponent {
            pub source: Box<SlotDisplay>,
            pub component: VarInt,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Dyed {
            pub dye: Box<SlotDisplay>,
            pub target: Box<SlotDisplay>,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct SmithingTrim {
            pub base: SlotDisplay,
            pub material: SlotDisplay,
            pub pattern: VarInt,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct WithRemainder {
            pub ingredient: SlotDisplay,
            pub remainder: SlotDisplay,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]
        pub struct Composite {
            pub options: Vec<SlotDisplay>,
        }
    }
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityMetadata {
    pub data: Vec<EntityMetadataSub>,
}
impl PacketCodec for EntityMetadata {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        Ok(for i in &self.data {
            i.serialize(w)?;
        })
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.data.clear();
        loop {
            let mut data = EntityMetadataSub::default();
            data.deserialize(r)?;
            if data.index == 0xff {
                self.data.push(data);
                return Ok(());
            }
            self.data.push(data);
        }
    }
}
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityMetadataSub {
    pub index: u8,
    pub data: Option<EntityMetadataEnum>,
}
impl PacketCodec for EntityMetadataSub {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.index.serialize(w)?;
        if self.index != 0xff {
            if let Some(data) = &self.data {
                data.serialize(w)?;
            } else {
                return Err(anyhow::anyhow!("EntityMetadata Lose"));
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.index.deserialize(r)?;
        if self.index != 0xff {
            let mut data = EntityMetadataEnum::default();
            data.deserialize(r)?;
            self.data = Some(data);
        }
        Ok(())
    }
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum EntityMetadataEnum {
    Byte(u8),
    VarInt(VarInt),
    VarLong(VarLong),
    Float(f32),
    String(String),
    TextComponent(TextComponent),
    OptionTextComponent(Option<TextComponent>),
    Slot(Slot),
    Boolean(bool),
    Rotations(Rotations),
    Position(Position),
    OptionPosition(Option<Position>),
    Direction(VarInt),
    OptionLivingEntityReference(Option<Uuid>),
    BlockState(VarInt),
    OptionBlockState(VarInt),
    NBT(qexed_nbt::Tag),
    // Particle(Particle),
    // Particles(Vec<Particle>),
    // VillagerData(Villager_Data),
    // OptionVarInt(VarInt),
    // Pose(VarInt),
    // CatVariant(VarInt),
    // CowVariant(VarInt),
    // WolfVariant(VarInt),
    // WolfSoundVariant(VarInt),
    // FrogVariant(VarInt),
    // PigVariant(VarInt),
    // ChickenVariant(VarInt),
    // OptionGlobalPosition(OptionGlobalPosition),
    // PaintingVariant(PaintingVariant),
    // SnifferState(VarInt),
    // ArmadilloState(VarInt),
    // Vector3(Vector3),
    // Quaternion(Quaternion),
    Unknown,
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct Rotations {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}
// #[qexed_packet_macros::subenum]
// #[derive(Debug, PartialEq, Clone)]
// pub enum Particle{
//     AngryVillager,
//     Block(VarInt),
//     BlockMarker(VarInt),
//     Bubble,
//     Cloud,
//     CopperFireFlame,
//     Crit,
//     DamageIndicator,
//     DragonBreath(f32),
//     DrippingLava,
//     FallingLava,
//     LandingLava,
//     DrippingWater,
//     FallingWater,
//     Dust(minecraft::particle::Dust),
//     DustColorTransition(minecraft::particle::DustColorTransition),
//     Effect(minecraft::particle::Effect),
//     ElderGuardian,
//     EnchantedHit,
//     Enchant,
//     EndRod,
//     EntityEffect(VarInt),
//     ExplosionEmitter,
//     Explosion,
//     Gust,
//     SmallGust,
//     GustEmitterLarge,
//     GustEmitterSmall,
//     SonicBoom,
//     FallingDust(VarInt),
//     Firework,
//     Fishing,
//     Flame,
//     Infested,
//     CherryLeaves,
//     PaleOakLeaves,
//     TintedLeaves(i32),
//     SculkSoul,
//     SculkCharge(f32),
//     SculkChargePop,
//     SoulFireFlame,
//     Soul,
//     Flash(i32),
//     HappyVillager,
//     Composter,
//     Heart,
//     InstantEffect
//     Item
//     Vibration
//     Trail
//     ItemSlime
//     ItemCobweb
//     ItemSnowball
//     LargeSmoke
//     Lava
//     Mycelium
//     Note
//     Poof
//     Portal
//     Rain
//     Smoke
//     WhiteSmoke
//     Sneeze
//     Spit
//     SquidInk
//     SweepAttack
//     TotemOfUndying
//     Underwater
//     Splash
//     Witch
//     BubblePop
//     CurrentDown
//     BubbleColumnUp
//     Nautilus
//     Dolphin
//     CampfireCosySmoke
//     CampfireSignalSmoke
//     DrippingHoney
//     FallingHoney
//     LandingHoney
//     FallingNectar
//     FallingSporeBlossom
//     Ash
//     CrimsonSpore
//     WarpedSpore
//     SporeBlossomAir
//     DrippingObsidianTear
//     FallingObsidianTear
//     LandingObsidianTear
//     ReversePortal
//     WhiteAsh
//     SmallFlame
//     Snowflake
//     DrippingDripstoneLava
//     FallingDripstoneLava
//     DrippingDripstoneWater
//     FallingDripstoneWater
//     GlowSquidInk
//     Glow
//     WaxOn
//     WaxOff
//     ElectricSpark
//     Scrape
//     Shriek
//     EggCrack
//     DustPlume
//     TrialSpawnerDetection
//     TrialSpawnerDetectionOminous
//     VaultConnection
//     DustPillar
//     OminousSpawning
//     RaidOmen
//     TrialOmen
//     BlockCrumble
//     Firefly

// }
