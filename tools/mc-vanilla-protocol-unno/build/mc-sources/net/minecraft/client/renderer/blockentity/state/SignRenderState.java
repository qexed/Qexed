package net.minecraft.client.renderer.blockentity.state;

import com.mojang.math.Transformation;
import net.minecraft.world.level.block.entity.SignText;
import net.minecraft.world.level.block.state.properties.WoodType;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class SignRenderState extends BlockEntityRenderState {
    public WoodType woodType = WoodType.OAK;
    public @Nullable SignText frontText;
    public @Nullable SignText backText;
    public int textLineHeight;
    public int maxTextLineWidth;
    public boolean isTextFilteringEnabled;
    public boolean drawOutline;
    public SignRenderState.SignTransformations transformations = SignRenderState.SignTransformations.IDENTITY;

    @OnlyIn(Dist.CLIENT)
    public record SignTransformations(Transformation body, Transformation frontText, Transformation backText) {
        public static final SignRenderState.SignTransformations IDENTITY = new SignRenderState.SignTransformations(
            Transformation.IDENTITY, Transformation.IDENTITY, Transformation.IDENTITY
        );
    }
}
