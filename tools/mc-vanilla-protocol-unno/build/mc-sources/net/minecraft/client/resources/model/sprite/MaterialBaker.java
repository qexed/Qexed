package net.minecraft.client.resources.model.sprite;

import net.minecraft.client.resources.model.ModelDebugName;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public interface MaterialBaker {
    Material.Baked get(Material material, ModelDebugName name);

    Material.Baked reportMissingReference(String reference, ModelDebugName name);

    default Material.Baked resolveSlot(TextureSlots slots, String id, ModelDebugName name) {
        Material resolvedMaterial = slots.getMaterial(id);
        return resolvedMaterial != null ? this.get(resolvedMaterial, name) : this.reportMissingReference(id, name);
    }
}
