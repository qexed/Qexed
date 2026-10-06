package net.minecraft.client.resources.model.geometry;

import net.minecraft.client.renderer.block.dispatch.ModelState;
import net.minecraft.client.resources.model.ModelBaker;
import net.minecraft.client.resources.model.ModelDebugName;
import net.minecraft.client.resources.model.sprite.TextureSlots;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@FunctionalInterface
@OnlyIn(Dist.CLIENT)
public interface UnbakedGeometry extends net.neoforged.neoforge.client.extensions.UnbakedGeometryExtension {
    UnbakedGeometry EMPTY = (var0, var1, var2, var3) -> QuadCollection.EMPTY;

    /**
     * @deprecated Neo: Use {@link #bake(TextureSlots, ModelBaker, ModelState, ModelDebugName, net.minecraft.util.context.ContextMap)}.
     */
    @Deprecated
    QuadCollection bake(TextureSlots textureSlots, ModelBaker modelBaker, ModelState modelState, ModelDebugName name);
}
