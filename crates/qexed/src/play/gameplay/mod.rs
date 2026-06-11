pub(super) mod advancements;
pub(super) mod combat;
pub(super) mod crafting;
pub(super) mod durability;
pub(super) mod effects;
pub(super) mod enchanting;
pub(super) mod furnace;
pub(super) mod items;
pub(super) mod oxygen;
pub(super) mod redstone;
pub(super) mod sounds;

use std::time::{Duration, Instant};

use qexed_config::app::qexed::server::Gameplay;

#[derive(Debug)]
pub(super) struct GameplayRuntime {
    pub(super) crafting: crafting::CraftingRuntime,
    pub(super) enchanting: enchanting::EnchantingRuntime,
    pub(super) furnace: furnace::FurnaceRuntime,
    pub(super) effects: effects::EffectRuntime,
    pub(super) oxygen: oxygen::OxygenRuntime,
    pub(super) advancements: advancements::AdvancementRuntime,
    pub(super) redstone: redstone::RedstoneRuntime,
    furnace_tick_due: Instant,
    redstone_tick_due: Instant,
    oxygen_tick_due: Instant,
    farmland_tick_due: Instant,
}

impl GameplayRuntime {
    pub(super) fn new(config: &Gameplay) -> Self {
        let now = Instant::now();
        let furnace_interval = Duration::from_millis(config.furnace_tick_ms.max(1));
        let redstone_interval = Duration::from_millis(config.redstone_tick_ms.max(1));
        let oxygen_interval = Duration::from_millis(config.oxygen_tick_ms.max(1));
        let farmland_interval = Duration::from_millis(config.farmland_tick_ms.max(1));
        Self {
            crafting: crafting::CraftingRuntime::new(),
            enchanting: enchanting::EnchantingRuntime::new(),
            furnace: furnace::FurnaceRuntime::new(),
            effects: effects::EffectRuntime::new(),
            oxygen: oxygen::OxygenRuntime::new(),
            advancements: advancements::AdvancementRuntime::new(config),
            redstone: redstone::RedstoneRuntime::new(),
            furnace_tick_due: now + furnace_interval,
            redstone_tick_due: now + redstone_interval,
            oxygen_tick_due: now + oxygen_interval,
            farmland_tick_due: now + farmland_interval,
        }
    }

    pub(super) fn should_tick_furnace(&mut self, config: &Gameplay) -> bool {
        tick_due(&mut self.furnace_tick_due, config.furnace_tick_ms)
    }

    pub(super) fn should_tick_oxygen(&mut self, config: &Gameplay) -> bool {
        tick_due(&mut self.oxygen_tick_due, config.oxygen_tick_ms)
    }

    pub(super) fn should_tick_redstone(&mut self, config: &Gameplay) -> bool {
        tick_due(&mut self.redstone_tick_due, config.redstone_tick_ms)
    }

    pub(super) fn should_tick_farmland(&mut self, config: &Gameplay) -> bool {
        tick_due(&mut self.farmland_tick_due, config.farmland_tick_ms)
    }
}

fn tick_due(deadline: &mut Instant, interval_ms: u64) -> bool {
    let now = Instant::now();
    if now < *deadline {
        return false;
    }
    let interval = Duration::from_millis(interval_ms.max(1));
    while *deadline <= now {
        *deadline += interval;
    }
    true
}

#[derive(Debug, Default)]
pub(super) struct GameplayActionOutcome {
    pub(super) handled: bool,
    pub(super) inventory_changes: Vec<crate::inventory::InventorySlotChange>,
    pub(super) close_window: bool,
    pub(super) grant_triggers: Vec<qexed_config::app::qexed::server::CustomAdvancementTrigger>,
}

impl GameplayActionOutcome {
    pub(super) fn handled() -> Self {
        Self {
            handled: true,
            ..Self::default()
        }
    }

    pub(super) fn merge(&mut self, mut other: Self) {
        self.handled |= other.handled;
        self.inventory_changes.append(&mut other.inventory_changes);
        self.close_window |= other.close_window;
        self.grant_triggers.append(&mut other.grant_triggers);
    }
}
