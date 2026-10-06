package com.mojang.blaze3d.systems;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.textures.GpuSampler;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.vertex.VertexFormat;
import com.mojang.logging.LogUtils;
import java.util.Collection;
import java.util.function.BiConsumer;
import java.util.function.Supplier;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
public class RenderPass implements AutoCloseable {
    private static final Logger LOGGER = LogUtils.getLogger();
    private final RenderPassBackend backend;
    private final GpuDeviceBackend device;
    private int pushedDebugGroups;

    public RenderPass(RenderPassBackend backend, GpuDeviceBackend device) {
        this.backend = backend;
        this.device = device;
    }

    public void pushDebugGroup(Supplier<String> label) {
        if (this.backend.isClosed()) {
            throw new IllegalStateException("Can't use a closed render pass");
        } else {
            this.pushedDebugGroups++;
            this.backend.pushDebugGroup(label);
        }
    }

    public void popDebugGroup() {
        if (this.backend.isClosed()) {
            throw new IllegalStateException("Can't use a closed render pass");
        } else if (this.pushedDebugGroups == 0) {
            throw new IllegalStateException("Can't pop more debug groups than was pushed!");
        } else {
            this.pushedDebugGroups--;
            this.backend.popDebugGroup();
        }
    }

    public void setPipeline(RenderPipeline pipeline) {
        this.backend.setPipeline(pipeline);
    }

    public void bindTexture(String name, @Nullable GpuTextureView textureView, @Nullable GpuSampler sampler) {
        this.backend.bindTexture(name, textureView, sampler);
    }

    public void setUniform(String name, GpuBuffer value) {
        this.backend.setUniform(name, value);
    }

    public void setUniform(String name, GpuBufferSlice value) {
        int alignment = this.device.getUniformOffsetAlignment();
        if (value.offset() % alignment > 0L) {
            throw new IllegalArgumentException("Uniform buffer offset must be aligned to " + alignment);
        } else {
            this.backend.setUniform(name, value);
        }
    }

    public void setViewport(int x, int y, int width, int height) {
        this.backend.setViewport(x, y, width, height);
    }

    public void enableScissor(int x, int y, int width, int height) {
        this.backend.enableScissor(x, y, width, height);
    }

    public void disableScissor() {
        this.backend.disableScissor();
    }

    public void setVertexBuffer(int slot, GpuBuffer vertexBuffer) {
        this.backend.setVertexBuffer(slot, vertexBuffer);
    }

    public void setIndexBuffer(GpuBuffer indexBuffer, VertexFormat.IndexType indexType) {
        this.backend.setIndexBuffer(indexBuffer, indexType);
    }

    public void drawIndexed(int baseVertex, int firstIndex, int indexCount, int instanceCount) {
        if (this.backend.isClosed()) {
            throw new IllegalStateException("Can't use a closed render pass");
        } else {
            this.backend.drawIndexed(baseVertex, firstIndex, indexCount, instanceCount);
        }
    }

    public <T> void drawMultipleIndexed(
        Collection<RenderPass.Draw<T>> draws,
        @Nullable GpuBuffer defaultIndexBuffer,
        VertexFormat.@Nullable IndexType defaultIndexType,
        Collection<String> dynamicUniforms,
        T uniformArgument
    ) {
        if (this.backend.isClosed()) {
            throw new IllegalStateException("Can't use a closed render pass");
        } else {
            this.backend.drawMultipleIndexed(draws, defaultIndexBuffer, defaultIndexType, dynamicUniforms, uniformArgument);
        }
    }

    public void draw(int firstVertex, int vertexCount) {
        if (this.backend.isClosed()) {
            throw new IllegalStateException("Can't use a closed render pass");
        } else {
            this.backend.draw(firstVertex, vertexCount);
        }
    }

    @Override
    public void close() {
        if (!this.backend.isClosed()) {
            if (this.pushedDebugGroups > 0) {
                throw new IllegalStateException("Render pass had debug groups left open!");
            }

            this.backend.close();
        }
    }

    @OnlyIn(Dist.CLIENT)
    public record Draw<T>(
        int slot,
        GpuBuffer vertexBuffer,
        @Nullable GpuBuffer indexBuffer,
        VertexFormat.@Nullable IndexType indexType,
        int firstIndex,
        int indexCount,
        int baseVertex,
        @Nullable BiConsumer<T, RenderPass.UniformUploader> uniformUploaderConsumer
    ) {
        public Draw(int slot, GpuBuffer vertexBuffer, GpuBuffer indexBuffer, VertexFormat.IndexType indexType, int firstIndex, int indexCount, int baseVertex) {
            this(slot, vertexBuffer, indexBuffer, indexType, firstIndex, indexCount, baseVertex, null);
        }
    }

    @OnlyIn(Dist.CLIENT)
    public interface UniformUploader {
        void upload(String name, GpuBufferSlice buffer);
    }
}
