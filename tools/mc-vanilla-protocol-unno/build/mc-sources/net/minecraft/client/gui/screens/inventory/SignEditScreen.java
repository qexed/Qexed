package net.minecraft.client.gui.screens.inventory;

import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.model.Model;
import net.minecraft.client.renderer.blockentity.StandingSignRenderer;
import net.minecraft.world.level.block.PlainSignBlock;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.joml.Vector3f;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class SignEditScreen extends AbstractSignEditScreen {
    public static final float MAGIC_SCALE_NUMBER = 62.500004F;
    public static final float MAGIC_TEXT_SCALE = 0.9765628F;
    private static final Vector3f TEXT_SCALE = new Vector3f(0.9765628F, 0.9765628F, 0.9765628F);
    private Model.@Nullable Simple signModel;

    public SignEditScreen(SignBlockEntity sign, boolean isFrontText, boolean shouldFilter) {
        super(sign, isFrontText, shouldFilter);
    }

    @Override
    protected void init() {
        super.init();
        PlainSignBlock.Attachment attachment = PlainSignBlock.getAttachmentPoint(this.sign.getBlockState());
        this.signModel = StandingSignRenderer.createSignModel(this.minecraft.getEntityModels(), this.woodType, attachment);
    }

    @Override
    protected float getSignYOffset() {
        return 90.0F;
    }

    @Override
    protected void extractSignBackground(GuiGraphicsExtractor graphics) {
        if (this.signModel != null) {
            int centerX = this.width / 2;
            int x0 = centerX - 48;
            int y0 = 66;
            int x1 = centerX + 48;
            int y1 = 168;
            graphics.sign(this.signModel, 62.500004F, this.woodType, x0, 66, x1, 168);
        }
    }

    @Override
    protected Vector3f getSignTextScale() {
        return TEXT_SCALE;
    }
}
