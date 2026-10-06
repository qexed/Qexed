package net.minecraft.client.renderer;

import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import it.unimi.dsi.fastutil.objects.Object2ObjectLinkedOpenHashMap;
import java.util.SequencedMap;
import net.minecraft.client.renderer.chunk.ChunkSectionLayer;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.resources.model.ModelBakery;
import net.minecraft.util.Util;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class RenderBuffers {
    private final SectionBufferBuilderPack fixedBufferPack = new SectionBufferBuilderPack();
    private final SectionBufferBuilderPool sectionBufferPool;
    private final MultiBufferSource.BufferSource bufferSource;
    private final MultiBufferSource.BufferSource crumblingBufferSource;
    private final OutlineBufferSource outlineBufferSource;

    public RenderBuffers(int maxSectionBuilders) {
        this.sectionBufferPool = SectionBufferBuilderPool.allocate(maxSectionBuilders);
        SequencedMap<RenderType, ByteBufferBuilder> fixedBuffers = Util.make(new Object2ObjectLinkedOpenHashMap<>(), map -> {
            map.put(Sheets.cutoutBlockItemSheet(), this.fixedBufferPack.buffer(ChunkSectionLayer.CUTOUT));
            map.put(Sheets.translucentBlockItemSheet(), this.fixedBufferPack.buffer(ChunkSectionLayer.TRANSLUCENT));
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, Sheets.cutoutItemSheet());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, Sheets.translucentItemSheet());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, RenderTypes.armorEntityGlint());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, RenderTypes.glint());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, RenderTypes.glintTranslucent());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, RenderTypes.entityGlint());
            put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, RenderTypes.waterMask());
        });
        net.neoforged.fml.ModLoader.postEvent(new net.neoforged.neoforge.client.event.RegisterRenderBuffersEvent(fixedBuffers));
        this.bufferSource = MultiBufferSource.immediateWithBuffers(fixedBuffers, new ByteBufferBuilder(786432));
        this.outlineBufferSource = new OutlineBufferSource();
        SequencedMap<RenderType, ByteBufferBuilder> crumblingBuffers = Util.make(
            new Object2ObjectLinkedOpenHashMap<>(),
            map -> ModelBakery.DESTROY_TYPES.forEach(type -> put((Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder>)map, type))
        );
        this.crumblingBufferSource = MultiBufferSource.immediateWithBuffers(crumblingBuffers, new ByteBufferBuilder(0));
    }

    private static void put(Object2ObjectLinkedOpenHashMap<RenderType, ByteBufferBuilder> map, RenderType type) {
        map.put(type, new ByteBufferBuilder(type.bufferSize()));
    }

    public SectionBufferBuilderPack fixedBufferPack() {
        return this.fixedBufferPack;
    }

    public SectionBufferBuilderPool sectionBufferPool() {
        return this.sectionBufferPool;
    }

    public MultiBufferSource.BufferSource bufferSource() {
        return this.bufferSource;
    }

    public MultiBufferSource.BufferSource crumblingBufferSource() {
        return this.crumblingBufferSource;
    }

    public OutlineBufferSource outlineBufferSource() {
        return this.outlineBufferSource;
    }
}
