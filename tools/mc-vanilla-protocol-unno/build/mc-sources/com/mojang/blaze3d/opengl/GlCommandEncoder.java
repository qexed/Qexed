package com.mojang.blaze3d.opengl;

import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.buffers.GpuBufferSlice;
import com.mojang.blaze3d.buffers.GpuFence;
import com.mojang.blaze3d.pipeline.BlendFunction;
import com.mojang.blaze3d.pipeline.DepthStencilState;
import com.mojang.blaze3d.pipeline.RenderPipeline;
import com.mojang.blaze3d.platform.NativeImage;
import com.mojang.blaze3d.shaders.UniformType;
import com.mojang.blaze3d.systems.CommandEncoderBackend;
import com.mojang.blaze3d.systems.GpuQuery;
import com.mojang.blaze3d.systems.RenderPass;
import com.mojang.blaze3d.systems.RenderPassBackend;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.GpuTextureView;
import com.mojang.blaze3d.textures.TextureFormat;
import com.mojang.blaze3d.vertex.VertexFormat;
import com.mojang.logging.LogUtils;
import java.lang.runtime.SwitchBootstraps;
import java.nio.ByteBuffer;
import java.util.Collection;
import java.util.Collections;
import java.util.Objects;
import java.util.OptionalDouble;
import java.util.OptionalInt;
import java.util.Map.Entry;
import java.util.function.BiConsumer;
import java.util.function.Supplier;
import net.minecraft.util.ARGB;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL11C;
import org.lwjgl.opengl.GL31;
import org.lwjgl.opengl.GL32;
import org.lwjgl.opengl.GL32C;
import org.lwjgl.opengl.GL33C;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
class GlCommandEncoder implements CommandEncoderBackend {
    private static final Logger LOGGER = LogUtils.getLogger();
    private final GlDevice device;
    private final int readFbo;
    private final int drawFbo;
    private @Nullable RenderPipeline lastPipeline;
    private boolean inRenderPass;
    private @Nullable GlProgram lastProgram;
    private @Nullable GlTimerQuery activeTimerQuery;

    protected GlCommandEncoder(GlDevice device) {
        this.device = device;
        this.readFbo = device.directStateAccess().createFrameBufferObject();
        this.drawFbo = device.directStateAccess().createFrameBufferObject();
    }

    @Override
    public RenderPassBackend createRenderPass(Supplier<String> label, GpuTextureView colorTexture, OptionalInt clearColor) {
        return this.createRenderPass(label, colorTexture, clearColor, null, OptionalDouble.empty());
    }

    @Override
    public RenderPassBackend createRenderPass(
        Supplier<String> label, GpuTextureView colorTexture, OptionalInt clearColor, @Nullable GpuTextureView depthTexture, OptionalDouble clearDepth
    ) {
        this.inRenderPass = true;
        this.device.debugLabels().pushDebugGroup(label);
        int fbo = ((GlTextureView)colorTexture).getFbo(this.device.directStateAccess(), depthTexture == null ? null : depthTexture.texture());
        GlStateManager._glBindFramebuffer(36160, fbo);
        int clearMask = 0;
        if (clearColor.isPresent()) {
            int argb = clearColor.getAsInt();
            GL11.glClearColor(ARGB.redFloat(argb), ARGB.greenFloat(argb), ARGB.blueFloat(argb), ARGB.alphaFloat(argb));
            clearMask |= 16384;
        }

        if (depthTexture != null && clearDepth.isPresent()) {
            GL11.glClearDepth(clearDepth.getAsDouble());
            clearMask |= 256;
        }

        if (clearMask != 0) {
            GlStateManager._disableScissorTest();
            GlStateManager._depthMask(true);
            GlStateManager._colorMask(15);
            GlStateManager._clear(clearMask);
        }

        GlStateManager._viewport(0, 0, colorTexture.getWidth(0), colorTexture.getHeight(0));
        this.lastPipeline = null;
        return new GlRenderPass(this, this.device, depthTexture != null);
    }

    @Override
    public boolean isInRenderPass() {
        return this.inRenderPass;
    }

