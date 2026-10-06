package net.minecraft.client.renderer;

import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.VertexConsumer;
import java.util.Optional;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class OutlineBufferSource implements MultiBufferSource {
    private final MultiBufferSource.BufferSource outlineBufferSource = MultiBufferSource.immediate(new ByteBufferBuilder(1536));
    private int outlineColor = -1;

    @Override
    public VertexConsumer getBuffer(RenderType renderType) {
        if (renderType.isOutline()) {
            VertexConsumer delegate = this.outlineBufferSource.getBuffer(renderType);
            return new OutlineBufferSource.EntityOutlineGenerator(delegate, this.outlineColor);
        } else {
            Optional<RenderType> outline = renderType.outline();
            if (outline.isPresent()) {
                VertexConsumer delegate = this.outlineBufferSource.getBuffer(outline.get());
                return new OutlineBufferSource.EntityOutlineGenerator(delegate, this.outlineColor);
            } else {
                throw new IllegalStateException("Can't render an outline for this rendertype!");
            }
        }
    }

    public void setColor(int color) {
        this.outlineColor = color;
    }

    public void endOutlineBatch() {
        this.outlineBufferSource.endBatch();
    }

    @OnlyIn(Dist.CLIENT)
    private record EntityOutlineGenerator(VertexConsumer delegate, int color) implements VertexConsumer {
        @Override
        public VertexConsumer addVertex(float x, float y, float z) {
            this.delegate.addVertex(x, y, z).setColor(this.color);
            return this;
        }

        @Override
        public VertexConsumer setColor(int r, int g, int b, int a) {
            return this;
        }

        @Override
        public VertexConsumer setColor(int color) {
            return this;
        }

        @Override
        public VertexConsumer setUv(float u, float v) {
            this.delegate.setUv(u, v);
            return this;
        }

        @Override
        public VertexConsumer setUv1(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer setUv2(int u, int v) {
            return this;
        }

        @Override
        public VertexConsumer setNormal(float x, float y, float z) {
            return this;
        }

        @Override
        public VertexConsumer setLineWidth(float width) {
            return this;
        }
    }
}
