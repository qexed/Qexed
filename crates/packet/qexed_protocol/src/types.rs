use anyhow::Ok;
use qexed_packet::{PacketCodec, net_types::{Position, VarInt, VarLong}};
use uuid::Uuid;
pub type TextComponent = qexed_nbt::Tag;
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
#[qexed_packet_macros::subenum]
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
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct ComponentsToAddHash {
    pub component_type: VarInt,
    pub component_data: i32,
}


#[derive(Debug, Default, PartialEq, Clone)]
pub struct IDSet{
    pub r#type:VarInt,
    pub tag_name:Option<String>,
    pub ids:Option<Vec<VarInt>>,
}
impl PacketCodec for IDSet {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.r#type.serialize(w)?;
        if self.r#type.0==0{
            if let Some(tag_name) = &self.tag_name{
                tag_name.serialize(w)?;
            } else {
                return Err(anyhow::anyhow!("未定义tag_name"))
            }
        } else {
            if let Some(ids) = &self.ids{
                for prop in ids {
                    prop.serialize(w)?;
                }
            } else {
                return Err(anyhow::anyhow!("未定义ids"))
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.r#type.deserialize(r)?;
        if self.r#type.0==0{
            let mut tag_name:String = Default::default();
            tag_name.deserialize(r)?;
            self.tag_name = Some(tag_name);
        } else {
            let mut ids:Vec<VarInt> = Default::default();
            for _ in 0..self.r#type.0-1 {
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
pub enum RecipeDisplay{
    MinecraftCraftingShapeless(minecraft::CraftingShapeless),
    MinecraftCraftingShaped(minecraft::CraftingShaped),
    MinecraftFurnace(minecraft::Furnace),
    MinecraftStonecutter(minecraft::Stonecutter),
    MinecraftSmithing(minecraft::Smithing),
    Unknown,
}
pub mod minecraft {
    use qexed_packet::net_types::{Position, VarInt};
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

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct BlockPredicate;

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CanPlaceOn {
        pub block_predicates: Vec<BlockPredicate>,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CanBreak {
        pub block_predicates: Vec<BlockPredicate>,
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
    pub struct CraftingShapeless{
        pub ingredients:Vec<SlotDisplay>,
        pub result:SlotDisplay,
        pub crafting_station:SlotDisplay,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct CraftingShaped{
        pub width:VarInt,
        pub height:VarInt,
        pub ingredients:Vec<SlotDisplay>,
        pub result:SlotDisplay,
        pub crafting_station:SlotDisplay,
    }

    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Furnace{
        pub ingredient:SlotDisplay,
        pub fuel:SlotDisplay,
        pub result:SlotDisplay,
        pub crafting_station:SlotDisplay,
        pub cooking_time:VarInt,
        pub experience:f32,
    }    
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Stonecutter{
        pub ingredient:SlotDisplay,
        pub result:SlotDisplay,
        pub crafting_station:SlotDisplay,
    } 
    #[qexed_packet_macros::substruct]
    #[derive(Debug, Default, PartialEq, Clone)]
    pub struct Smithing{
        pub template:SlotDisplay,
        pub base:SlotDisplay,
        pub addition:SlotDisplay,
        pub result:SlotDisplay,
        pub crafting_station:SlotDisplay,
    }

    pub mod particle{
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct Dust{
            color:i32,
            scale:f32,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct DustColorTransition{
            form_color:i32,
            to_color:i32,
            scale:f32,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct Effect{
            color:i32,
            power:f32,
        }
    }
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum SlotDisplay{
    Empty,
    AnyFuel,
    Item(slot_display_types::minecraft::Item),
    ItemStack(slot_display_types::minecraft::ItemStack),
    Tag(slot_display_types::minecraft::Tag),
    SmithingTrim(Box<slot_display_types::minecraft::SmithingTrim>),
    WithRemainder(Box<slot_display_types::minecraft::WithRemainder>),
    Composite(slot_display_types::minecraft::Composite),
    Unknown,
}
pub mod slot_display_types{

    pub mod minecraft{
        use qexed_packet::net_types::VarInt;

        use crate::types::{Slot, SlotDisplay};

        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct Item{
            pub item_type:VarInt,
        }
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct ItemStack{
            pub item_stack:Slot,
        }    
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct Tag{
            pub tag:String,
        }    
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct SmithingTrim{
            pub base:SlotDisplay,
            pub material:SlotDisplay,
            pub pattern:VarInt,
        }    
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct WithRemainder{
            pub ingredient:SlotDisplay,
            pub remainder:SlotDisplay,
        }    
        #[qexed_packet_macros::substruct]
        #[derive(Debug, Default, PartialEq, Clone)]        
        pub struct Composite{
            pub options:Vec<SlotDisplay>,
        }    
    }
    
}

#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityMetadata{pub data:Vec<EntityMetadataSub>}
impl PacketCodec for EntityMetadata {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        Ok(for i in &self.data{
            i.serialize(w)?;
        })
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.data.clear();
        loop {
            let mut data = EntityMetadataSub::default();
            data.deserialize(r)?;
            if data.index==0xff{
                self.data.push(data);
                return Ok(());
            }
            self.data.push(data);

        }
    }
}
#[derive(Debug, Default, PartialEq, Clone)]
pub struct EntityMetadataSub {
    pub index:u8,
    pub data:Option<EntityMetadataEnum>,
}
impl PacketCodec for EntityMetadataSub {
    fn serialize(&self, w: &mut qexed_packet::PacketWriter) -> anyhow::Result<()> {
        self.index.serialize(w)?;
        if self.index!=0xff{
            if let Some(data) = &self.data{
                data.serialize(w)?;
            } else {
                return Err(anyhow::anyhow!("EntityMetadata Lose"));
            }
        }
        Ok(())
    }

    fn deserialize(&mut self, r: &mut qexed_packet::PacketReader) -> anyhow::Result<()> {
        self.index.deserialize(r)?;
        if self.index!=0xff{
            let mut data =EntityMetadataEnum::default();
            data.deserialize(r)?;
            self.data = Some(data);
        }
        Ok(())
    }
}
#[qexed_packet_macros::subenum]
#[derive(Debug, PartialEq, Clone)]
pub enum EntityMetadataEnum{
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
    Unknown
}
#[qexed_packet_macros::substruct]
#[derive(Debug, Default, PartialEq,Clone)]
pub struct Rotations{
    pub x:f32,
    pub y:f32,
    pub z:f32,
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