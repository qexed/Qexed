package net.minecraft.world.level.levelgen.structure.templatesystem;

import com.mojang.brigadier.exceptions.CommandSyntaxException;
import com.mojang.logging.LogUtils;
import com.mojang.serialization.MapCodec;
import net.minecraft.SharedConstants;
import net.minecraft.commands.arguments.blocks.BlockStateParser;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.state.BlockState;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

public class JigsawReplacementProcessor extends StructureProcessor {
    private static final Logger LOGGER = LogUtils.getLogger();
    public static final MapCodec<JigsawReplacementProcessor> CODEC = MapCodec.unit(() -> JigsawReplacementProcessor.INSTANCE);
    public static final JigsawReplacementProcessor INSTANCE = new JigsawReplacementProcessor();

    private JigsawReplacementProcessor() {
    }

    @Override
    public StructureTemplate.@Nullable StructureBlockInfo processBlock(
        LevelReader level,
        BlockPos targetPosition,
        BlockPos referencePos,
        StructureTemplate.StructureBlockInfo originalBlockInfo,
        StructureTemplate.StructureBlockInfo processedBlockInfo,
        StructurePlaceSettings settings
    ) {
        BlockState state = processedBlockInfo.state();
        if (!state.is(Blocks.JIGSAW) || SharedConstants.DEBUG_KEEP_JIGSAW_BLOCKS_DURING_STRUCTURE_GEN) {
            return processedBlockInfo;
        } else if (processedBlockInfo.nbt() == null) {
            LOGGER.warn("Jigsaw block at {} is missing nbt, will not replace", targetPosition);
            return processedBlockInfo;
        } else {
            String stateString = processedBlockInfo.nbt().getStringOr("final_state", "minecraft:air");

            BlockState blockState;
            try {
                BlockStateParser.BlockResult result = BlockStateParser.parseForBlock(level.holderLookup(Registries.BLOCK), stateString, true);
                blockState = result.blockState();
            } catch (CommandSyntaxException var11) {
                LOGGER.error("Failed to parse jigsaw replacement state '{}' at {}: {}", stateString, targetPosition, var11.getMessage());
                return null;
            }

            return blockState.is(Blocks.STRUCTURE_VOID) ? null : new StructureTemplate.StructureBlockInfo(processedBlockInfo.pos(), blockState, null);
        }
    }

    @Override
    protected StructureProcessorType<?> getType() {
        return StructureProcessorType.JIGSAW_REPLACEMENT;
    }
}
