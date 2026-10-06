package net.minecraft.client.resources.model;

import com.google.common.collect.HashMultimap;
import com.google.common.collect.Multimap;
import com.google.common.collect.Multimaps;
import com.google.common.collect.Sets;
import com.mojang.datafixers.util.Pair;
import com.mojang.logging.LogUtils;
import it.unimi.dsi.fastutil.objects.Object2IntMap;
import it.unimi.dsi.fastutil.objects.Object2IntMaps;
import java.io.Reader;
import java.util.ArrayList;
import java.util.IdentityHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.Map.Entry;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executor;
import java.util.function.Function;
import java.util.function.Supplier;
import java.util.stream.Collectors;
import net.minecraft.client.color.block.BlockColors;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.renderer.PlayerSkinRenderCache;
import net.minecraft.client.renderer.block.BlockModelSet;
import net.minecraft.client.renderer.block.BlockStateModelSet;
import net.minecraft.client.renderer.block.BuiltInBlockModels;
import net.minecraft.client.renderer.block.FluidModel;
import net.minecraft.client.renderer.block.FluidStateModelSet;
import net.minecraft.client.renderer.block.LoadedBlockModels;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.model.BlockModel;
import net.minecraft.client.renderer.item.ClientItem;
import net.minecraft.client.renderer.item.ItemModel;
import net.minecraft.client.renderer.texture.SpriteLoader;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.cuboid.CuboidModel;
import net.minecraft.client.resources.model.cuboid.ItemModelGenerator;
import net.minecraft.client.resources.model.cuboid.MissingCuboidModel;
import net.minecraft.client.resources.model.sprite.AtlasManager;
import net.minecraft.client.resources.model.sprite.Material;
import net.minecraft.client.resources.model.sprite.MaterialBaker;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.data.AtlasIds;
import net.minecraft.resources.FileToIdConverter;
import net.minecraft.resources.Identifier;
import net.minecraft.server.packs.resources.PreparableReloadListener;
import net.minecraft.server.packs.resources.Resource;
import net.minecraft.server.packs.resources.ResourceManager;
import net.minecraft.util.Util;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.Zone;
import net.minecraft.world.level.block.Block;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.material.Fluid;
import net.minecraft.world.level.material.FluidState;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
public class ModelManager implements PreparableReloadListener {
    private static final Logger LOGGER = LogUtils.getLogger();
    private static final FileToIdConverter MODEL_LISTER = FileToIdConverter.json("models");
    private Map<Identifier, ItemModel> bakedItemStackModels = Map.of();
    private Map<Identifier, ClientItem.Properties> itemProperties = Map.of();
    private final AtlasManager atlasManager;
    private final PlayerSkinRenderCache playerSkinRenderCache;
    private final BlockColors blockColors;
    private EntityModelSet entityModelSet = EntityModelSet.EMPTY;
    private ModelBakery.MissingModels missingModels;
    private @Nullable BlockStateModelSet blockStateModelSet;
    private @Nullable BlockModelSet blockModelSet;
    private @Nullable FluidStateModelSet fluidStateModelSet;
    private Object2IntMap<BlockState> modelGroups = Object2IntMaps.emptyMap();
    private final java.util.concurrent.atomic.AtomicReference<ModelBakery> modelBakery = new java.util.concurrent.atomic.AtomicReference<>(null);
    private net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.BakedModels bakedStandaloneModels;
    private Set<Identifier> reportedMissingItemModels = new java.util.HashSet<>();

    public ModelManager(BlockColors blockColors, AtlasManager atlasManager, PlayerSkinRenderCache playerSkinRenderCache) {
        this.blockColors = blockColors;
        this.atlasManager = atlasManager;
        this.playerSkinRenderCache = playerSkinRenderCache;
    }

    public ItemModel getItemModel(Identifier id) {
        ItemModel model = this.bakedItemStackModels.get(id);
        if (model == null) {
            if (this.reportedMissingItemModels.add(id)) {
                LOGGER.warn("Missing item model for location {}", id);
            }
            return this.missingModels.item();
        }
        return model;
    }

    public ClientItem.Properties getItemProperties(Identifier id) {
        return this.itemProperties.getOrDefault(id, ClientItem.Properties.DEFAULT);
    }

