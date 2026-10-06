package net.minecraft.world.level.block;

import com.mojang.serialization.MapCodec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import java.util.Map;
import java.util.Optional;
import java.util.function.Supplier;
import net.minecraft.core.BlockPos;
import net.minecraft.core.Direction;
import net.minecraft.core.registries.BuiltInRegistries;
import net.minecraft.sounds.SoundEvent;
import net.minecraft.tags.BlockTags;
import net.minecraft.util.RandomSource;
import net.minecraft.world.item.HoneycombItem;
import net.minecraft.world.item.context.BlockPlaceContext;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.LevelReader;
import net.minecraft.world.level.ScheduledTickAccess;
import net.minecraft.world.level.block.entity.BlockEntityType;
import net.minecraft.world.level.block.state.BlockBehaviour;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.ChestType;

public class CopperChestBlock extends ChestBlock {
    public static final MapCodec<CopperChestBlock> CODEC = RecordCodecBuilder.mapCodec(
        i -> i.group(
                WeatheringCopper.WeatherState.CODEC.fieldOf("weathering_state").forGetter(CopperChestBlock::getState),
                BuiltInRegistries.SOUND_EVENT.byNameCodec().fieldOf("open_sound").forGetter(ChestBlock::getOpenChestSound),
                BuiltInRegistries.SOUND_EVENT.byNameCodec().fieldOf("close_sound").forGetter(ChestBlock::getCloseChestSound),
                propertiesCodec()
            )
            .apply(i, CopperChestBlock::new)
    );
    private static final Map<Block, Supplier<Block>> COPPER_TO_COPPER_CHEST_MAPPING = Map.of(
        Blocks.COPPER_BLOCK,
        () -> Blocks.COPPER_CHEST,
        Blocks.EXPOSED_COPPER,
        () -> Blocks.EXPOSED_COPPER_CHEST,
        Blocks.WEATHERED_COPPER,
        () -> Blocks.WEATHERED_COPPER_CHEST,
        Blocks.OXIDIZED_COPPER,
        () -> Blocks.OXIDIZED_COPPER_CHEST,
        Blocks.WAXED_COPPER_BLOCK,
        () -> Blocks.COPPER_CHEST,
        Blocks.WAXED_EXPOSED_COPPER,
        () -> Blocks.EXPOSED_COPPER_CHEST,
        Blocks.WAXED_WEATHERED_COPPER,
        () -> Blocks.WEATHERED_COPPER_CHEST,
        Blocks.WAXED_OXIDIZED_COPPER,
        () -> Blocks.OXIDIZED_COPPER_CHEST
    );
    private final WeatheringCopper.WeatherState weatherState;

    @Override
    public MapCodec<? extends CopperChestBlock> codec() {
        return CODEC;
    }

    public CopperChestBlock(WeatheringCopper.WeatherState weatherState, SoundEvent openSound, SoundEvent closeSound, BlockBehaviour.Properties properties) {
        super(() -> BlockEntityType.CHEST, openSound, closeSound, properties);
        this.weatherState = weatherState;
    }

    @Override
    public boolean chestCanConnectTo(BlockState blockState) {
        return blockState.is(BlockTags.COPPER_CHESTS) && blockState.hasProperty(ChestBlock.TYPE);
    }

    @Override
    public BlockState getStateForPlacement(BlockPlaceContext context) {
        BlockState state = super.getStateForPlacement(context);
        return getLeastOxidizedChestOfConnectedBlocks(state, context.getLevel(), context.getClickedPos());
    }

    private static BlockState getLeastOxidizedChestOfConnectedBlocks(BlockState state, Level level, BlockPos pos) {
        BlockState connectedState = level.getBlockState(pos.relative(getConnectedDirection(state)));
        if (!state.getValue(ChestBlock.TYPE).equals(ChestType.SINGLE)
            && state.getBlock() instanceof CopperChestBlock copperChestBlock
            && connectedState.getBlock() instanceof CopperChestBlock connectedCopperChestBlock) {
            BlockState updatedBlockState = state;
            BlockState connectedPredictedBlockState = connectedState;
            if (copperChestBlock.isWaxed() != connectedCopperChestBlock.isWaxed()) {
                updatedBlockState = unwaxBlock(copperChestBlock, state).orElse(state);
                connectedPredictedBlockState = unwaxBlock(connectedCopperChestBlock, connectedState).orElse(connectedState);
            }

            Block leastOxidizedBlock = copperChestBlock.weatherState.ordinal() <= connectedCopperChestBlock.weatherState.ordinal()
                ? updatedBlockState.getBlock()
                : connectedPredictedBlockState.getBlock();
            return leastOxidizedBlock.withPropertiesOf(updatedBlockState);
        } else {
            return state;
        }
    }

    @Override
    protected BlockState updateShape(
        BlockState state,
        LevelReader level,
        ScheduledTickAccess ticks,
        BlockPos pos,
        Direction directionToNeighbour,
        BlockPos neighbourPos,
        BlockState neighbourState,
        RandomSource random
    ) {
        BlockState blockState = super.updateShape(state, level, ticks, pos, directionToNeighbour, neighbourPos, neighbourState, random);
        if (this.chestCanConnectTo(neighbourState)) {
            ChestType chestType = blockState.getValue(ChestBlock.TYPE);
            if (!chestType.equals(ChestType.SINGLE) && getConnectedDirection(blockState) == directionToNeighbour) {
                return neighbourState.getBlock().withPropertiesOf(blockState);
            }
        }

        return blockState;
    }

    private static Optional<BlockState> unwaxBlock(CopperChestBlock copperChestBlock, BlockState state) {
        return !copperChestBlock.isWaxed()
            ? Optional.of(state)
            : Optional.ofNullable(HoneycombItem.WAX_OFF_BY_BLOCK.get().get(state.getBlock())).map(b -> b.withPropertiesOf(state));
    }

    public WeatheringCopper.WeatherState getState() {
        return this.weatherState;
    }

    public static BlockState getFromCopperBlock(Block copperBlock, Direction facing, Level level, BlockPos pos) {
        CopperChestBlock block = (CopperChestBlock)COPPER_TO_COPPER_CHEST_MAPPING.getOrDefault(copperBlock, Blocks.COPPER_CHEST::asBlock).get();
        ChestType chestType = block.getChestType(level, pos, facing);
        BlockState state = block.defaultBlockState().setValue(FACING, facing).setValue(TYPE, chestType);
        return getLeastOxidizedChestOfConnectedBlocks(state, level, pos);
    }

    public boolean isWaxed() {
        return true;
    }

    @Override
    public boolean shouldChangedStateKeepBlockEntity(BlockState oldState) {
        return oldState.is(BlockTags.COPPER_CHESTS);
    }
}
