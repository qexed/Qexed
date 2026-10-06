package com.mojang.blaze3d.vertex;

import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class Tesselator {
    private static final int MAX_BYTES = 786432;
    private final ByteBufferBuilder buffer;
    private static @Nullable Tesselator instance;

    public static void init() {
        if (instance != null) {
            throw new IllegalStateException("Tesselator has already been initialized");
        } else {
            instance = new Tesselator();
        }
    }

    public static Tesselator getInstance() {
        if (instance == null) {
            throw new IllegalStateException("Tesselator has not been initialized");
        } else {
            return instance;
        }
    }

    public Tesselator(int size) {
        this.buffer = new ByteBufferBuilder(size);
    }

    public Tesselator() {
        this(786432);
    }

    public BufferBuilder begin(VertexFormat.Mode mode, VertexFormat format) {
        return new BufferBuilder(this.buffer, mode, format);
    }

    public void clear() {
        this.buffer.clear();
    }
}
