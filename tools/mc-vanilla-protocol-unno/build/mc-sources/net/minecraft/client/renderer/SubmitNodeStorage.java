package net.minecraft.client.renderer;

import com.mojang.blaze3d.vertex.PoseStack;
import it.unimi.dsi.fastutil.ints.Int2ObjectAVLTreeMap;
import java.util.List;
import net.minecraft.client.gui.Font;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.ModelPart;
import net.minecraft.client.renderer.block.MovingBlockRenderState;
import net.minecraft.client.renderer.block.dispatch.BlockStateModel;
import net.minecraft.client.renderer.block.dispatch.BlockStateModelPart;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
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
import org.joml.Matrix4fc;
import org.joml.Quaternionf;
import org.joml.Vector3f;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class SubmitNodeStorage implements SubmitNodeCollector, net.neoforged.neoforge.client.extensions.SubmitNodeStorageExtension {
    private final Int2ObjectAVLTreeMap<SubmitNodeCollection> submitsPerOrder = new Int2ObjectAVLTreeMap<>();

    public SubmitNodeCollection order(int order) {
        return this.submitsPerOrder.computeIfAbsent(order, ignored -> new SubmitNodeCollection(this));
    }

    @Override
    public void submitShadow(PoseStack poseStack, float radius, List<EntityRenderState.ShadowPiece> pieces) {
        this.order(0).submitShadow(poseStack, radius, pieces);
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
        this.order(0).submitNameTag(poseStack, nameTagAttachment, offset, name, seeThrough, lightCoords, distanceToCameraSq, camera);
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
        this.order(0).submitText(poseStack, x, y, string, dropShadow, displayMode, lightCoords, color, backgroundColor, outlineColor);
    }

    @Override
    public void submitFlame(PoseStack poseStack, EntityRenderState renderState, Quaternionf rotation) {
        this.order(0).submitFlame(poseStack, renderState, rotation);
    }

    @Override
    public void submitLeash(PoseStack poseStack, EntityRenderState.LeashState leashState) {
        this.order(0).submitLeash(poseStack, leashState);
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
        this.order(0).submitModel(model, state, poseStack, renderType, lightCoords, overlayCoords, tintedColor, sprite, outlineColor, crumblingOverlay);
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
        this.order(0)
            .submitModelPart(
                modelPart, poseStack, renderType, lightCoords, overlayCoords, sprite, sheeted, hasFoil, tintedColor, crumblingOverlay, outlineColor
            );
    }

    @Override
    public void submitMovingBlock(PoseStack poseStack, MovingBlockRenderState movingBlockRenderState) {
        this.order(0).submitMovingBlock(poseStack, movingBlockRenderState);
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
        this.order(0).submitBlockModel(poseStack, renderType, modelParts, tintLayers, lightCoords, overlayCoords, outlineColor);
    }

    @Override
    public void submitBreakingBlockModel(PoseStack poseStack, BlockStateModel model, long seed, int progress) {
        this.order(0).submitBreakingBlockModel(poseStack, model, seed, progress);
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
        this.order(0).submitItem(poseStack, displayContext, lightCoords, overlayCoords, outlineColor, tintLayers, quads, foilType);
    }

    @Override
    public void submitCustomGeometry(PoseStack poseStack, RenderType renderType, SubmitNodeCollector.CustomGeometryRenderer customGeometryRenderer) {
        this.order(0).submitCustomGeometry(poseStack, renderType, customGeometryRenderer);
    }

    @Override
    public void submitParticleGroup(SubmitNodeCollector.ParticleGroupRenderer particleGroupRenderer) {
        this.order(0).submitParticleGroup(particleGroupRenderer);
    }

    public void clear() {
        this.submitsPerOrder.values().forEach(SubmitNodeCollection::clear);
    }

    public void endFrame() {
        this.submitsPerOrder.values().removeIf(collection -> !collection.wasUsed());
        this.submitsPerOrder.values().forEach(SubmitNodeCollection::endFrame);
    }

    public Int2ObjectAVLTreeMap<SubmitNodeCollection> getSubmitsPerOrder() {
        return this.submitsPerOrder;
    }

    @OnlyIn(Dist.CLIENT)
    public record BlockModelSubmit(
        PoseStack.Pose pose,
        RenderType renderType,
        List<BlockStateModelPart> modelParts,
        int[] tintLayers,
        int lightCoords,
        int overlayCoords,
        int outlineColor
    ) {
    }

    @OnlyIn(Dist.CLIENT)
    public record BreakingBlockModelSubmit(PoseStack.Pose pose, BlockStateModel model, long seed, int progress) {
    }

    @OnlyIn(Dist.CLIENT)
    public record CustomGeometrySubmit(PoseStack.Pose pose, SubmitNodeCollector.CustomGeometryRenderer customGeometryRenderer) {
    }

    @OnlyIn(Dist.CLIENT)
    public record FlameSubmit(PoseStack.Pose pose, EntityRenderState entityRenderState, Quaternionf rotation) {
    }

    @OnlyIn(Dist.CLIENT)
    public record ItemSubmit(
        PoseStack.Pose pose,
        ItemDisplayContext displayContext,
        int lightCoords,
        int overlayCoords,
        int outlineColor,
        int[] tintLayers,
        List<BakedQuad> quads,
        ItemStackRenderState.FoilType foilType
    ) {
    }

    @OnlyIn(Dist.CLIENT)
    public record LeashSubmit(Matrix4f pose, EntityRenderState.LeashState leashState) {
    }

    @OnlyIn(Dist.CLIENT)
    public record ModelPartSubmit(
        PoseStack.Pose pose,
        ModelPart modelPart,
        int lightCoords,
        int overlayCoords,
        @Nullable TextureAtlasSprite sprite,
        boolean sheeted,
        boolean hasFoil,
        int tintedColor,
        ModelFeatureRenderer.@Nullable CrumblingOverlay crumblingOverlay,
        int outlineColor
    ) {
    }

    @OnlyIn(Dist.CLIENT)
    public record ModelSubmit<S>(
        PoseStack.Pose pose,
        Model<? super S> model,
        S state,
        int lightCoords,
        int overlayCoords,
        int tintedColor,
        @Nullable TextureAtlasSprite sprite,
        int outlineColor,
        ModelFeatureRenderer.@Nullable CrumblingOverlay crumblingOverlay
    ) {
    }

    @OnlyIn(Dist.CLIENT)
    public record MovingBlockSubmit(Matrix4fc pose, MovingBlockRenderState movingBlockRenderState) {
    }

    @OnlyIn(Dist.CLIENT)
    public record NameTagSubmit(Matrix4fc pose, float x, float y, Component text, int lightCoords, int color, int backgroundColor, double distanceToCameraSq) {
    }

    @OnlyIn(Dist.CLIENT)
    public record ShadowSubmit(Matrix4fc pose, float radius, List<EntityRenderState.ShadowPiece> pieces) {
    }

    @OnlyIn(Dist.CLIENT)
    public record TextSubmit(
        Matrix4fc pose,
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
    }

    @OnlyIn(Dist.CLIENT)
    public record TranslucentModelSubmit<S>(SubmitNodeStorage.ModelSubmit<S> modelSubmit, RenderType renderType, Vector3f position) {
    }
}
