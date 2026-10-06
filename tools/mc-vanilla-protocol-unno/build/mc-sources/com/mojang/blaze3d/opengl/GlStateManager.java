package com.mojang.blaze3d.opengl;

import com.mojang.blaze3d.pipeline.ColorTargetState;
import com.mojang.blaze3d.platform.MacosUtil;
import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.jtracy.Plot;
import com.mojang.jtracy.TracyClient;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.stream.IntStream;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.lwjgl.PointerBuffer;
import org.lwjgl.opengl.GL11;
import org.lwjgl.opengl.GL13;
import org.lwjgl.opengl.GL14;
import org.lwjgl.opengl.GL15;
import org.lwjgl.opengl.GL20;
import org.lwjgl.opengl.GL20C;
import org.lwjgl.opengl.GL30;
import org.lwjgl.opengl.GL32;
import org.lwjgl.system.MemoryStack;
import org.lwjgl.system.MemoryUtil;

@OnlyIn(Dist.CLIENT)
public class GlStateManager {
    private static final Plot PLOT_TEXTURES = TracyClient.createPlot("GPU Textures");
    private static int numTextures = 0;
    private static final Plot PLOT_BUFFERS = TracyClient.createPlot("GPU Buffers");
    private static int numBuffers = 0;
    private static final GlStateManager.BlendState BLEND = new GlStateManager.BlendState();
    private static final GlStateManager.DepthState DEPTH = new GlStateManager.DepthState();
    private static final GlStateManager.CullState CULL = new GlStateManager.CullState();
    private static final GlStateManager.PolygonOffsetState POLY_OFFSET = new GlStateManager.PolygonOffsetState();
    private static final GlStateManager.ColorLogicState COLOR_LOGIC = new GlStateManager.ColorLogicState();
    private static final GlStateManager.StencilState STENCIL = new GlStateManager.StencilState();
    private static final GlStateManager.ScissorState SCISSOR = new GlStateManager.ScissorState();
    private static int activeTexture;
    private static final int TEXTURE_COUNT = 12;
    private static final GlStateManager.TextureState[] TEXTURES = IntStream.range(0, 12)
        .mapToObj(i -> new GlStateManager.TextureState())
        .toArray(GlStateManager.TextureState[]::new);
    @ColorTargetState.WriteMask
    private static int COLOR_MASK = 15;
    private static int readFbo;
    private static int writeFbo;

    public static void _disableScissorTest() {
        RenderSystem.assertOnRenderThread();
        SCISSOR.mode.disable();
    }

    public static void _enableScissorTest() {
        RenderSystem.assertOnRenderThread();
        SCISSOR.mode.enable();
    }

    public static void _scissorBox(int x, int y, int width, int height) {
        RenderSystem.assertOnRenderThread();
        GL20.glScissor(x, y, width, height);
    }

    public static void _disableDepthTest() {
        RenderSystem.assertOnRenderThread();
        DEPTH.mode.disable();
    }

    public static void _enableDepthTest() {
        RenderSystem.assertOnRenderThread();
        DEPTH.mode.enable();
    }

    public static void _depthFunc(int func) {
        RenderSystem.assertOnRenderThread();
        if (func != DEPTH.func) {
            DEPTH.func = func;
            GL11.glDepthFunc(func);
        }
    }

    public static void _depthMask(boolean mask) {
        RenderSystem.assertOnRenderThread();
        if (mask != DEPTH.mask) {
            DEPTH.mask = mask;
            GL11.glDepthMask(mask);
        }
    }

    public static void _disableBlend() {
        RenderSystem.assertOnRenderThread();
        BLEND.mode.disable();
    }

    public static void _enableBlend() {
        RenderSystem.assertOnRenderThread();
        BLEND.mode.enable();
    }

    public static void _blendFuncSeparate(int srcRgb, int dstRgb, int srcAlpha, int dstAlpha) {
        RenderSystem.assertOnRenderThread();
        if (srcRgb != BLEND.srcRgb || dstRgb != BLEND.dstRgb || srcAlpha != BLEND.srcAlpha || dstAlpha != BLEND.dstAlpha) {
            BLEND.srcRgb = srcRgb;
            BLEND.dstRgb = dstRgb;
            BLEND.srcAlpha = srcAlpha;
            BLEND.dstAlpha = dstAlpha;
            glBlendFuncSeparate(srcRgb, dstRgb, srcAlpha, dstAlpha);
        }
    }

