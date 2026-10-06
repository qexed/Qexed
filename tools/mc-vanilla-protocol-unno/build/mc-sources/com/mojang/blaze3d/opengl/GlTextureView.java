package com.mojang.blaze3d.opengl;

import com.mojang.blaze3d.textures.GpuTexture;
import com.mojang.blaze3d.textures.GpuTextureView;
import it.unimi.dsi.fastutil.ints.Int2IntArrayMap;
import it.unimi.dsi.fastutil.ints.Int2IntMap;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class GlTextureView extends GpuTextureView {
    private static final int EMPTY = -1;
    private boolean closed;
    private int firstFboId = -1;
    private int firstFboDepthId = -1;
    private @Nullable Int2IntMap fboCache;

    protected GlTextureView(GlTexture texture, int baseMipLevel, int mipLevels) {
        super(texture, baseMipLevel, mipLevels);
        texture.addViews();
    }

    @Override
    public boolean isClosed() {
        return this.closed;
    }

    @Override
    public void close() {
        if (!this.closed) {
            this.closed = true;
            this.texture().removeViews();
            if (this.firstFboId != -1) {
                GlStateManager._glDeleteFramebuffers(this.firstFboId);
            }

            if (this.fboCache != null) {
                for (int fbo : this.fboCache.values()) {
                    GlStateManager._glDeleteFramebuffers(fbo);
                }
            }
        }
    }

    public int getFbo(DirectStateAccess dsa, @Nullable GpuTexture depth) {
        int depthId = depth == null ? 0 : ((GlTexture)depth).id;
        var useStencil = depth != null && depth.getFormat().hasStencilAspect();
        if (this.firstFboDepthId == depthId) {
            return this.firstFboId;
        } else if (this.firstFboId == -1) {
            this.firstFboId = this.createFbo(dsa, depthId, useStencil);
            this.firstFboDepthId = depthId;
            return this.firstFboId;
        } else {
            if (this.fboCache == null) {
                this.fboCache = new Int2IntArrayMap();
            }

            return this.fboCache.computeIfAbsent(depthId, _depthId -> this.createFbo(dsa, _depthId, useStencil));
        }
    }

    /** @deprecated use overload that takes useStencil */
    private int createFbo(DirectStateAccess dsa, int depthid) {
        return createFbo(dsa, depthid, false);
    }

    // Neo: Allow for stencil use
    private int createFbo(DirectStateAccess dsa, int depthid, boolean useStencil) {
        int fbo = dsa.createFrameBufferObject();
        dsa.bindFrameBufferTextures(fbo, this.texture().id, depthid, this.baseMipLevel(), 0, useStencil);
        return fbo;
    }

    public GlTexture texture() {
        return (GlTexture)super.texture();
    }
}
