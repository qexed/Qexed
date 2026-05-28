use qexed_config::{
    app::qexed::{Qexed, server::World},
    tool::AppConfigTrait,
};

#[derive(Debug)]
pub struct RuntimeConfig {
    pub qexed: Qexed,
    pub world: World,
}

impl RuntimeConfig {
    pub fn load(language: Option<String>) -> anyhow::Result<Self> {
        let mut qexed = Qexed::load_or_create_default(language.clone(), None, None)?;
        let world = World::load_or_create_default(language, None, None)?;
        qexed.server.world = world.clone();
        Ok(Self { qexed, world })
    }
}

impl From<Qexed> for RuntimeConfig {
    fn from(mut qexed: Qexed) -> Self {
        let world = qexed.server.world.clone();
        qexed.server.world = world.clone();
        Self { qexed, world }
    }
}

impl std::ops::Deref for RuntimeConfig {
    type Target = Qexed;

    fn deref(&self) -> &Self::Target {
        &self.qexed
    }
}