    public static int glGetProgrami(int program, int pname) {
        RenderSystem.assertOnRenderThread();
        return GL20.glGetProgrami(program, pname);
    }

    public static void glAttachShader(int program, int shader) {
        RenderSystem.assertOnRenderThread();
        GL20.glAttachShader(program, shader);
    }

    public static void glDeleteShader(int shader) {
        RenderSystem.assertOnRenderThread();
        GL20.glDeleteShader(shader);
    }

    public static int glCreateShader(int type) {
        RenderSystem.assertOnRenderThread();
        return GL20.glCreateShader(type);
    }

    public static void glShaderSource(int shader, String source) {
        RenderSystem.assertOnRenderThread();
        byte[] encoded = source.getBytes(StandardCharsets.UTF_8);
        ByteBuffer buffer = MemoryUtil.memAlloc(encoded.length + 1);
        buffer.put(encoded);
        buffer.put((byte)0);
        buffer.flip();

        try (MemoryStack stack = MemoryStack.stackPush()) {
            PointerBuffer pointers = stack.mallocPointer(1);
            pointers.put(buffer);
            GL20C.nglShaderSource(shader, 1, pointers.address0(), 0L);
        } finally {
            MemoryUtil.memFree(buffer);
        }
    }

    public static void glCompileShader(int shader) {
        RenderSystem.assertOnRenderThread();
        GL20.glCompileShader(shader);
    }

    public static int glGetShaderi(int shader, int pname) {
        RenderSystem.assertOnRenderThread();
        return GL20.glGetShaderi(shader, pname);
    }

    public static void _glUseProgram(int program) {
        RenderSystem.assertOnRenderThread();
        GL20.glUseProgram(program);
    }

    public static int glCreateProgram() {
        RenderSystem.assertOnRenderThread();
        return GL20.glCreateProgram();
    }

    public static void glDeleteProgram(int program) {
        RenderSystem.assertOnRenderThread();
        GL20.glDeleteProgram(program);
    }

    public static void glLinkProgram(int program) {
        RenderSystem.assertOnRenderThread();
        GL20.glLinkProgram(program);
    }

    public static int _glGetUniformLocation(int program, CharSequence name) {
        RenderSystem.assertOnRenderThread();
        return GL20.glGetUniformLocation(program, name);
    }

    public static void _glUniform1i(int location, int v0) {
        RenderSystem.assertOnRenderThread();
        GL20.glUniform1i(location, v0);
    }

    public static void _glBindAttribLocation(int program, int location, CharSequence name) {
        RenderSystem.assertOnRenderThread();
        GL20.glBindAttribLocation(program, location, name);
    }

    static void incrementTrackedBuffers() {
        numBuffers++;
        PLOT_BUFFERS.setValue(numBuffers);
    }

    public static int _glGenBuffers() {
        RenderSystem.assertOnRenderThread();
        incrementTrackedBuffers();
        return GL15.glGenBuffers();
    }

    public static int _glGenVertexArrays() {
        RenderSystem.assertOnRenderThread();
        return GL30.glGenVertexArrays();
    }

    public static void _glBindBuffer(int target, int buffer) {
        RenderSystem.assertOnRenderThread();
        GL15.glBindBuffer(target, buffer);
    }

    public static void _glBindVertexArray(int arrayId) {
        RenderSystem.assertOnRenderThread();
        GL30.glBindVertexArray(arrayId);
    }

    public static void _glBufferData(int target, ByteBuffer data, int usage) {
        RenderSystem.assertOnRenderThread();
        GL15.glBufferData(target, data, usage);
    }

    public static void _glBufferSubData(int target, long offset, ByteBuffer data) {
        RenderSystem.assertOnRenderThread();
        GL15.glBufferSubData(target, offset, data);
    }

    public static void _glBufferData(int target, long size, int usage) {
        RenderSystem.assertOnRenderThread();
        GL15.glBufferData(target, size, usage);
    }

    public static @Nullable ByteBuffer _glMapBufferRange(int target, long offset, long length, int access) {
        RenderSystem.assertOnRenderThread();
        return GL30.glMapBufferRange(target, offset, length, access);
    }