    @Override
    public void clearColorTexture(GpuTexture colorTexture, int clearColor) {
        this.device.directStateAccess().bindFrameBufferTextures(this.drawFbo, ((GlTexture)colorTexture).id, 0, 0, 36160);
        GL11.glClearColor(ARGB.redFloat(clearColor), ARGB.greenFloat(clearColor), ARGB.blueFloat(clearColor), ARGB.alphaFloat(clearColor));
        GlStateManager._disableScissorTest();
        GlStateManager._colorMask(15);
        GlStateManager._clear(16384);
        GlStateManager._glFramebufferTexture2D(36160, 36064, 3553, 0, 0);
        GlStateManager._glBindFramebuffer(36160, 0);
    }

    @Override
    public void clearColorAndDepthTextures(GpuTexture colorTexture, int clearColor, GpuTexture depthTexture, double clearDepth) {
        int fbo = ((GlTexture)colorTexture).getFbo(this.device.directStateAccess(), depthTexture);
        GlStateManager._glBindFramebuffer(36160, fbo);
        GlStateManager._disableScissorTest();
        GL11.glClearDepth(clearDepth);
        GL11.glClearColor(ARGB.redFloat(clearColor), ARGB.greenFloat(clearColor), ARGB.blueFloat(clearColor), ARGB.alphaFloat(clearColor));
        GlStateManager._depthMask(true);
        GlStateManager._colorMask(15);
        GlStateManager._clear(16640);
        GlStateManager._glBindFramebuffer(36160, 0);
    }

    @Override
    public void clearColorAndDepthTextures(
        GpuTexture colorTexture, int clearColor, GpuTexture depthTexture, double clearDepth, int regionX, int regionY, int regionWidth, int regionHeight
    ) {
        int fbo = ((GlTexture)colorTexture).getFbo(this.device.directStateAccess(), depthTexture);
        GlStateManager._glBindFramebuffer(36160, fbo);
        GlStateManager._scissorBox(regionX, regionY, regionWidth, regionHeight);
        GlStateManager._enableScissorTest();
        GL11.glClearDepth(clearDepth);
        GL11.glClearColor(ARGB.redFloat(clearColor), ARGB.greenFloat(clearColor), ARGB.blueFloat(clearColor), ARGB.alphaFloat(clearColor));
        GlStateManager._depthMask(true);
        GlStateManager._colorMask(15);
        GlStateManager._clear(16640);
        GlStateManager._glBindFramebuffer(36160, 0);
    }

    @Override
    public void clearDepthTexture(GpuTexture depthTexture, double clearDepth) {
        boolean hasStencil = depthTexture.getFormat().hasStencilAspect();
        this.device.directStateAccess().bindFrameBufferTextures(this.drawFbo, 0, ((GlTexture)depthTexture).id, 0, 36160, hasStencil);
        GL11.glDrawBuffer(0);
        GL11.glClearDepth(clearDepth);
        GlStateManager._depthMask(true);
        GlStateManager._disableScissorTest();
        GlStateManager._clear(256);
        GL11.glDrawBuffer(36064);
        GlStateManager._glFramebufferTexture2D(36160, 36096, 3553, 0, 0);
        GlStateManager._glBindFramebuffer(36160, 0);
    }

    @Override
    public void clearStencilTexture(GpuTexture texture, int value) {
        this.device.directStateAccess().bindFrameBufferTextures(this.drawFbo, 0, ((GlTexture)texture).id, 0, GlConst.GL_FRAMEBUFFER, true);
        GL11.glDrawBuffer(GlConst.GL_NONE);
        GL11.glClearStencil(value);
        GlStateManager._depthMask(true);
        GlStateManager._clear(GL11.GL_STENCIL_BUFFER_BIT);
        GL11.glDrawBuffer(GlConst.GL_COLOR_ATTACHMENT0);
        GlStateManager._glBindFramebuffer(GlConst.GL_FRAMEBUFFER, 0);
    }

