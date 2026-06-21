package dev.qexed.worldgen;

public record GenerateChunkRequest(
        RpcDimension dimension,
        int chunkX,
        int chunkZ,
        String saveRoot,
        String dimensionRoot,
        String regionPath) {}