    public static void _glUnmapBuffer(int target) {
        RenderSystem.assertOnRenderThread();
        GL15.glUnmapBuffer(target);
    }

    public static void _glDeleteBuffers(int buffer) {
        RenderSystem.assertOnRenderThread();
        numBuffers--;
        PLOT_BUFFERS.setValue(numBuffers);
        GL15.glDeleteBuffers(buffer);
    }

    public static void _glBindFramebuffer(int target, int framebuffer) {
        if ((target == 36008 || target == 36160) && readFbo != framebuffer) {
            GL30.glBindFramebuffer(36008, framebuffer);
            readFbo = framebuffer;
        }

        if ((target == 36009 || target == 36160) && writeFbo != framebuffer) {
            GL30.glBindFramebuffer(36009, framebuffer);
            writeFbo = framebuffer;
        }
    }

    public static int getFrameBuffer(int target) {
        if (target == 36008) {
            return readFbo;
        } else {
            return target == 36009 ? writeFbo : 0;
        }
    }

    public static void _glBlitFrameBuffer(int srcX0, int srcY0, int srcX1, int srcY1, int dstX0, int dstY0, int dstX1, int dstY1, int mask, int filter) {
        RenderSystem.assertOnRenderThread();
        GL30.glBlitFramebuffer(srcX0, srcY0, srcX1, srcY1, dstX0, dstY0, dstX1, dstY1, mask, filter);
    }

    public static void _glDeleteFramebuffers(int framebuffer) {
        RenderSystem.assertOnRenderThread();
        GL30.glDeleteFramebuffers(framebuffer);
        if (readFbo == framebuffer) {
            readFbo = 0;
        }

        if (writeFbo == framebuffer) {
            writeFbo = 0;
        }
    }

    public static int glGenFramebuffers() {
        RenderSystem.assertOnRenderThread();
        return GL30.glGenFramebuffers();
    }

    public static void _glFramebufferTexture2D(int target, int attachment, int textarget, int texture, int level) {
        RenderSystem.assertOnRenderThread();
        GL30.glFramebufferTexture2D(target, attachment, textarget, texture, level);
    }

    public static void glBlendFuncSeparate(int srcColor, int dstColor, int srcAlpha, int dstAlpha) {
        RenderSystem.assertOnRenderThread();
        GL14.glBlendFuncSeparate(srcColor, dstColor, srcAlpha, dstAlpha);
    }

    public static String glGetShaderInfoLog(int shader, int maxLength) {
        RenderSystem.assertOnRenderThread();
        return GL20.glGetShaderInfoLog(shader, maxLength);
    }

    public static String glGetProgramInfoLog(int program, int maxLength) {
        RenderSystem.assertOnRenderThread();
        return GL20.glGetProgramInfoLog(program, maxLength);
    }

    public static void _enableCull() {
        RenderSystem.assertOnRenderThread();
        CULL.enable.enable();
    }

    public static void _disableCull() {
        RenderSystem.assertOnRenderThread();
        CULL.enable.disable();
    }

    public static void _polygonMode(int face, int mode) {
        RenderSystem.assertOnRenderThread();
        GL11.glPolygonMode(face, mode);
    }

    public static void _enablePolygonOffset() {
        RenderSystem.assertOnRenderThread();
        POLY_OFFSET.fill.enable();
    }

    public static void _disablePolygonOffset() {
        RenderSystem.assertOnRenderThread();
        POLY_OFFSET.fill.disable();
    }

    public static void _polygonOffset(float factor, float units) {
        RenderSystem.assertOnRenderThread();
        if (factor != POLY_OFFSET.factor || units != POLY_OFFSET.units) {
            POLY_OFFSET.factor = factor;
            POLY_OFFSET.units = units;
            GL11.glPolygonOffset(factor, units);
        }
    }

    public static void _enableColorLogicOp() {
        RenderSystem.assertOnRenderThread();
        COLOR_LOGIC.enable.enable();
    }

    public static void _disableColorLogicOp() {
        RenderSystem.assertOnRenderThread();
        COLOR_LOGIC.enable.disable();
    }