    @Override
    public void writeToBuffer(GpuBufferSlice slice, ByteBuffer data) {
        GlBuffer buffer = (GlBuffer)slice.buffer();
        if (buffer.closed) {
            throw new IllegalStateException("Buffer already closed");
        } else if ((buffer.usage() & 8) == 0) {
            throw new IllegalStateException("Buffer needs USAGE_COPY_DST to be a destination for a copy");
        } else {
            int length = data.remaining();
            if (length > slice.length()) {
                throw new IllegalArgumentException(
                    "Cannot write more data than the slice allows (attempting to write " + length + " bytes into a slice of length " + slice.length() + ")"
                );
            } else if (slice.length() + slice.offset() > buffer.size()) {
                throw new IllegalArgumentException(
                    "Cannot write more data than this buffer can hold (attempting to write "
                        + length
                        + " bytes at offset "
                        + slice.offset()
                        + " to "
                        + buffer.size()
                        + " size buffer)"
                );
            } else {
                this.device.directStateAccess().bufferSubData(buffer.handle, slice.offset(), data, buffer.usage());
            }
        }
    }

    @Override
    public GpuBuffer.MappedView mapBuffer(GpuBufferSlice slice, boolean read, boolean write) {
        GlBuffer buffer = (GlBuffer)slice.buffer();
        int flags = 0;
        if (read) {
            flags |= 1;
        }

        if (write) {
            flags |= 34;
        }

        return this.device.getBufferStorage().mapBuffer(this.device.directStateAccess(), buffer, slice.offset(), slice.length(), flags);
    }

    @Override
    public void copyToBuffer(GpuBufferSlice source, GpuBufferSlice target) {
        GlBuffer sourceBuffer = (GlBuffer)source.buffer();
        GlBuffer targetBuffer = (GlBuffer)target.buffer();
        this.device.directStateAccess().copyBufferSubData(sourceBuffer.handle, targetBuffer.handle, source.offset(), target.offset(), source.length());
    }

    @Override
    public void writeToTexture(
        GpuTexture destination, NativeImage source, int mipLevel, int depthOrLayer, int destX, int destY, int width, int height, int sourceX, int sourceY
    ) {
        int target;
        if ((destination.usage() & 16) != 0) {
            target = GlConst.CUBEMAP_TARGETS[depthOrLayer % 6];
            GL11.glBindTexture(34067, ((GlTexture)destination).id);
        } else {
            target = 3553;
            GlStateManager._bindTexture(((GlTexture)destination).id);
        }

        GlStateManager._pixelStore(3314, source.getWidth());
        GlStateManager._pixelStore(3316, sourceX);
        GlStateManager._pixelStore(3315, sourceY);
        GlStateManager._pixelStore(3317, source.format().components());
        GlStateManager._texSubImage2D(target, mipLevel, destX, destY, width, height, GlConst.toGl(source.format()), 5121, source.getPointer());
    }

    @Override
    public void writeToTexture(
        GpuTexture destination, ByteBuffer source, NativeImage.Format format, int mipLevel, int depthOrLayer, int destX, int destY, int width, int height
    ) {
        int target;
        if ((destination.usage() & 16) != 0) {
            target = GlConst.CUBEMAP_TARGETS[depthOrLayer % 6];
            GL11.glBindTexture(34067, ((GlTexture)destination).id);
        } else {
            target = 3553;
            GlStateManager._bindTexture(((GlTexture)destination).id);
        }

        GlStateManager._pixelStore(3314, width);
        GlStateManager._pixelStore(3316, 0);
        GlStateManager._pixelStore(3315, 0);
        GlStateManager._pixelStore(3317, format.components());
        GlStateManager._texSubImage2D(target, mipLevel, destX, destY, width, height, GlConst.toGl(format), 5121, source);
    }

    @Override
    public void copyTextureToBuffer(GpuTexture source, GpuBuffer destination, long offset, Runnable callback, int mipLevel) {
        this.copyTextureToBuffer(source, destination, offset, callback, mipLevel, 0, 0, source.getWidth(mipLevel), source.getHeight(mipLevel));
    }

    @Override
    public void copyTextureToBuffer(GpuTexture source, GpuBuffer destination, long offset, Runnable callback, int mipLevel, int x, int y, int width, int height) {
        GlStateManager.clearGlErrors();
        this.device.directStateAccess().bindFrameBufferTextures(this.readFbo, ((GlTexture)source).glId(), 0, mipLevel, 36008);
        GlStateManager._glBindBuffer(35051, ((GlBuffer)destination).handle);
        GlStateManager._pixelStore(3330, width);
        GlStateManager._readPixels(x, y, width, height, GlConst.toGlExternalId(source.getFormat()), GlConst.toGlType(source.getFormat()), offset);
        RenderSystem.queueFencedTask(callback);
        GlStateManager._glFramebufferTexture2D(36008, 36064, 3553, 0, mipLevel);
        GlStateManager._glBindFramebuffer(36008, 0);
        GlStateManager._glBindBuffer(35051, 0);
        int error = GlStateManager._getError();
        if (error != 0) {
            throw new IllegalStateException("Couldn't perform copyTobuffer for texture " + source.getLabel() + ": GL error " + error);
        }
    }