    public BlockStateModelSet getBlockStateModelSet() {
        return Objects.requireNonNull(this.blockStateModelSet, "Block models not yet initialized");
    }

    public BlockModelSet getBlockModelSet() {
        return Objects.requireNonNull(this.blockModelSet, "Block models not yet initialized");
    }

    public FluidStateModelSet getFluidStateModelSet() {
        return Objects.requireNonNull(this.fluidStateModelSet, "Fluid models not yet initialized");
    }

    @Override
    public final CompletableFuture<Void> reload(
        PreparableReloadListener.SharedState currentReload,
        Executor taskExecutor,
        PreparableReloadListener.PreparationBarrier preparationBarrier,
        Executor reloadExecutor
    ) {
        ResourceManager manager = currentReload.resourceManager();
        CompletableFuture<EntityModelSet> entityModelSet = CompletableFuture.supplyAsync(EntityModelSet::vanilla, taskExecutor);
        CompletableFuture<Map<Identifier, UnbakedModel>> modelCache = loadBlockModels(manager, taskExecutor);
        CompletableFuture<BlockStateModelLoader.LoadedModels> blockStateModels = BlockStateModelLoader.loadBlockStates(manager, taskExecutor);
        CompletableFuture<Map<BlockState, BlockModel.Unbaked>> blockModelContents = CompletableFuture.supplyAsync(
            () -> BuiltInBlockModels.createBlockModels(this.blockColors), taskExecutor
        );
        CompletableFuture<ClientItemInfoLoader.LoadedClientInfos> itemStackModels = ClientItemInfoLoader.scheduleLoad(manager, taskExecutor);
        CompletableFuture<net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.LoadedModels> standaloneModelsFuture =
                net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.load(taskExecutor);
        CompletableFuture<ModelManager.ResolvedModels> modelDiscovery = CompletableFuture.allOf(modelCache, blockStateModels, itemStackModels, standaloneModelsFuture)
            .thenApplyAsync(var3 -> discoverModelDependencies(modelCache.join(), blockStateModels.join(), itemStackModels.join(), standaloneModelsFuture.join()), taskExecutor);
        CompletableFuture<Object2IntMap<BlockState>> modelGroups = blockStateModels.thenApplyAsync(
            models -> buildModelGroups(this.blockColors, models), taskExecutor
        );
        AtlasManager.PendingStitchResults pendingStitches = currentReload.get(AtlasManager.PENDING_STITCH);
        CompletableFuture<SpriteLoader.Preparations> pendingBlockAtlasSprites = pendingStitches.get(AtlasIds.BLOCKS);
        CompletableFuture<SpriteLoader.Preparations> pendingItemAtlasSprites = pendingStitches.get(AtlasIds.ITEMS);
        CompletableFuture<LoadedBlockModels> blockModels = CompletableFuture.allOf(blockModelContents, entityModelSet)
            .thenApplyAsync(var3 -> new LoadedBlockModels(blockModelContents.join(), entityModelSet.join(), this.atlasManager, this.playerSkinRenderCache));
        var pendingAnimations = currentReload.get(net.neoforged.neoforge.client.entity.animation.json.AnimationLoader.STATE_KEY);
        return CompletableFuture.allOf(
                pendingBlockAtlasSprites,
                pendingItemAtlasSprites,
                modelDiscovery,
                modelGroups,
                blockStateModels,
                itemStackModels,
                entityModelSet,
                blockModels,
                modelCache,
                standaloneModelsFuture
            )
            .thenComposeAsync(
                var11x -> {
                    SpriteLoader.Preparations blockAtlasSprites = pendingBlockAtlasSprites.join();
                    SpriteLoader.Preparations itemAtlasSprites = pendingItemAtlasSprites.join();
                    ModelManager.ResolvedModels resolvedModels = modelDiscovery.join();
                    Object2IntMap<BlockState> groups = modelGroups.join();
                    Set<Identifier> unreferencedModels = Sets.difference(modelCache.join().keySet(), resolvedModels.models.keySet());
                    if (!unreferencedModels.isEmpty()) {
                        LOGGER.debug(
                            "Unreferenced models: \n{}",
                            unreferencedModels.stream().sorted().map(modelId -> "\t" + modelId + "\n").collect(Collectors.joining())
                        );
                    }

                    ModelBakery bakery = new ModelBakery(
                        entityModelSet.join(),
                        this.atlasManager,
                        this.playerSkinRenderCache,
                        blockStateModels.join().models(),
                        itemStackModels.join().contents(),
                        resolvedModels.models(),
                        resolvedModels.missing()
                        , standaloneModelsFuture.join(),
                        pendingAnimations
                    );
                    this.modelBakery.set(bakery);
                    return loadModels(blockAtlasSprites, itemAtlasSprites, bakery, blockModels.join(), groups, entityModelSet.join(), taskExecutor, pendingAnimations);
                },
                taskExecutor
            )
            .thenCompose(preparationBarrier::wait)
            .thenAcceptAsync(this::apply, reloadExecutor);
    }

