package net.minecraft.data.tags;

import java.util.concurrent.CompletableFuture;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.registries.Registries;
import net.minecraft.data.PackOutput;
import net.minecraft.tags.FluidTags;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.Fluids;

public class FluidTagsProvider extends IntrinsicHolderTagsProvider<Fluid> {
    /** @deprecated Forge: Use the {@linkplain #FluidTagsProvider(PackOutput, CompletableFuture, String) mod id variant} */
    @Deprecated
    public FluidTagsProvider(PackOutput output, CompletableFuture<HolderLookup.Provider> lookupProvider) {
        this(output, lookupProvider, "vanilla");
    }
    public FluidTagsProvider(PackOutput output, CompletableFuture<HolderLookup.Provider> lookupProvider, String modId) {
        super(output, Registries.FLUID, lookupProvider, e -> e.builtInRegistryHolder().key(), modId);
    }

    @Override
    protected void addTags(HolderLookup.Provider registries) {
        this.tag(FluidTags.WATER).add(Fluids.WATER, Fluids.FLOWING_WATER);
        this.tag(FluidTags.LAVA).add(Fluids.LAVA, Fluids.FLOWING_LAVA);
        this.tag(FluidTags.SUPPORTS_SUGAR_CANE_ADJACENTLY).addTag(FluidTags.WATER);
        this.tag(FluidTags.SUPPORTS_LILY_PAD).add(Fluids.WATER);
        this.tag(FluidTags.SUPPORTS_FROGSPAWN).add(Fluids.WATER);
        this.tag(FluidTags.BUBBLE_COLUMN_CAN_OCCUPY).add(Fluids.WATER);
    }
}