    @Override
    public void copyTextureToTexture(
        GpuTexture source, GpuTexture destination, int mipLevel, int destX, int destY, int sourceX, int sourceY, int width, int height
    ) {
        TextureFormat sourceFormat = source.getFormat();
        TextureFormat destFormat = destination.getFormat();

        if (sourceFormat.hasDepthAspect() || sourceFormat.hasStencilAspect()) {
            if (sourceFormat != destFormat) {
                throw new IllegalArgumentException(
                        "When copying depth or stencil data, the source and destination texture formats must be identical. Source: "
                                + sourceFormat + ", Destination: " + destFormat
                );
            }
        }

        if (sourceFormat.hasColorAspect() != destFormat.hasColorAspect()) {
            throw new IllegalArgumentException(
                    "Source and destination texture formats must have consistent color aspects. Source: "
                            + sourceFormat + ", Destination: " + destFormat
            );
        }

        GlStateManager.clearGlErrors();
        GlStateManager._disableScissorTest();
        boolean isDepth = source.getFormat().hasDepthAspect();
        int sourceId = ((GlTexture)source).glId();
        int destId = ((GlTexture)destination).glId();
        boolean hasStencil = source.getFormat().hasStencilAspect();
        this.device.directStateAccess().bindFrameBufferTextures(this.readFbo, isDepth ? 0 : sourceId, isDepth ? sourceId : 0, 0, 0, hasStencil);
        this.device.directStateAccess().bindFrameBufferTextures(this.drawFbo, isDepth ? 0 : destId, isDepth ? destId : 0, 0, 0, hasStencil);
        int bufferMask = 0;
        if (source.getFormat().hasColorAspect()) {
            bufferMask |= GlConst.GL_COLOR_BUFFER_BIT;
        }
        if (source.getFormat().hasDepthAspect()) {
            bufferMask |= GlConst.GL_DEPTH_BUFFER_BIT;
        }
        if (source.getFormat().hasStencilAspect()) {
            bufferMask |= GL11.GL_STENCIL_BUFFER_BIT;
        }
        this.device
            .directStateAccess()
            .blitFrameBuffers(this.readFbo, this.drawFbo, sourceX, sourceY, width, height, destX, destY, width, height, bufferMask, 9728);
        int error = GlStateManager._getError();
        if (error != 0) {
            throw new IllegalStateException(
                "Couldn't perform copyToTexture for texture " + source.getLabel() + " to " + destination.getLabel() + ": GL error " + error
            );
        }
    }

    @Override
    public void presentTexture(GpuTextureView textureView) {
        GlStateManager._disableScissorTest();
        GlStateManager._viewport(0, 0, textureView.getWidth(0), textureView.getHeight(0));
        GlStateManager._depthMask(true);
        GlStateManager._colorMask(15);
        this.device.directStateAccess().bindFrameBufferTextures(this.drawFbo, ((GlTexture)textureView.texture()).glId(), 0, 0, 0);
        this.device
            .directStateAccess()
            .blitFrameBuffers(
                this.drawFbo, 0, 0, 0, textureView.getWidth(0), textureView.getHeight(0), 0, 0, textureView.getWidth(0), textureView.getHeight(0), 16384, 9728
            );
    }

    @Override
    public GpuFence createFence() {
        return new GlFence();
    }