    private static CompletableFuture<Map<Identifier, UnbakedModel>> loadBlockModels(ResourceManager manager, Executor executor) {
        return CompletableFuture.<Map<Identifier, Resource>>supplyAsync(() -> MODEL_LISTER.listMatchingResources(manager), executor)
            .thenCompose(
                resources -> {
                    List<CompletableFuture<Pair<Identifier, UnbakedModel>>> result = new ArrayList<>(resources.size());

                    for (Entry<Identifier, Resource> resource : resources.entrySet()) {
                        result.add(CompletableFuture.supplyAsync(() -> {
                            Identifier modelId = MODEL_LISTER.fileToId(resource.getKey());

                            try {
                                Pair t$;
                                try (Reader reader = resource.getValue().openAsReader()) {
                                    t$ = Pair.of(modelId, net.neoforged.neoforge.client.model.UnbakedModelParser.parse(reader));
                                }

                                return t$;
                            } catch (Exception var7) {
                                LOGGER.error("Failed to load model {}", resource.getKey(), var7);
                                return null;
                            }
                        }, executor));
                    }

                    return Util.sequence(result)
                        .thenApply(pairs -> pairs.stream().filter(Objects::nonNull).collect(Collectors.toUnmodifiableMap(Pair::getFirst, Pair::getSecond)));
                }
            );
    }

    /**
     * @deprecated Neo: use {@link #discoverModelDependencies(Map, BlockStateModelLoader.LoadedModels, ClientItemInfoLoader.LoadedClientInfos, net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.LoadedModels)} instead
     */
    @Deprecated
    private static ModelManager.ResolvedModels discoverModelDependencies(
        Map<Identifier, UnbakedModel> allModels, BlockStateModelLoader.LoadedModels blockStateModels, ClientItemInfoLoader.LoadedClientInfos itemInfos
    ) {
        return discoverModelDependencies(allModels, blockStateModels, itemInfos, net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.LoadedModels.EMPTY);
    }

    private static ModelManager.ResolvedModels discoverModelDependencies(
            Map<Identifier, UnbakedModel> allModels, BlockStateModelLoader.LoadedModels blockStateModels, ClientItemInfoLoader.LoadedClientInfos itemInfos, net.neoforged.neoforge.client.model.standalone.StandaloneModelLoader.LoadedModels standaloneModels
    ) {
        ModelManager.ResolvedModels var5;
        try (Zone ignored = Profiler.get().zone("dependencies")) {
            ModelDiscovery result = new ModelDiscovery(allModels, MissingCuboidModel.missingModel());
            result.addSpecialModel(ItemModelGenerator.GENERATED_ITEM_MODEL_ID, new ItemModelGenerator());
            blockStateModels.models().values().forEach(result::addRoot);
            itemInfos.contents().values().forEach(info -> result.addRoot(info.model()));
            standaloneModels.models().values().forEach(result::addRoot);
            var5 = new ModelManager.ResolvedModels(result.missingModel(), result.resolve());
        }

        return var5;
    }

