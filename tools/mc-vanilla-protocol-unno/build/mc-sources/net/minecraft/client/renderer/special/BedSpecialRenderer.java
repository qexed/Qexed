package net.minecraft.client.renderer.special;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.serialization.MapCodec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import java.util.function.Consumer;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.BedRenderer;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.resources.Identifier;
import net.minecraft.world.item.DyeColor;
import net.minecraft.world.level.block.state.properties.BedPart;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.joml.Vector3fc;

@OnlyIn(Dist.CLIENT)
public class BedSpecialRenderer implements NoDataSpecialModelRenderer {
    private final BedRenderer bedRenderer;
    private final SpriteId sprite;
    private final BedPart part;

    public BedSpecialRenderer(BedRenderer bedRenderer, SpriteId sprite, BedPart part) {
        this.bedRenderer = bedRenderer;
        this.sprite = sprite;
        this.part = part;
    }

    @Override
    public void submit(PoseStack poseStack, SubmitNodeCollector submitNodeCollector, int lightCoords, int overlayCoords, boolean hasFoil, int outlineColor) {
        this.bedRenderer.submitPiece(this.part, this.sprite, poseStack, submitNodeCollector, lightCoords, overlayCoords, null, outlineColor);
    }

    @Override
    public void getExtents(Consumer<Vector3fc> output) {
        this.bedRenderer.getExtents(this.part, output);
    }

    @OnlyIn(Dist.CLIENT)
    public record Unbaked(Identifier texture, BedPart part) implements NoDataSpecialModelRenderer.Unbaked {
        public static final MapCodec<BedSpecialRenderer.Unbaked> MAP_CODEC = RecordCodecBuilder.mapCodec(
            i -> i.group(
                    Identifier.CODEC.fieldOf("texture").forGetter(BedSpecialRenderer.Unbaked::texture),
                    BedPart.CODEC.fieldOf("part").forGetter(BedSpecialRenderer.Unbaked::part)
                )
                .apply(i, BedSpecialRenderer.Unbaked::new)
        );

        public Unbaked(DyeColor dyeColor, BedPart part) {
            this(Sheets.colorToResourceSprite(dyeColor), part);
        }

        @Override
        public MapCodec<BedSpecialRenderer.Unbaked> type() {
            return MAP_CODEC;
        }

        public BedSpecialRenderer bake(SpecialModelRenderer.BakingContext context) {
            return new BedSpecialRenderer(new BedRenderer(context), Sheets.BED_MAPPER.apply(this.texture), this.part);
        }
    }
}