    protected <T> void executeDrawMultiple(
        GlRenderPass renderPass,
        Collection<RenderPass.Draw<T>> draws,
        @Nullable GpuBuffer defaultIndexBuffer,
        VertexFormat.@Nullable IndexType defaultIndexType,
        Collection<String> dynamicUniforms,
        T uniformArgument
    ) {
        if (this.trySetup(renderPass, dynamicUniforms)) {
            if (defaultIndexType == null) {
                defaultIndexType = VertexFormat.IndexType.SHORT;
            }

            for (RenderPass.Draw<T> draw : draws) {
                VertexFormat.IndexType indexType = draw.indexType() == null ? defaultIndexType : draw.indexType();
                renderPass.setIndexBuffer(draw.indexBuffer() == null ? defaultIndexBuffer : draw.indexBuffer(), indexType);
                renderPass.setVertexBuffer(draw.slot(), draw.vertexBuffer());
                if (GlRenderPass.VALIDATION) {
                    if (renderPass.indexBuffer == null) {
                        throw new IllegalStateException("Missing index buffer");
                    }

                    if (renderPass.indexBuffer.isClosed()) {
                        throw new IllegalStateException("Index buffer has been closed!");
                    }

                    if (renderPass.vertexBuffers[0] == null) {
                        throw new IllegalStateException("Missing vertex buffer at slot 0");
                    }

                    if (renderPass.vertexBuffers[0].isClosed()) {
                        throw new IllegalStateException("Vertex buffer at slot 0 has been closed!");
                    }
                }

                BiConsumer<T, RenderPass.UniformUploader> uniformUploaderConsumer = draw.uniformUploaderConsumer();
                if (uniformUploaderConsumer != null) {
                    uniformUploaderConsumer.accept(uniformArgument, (name, buffer) -> {
                        if (renderPass.pipeline.program().getUniform(name) instanceof Uniform.Ubo(int var9x)) {
                            int patt2$temp = var9x;
                            if (true) {
                                GL32.glBindBufferRange(35345, patt2$temp, ((GlBuffer)buffer.buffer()).handle, buffer.offset(), buffer.length());
                            }
                        }
                    });
                }

                this.drawFromBuffers(renderPass, draw.baseVertex(), draw.firstIndex(), draw.indexCount(), indexType, renderPass.pipeline, 1);
            }
        }
    }

    protected void executeDraw(
        GlRenderPass renderPass, int baseVertex, int firstIndex, int drawCount, VertexFormat.@Nullable IndexType indexType, int instanceCount
    ) {
        if (this.trySetup(renderPass, Collections.emptyList())) {
            if (GlRenderPass.VALIDATION) {
                if (indexType != null) {
                    if (renderPass.indexBuffer == null) {
                        throw new IllegalStateException("Missing index buffer");
                    }

                    if (renderPass.indexBuffer.isClosed()) {
                        throw new IllegalStateException("Index buffer has been closed!");
                    }

                    if ((renderPass.indexBuffer.usage() & 64) == 0) {
                        throw new IllegalStateException("Index buffer must have GpuBuffer.USAGE_INDEX!");
                    }
                }

                GlRenderPipeline pipeline = renderPass.pipeline;
                if (renderPass.vertexBuffers[0] == null && pipeline != null && !pipeline.info().getVertexFormat().getElements().isEmpty()) {
                    throw new IllegalStateException("Vertex format contains elements but vertex buffer at slot 0 is null");
                }

                if (renderPass.vertexBuffers[0] != null && renderPass.vertexBuffers[0].isClosed()) {
                    throw new IllegalStateException("Vertex buffer at slot 0 has been closed!");
                }

                if (renderPass.vertexBuffers[0] != null && (renderPass.vertexBuffers[0].usage() & 32) == 0) {
                    throw new IllegalStateException("Vertex buffer must have GpuBuffer.USAGE_VERTEX!");
                }
            }

            this.drawFromBuffers(renderPass, baseVertex, firstIndex, drawCount, indexType, renderPass.pipeline, instanceCount);
        }
    }