    /// @deprecated Neo: use [#loadModels(SpriteLoader.Preparations, SpriteLoader.Preparations, ModelBakery, LoadedBlockModels, Object2IntMap, EntityModelSet, Executor, net.neoforged.neoforge.client.entity.animation.json.AnimationLoader.PendingAnimations)] instead
    @Deprecated
    private static CompletableFuture<ModelManager.ReloadState> loadModels(
        SpriteLoader.Preparations blockAtlas,
        SpriteLoader.Preparations itemAtlas,
        ModelBakery bakery,
        LoadedBlockModels blockModels,
        Object2IntMap<BlockState> modelGroups,
        EntityModelSet entityModelSet,
        Executor taskExecutor
    ) {
        return loadModels(blockAtlas, itemAtlas, bakery, blockModels, modelGroups, entityModelSet, taskExecutor, net.neoforged.neoforge.client.entity.animation.json.AnimationLoader.PendingAnimations.EMPTY);
    }

    private static CompletableFuture<ModelManager.ReloadState> loadModels(
        SpriteLoader.Preparations blockAtlas,
        SpriteLoader.Preparations itemAtlas,
        ModelBakery bakery,
        LoadedBlockModels blockModels,
        Object2IntMap<BlockState> modelGroups,
        EntityModelSet entityModelSet,
        Executor taskExecutor
        , net.neoforged.neoforge.client.entity.animation.json.AnimationLoader.PendingAnimations pendingAnimations
    ) {
        final Multimap<String, Identifier> missingSprites = Multimaps.synchronizedMultimap(HashMultimap.create());
        final Multimap<String, String> missingReferences = Multimaps.synchronizedMultimap(HashMultimap.create());
        MaterialBaker materialBaker = new MaterialBaker() {
            private final Material.Baked blockMissing = new Material.Baked(blockAtlas.missing(), false);
            private final Map<Material, Material.@Nullable Baked> bakedMaterials = new ConcurrentHashMap<>();
            private final Function<Material, Material.@Nullable Baked> bakerFunction = this::bake;

            @Override
            public Material.Baked get(Material material, ModelDebugName name) {
                Material.Baked baked = this.bakedMaterials.computeIfAbsent(material, this.bakerFunction);
                if (baked == null) {
                    missingSprites.put(name.debugName(), material.sprite());
                    return this.blockMissing;
                } else {
                    return baked;
                }
            }

            private Material.@Nullable Baked bake(Material material) {
                Material.Baked itemMaterial = this.bakeForAtlas(material, itemAtlas);
                return itemMaterial != null ? itemMaterial : this.bakeForAtlas(material, blockAtlas);
            }

            private Material.@Nullable Baked bakeForAtlas(Material material, SpriteLoader.Preparations atlas) {
                TextureAtlasSprite sprite = atlas.getSprite(material.sprite());
                return sprite != null ? new Material.Baked(sprite, material.forceTranslucent()) : null;
            }

            @Override
            public Material.Baked reportMissingReference(String reference, ModelDebugName responsibleModel) {
                missingReferences.put(responsibleModel.debugName(), reference);
                return this.blockMissing;
            }
        };
        CompletableFuture<ModelBakery.BakingResult> bakedStateResults = bakery.bakeModels(materialBaker, taskExecutor);
        CompletableFuture<Map<BlockState, BlockModel>> bakedModelsFuture = bakedStateResults.thenCompose(
            bakingResult -> blockModels.bake(bakingResult::getBlockStateModel, bakingResult.missingModels().block(), taskExecutor, pendingAnimations)
        );
        return bakedStateResults.thenCombine(
            bakedModelsFuture,
            (bakingResult, bakedModels) -> {
                Map<Fluid, FluidModel> fluidModels = FluidStateModelSet.bake(materialBaker);
                missingSprites.asMap()
                    .forEach(
                        (location, sprites) -> LOGGER.warn(
                            "Missing textures in model {}:\n{}",
                            location,
                            sprites.stream().sorted().map(sprite -> "    " + sprite).collect(Collectors.joining("\n"))
                        )
                    );
                missingReferences.asMap()
                    .forEach(
                        (location, references) -> LOGGER.warn(
                            "Missing texture references in model {}:\n{}",
                            location,
                            references.stream().sorted().map(reference -> "    " + reference).collect(Collectors.joining("\n"))
                        )
                    );
                try (Zone ignored = Profiler.get().zone("neoforge_modify_baking_result")) {
                    net.neoforged.neoforge.client.ClientHooks.onModifyBakingResult(bakingResult, blockAtlas, bakery);
                }
                Map<BlockState, BlockStateModel> modelByStateCache = createBlockStateToModelDispatch(
                    bakingResult.blockStateModels(), bakingResult.missingModels().block()
                );
                return new ModelManager.ReloadState(
                    bakingResult, modelGroups, modelByStateCache, (Map<BlockState, BlockModel>)bakedModels, fluidModels, entityModelSet
                );
            }
        );
    }

