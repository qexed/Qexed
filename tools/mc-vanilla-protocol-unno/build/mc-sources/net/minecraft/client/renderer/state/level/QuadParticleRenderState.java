package net.minecraft.client.renderer.state.level;

import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.vertex.BufferBuilder;
import com.mojang.blaze3d.vertex.ByteBufferBuilder;
import com.mojang.blaze3d.vertex.DefaultVertexFormat;
import com.mojang.blaze3d.vertex.MeshData;
import com.mojang.blaze3d.vertex.VertexConsumer;
import com.mojang.blaze3d.vertex.VertexFormat;
import java.util.Arrays;
import java.util.HashMap;
import java.util.Map;
import java.util.Map.Entry;
import net.minecraft.client.particle.SingleQuadParticle;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.feature.ParticleFeatureRenderer;
import net.minecraft.client.renderer.texture.AbstractTexture;
import net.minecraft.client.renderer.texture.TextureManager;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.joml.Matrix4f;
import org.joml.Quaternionf;
import org.joml.Vector3f;
import org.joml.Vector4f;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class QuadParticleRenderState implements ParticleGroupRenderState, SubmitNodeCollector.ParticleGroupRenderer {
    private static final int INITIAL_PARTICLE_CAPACITY = 1024;
    private static final int FLOATS_PER_PARTICLE = 12;
    private static final int INTS_PER_PARTICLE = 2;
    private final Map<SingleQuadParticle.Layer, QuadParticleRenderState.Storage> particles = new HashMap<>();
    private int particleCount;

    public void add(
        SingleQuadParticle.Layer layer,
        float x,
        float y,
        float z,
        float xRot,
        float yRot,
        float zRot,
        float wRot,
        float scale,
        float u0,
        float u1,
        float v0,
        float v1,
        int color,
        int lightCoords
    ) {
        this.particles
            .computeIfAbsent(layer, ignored -> new QuadParticleRenderState.Storage())
            .add(x, y, z, xRot, yRot, zRot, wRot, scale, u0, u1, v0, v1, color, lightCoords);
        this.particleCount++;
    }

    @Override
    public void clear() {
        this.particles.values().forEach(QuadParticleRenderState.Storage::clear);
        this.particleCount = 0;
    }

    @Override
    public boolean isEmpty() {
        return this.particleCount == 0;
    }

    @Override
    public QuadParticleRenderState.@Nullable PreparedBuffers prepare(ParticleFeatureRenderer.ParticleBufferCache cachedBuffer, boolean translucent) {
        if (this.isEmpty()) {
            return null;
        } else {
            int vertexCount = this.particleCount * 4;

            Object var14;
            try (ByteBufferBuilder builder = ByteBufferBuilder.exactlySized(vertexCount * DefaultVertexFormat.PARTICLE.getVertexSize())) {
                BufferBuilder bufferBuilder = new BufferBuilder(builder, VertexFormat.Mode.QUADS, DefaultVertexFormat.PARTICLE);
                Map<SingleQuadParticle.Layer, QuadParticleRenderState.PreparedLayer> preparedLayers = new HashMap<>();
                int offset = 0;

                for (Entry<SingleQuadParticle.Layer, QuadParticleRenderState.Storage> entry : this.particles.entrySet()) {
                    if (entry.getKey().translucent() == translucent) {
                        entry.getValue()
                            .forEachParticle(
                                (x, y, z, xRot, yRot, zRot, wRot, scale, u0, u1, v0, v1, color, lightCoords) -> this.renderRotatedQuad(
                                    bufferBuilder, x, y, z, xRot, yRot, zRot, wRot, scale, u0, u1, v0, v1, color, lightCoords
                                )
                            );
                        if (entry.getValue().count() > 0) {
                            preparedLayers.put(entry.getKey(), new QuadParticleRenderState.PreparedLayer(offset, entry.getValue().count() * 6));
                        }

                        offset += entry.getValue().count() * 4;
                    }
                }

                MeshData mesh = bufferBuilder.build();
                if (mesh != null) {
                    cachedBuffer.write(mesh.vertexBuffer());
                    RenderSystem.getSequentialBuffer(VertexFormat.Mode.QUADS).getBuffer(mesh.drawState().indexCount());
                    GpuBufferSlice dynamicTransforms = RenderSystem.getDynamicUniforms()
                        .writeTransform(RenderSystem.getModelViewMatrix(), new Vector4f(1.0F, 1.0F, 1.0F, 1.0F), new Vector3f(), new Matrix4f());
                    return new QuadParticleRenderState.PreparedBuffers(mesh.drawState().indexCount(), dynamicTransforms, preparedLayers);
                }

                var14 = null;
            }

            return (QuadParticleRenderState.PreparedBuffers)var14;
        }
    }

    @Override
    public void render(
        QuadParticleRenderState.PreparedBuffers preparedBuffers,
        ParticleFeatureRenderer.ParticleBufferCache bufferCache,
        RenderPass renderPass,
        TextureManager textureManager
    ) {
        RenderSystem.AutoStorageIndexBuffer indexBuffer = RenderSystem.getSequentialBuffer(VertexFormat.Mode.QUADS);
        renderPass.setVertexBuffer(0, bufferCache.get());
        renderPass.setIndexBuffer(indexBuffer.getBuffer(preparedBuffers.indexCount), indexBuffer.type());
        renderPass.setUniform("DynamicTransforms", preparedBuffers.dynamicTransforms);

        for (Entry<SingleQuadParticle.Layer, QuadParticleRenderState.PreparedLayer> entry : preparedBuffers.layers.entrySet()) {
            renderPass.setPipeline(entry.getKey().pipeline());
            AbstractTexture texture = textureManager.getTexture(entry.getKey().textureAtlasLocation());
            renderPass.bindTexture("Sampler0", texture.getTextureView(), texture.getSampler());
            renderPass.drawIndexed(entry.getValue().vertexOffset, 0, entry.getValue().indexCount, 1);
        }
    }

    protected void renderRotatedQuad(
        VertexConsumer builder,
        float x,
        float y,
        float z,
        float xRot,
        float yRot,
        float zRot,
        float wRot,
        float scale,
        float u0,
        float u1,
        float v0,
        float v1,
        int color,
        int lightCoords
    ) {
        Quaternionf rotation = new Quaternionf(xRot, yRot, zRot, wRot);
        this.renderVertex(builder, rotation, x, y, z, 1.0F, -1.0F, scale, u1, v1, color, lightCoords);
        this.renderVertex(builder, rotation, x, y, z, 1.0F, 1.0F, scale, u1, v0, color, lightCoords);
        this.renderVertex(builder, rotation, x, y, z, -1.0F, 1.0F, scale, u0, v0, color, lightCoords);
        this.renderVertex(builder, rotation, x, y, z, -1.0F, -1.0F, scale, u0, v1, color, lightCoords);
    }

    private void renderVertex(
        VertexConsumer builder, Quaternionf rotation, float x, float y, float z, float nx, float ny, float scale, float u, float v, int color, int lightCoords
    ) {
        Vector3f scratch = new Vector3f(nx, ny, 0.0F).rotate(rotation).mul(scale).add(x, y, z);
        builder.addVertex(scratch.x(), scratch.y(), scratch.z()).setUv(u, v).setColor(color).setLight(lightCoords);
    }

    @Override
    public void submit(SubmitNodeCollector submitNodeCollector, CameraRenderState camera) {
        if (this.particleCount > 0) {
            submitNodeCollector.submitParticleGroup(this);
        }
    }

    @FunctionalInterface
    @OnlyIn(Dist.CLIENT)
    public interface ParticleConsumer {
        void consume(
            final float x,
            final float y,
            final float z,
            final float xRot,
            final float yRot,
            final float zRot,
            final float wRot,
            final float scale,
            final float u0,
            final float u1,
            final float v0,
            final float v1,
            final int color,
            final int lightCoords
        );
    }

    @OnlyIn(Dist.CLIENT)
    public record PreparedBuffers(int indexCount, GpuBufferSlice dynamicTransforms, Map<SingleQuadParticle.Layer, QuadParticleRenderState.PreparedLayer> layers) {
    }

    @OnlyIn(Dist.CLIENT)
    public record PreparedLayer(int vertexOffset, int indexCount) {
    }

    @OnlyIn(Dist.CLIENT)
    private static class Storage {
        private int capacity = 1024;
        private float[] floatValues = new float[12288];
        private int[] intValues = new int[2048];
        private int currentParticleIndex;

        public void add(
            float x,
            float y,
            float z,
            float xRot,
            float yRot,
            float zRot,
            float wRot,
            float scale,
            float u0,
            float u1,
            float v0,
            float v1,
            int color,
            int lightCoords
        ) {
            if (this.currentParticleIndex >= this.capacity) {
                this.grow();
            }

            int index = this.currentParticleIndex * 12;
            this.floatValues[index++] = x;
            this.floatValues[index++] = y;
            this.floatValues[index++] = z;
            this.floatValues[index++] = xRot;
            this.floatValues[index++] = yRot;
            this.floatValues[index++] = zRot;
            this.floatValues[index++] = wRot;
            this.floatValues[index++] = scale;
            this.floatValues[index++] = u0;
            this.floatValues[index++] = u1;
            this.floatValues[index++] = v0;
            this.floatValues[index] = v1;
            index = this.currentParticleIndex * 2;
            this.intValues[index++] = color;
            this.intValues[index] = lightCoords;
            this.currentParticleIndex++;
        }

        public void forEachParticle(QuadParticleRenderState.ParticleConsumer consumer) {
            for (int particleIndex = 0; particleIndex < this.currentParticleIndex; particleIndex++) {
                int floatIndex = particleIndex * 12;
                int intIndex = particleIndex * 2;
                consumer.consume(
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex++],
                    this.floatValues[floatIndex],
                    this.intValues[intIndex++],
                    this.intValues[intIndex]
                );
            }
        }

        public void clear() {
            this.currentParticleIndex = 0;
        }

        private void grow() {
            this.capacity *= 2;
            this.floatValues = Arrays.copyOf(this.floatValues, this.capacity * 12);
            this.intValues = Arrays.copyOf(this.intValues, this.capacity * 2);
        }

        public int count() {
            return this.currentParticleIndex;
        }
    }
}