    private void drawFromBuffers(
        GlRenderPass renderPass,
        int baseVertex,
        int firstIndex,
        int drawCount,
        VertexFormat.@Nullable IndexType indexType,
        GlRenderPipeline pipeline,
        int instanceCount
    ) {
        this.device.vertexArrayCache().bindVertexArray(pipeline.info().getVertexFormat(), (GlBuffer)renderPass.vertexBuffers[0]);
        if (indexType != null) {
            GlStateManager._glBindBuffer(34963, ((GlBuffer)renderPass.indexBuffer).handle);
            if (instanceCount > 1) {
                if (baseVertex > 0) {
                    GL32.glDrawElementsInstancedBaseVertex(
                        GlConst.toGl(pipeline.info().getVertexFormatMode()),
                        drawCount,
                        GlConst.toGl(indexType),
                        (long)firstIndex * indexType.bytes,
                        instanceCount,
                        baseVertex
                    );
                } else {
                    GL31.glDrawElementsInstanced(
                        GlConst.toGl(pipeline.info().getVertexFormatMode()),
                        drawCount,
                        GlConst.toGl(indexType),
                        (long)firstIndex * indexType.bytes,
                        instanceCount
                    );
                }
            } else if (baseVertex > 0) {
                GL32.glDrawElementsBaseVertex(
                    GlConst.toGl(pipeline.info().getVertexFormatMode()), drawCount, GlConst.toGl(indexType), (long)firstIndex * indexType.bytes, baseVertex
                );
            } else {
                GlStateManager._drawElements(
                    GlConst.toGl(pipeline.info().getVertexFormatMode()), drawCount, GlConst.toGl(indexType), (long)firstIndex * indexType.bytes
                );
            }
        } else if (instanceCount > 1) {
            GL31.glDrawArraysInstanced(GlConst.toGl(pipeline.info().getVertexFormatMode()), baseVertex, drawCount, instanceCount);
        } else {
            GlStateManager._drawArrays(GlConst.toGl(pipeline.info().getVertexFormatMode()), baseVertex, drawCount);
        }
    }