    public static void _logicOp(int op) {
        RenderSystem.assertOnRenderThread();
        if (op != COLOR_LOGIC.op) {
            COLOR_LOGIC.op = op;
            GL11.glLogicOp(op);
        }
    }

    public static void _activeTexture(int texture) {
        RenderSystem.assertOnRenderThread();
        if (activeTexture != texture - 33984) {
            activeTexture = texture - 33984;
            GL13.glActiveTexture(texture);
        }
    }

    public static void _texParameter(int target, int name, int value) {
        RenderSystem.assertOnRenderThread();
        GL11.glTexParameteri(target, name, value);
    }

    public static int _getTexLevelParameter(int target, int level, int name) {
        return GL11.glGetTexLevelParameteri(target, level, name);
    }

    public static int _genTexture() {
        RenderSystem.assertOnRenderThread();
        numTextures++;
        PLOT_TEXTURES.setValue(numTextures);
        return GL11.glGenTextures();
    }

    public static void _deleteTexture(int id) {
        RenderSystem.assertOnRenderThread();
        GL11.glDeleteTextures(id);

        for (GlStateManager.TextureState state : TEXTURES) {
            if (state.binding == id) {
                state.binding = -1;
            }
        }

        numTextures--;
        PLOT_TEXTURES.setValue(numTextures);
    }

    public static void _bindTexture(int id) {
        RenderSystem.assertOnRenderThread();
        if (id != TEXTURES[activeTexture].binding) {
            TEXTURES[activeTexture].binding = id;
            GL11.glBindTexture(3553, id);
        }
    }

    public static void _texImage2D(
        int target, int level, int internalformat, int width, int height, int border, int format, int type, @Nullable ByteBuffer pixels
    ) {
        RenderSystem.assertOnRenderThread();
        GL11.glTexImage2D(target, level, internalformat, width, height, border, format, type, pixels);
    }

    public static void _texSubImage2D(int target, int level, int xoffset, int yoffset, int width, int height, int format, int type, long pixels) {
        RenderSystem.assertOnRenderThread();
        GL11.glTexSubImage2D(target, level, xoffset, yoffset, width, height, format, type, pixels);
    }

    public static void _texSubImage2D(int target, int level, int xoffset, int yoffset, int width, int height, int format, int type, ByteBuffer pixels) {
        RenderSystem.assertOnRenderThread();
        GL11.glTexSubImage2D(target, level, xoffset, yoffset, width, height, format, type, pixels);
    }

    public static void _viewport(int x, int y, int width, int height) {
        GL11.glViewport(x, y, width, height);
    }

    public static void _colorMask(@ColorTargetState.WriteMask int writeMask) {
        RenderSystem.assertOnRenderThread();
        if (writeMask != COLOR_MASK) {
            COLOR_MASK = writeMask;
            GL11.glColorMask((writeMask & 1) != 0, (writeMask & 2) != 0, (writeMask & 4) != 0, (writeMask & 8) != 0);
        }
    }

    public static void _clear(int mask) {
        RenderSystem.assertOnRenderThread();
        GL11.glClear(mask);
        if (MacosUtil.IS_MACOS) {
            _getError();
        }
    }

    public static void _vertexAttribPointer(int index, int size, int type, boolean normalized, int stride, long value) {
        RenderSystem.assertOnRenderThread();
        GL20.glVertexAttribPointer(index, size, type, normalized, stride, value);
    }

    public static void _vertexAttribIPointer(int index, int size, int type, int stride, long value) {
        RenderSystem.assertOnRenderThread();
        GL30.glVertexAttribIPointer(index, size, type, stride, value);
    }

    public static void _enableVertexAttribArray(int index) {
        RenderSystem.assertOnRenderThread();
        GL20.glEnableVertexAttribArray(index);
    }

    public static void _drawElements(int mode, int count, int type, long indices) {
        RenderSystem.assertOnRenderThread();
        GL11.glDrawElements(mode, count, type, indices);
    }

    public static void _drawArrays(int mode, int first, int count) {
        RenderSystem.assertOnRenderThread();
        GL11.glDrawArrays(mode, first, count);
    }

    public static void _pixelStore(int name, int value) {
        RenderSystem.assertOnRenderThread();
        GL11.glPixelStorei(name, value);
    }

