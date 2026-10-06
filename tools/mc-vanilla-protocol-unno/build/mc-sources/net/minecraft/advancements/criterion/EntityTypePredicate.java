package net.minecraft.advancements.criterion;

import com.mojang.serialization.Codec;
import net.minecraft.core.Holder;
import net.minecraft.core.HolderGetter;
import net.minecraft.core.HolderSet;
import net.minecraft.core.RegistryCodecs;
import net.minecraft.core.registries.Registries;
import net.minecraft.tags.TagKey;
import net.minecraft.world.entity.EntityType;

public record EntityTypePredicate(HolderSet<EntityType<?>> types) {
    public static final Codec<EntityTypePredicate> CODEC = RegistryCodecs.homogeneousList(Registries.ENTITY_TYPE)
        .xmap(EntityTypePredicate::new, EntityTypePredicate::types);

    public static EntityTypePredicate of(HolderGetter<EntityType<?>> lookup, EntityType<?> type) {
        return new EntityTypePredicate(HolderSet.direct(type.builtInRegistryHolder()));
    }

    public static EntityTypePredicate of(HolderGetter<EntityType<?>> lookup, TagKey<EntityType<?>> type) {
        return new EntityTypePredicate(lookup.getOrThrow(type));
    }

    public boolean matches(Holder<EntityType<?>> type) {
        return this.types.contains(type);
    }
}