    private boolean trySetup(GlRenderPass renderPass, Collection<String> dynamicUniforms) {
        if (GlRenderPass.VALIDATION) {
            if (renderPass.pipeline == null) {
                throw new IllegalStateException("Can't draw without a render pipeline");
            }

            if (renderPass.pipeline.program() == GlProgram.INVALID_PROGRAM) {
                throw new IllegalStateException("Pipeline contains invalid shader program");
            }

            for (RenderPipeline.UniformDescription uniform : renderPass.pipeline.info().getUniforms()) {
                GpuBufferSlice value = renderPass.uniforms.get(uniform.name());
                if (!dynamicUniforms.contains(uniform.name())) {
                    if (value == null) {
                        throw new IllegalStateException("Missing uniform " + uniform.name() + " (should be " + uniform.type() + ")");
                    }

                    if (uniform.type() == UniformType.UNIFORM_BUFFER) {
                        if (value.buffer().isClosed()) {
                            throw new IllegalStateException("Uniform buffer " + uniform.name() + " is already closed");
                        }

                        if ((value.buffer().usage() & 128) == 0) {
                            throw new IllegalStateException("Uniform buffer " + uniform.name() + " must have GpuBuffer.USAGE_UNIFORM");
                        }
                    }

                    if (uniform.type() == UniformType.TEXEL_BUFFER) {
                        if (value.offset() != 0L || value.length() != value.buffer().size()) {
                            throw new IllegalStateException("Uniform texel buffers do not support a slice of a buffer, must be entire buffer");
                        }

                        if (uniform.textureFormat() == null) {
                            throw new IllegalStateException("Invalid uniform texel buffer " + uniform.name() + " (missing a texture format)");
                        }
                    }
                }
            }

            for (Entry<String, Uniform> entry : renderPass.pipeline.program().getUniforms().entrySet()) {
                if (entry.getValue() instanceof Uniform.Sampler) {
                    String name = entry.getKey();
                    GlRenderPass.TextureViewAndSampler viewAndSampler = renderPass.samplers.get(name);
                    if (viewAndSampler == null) {
                        throw new IllegalStateException("Missing sampler " + name);
                    }

                    GlTextureView textureView = viewAndSampler.view();
                    if (textureView.isClosed()) {
                        throw new IllegalStateException("Texture view " + name + " (" + textureView.texture().getLabel() + ") has been closed!");
                    }

                    if ((textureView.texture().usage() & 4) == 0) {
                        throw new IllegalStateException("Texture view " + name + " (" + textureView.texture().getLabel() + ") must have USAGE_TEXTURE_BINDING!");
                    }

                    if (viewAndSampler.sampler().isClosed()) {
                        throw new IllegalStateException("Sampler for " + name + " (" + textureView.texture().getLabel() + ") has been closed!");
                    }
                }
            }

            if (renderPass.pipeline.info().wantsDepthTexture() && !renderPass.hasDepthTexture()) {
                LOGGER.warn("Render pipeline {} wants a depth texture but none was provided - this is probably a bug", renderPass.pipeline.info().getLocation());
            }
        } else if (renderPass.pipeline == null || renderPass.pipeline.program() == GlProgram.INVALID_PROGRAM) {
            return false;
        }

        RenderPipeline pipeline = renderPass.pipeline.info();
        GlProgram glProgram = renderPass.pipeline.program();
        this.applyPipelineState(pipeline);
        boolean differentProgram = this.lastProgram != glProgram;
        if (differentProgram) {
            GlStateManager._glUseProgram(glProgram.getProgramId());
            this.lastProgram = glProgram;
        }

        label200:
        for (Entry<String, Uniform> entryx : glProgram.getUniforms().entrySet()) {
            String namex = entryx.getKey();
            boolean isDirty = renderPass.dirtyUniforms.contains(namex);
            Uniform var10000 = entryx.getValue();
            Objects.requireNonNull(var10000);
            Uniform var10 = var10000;
            byte var11 = 0;

            {
                switch (var10) {
                    case Uniform.Ubo var12:
                        int var63;
                        Uniform.Ubo var62 = var12;

                        try {
                            var63 = var62.blockBinding();
                        } catch (Throwable var32) {
                            throw new MatchException(var32.toString(), var32);
                        }

                        int var41 = var63;
                        if (true) {
                            if (isDirty) {
                                GpuBufferSlice bufferView = renderPass.uniforms.get(namex);
                                GL32.glBindBufferRange(35345, var41, ((GlBuffer)bufferView.buffer()).handle, bufferView.offset(), bufferView.length());
                            }
                            continue label200;
                        }

                        var11 = 1;
                        break;
                    case Uniform.Utb(int location, int samplerIdx, TextureFormat var45, int var43):
                                    if (differentProgram || isDirty) {
                                        GlStateManager._glUniform1i(location, samplerIdx);
                                    }

                                    GlStateManager._activeTexture(33984 + samplerIdx);
                                    GL11C.glBindTexture(35882, var43);
                                    if (isDirty) {
                                        GpuBufferSlice bufferViewx = renderPass.uniforms.get(namex);
                                        GL31.glTexBuffer(35882, GlConst.toGlInternalId(var45), ((GlBuffer)bufferViewx.buffer()).handle);
                                    }
                                    continue label200;
                    case Uniform.Sampler(int location, int samplerIndex):
                                GlRenderPass.TextureViewAndSampler viewAndSamplerxx = renderPass.samplers.get(namex);
                                if (viewAndSamplerxx == null) {
                                    continue label200;
                                }

                                GlTextureView textureViewx = viewAndSamplerxx.view();
                                if (differentProgram || isDirty) {
                                    GlStateManager._glUniform1i(location, samplerIndex);
                                }

                                GlStateManager._activeTexture(33984 + samplerIndex);
                                GlTexture texture = textureViewx.texture();
                                int target;
                                if ((texture.usage() & 16) != 0) {
                                    target = 34067;
                                    GL11.glBindTexture(34067, texture.id);
                                } else {
                                    target = 3553;
                                    GlStateManager._bindTexture(texture.id);
                                }

                                GL33C.glBindSampler(samplerIndex, viewAndSamplerxx.sampler().getId());
                                GlStateManager._texParameter(target, 33084, textureViewx.baseMipLevel());
                                GlStateManager._texParameter(target, 33085, textureViewx.baseMipLevel() + textureViewx.mipLevels() - 1);
                                continue label200;
                    default:
                        throw new MatchException(null, null);
                }
            }
        }

        renderPass.dirtyUniforms.clear();
        if (renderPass.isScissorEnabled()) {
            GlStateManager._enableScissorTest();
            GlStateManager._scissorBox(renderPass.getScissorX(), renderPass.getScissorY(), renderPass.getScissorWidth(), renderPass.getScissorHeight());
        } else {
            GlStateManager._disableScissorTest();
        }

        var stencilTestOpt = renderPass.pipeline.info().getStencilTest();
        if (stencilTestOpt.isPresent()) {
            var stencilTest = stencilTestOpt.get();
            GlStateManager._enableStencilTest();
            var front = stencilTest.front();
            var back = stencilTest.back();
            if (front.equals(back)) {
                GlStateManager._stencilFunc(GlConst.toGl(front.compare()), stencilTest.referenceValue(), stencilTest.readMask());
                GlStateManager._stencilOp(GlConst.toGl(front.fail()), GlConst.toGl(front.depthFail()), GlConst.toGl(front.pass()));
            } else {
                GlStateManager._stencilFuncFront(GlConst.toGl(front.compare()), stencilTest.referenceValue(), stencilTest.readMask());
                GlStateManager._stencilFuncBack(GlConst.toGl(back.compare()), stencilTest.referenceValue(), stencilTest.readMask());
                GlStateManager._stencilOpFront(GlConst.toGl(front.fail()), GlConst.toGl(front.depthFail()), GlConst.toGl(front.pass()));
                GlStateManager._stencilOpBack(GlConst.toGl(back.fail()), GlConst.toGl(back.depthFail()), GlConst.toGl(back.pass()));
            }
            GlStateManager._stencilMask(stencilTest.writeMask());
        } else {
            GlStateManager._disableStencilTest();
        }

        return true;
    }

