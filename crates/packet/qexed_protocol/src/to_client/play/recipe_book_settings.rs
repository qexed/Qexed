use qexed_packet::PacketCodec;
#[qexed_packet_macros::packet(id = 0x45)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct RecipeBookSettings {
    /// If true, then the crafting recipe book will be open when the player opens its inventory.
    pub crafting_recipe_book_open:bool,
    /// If true, then the filtering option is active when the player opens its inventory.
    pub crafting_recipe_book_filter_active:bool,
    /// If true, then the smelting recipe book will be open when the player opens its inventory.
    pub smelting_recipe_book_open:bool,
    /// If true, then the filtering option is active when the player opens its inventory.
    pub smelting_recipe_book_filter_active:bool,
    /// If true, then the blast furnace recipe book will be open when the player opens its inventory.
    pub blast_furnace_recipe_book_open:bool,
    /// If true, then the filtering option is active when the player opens its inventory.
    pub blast_furnace_recipe_book_filter_active:bool,
    /// If true, then the smoker recipe book will be open when the player opens its inventory.
    pub smoker_recipe_book_open:bool,
    /// If true, then the filtering option is active when the player opens its inventory.
    pub smoker_recipe_book_filter_active:bool,

}
