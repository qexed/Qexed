package net.minecraft.world.level.levelgen.structure.templatesystem;

import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ServerLevelAccessor;
import org.jspecify.annotations.Nullable;

public abstract class StructureProcessor {
    /**
     * @deprecated Forge: Use {@link #process(LevelReader, BlockPos, BlockPos, StructureTemplate.StructureBlockInfo, StructureTemplate.StructureBlockInfo, StructurePlaceSettings, StructureTemplate)} instead.
     */
    @Deprecated
    public StructureTemplate.@Nullable StructureBlockInfo processBlock(
        LevelReader level,
        BlockPos targetPosition,
        BlockPos referencePos,
        StructureTemplate.StructureBlockInfo originalBlockInfo,
        StructureTemplate.StructureBlockInfo processedBlockInfo,
        StructurePlaceSettings settings
    ) {
        return processedBlockInfo;
    }

    protected abstract StructureProcessorType<?> getType();

    public List<StructureTemplate.StructureBlockInfo> finalizeProcessing(
        ServerLevelAccessor level,
        BlockPos position,
        BlockPos referencePos,
        List<StructureTemplate.StructureBlockInfo> originalBlockInfoList,
        List<StructureTemplate.StructureBlockInfo> processedBlockInfoList,
        StructurePlaceSettings settings
    ) {
        return processedBlockInfoList;
    }

    public StructureTemplate.@Nullable StructureBlockInfo process(LevelReader level, BlockPos targetPosition, BlockPos referencePos, StructureTemplate.StructureBlockInfo originalBlockInfo, StructureTemplate.StructureBlockInfo processedBlockInfo, StructurePlaceSettings settings, @Nullable StructureTemplate template) {
        return processBlock(level, targetPosition, referencePos, originalBlockInfo, processedBlockInfo, settings);
    }
    /**
     * FORGE: Add entity processing.
     * <p>
     * Use this method to process entities from a structure in much the same way as
     * blocks, parameters are analogous.
     *
     * @param world
     * @param seedPos
     * @param rawEntityInfo
     * @param entityInfo
     * @param placementSettings
     * @param template
     *
     * @see #process(LevelReader, BlockPos, BlockPos, StructureTemplate.StructureBlockInfo, StructureTemplate.StructureBlockInfo, StructurePlaceSettings, StructureTemplate)
     */
    public StructureTemplate.StructureEntityInfo processEntity(LevelReader world, BlockPos seedPos, StructureTemplate.StructureEntityInfo rawEntityInfo, StructureTemplate.StructureEntityInfo entityInfo, StructurePlaceSettings placementSettings, StructureTemplate template) {
        return entityInfo;
    }
}
