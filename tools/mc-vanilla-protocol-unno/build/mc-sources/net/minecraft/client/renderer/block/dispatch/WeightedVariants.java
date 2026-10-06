package net.minecraft.client.renderer.block.dispatch;

import java.util.List;
import net.minecraft.client.resources.model.ModelBaker;
import net.minecraft.client.resources.model.ResolvableModel;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.util.RandomSource;
import net.minecraft.util.random.Weighted;
import net.minecraft.util.random.WeightedList;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class WeightedVariants implements BlockStateModel, net.neoforged.neoforge.client.model.DynamicBlockStateModel {
    private final WeightedList<BlockStateModel> list;
    private final Material.Baked particleMaterial;
    @BakedQuad.MaterialFlags
    private final int materialFlags;

    public WeightedVariants(WeightedList<BlockStateModel> list) {
        this.list = list;
        BlockStateModel firstModel = list.unwrap().getFirst().value();
        this.particleMaterial = firstModel.particleMaterial();
        this.materialFlags = computeMaterialFlags(list);
    }

    @BakedQuad.MaterialFlags
    private static int computeMaterialFlags(WeightedList<BlockStateModel> list) {
        int flags = 0;

        for (Weighted<BlockStateModel> entry : list.unwrap()) {
            flags |= entry.value().materialFlags();
        }

        return flags;
    }

    @Override
    public Material.Baked particleMaterial() {
        return this.particleMaterial;
    }

    @BakedQuad.MaterialFlags
    @Override
    public int materialFlags() {
        return this.materialFlags;
    }

    // Neo: Implement our overload so child models can have custom logic
    @Override
    public void collectParts(net.minecraft.client.renderer.block.BlockAndTintGetter level, net.minecraft.core.BlockPos pos, net.minecraft.world.level.block.state.BlockState state, RandomSource random, List<BlockStateModelPart> output) {
        this.list.getRandomOrThrow(random).collectParts(level, pos, state, random, output);
    }

    @Override
    @org.jspecify.annotations.Nullable
    public Object createGeometryKey(net.minecraft.client.renderer.block.BlockAndTintGetter level, net.minecraft.core.BlockPos pos, net.minecraft.world.level.block.state.BlockState state, RandomSource random) {
        return this.list.getRandomOrThrow(random).createGeometryKey(level, pos, state, random);
    }

    @Override
    public Material.Baked particleMaterial(net.minecraft.client.renderer.block.BlockAndTintGetter level, net.minecraft.core.BlockPos pos, net.minecraft.world.level.block.state.BlockState state) {
        return this.list.unwrap().getFirst().value().particleMaterial(level, pos, state);
    }

    @Override
    @BakedQuad.MaterialFlags
    public int materialFlags(net.minecraft.client.renderer.block.BlockAndTintGetter level, net.minecraft.core.BlockPos pos, net.minecraft.world.level.block.state.BlockState state) {
        int flags = 0;
        for (Weighted<BlockStateModel> model : this.list.unwrap()) {
            flags |= model.value().materialFlags(level, pos, state);
        }
        return flags;
    }

    @OnlyIn(Dist.CLIENT)
    public record Unbaked(WeightedList<BlockStateModel.Unbaked> entries) implements BlockStateModel.Unbaked {
        @Override
        public BlockStateModel bake(ModelBaker modelBakery) {
            return new WeightedVariants(this.entries.map(m -> m.bake(modelBakery)));
        }

        @Override
        public void resolveDependencies(ResolvableModel.Resolver resolver) {
            this.entries.unwrap().forEach(v -> v.value().resolveDependencies(resolver));
        }
    }
}