    public static void _readPixels(int x, int y, int width, int height, int format, int type, long pixels) {
        RenderSystem.assertOnRenderThread();
        GL11.glReadPixels(x, y, width, height, format, type, pixels);
    }

    public static int _getError() {
        RenderSystem.assertOnRenderThread();
        return GL11.glGetError();
    }

    public static void clearGlErrors() {
        RenderSystem.assertOnRenderThread();

        while (GL11.glGetError() != 0) {
        }
    }

    public static String _getString(int id) {
        RenderSystem.assertOnRenderThread();
        return GL11.glGetString(id);
    }

    public static int _getInteger(int name) {
        RenderSystem.assertOnRenderThread();
        return GL11.glGetInteger(name);
    }

    public static long _glFenceSync(int condition, int flags) {
        RenderSystem.assertOnRenderThread();
        return GL32.glFenceSync(condition, flags);
    }

    public static int _glClientWaitSync(long sync, int flags, long timeout) {
        RenderSystem.assertOnRenderThread();
        return GL32.glClientWaitSync(sync, flags, timeout);
    }

    public static void _glDeleteSync(long sync) {
        RenderSystem.assertOnRenderThread();
        GL32.glDeleteSync(sync);
    }

    @OnlyIn(Dist.CLIENT)
    private static class BlendState {
        public final GlStateManager.BooleanState mode = new GlStateManager.BooleanState(3042);
        public int srcRgb = 1;
        public int dstRgb = 0;
        public int srcAlpha = 1;
        public int dstAlpha = 0;
    }

    @OnlyIn(Dist.CLIENT)
    private static class BooleanState {
        private final int state;
        private boolean enabled;

        public BooleanState(int state) {
            this.state = state;
        }

        public void disable() {
            this.setEnabled(false);
        }

        public void enable() {
            this.setEnabled(true);
        }

        public void setEnabled(boolean enabled) {
            RenderSystem.assertOnRenderThread();
            if (enabled != this.enabled) {
                this.enabled = enabled;
                if (enabled) {
                    GL11.glEnable(this.state);
                } else {
                    GL11.glDisable(this.state);
                }
            }
        }
    }

    @OnlyIn(Dist.CLIENT)
    private static class ColorLogicState {
        public final GlStateManager.BooleanState enable = new GlStateManager.BooleanState(3058);
        public int op = 5379;
    }

    @OnlyIn(Dist.CLIENT)
    private static class CullState {
        public final GlStateManager.BooleanState enable = new GlStateManager.BooleanState(2884);
    }

    @OnlyIn(Dist.CLIENT)
    private static class DepthState {
        public final GlStateManager.BooleanState mode = new GlStateManager.BooleanState(2929);
        public boolean mask = true;
        public int func = 513;
    }

    @OnlyIn(Dist.CLIENT)
    private static class PolygonOffsetState {
        public final GlStateManager.BooleanState fill = new GlStateManager.BooleanState(32823);
        public float factor;
        public float units;
    }

    @OnlyIn(Dist.CLIENT)
    private static class ScissorState {
        public final GlStateManager.BooleanState mode = new GlStateManager.BooleanState(3089);
    }

    @OnlyIn(Dist.CLIENT)
    private static class TextureState {
        public int binding;
    }

    public static void _disableStencilTest() {
        RenderSystem.assertOnRenderThread();
        STENCIL.mode.disable();
    }

    public static void _enableStencilTest() {
        RenderSystem.assertOnRenderThread();
        STENCIL.mode.enable();
    }

    public static void _stencilFunc(int func, int ref, int readMask) {
        RenderSystem.assertOnRenderThread();
        if (func != STENCIL.frontFunc || ref != STENCIL.frontRef || readMask != STENCIL.frontReadMask
                || func != STENCIL.backFunc || ref != STENCIL.backRef || readMask != STENCIL.backReadMask) {
            STENCIL.frontFunc = func;
            STENCIL.frontRef = ref;
            STENCIL.frontReadMask = readMask;
            STENCIL.backFunc = func;
            STENCIL.backRef = ref;
            STENCIL.backReadMask = readMask;
            GL32.glStencilFuncSeparate(GL32.GL_FRONT_AND_BACK, func, ref, readMask);
        }
    }

