package dev.qexed.worldgen;

import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Optional;
import java.util.zip.InflaterInputStream;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.ListTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;

public final class TargetBlockWriteTraceDiagnostic {
    private TargetBlockWriteTraceDiagnostic() {
    }

    public static void main(String[] args) throws Exception {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 0L;
        int chunkX = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int chunkZ = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        int targetX = args.length > 3 ? Integer.parseInt(args[3]) : 6;
        int targetY = args.length > 4 ? Integer.parseInt(args[4]) : -61;
        int targetZ = args.length > 5 ? Integer.parseInt(args[5]) : 0;
        Path outputRoot = Files.createTempDirectory("qexed-target-block-trace-");
        Path regionPath = outputRoot.resolve("region").resolve("r.0.0.mca");
        Files.createDirectories(regionPath.getParent());

        try (MinecraftServerChunkWriter writer = new MinecraftServerChunkWriter(seed)) {
            writer.generateChunk(new GenerateChunkRequest(
                    new RpcDimension("minecraft", "overworld"),
                    chunkX,
                    chunkZ,
                    outputRoot.resolve("world").toString(),
                    outputRoot.resolve("world/dimensions/minecraft/overworld").toString(),
                    regionPath.toString()));
        }
        Optional<String> finalBlock = readBlockState(regionPath, chunkX, chunkZ, targetX, targetY, targetZ);
        System.out.printf("java target write trace diagnostic done seed=%d chunk=(%d,%d) region=%s%n",
                seed,
                chunkX,
                chunkZ,
                regionPath);
        System.out.printf("java target write trace final coord=(%d,%d,%d) block=%s%n",
                targetX,
                targetY,
                targetZ,
                finalBlock.orElse("missing"));
    }

    private static Optional<String> readBlockState(Path regionPath, int chunkX, int chunkZ, int x, int y, int z) throws Exception {
        byte[] compressedChunk = AnvilRegionWriter.readCompressedChunk(regionPath, chunkX, chunkZ);
        if (compressedChunk == null) {
            return Optional.empty();
        }
        try (DataInputStream input = new DataInputStream(
                new InflaterInputStream(new ByteArrayInputStream(compressedChunk)))) {
            CompoundTag root = NbtIo.read(input, NbtAccounter.unlimitedHeap());
            if (root == null) {
                return Optional.empty();
            }
            ListTag sections = root.getListOrEmpty("sections");
            int sectionY = Math.floorDiv(y, 16);
            for (int sectionIndex = 0; sectionIndex < sections.size(); sectionIndex++) {
                Optional<CompoundTag> section = sections.getCompound(sectionIndex);
                if (section.isEmpty() || section.get().getByteOr("Y", (byte) 0) != (byte) sectionY) {
                    continue;
                }
                CompoundTag blockStates = section.get().getCompoundOrEmpty("block_states");
                ListTag palette = blockStates.getListOrEmpty("palette");
                if (palette.isEmpty()) {
                    return Optional.empty();
                }
                int paletteIndex = paletteIndex(blockStates, x, y, z);
                if (paletteIndex < 0 || paletteIndex >= palette.size()) {
                    return Optional.empty();
                }
                return palette.getCompound(paletteIndex).flatMap(state -> state.getString("Name"));
            }
            return Optional.empty();
        }
    }

    private static int paletteIndex(CompoundTag blockStates, int x, int y, int z) {
        long[] data = blockStates.getLongArray("data").orElse(null);
        if (data == null || data.length == 0) {
            return 0;
        }
        int index = (y & 15) * 256 + (z & 15) * 16 + (x & 15);
        int paletteSize = blockStates.getListOrEmpty("palette").size();
        int bits = Math.max(4, 32 - Integer.numberOfLeadingZeros(paletteSize - 1));
        int perLong = 64 / bits;
        int dataIndex = index / perLong;
        int bitOffset = (index - dataIndex * perLong) * bits;
        long mask = (1L << bits) - 1L;
        return (int) ((data[dataIndex] >>> bitOffset) & mask);
    }
}
