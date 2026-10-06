package com.mojang.blaze3d.platform;

import com.mojang.blaze3d.GLFWErrorScope;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import net.minecraft.util.StringDecomposer;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.lwjgl.BufferUtils;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.glfw.GLFWErrorCallbackI;
import org.lwjgl.system.MemoryUtil;

@OnlyIn(Dist.CLIENT)
public class ClipboardManager {
    public static final int FORMAT_UNAVAILABLE = 65545;
    private final ByteBuffer clipboardScratchBuffer = BufferUtils.createByteBuffer(8192);

    public String getClipboard(Window window, GLFWErrorCallbackI errorCallback) {
        String var5;
        try (GLFWErrorScope ignored = new GLFWErrorScope(errorCallback)) {
            String clipboard = GLFW.glfwGetClipboardString(window.handle());
            clipboard = clipboard != null ? StringDecomposer.filterBrokenSurrogates(clipboard) : "";
            var5 = clipboard;
        }

        return var5;
    }

    private static void pushClipboard(Window window, ByteBuffer buffer, byte[] data) {
        buffer.clear();
        buffer.put(data);
        buffer.put((byte)0);
        buffer.flip();
        GLFW.glfwSetClipboardString(window.handle(), buffer);
    }

    public void setClipboard(Window window, String clipboard) {
        byte[] encoded = clipboard.getBytes(StandardCharsets.UTF_8);
        int encodedLength = encoded.length + 1;
        if (encodedLength < this.clipboardScratchBuffer.capacity()) {
            pushClipboard(window, this.clipboardScratchBuffer, encoded);
        } else {
            ByteBuffer buffer = MemoryUtil.memAlloc(encodedLength);

            try {
                pushClipboard(window, buffer, encoded);
            } finally {
                MemoryUtil.memFree(buffer);
            }
        }
    }
}
