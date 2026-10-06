package net.minecraft.world.level.levelgen.structure.templatesystem;

import com.google.common.collect.ImmutableList;
import com.mojang.serialization.MapCodec;
import java.util.List;
import net.minecraft.core.BlockPos;
import net.minecraft.util.Mth;
import net.minecraft.util.RandomSource;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.block.state.BlockState;
import org.jspecify.annotations.Nullable;

public class RuleProcessor extends StructureProcessor {
    public static final MapCodec<RuleProcessor> CODEC = ProcessorRule.CODEC.listOf().fieldOf("rules").xmap(RuleProcessor::new, p -> p.rules);
    private final ImmutableList<ProcessorRule> rules;

    public RuleProcessor(List<? extends ProcessorRule> rules) {
        this.rules = ImmutableList.copyOf(rules);
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
        RandomSource random = RandomSource.create(Mth.getSeed(processedBlockInfo.pos()));
        BlockState locState = level.getBlockState(processedBlockInfo.pos());

        for (ProcessorRule rule : this.rules) {
            if (rule.test(processedBlockInfo.state(), locState, originalBlockInfo.pos(), processedBlockInfo.pos(), referencePos, random)) {
                return new StructureTemplate.StructureBlockInfo(
                    processedBlockInfo.pos(), rule.getOutputState(), rule.getOutputTag(random, processedBlockInfo.nbt())
                );
            }
        }

        return processedBlockInfo;
    }

    @Override
    protected StructureProcessorType<?> getType() {
        return StructureProcessorType.RULE;
    }
}