    private static Map<BlockState, BlockStateModel> createBlockStateToModelDispatch(Map<BlockState, BlockStateModel> bakedModels, BlockStateModel missingModel) {
        Object var8;
        try (Zone ignored = Profiler.get().zone("block state dispatch")) {
            Map<BlockState, BlockStateModel> modelByStateCache = new IdentityHashMap<>(bakedModels);

            for (Block block : BuiltInRegistries.BLOCK) {
                block.getStateDefinition().getPossibleStates().forEach(state -> {
                    if (bakedModels.putIfAbsent(state, missingModel) == null) {
                        LOGGER.warn("Missing model for variant: '{}'", state);
                    }
                });
            }

            var8 = modelByStateCache;
        }

        return (Map<BlockState, BlockStateModel>)var8;
    }

    private static Object2IntMap<BlockState> buildModelGroups(BlockColors blockColors, BlockStateModelLoader.LoadedModels blockStateModels) {
        Object2IntMap var3;
        try (Zone ignored = Profiler.get().zone("block groups")) {
            var3 = ModelGroupCollector.build(blockColors, blockStateModels);
        }

        return var3;
    }

    private void apply(ModelManager.ReloadState preparations) {
        ModelBakery.BakingResult bakedModels = preparations.bakedModels;
        this.bakedItemStackModels = bakedModels.itemStackModels();
        this.itemProperties = bakedModels.itemProperties();
        this.modelGroups = preparations.modelGroups;
        this.missingModels = bakedModels.missingModels();
        this.blockStateModelSet = new BlockStateModelSet(preparations.blockStateModels, this.missingModels.block());
        this.blockModelSet = new BlockModelSet(this.blockStateModelSet, preparations.blockModels, this.blockColors);
        this.fluidStateModelSet = new FluidStateModelSet(preparations.fluidModels, this.missingModels.fluid());
        this.entityModelSet = preparations.entityModelSet;
        this.bakedStandaloneModels = bakedModels.standaloneModels();
        net.neoforged.neoforge.client.ClientHooks.onModelBake(this, bakedModels, this.modelBakery.get());
        this.reportedMissingItemModels = new java.util.HashSet<>();
    }

    public boolean requiresRender(BlockState oldState, BlockState newState) {
        if (oldState == newState) {
            return false;
        } else {
            int oldModelGroup = this.modelGroups.getInt(oldState);
            if (oldModelGroup != -1) {
                int newModelGroup = this.modelGroups.getInt(newState);
                if (oldModelGroup == newModelGroup) {
                    FluidState oldFluidState = oldState.getFluidState();
                    FluidState newFluidState = newState.getFluidState();
                    return oldFluidState != newFluidState;
                }
            }

            return true;
        }
    }

    public Supplier<EntityModelSet> entityModels() {
        return () -> this.entityModelSet;
    }

    public ModelBakery getModelBakery() {
        return this.modelBakery.get();
    }

    @org.jspecify.annotations.Nullable
    public <T> T getStandaloneModel(net.neoforged.neoforge.client.model.standalone.StandaloneModelKey<T> modelKey) {
        return this.bakedStandaloneModels.get(modelKey);
    }

    @OnlyIn(Dist.CLIENT)
    private record ReloadState(
        ModelBakery.BakingResult bakedModels,
        Object2IntMap<BlockState> modelGroups,
        Map<BlockState, BlockStateModel> blockStateModels,
        Map<BlockState, BlockModel> blockModels,
        Map<Fluid, FluidModel> fluidModels,
        EntityModelSet entityModelSet
    ) {
    }

    @OnlyIn(Dist.CLIENT)
    private record ResolvedModels(ResolvedModel missing, Map<Identifier, ResolvedModel> models) {
    }
}