    private void applyPipelineState(RenderPipeline pipeline) {
        if (this.lastPipeline != pipeline) {
            this.lastPipeline = pipeline;
            DepthStencilState depthStencilState = pipeline.getDepthStencilState();
            if (depthStencilState != null) {
                GlStateManager._enableDepthTest();
                GlStateManager._depthFunc(GlConst.toGl(depthStencilState.depthTest()));
                GlStateManager._depthMask(depthStencilState.writeDepth());
                if (depthStencilState.depthBiasConstant() == 0.0F && depthStencilState.depthBiasScaleFactor() == 0.0F) {
                    GlStateManager._disablePolygonOffset();
                } else {
                    GlStateManager._polygonOffset(depthStencilState.depthBiasScaleFactor(), depthStencilState.depthBiasConstant());
                    GlStateManager._enablePolygonOffset();
                }
            } else {
                GlStateManager._disableDepthTest();
                GlStateManager._depthMask(false);
                GlStateManager._disablePolygonOffset();
            }

            if (pipeline.isCull()) {
                GlStateManager._enableCull();
            } else {
                GlStateManager._disableCull();
            }

            if (pipeline.getColorTargetState().blendFunction().isPresent()) {
                GlStateManager._enableBlend();
                BlendFunction blendFunction = pipeline.getColorTargetState().blendFunction().get();
                GlStateManager._blendFuncSeparate(
                    GlConst.toGl(blendFunction.sourceColor()),
                    GlConst.toGl(blendFunction.destColor()),
                    GlConst.toGl(blendFunction.sourceAlpha()),
                    GlConst.toGl(blendFunction.destAlpha())
                );
            } else {
                GlStateManager._disableBlend();
            }

            GlStateManager._polygonMode(1032, GlConst.toGl(pipeline.getPolygonMode()));
            GlStateManager._colorMask(pipeline.getColorTargetState().writeMask());
        }
    }

    public void finishRenderPass() {
        this.inRenderPass = false;
        GlStateManager._glBindFramebuffer(36160, 0);
        this.device.debugLabels().popDebugGroup();
    }

    @Override
    public GpuQuery timerQueryBegin() {
        RenderSystem.assertOnRenderThread();
        if (this.activeTimerQuery != null) {
            throw new IllegalStateException("A GL_TIME_ELAPSED query is already active");
        } else {
            int queryId = GL32C.glGenQueries();
            GL32C.glBeginQuery(35007, queryId);
            this.activeTimerQuery = new GlTimerQuery(queryId);
            return this.activeTimerQuery;
        }
    }

    @Override
    public void timerQueryEnd(GpuQuery query) {
        RenderSystem.assertOnRenderThread();
        if (query != this.activeTimerQuery) {
            throw new IllegalStateException("Mismatched or duplicate GpuQuery when ending timerQuery");
        } else {
            GL32C.glEndQuery(35007);
            this.activeTimerQuery = null;
        }
    }
}