    public static void _stencilFuncFront(int func, int ref, int readMask) {
        RenderSystem.assertOnRenderThread();
        if (func != STENCIL.frontFunc || ref != STENCIL.frontRef || readMask != STENCIL.frontReadMask) {
            STENCIL.frontFunc = func;
            STENCIL.frontRef = ref;
            STENCIL.frontReadMask = readMask;
            GL32.glStencilFuncSeparate(GL32.GL_FRONT, func, ref, readMask);
        }
    }

    public static void _stencilFuncBack(int func, int ref, int readMask) {
        RenderSystem.assertOnRenderThread();
        if (func != STENCIL.backFunc || ref != STENCIL.backRef || readMask != STENCIL.backReadMask) {
            STENCIL.backFunc = func;
            STENCIL.backRef = ref;
            STENCIL.backReadMask = readMask;
            GL32.glStencilFuncSeparate(GL32.GL_BACK, func, ref, readMask);
        }
    }

    public static void _stencilMask(int mask) {
        RenderSystem.assertOnRenderThread();
        if (mask != STENCIL.writeMask) {
            STENCIL.writeMask = mask;
            GL11.glStencilMask(mask);
        }
    }

    /**
     * @param stencilFail  The action to take if the stencil test fails.
     * @param depthFail The action to take if the depth buffer test fails.
     * @param pass The action to take if both tests pass.
     */
    public static void _stencilOp(int stencilFail, int depthFail, int pass) {
        RenderSystem.assertOnRenderThread();
        if (stencilFail != STENCIL.frontStencilFail || depthFail != STENCIL.frontDepthFail || pass != STENCIL.frontPass
                || stencilFail != STENCIL.backStencilFail || depthFail != STENCIL.backDepthFail || pass != STENCIL.backPass) {
            STENCIL.frontStencilFail = stencilFail;
            STENCIL.frontDepthFail = depthFail;
            STENCIL.frontPass= pass;
            STENCIL.backStencilFail = stencilFail;
            STENCIL.backDepthFail = depthFail;
            STENCIL.backPass = pass;
            GL32.glStencilOpSeparate(GL32.GL_FRONT_AND_BACK, stencilFail, depthFail, pass);
        }
    }

    /**
     * Same as {@link #_stencilOp}, but affects only front-faces.
     */
    public static void _stencilOpFront(int stencilFail, int depthFail, int pass) {
        RenderSystem.assertOnRenderThread();
        if (stencilFail != STENCIL.frontStencilFail || depthFail != STENCIL.frontDepthFail || pass != STENCIL.frontPass) {
            STENCIL.frontStencilFail = stencilFail;
            STENCIL.frontDepthFail = depthFail;
            STENCIL.frontPass= pass;
            GL32.glStencilOpSeparate(GL32.GL_FRONT, stencilFail, depthFail, pass);
        }
    }

    /**
     * Same as {@link #_stencilOp}, but affects only back-faces.
     */
    public static void _stencilOpBack(int stencilFail, int depthFail, int pass) {
        RenderSystem.assertOnRenderThread();
        if (stencilFail != STENCIL.backStencilFail || depthFail != STENCIL.backDepthFail || pass != STENCIL.backPass) {
            STENCIL.backStencilFail = stencilFail;
            STENCIL.backDepthFail = depthFail;
            STENCIL.backPass = pass;
            GL32.glStencilOpSeparate(GL32.GL_BACK, stencilFail, depthFail, pass);
        }
    }

    @OnlyIn(Dist.CLIENT)
    static class StencilState {
        public final GlStateManager.BooleanState mode = new GlStateManager.BooleanState(GL11.GL_STENCIL_TEST);
        public int frontFunc = GL11.GL_ALWAYS;
        public int frontRef;
        public int frontReadMask = -1;
        public int backFunc = GL11.GL_ALWAYS;
        public int backRef;
        public int backReadMask = -1;
        public int writeMask = -1;
        public int frontStencilFail = GL11.GL_KEEP;
        public int frontDepthFail = GL11.GL_KEEP;
        public int frontPass = GL11.GL_KEEP;
        public int backStencilFail = GL11.GL_KEEP;
        public int backDepthFail = GL11.GL_KEEP;
        public int backPass = GL11.GL_KEEP;
     }
}
