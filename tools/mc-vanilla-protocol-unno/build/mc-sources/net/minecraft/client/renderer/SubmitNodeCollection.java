package net.minecraft.client.renderer;

import com.mojang.blaze3d.vertex.PoseStack;
import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.block.MovingBlockRenderState;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.feature.CustomFeatureRenderer;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.feature.ModelPartFeatureRenderer;
import net.minecraft.client.renderer.feature.NameTagFeatureRenderer;
import net.minecraft.client.renderer.item.ItemStackRenderState;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.renderer.state.level.CameraRenderState;
import net.minecraft.client.renderer.texture.TextureAtlasSprite;
import net.minecraft.client.resources.model.geometry.BakedQuad;
import net.minecraft.network.chat.Component;
import net.minecraft.util.FormattedCharSequence;
import net.minecraft.world.item.ItemDisplayContext;
import net.minecraft.world.phys.Vec3;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.joml.Matrix4f;
import org.joml.Quaternionf;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class SubmitNodeCollection implements OrderedSubmitNodeCollector {
    private final List<SubmitNodeStorage.ShadowSubmit> shadowSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.FlameSubmit> flameSubmits = new ArrayList<>();
    private final NameTagFeatureRenderer.Storage nameTagSubmits = new NameTagFeatureRenderer.Storage();
    private final List<SubmitNodeStorage.TextSubmit> textSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.LeashSubmit> leashSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.MovingBlockSubmit> movingBlockSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.BlockModelSubmit> blockModelSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.MultiLayerBlockModelSubmit> multiLayerBlockModelSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.BreakingBlockModelSubmit> breakingBlockModelSubmits = new ArrayList<>();
    private final List<SubmitNodeStorage.ItemSubmit> itemSubmits = new ArrayList<>();
    private final List<SubmitNodeCollector.ParticleGroupRenderer> particleGroupRenderers = new ArrayList<>();
    private final ModelFeatureRenderer.Storage modelSubmits = new ModelFeatureRenderer.Storage();
    private final ModelPartFeatureRenderer.Storage modelPartSubmits = new ModelPartFeatureRenderer.Storage();
    private final CustomFeatureRenderer.Storage customGeometrySubmits = new CustomFeatureRenderer.Storage();
    private final SubmitNodeStorage submitNodeStorage;
    private boolean wasUsed = false;

    public SubmitNodeCollection(SubmitNodeStorage submitNodeStorage) {
        this.submitNodeStorage = submitNodeStorage;
    }

    @Override
    public void submitShadow(PoseStack poseStack, float radius, List<EntityRenderState.ShadowPiece> pieces) {
        this.wasUsed = true;
        PoseStack.Pose pose = poseStack.last();
        this.shadowSubmits.add(new SubmitNodeStorage.ShadowSubmit(new Matrix4f(pose.pose()), radius, pieces));
    }

    @Override
    public void submitNameTag(
        PoseStack poseStack,
        @Nullable Vec3 nameTagAttachment,
        int offset,
        Component name,
        boolean seeThrough,
        int lightCoords,
        double distanceToCameraSq,
        CameraRenderState camera
    ) {
        this.wasUsed = true;
        this.nameTagSubmits.add(poseStack, nameTagAttachment, offset, name, seeThrough, lightCoords, distanceToCameraSq, camera);
    }

    @Override
    public void submitText(
        PoseStack poseStack,
        float x,
        float y,
        FormattedCharSequence string,
        boolean dropShadow,
        Font.DisplayMode displayMode,
        int lightCoords,
        int color,
        int backgroundColor,
        int outlineColor
    ) {
        this.wasUsed = true;
        this.textSubmits
            .add(
                new SubmitNodeStorage.TextSubmit(
                    new Matrix4f(poseStack.last().pose()), x, y, string, dropShadow, displayMode, lightCoords, color, backgroundColor, outlineColor
                )
            );
    }

    @Override
    public void submitFlame(PoseStack poseStack, EntityRenderState renderState, Quaternionf rotation) {
        this.wasUsed = true;
        this.flameSubmits.add(new SubmitNodeStorage.FlameSubmit(poseStack.last().copy(), renderState, rotation));
    }

    @Override
    public void submitLeash(PoseStack poseStack, EntityRenderState.LeashState leashState) {
        this.wasUsed = true;
        this.leashSubmits.add(new SubmitNodeStorage.LeashSubmit(new Matrix4f(poseStack.last().pose()), leashState));
    }

    @Override
    public <S> void submitModel(
        Model<? super S> model,
        S state,
        PoseStack poseStack,
        RenderType renderType,
        int lightCoords,
        int overlayCoords,
        int tintedColor,
        @Nullable TextureAtlasSprite sprite,
        int outlineColor,
        ModelFeatureRenderer.@Nullable CrumblingOverlay crumblingOverlay
    ) {
        this.wasUsed = true;
        SubmitNodeStorage.ModelSubmit<S> modelSubmit = new SubmitNodeStorage.ModelSubmit<>(
            poseStack.last().copy(), model, state, lightCoords, overlayCoords, tintedColor, sprite, outlineColor, crumblingOverlay
        );
        this.modelSubmits.add(renderType, modelSubmit);
    }

    @Override
    public void submitModelPart(
        ModelPart modelPart,
        PoseStack poseStack,
        RenderType renderType,
        int lightCoords,
        int overlayCoords,
        @Nullable TextureAtlasSprite sprite,
        boolean sheeted,
        boolean hasFoil,
        int tintedColor,
        ModelFeatureRenderer.@Nullable CrumblingOverlay crumblingOverlay,
        int outlineColor
    ) {
        this.wasUsed = true;
        this.modelPartSubmits
            .add(
                renderType,
                new SubmitNodeStorage.ModelPartSubmit(
                    poseStack.last().copy(), modelPart, lightCoords, overlayCoords, sprite, sheeted, hasFoil, tintedColor, crumblingOverlay, outlineColor
                )
            );
    }

    @Override
    public void submitMovingBlock(PoseStack poseStack, MovingBlockRenderState movingBlockRenderState) {
        this.wasUsed = true;
        this.movingBlockSubmits.add(new SubmitNodeStorage.MovingBlockSubmit(new Matrix4f(poseStack.last().pose()), movingBlockRenderState));
    }

    @Override
    public void submitBlockModel(
        PoseStack poseStack,
        RenderType renderType,
        List<BlockStateModelPart> modelParts,
        int[] tintLayers,
        int lightCoords,
        int overlayCoords,
        int outlineColor
    ) {
        this.wasUsed = true;
        this.blockModelSubmits
            .add(new SubmitNodeStorage.BlockModelSubmit(poseStack.last().copy(), renderType, modelParts, tintLayers, lightCoords, overlayCoords, outlineColor));
    }

    @Override
    public void submitMultiLayerBlockModel(
        PoseStack poseStack,
        List<BlockStateModelPart> modelParts,
        boolean translucent,
        int[] tintLayers,
        int lightCoords,
        int overlayCoords,
        int outlineColor
    ) {
        this.wasUsed = true;
        this.multiLayerBlockModelSubmits
            .add(new SubmitNodeStorage.MultiLayerBlockModelSubmit(poseStack.last().copy(), modelParts, translucent, tintLayers, lightCoords, overlayCoords, outlineColor));
    }

    @Override
    public void submitBreakingBlockModel(PoseStack poseStack, BlockStateModel model, long seed, int progress) {
        this.wasUsed = true;
        this.breakingBlockModelSubmits.add(new SubmitNodeStorage.BreakingBlockModelSubmit(poseStack.last().copy(), model, seed, progress));
    }

    @Override
    public void submitItem(
        PoseStack poseStack,
        ItemDisplayContext displayContext,
        int lightCoords,
        int overlayCoords,
        int outlineColor,
        int[] tintLayers,
        List<BakedQuad> quads,
        ItemStackRenderState.FoilType foilType
    ) {
        this.wasUsed = true;
        this.itemSubmits
            .add(
                new SubmitNodeStorage.ItemSubmit(poseStack.last().copy(), displayContext, lightCoords, overlayCoords, outlineColor, tintLayers, quads, foilType)
            );
    }

    @Override
    public void submitCustomGeometry(PoseStack poseStack, RenderType renderType, SubmitNodeCollector.CustomGeometryRenderer customGeometryRenderer) {
        this.wasUsed = true;
        this.customGeometrySubmits.add(poseStack, renderType, customGeometryRenderer);
    }

    @Override
    public void submitParticleGroup(SubmitNodeCollector.ParticleGroupRenderer particleGroupRenderer) {
        this.wasUsed = true;
        this.particleGroupRenderers.add(particleGroupRenderer);
    }

    public List<SubmitNodeStorage.ShadowSubmit> getShadowSubmits() {
        return this.shadowSubmits;
    }

    public List<SubmitNodeStorage.FlameSubmit> getFlameSubmits() {
        return this.flameSubmits;
    }

    public NameTagFeatureRenderer.Storage getNameTagSubmits() {
        return this.nameTagSubmits;
    }

    public List<SubmitNodeStorage.TextSubmit> getTextSubmits() {
        return this.textSubmits;
    }

    public List<SubmitNodeStorage.LeashSubmit> getLeashSubmits() {
        return this.leashSubmits;
    }

    public List<SubmitNodeStorage.MovingBlockSubmit> getMovingBlockSubmits() {
        return this.movingBlockSubmits;
    }

    public List<SubmitNodeStorage.BlockModelSubmit> getBlockModelSubmits() {
        return this.blockModelSubmits;
    }

    /// Neo: expose multi-layer block model submits with support for per-quad render types
    public List<SubmitNodeStorage.MultiLayerBlockModelSubmit> getMultiLayerBlockModelSubmits() {
        return this.multiLayerBlockModelSubmits;
    }

    public List<SubmitNodeStorage.BreakingBlockModelSubmit> getBreakingBlockModelSubmits() {
        return this.breakingBlockModelSubmits;
    }

    public ModelPartFeatureRenderer.Storage getModelPartSubmits() {
        return this.modelPartSubmits;
    }

    public List<SubmitNodeStorage.ItemSubmit> getItemSubmits() {
        return this.itemSubmits;
    }

    public List<SubmitNodeCollector.ParticleGroupRenderer> getParticleGroupRenderers() {
        return this.particleGroupRenderers;
    }

    public ModelFeatureRenderer.Storage getModelSubmits() {
        return this.modelSubmits;
    }

    public CustomFeatureRenderer.Storage getCustomGeometrySubmits() {
        return this.customGeometrySubmits;
    }

    public boolean wasUsed() {
        return this.wasUsed;
    }

    public void clear() {
        this.shadowSubmits.clear();
        this.flameSubmits.clear();
        this.nameTagSubmits.clear();
        this.textSubmits.clear();
        this.leashSubmits.clear();
        this.movingBlockSubmits.clear();
        this.blockModelSubmits.clear();
        this.multiLayerBlockModelSubmits.clear();
        this.breakingBlockModelSubmits.clear();
        this.itemSubmits.clear();
        this.particleGroupRenderers.clear();
        this.modelSubmits.clear();
        this.customGeometrySubmits.clear();
        this.modelPartSubmits.clear();
    }

    public void endFrame() {
        this.modelSubmits.endFrame();
        this.modelPartSubmits.endFrame();
        this.customGeometrySubmits.endFrame();
        this.wasUsed = false;
    }
}
