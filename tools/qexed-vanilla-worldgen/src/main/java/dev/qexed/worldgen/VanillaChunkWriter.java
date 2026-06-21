package dev.qexed.worldgen;

public interface VanillaChunkWriter {
    GenerateChunkResult generateChunk(GenerateChunkRequest request) throws Exception;
}
