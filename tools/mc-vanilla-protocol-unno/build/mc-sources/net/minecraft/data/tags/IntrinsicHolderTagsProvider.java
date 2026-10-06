package net.minecraft.data.tags;

import java.util.concurrent.CompletableFuture;
import java.util.function.Function;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.data.PackOutput;
import net.minecraft.resources.ResourceKey;
import net.minecraft.tags.TagBuilder;
import net.minecraft.tags.TagKey;

public abstract class IntrinsicHolderTagsProvider<T> extends TagsProvider<T> {
    private final Function<T, ResourceKey<T>> keyExtractor;

    @Deprecated
    public IntrinsicHolderTagsProvider(
        PackOutput output,
        ResourceKey<? extends Registry<T>> registryKey,
        CompletableFuture<HolderLookup.Provider> lookupProvider,
        Function<T, ResourceKey<T>> keyExtractor
    ) {
        this(output, registryKey, lookupProvider, keyExtractor, "vanilla");
    }

    @Deprecated
    public IntrinsicHolderTagsProvider(
        PackOutput output,
        ResourceKey<? extends Registry<T>> registryKey,
        CompletableFuture<HolderLookup.Provider> lookupProvider,
        CompletableFuture<TagsProvider.TagLookup<T>> parentProvider,
        Function<T, ResourceKey<T>> keyExtractor
    ) {
        this(output, registryKey, lookupProvider, parentProvider, keyExtractor, "vanilla");
    }

    public IntrinsicHolderTagsProvider(PackOutput output, ResourceKey<? extends Registry<T>> registryKey, CompletableFuture<HolderLookup.Provider> lookupProvider, Function<T, ResourceKey<T>> keyExtractor, String modId) {
        super(output, registryKey, lookupProvider, modId);
        this.keyExtractor = keyExtractor;
    }

    public IntrinsicHolderTagsProvider(PackOutput output, ResourceKey<? extends Registry<T>> registryKey, CompletableFuture<HolderLookup.Provider> lookupProvider, CompletableFuture<TagsProvider.TagLookup<T>> parentProvider, Function<T, ResourceKey<T>> keyExtractor, String modId) {
        super(output, registryKey, lookupProvider, parentProvider, modId);
        this.keyExtractor = keyExtractor;
    }

    protected TagAppender<T, T> tag(TagKey<T> tag) {
        TagBuilder builder = this.getOrCreateRawBuilder(tag);
        return TagAppender.<T>forBuilder(builder).map(this.keyExtractor);
    }
}
