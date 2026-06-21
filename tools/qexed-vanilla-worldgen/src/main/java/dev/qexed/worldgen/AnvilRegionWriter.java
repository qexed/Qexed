package dev.qexed.worldgen;

import java.io.ByteArrayOutputStream;
import java.io.ByteArrayInputStream;
import java.io.DataInputStream;
import java.io.RandomAccessFile;
import java.nio.ByteBuffer;
import java.nio.file.Path;
import java.nio.file.Files;
import java.util.zip.InflaterInputStream;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.nbt.NbtAccounter;
import net.minecraft.nbt.NbtIo;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.storage.RegionFileStorage;
import net.minecraft.world.level.chunk.storage.RegionStorageInfo;

final class AnvilRegionWriter {
    private static final int SECTOR_SIZE = 4096;
    private static final int HEADER_SIZE = SECTOR_SIZE * 2;
    private static final int COMPRESSION_ZLIB = 2;

    private AnvilRegionWriter() {}

    static byte[] readCompressedChunk(Path path, int chunkX, int chunkZ) throws Exception {
        if (!Files.exists(path)) {
            return null;
        }

        try (RandomAccessFile file = new RandomAccessFile(path.toFile(), "r")) {
            if (file.length() < HEADER_SIZE) {
                return null;
            }

            int index = chunkIndex(chunkX, chunkZ);
            file.seek(index * 4L);
            int offset = (file.readUnsignedByte() << 16)
                    | (file.readUnsignedByte() << 8)
                    | file.readUnsignedByte();
            int sectorCount = file.readUnsignedByte();
            if (offset == 0 || sectorCount == 0) {
                return null;
            }

            long chunkOffset = (long) offset * SECTOR_SIZE;
            if (chunkOffset + 5 > file.length()) {
                return null;
            }

            file.seek(chunkOffset);
            int length = file.readInt();
            int compression = file.readUnsignedByte();
            if (length <= 1 || chunkOffset + 4L + length > file.length()) {
                return null;
            }
            if (compression != COMPRESSION_ZLIB) {
                throw new IllegalStateException("unsupported vanilla chunk compression: " + compression);
            }

            byte[] compressed = new byte[length - 1];
            file.readFully(compressed);
            return compressed;
        }
    }

    static String readChunkStatus(Path path, int chunkX, int chunkZ) throws Exception {
        byte[] compressed = readCompressedChunk(path, chunkX, chunkZ);
        if (compressed == null) {
            return null;
        }

        try (DataInputStream input = new DataInputStream(
                new InflaterInputStream(new ByteArrayInputStream(compressed)))) {
            CompoundTag root = NbtIo.read(input, NbtAccounter.unlimitedHeap());
            if (root == null || !root.contains("Status")) {
                return null;
            }
            return root.getString("Status").orElse(null);
        }
    }

    static void writeChunk(Path path, int chunkX, int chunkZ, byte[] compressedNbt) throws Exception {
        Path parent = path.getParent();
        if (parent != null) {
            Files.createDirectories(parent);
        }

        try (RandomAccessFile file = new RandomAccessFile(path.toFile(), "rw")) {
            if (file.length() < HEADER_SIZE) {
                file.setLength(HEADER_SIZE);
            }

            byte[] payload = payload(compressedNbt);
            int sectorCount = (payload.length + SECTOR_SIZE - 1) / SECTOR_SIZE;
            if (sectorCount > 255) {
                throw new IllegalArgumentException("chunk payload too large for inline MCA storage");
            }

            long alignedLength = align(file.length());
            if (file.length() != alignedLength) {
                file.setLength(alignedLength);
            }
            int sectorOffset = Math.toIntExact(alignedLength / SECTOR_SIZE);
            file.seek(alignedLength);
            file.write(payload);
            long paddedEnd = alignedLength + (long) sectorCount * SECTOR_SIZE;
            if (file.length() < paddedEnd) {
                file.setLength(paddedEnd);
            }

            int index = chunkIndex(chunkX, chunkZ);
            file.seek(index * 4L);
            file.write((sectorOffset >>> 16) & 0xff);
            file.write((sectorOffset >>> 8) & 0xff);
            file.write(sectorOffset & 0xff);
            file.write(sectorCount);

            file.seek(SECTOR_SIZE + index * 4L);
            file.writeInt((int) Math.min(System.currentTimeMillis() / 1000L, Integer.MAX_VALUE));
        }
    }

    static void writeChunk(Path path, int chunkX, int chunkZ, CompoundTag chunk) throws Exception {
        Path parent = path.getParent();
        if (parent != null) {
            Files.createDirectories(parent);
        }

        ResourceKey<Level> dimension = ResourceKey.create(
                net.minecraft.core.registries.Registries.DIMENSION,
                Identifier.parse("minecraft:overworld"));
        RegionStorageInfo info = new RegionStorageInfo("qexed", dimension, "chunk");
        try (RegionFileStorage storage = new RegionFileStorage(info, parent, false)) {
            storage.write(new ChunkPos(chunkX, chunkZ), chunk);
            storage.flush();
        }
    }

    private static byte[] payload(byte[] compressedNbt) throws Exception {
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        output.write(ByteBuffer.allocate(4).putInt(compressedNbt.length + 1).array());
        output.write(COMPRESSION_ZLIB);
        output.write(compressedNbt);
        return output.toByteArray();
    }

    private static long align(long length) {
        return ((length + SECTOR_SIZE - 1) / SECTOR_SIZE) * SECTOR_SIZE;
    }

    private static int chunkIndex(int chunkX, int chunkZ) {
        return Math.floorMod(chunkX, 32) + Math.floorMod(chunkZ, 32) * 32;
    }
}
