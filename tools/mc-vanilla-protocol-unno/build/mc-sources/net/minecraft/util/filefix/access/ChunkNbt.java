package net.minecraft.util.filefix.access;

import java.io.IOException;
import java.nio.file.Path;
import java.util.concurrent.CompletableFuture;
import java.util.function.UnaryOperator;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.util.datafix.DataFixTypes;
import net.minecraft.util.datafix.DataFixers;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.chunk.storage.RegionStorageInfo;
import net.minecraft.world.level.chunk.storage.SimpleRegionStorage;

public class ChunkNbt implements AutoCloseable {
    private final SimpleRegionStorage storage;
    private final int targetVersion;

    public ChunkNbt(RegionStorageInfo info, Path path, DataFixTypes type, int targetVersion) {
        this.targetVersion = targetVersion;
        this.storage = new SimpleRegionStorage(info, path, DataFixers.getDataFixer(), false, type);
    }

    public void updateChunk(ChunkPos pos, CompoundTag dataFixContext, UnaryOperator<CompoundTag> fixer) {
        this.storage
            .read(pos)
            .thenApply(tag -> tag.<CompoundTag>map(tag1 -> this.storage.upgradeChunkTag(tag1, -1, dataFixContext, this.targetVersion)).map(fixer))
            .thenCompose(value -> value.<CompletableFuture<Void>>map(tag2 -> this.storage.write(pos, tag2)).orElse(CompletableFuture.completedFuture(null)))
            .join();
    }

    @Override
    public void close() throws IOException {
        this.storage.close();
    }
}
