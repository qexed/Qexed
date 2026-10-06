package net.minecraft.client.renderer.blockentity;

import com.google.common.collect.ImmutableMap;
import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.math.Axis;
import com.mojang.math.Transformation;
import java.util.Map;
import net.minecraft.client.model.Model;
import net.minecraft.client.model.geom.EntityModelSet;
import net.minecraft.client.model.geom.ModelLayers;
import net.minecraft.client.model.geom.PartPose;
import net.minecraft.client.model.geom.builders.CubeListBuilder;
import net.minecraft.client.model.geom.builders.LayerDefinition;
import net.minecraft.client.model.geom.builders.MeshDefinition;
import net.minecraft.client.model.geom.builders.PartDefinition;
import net.minecraft.client.renderer.Sheets;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.blockentity.state.HangingSignRenderState;
import net.minecraft.client.renderer.blockentity.state.SignRenderState;
import net.minecraft.client.renderer.feature.ModelFeatureRenderer;
import net.minecraft.client.renderer.rendertype.RenderTypes;
import net.minecraft.client.resources.model.sprite.SpriteGetter;
import net.minecraft.client.resources.model.sprite.SpriteId;
import net.minecraft.core.Direction;
import net.minecraft.util.Unit;
import net.minecraft.world.level.block.CeilingHangingSignBlock;
import net.minecraft.world.level.block.HangingSignBlock;
import net.minecraft.world.level.block.WallHangingSignBlock;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.minecraft.world.level.block.state.BlockState;
import net.minecraft.world.level.block.state.properties.RotationSegment;
import net.minecraft.world.level.block.state.properties.WoodType;
import net.minecraft.world.phys.Vec3;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.joml.Vector3fc;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class HangingSignRenderer extends AbstractSignRenderer<HangingSignRenderState> {
    private static final String PLANK = "plank";
    private static final String V_CHAINS = "vChains";
    private static final String NORMAL_CHAINS = "normalChains";
    private static final String CHAIN_L_1 = "chainL1";
    private static final String CHAIN_L_2 = "chainL2";
    private static final String CHAIN_R_1 = "chainR1";
    private static final String CHAIN_R_2 = "chainR2";
    private static final String BOARD = "board";
    private static final float MODEL_RENDER_SCALE = 1.0F;
    private static final float TEXT_RENDER_SCALE = 0.9F;
    private static final Vector3fc TEXT_OFFSET = new Vector3f(0.0F, -0.32F, 0.073F);
    public static final WallAndGroundTransformations<SignRenderState.SignTransformations> TRANSFORMATIONS = new WallAndGroundTransformations<>(
        HangingSignRenderer::createWallTransformation, HangingSignRenderer::createGroundTransformation, 16
    );
    private final Map<WoodType, HangingSignRenderer.Models> signModels;

    public HangingSignRenderer(BlockEntityRendererProvider.Context context) {
        super(context);
        this.signModels = WoodType.values()
            .collect(ImmutableMap.toImmutableMap(type -> (WoodType)type, type -> HangingSignRenderer.Models.create(context, type)));
    }

    public HangingSignRenderState createRenderState() {
        return new HangingSignRenderState();
    }

    public void extractRenderState(
        SignBlockEntity blockEntity,
        HangingSignRenderState state,
        float partialTicks,
        Vec3 cameraPosition,
        ModelFeatureRenderer.@Nullable CrumblingOverlay breakProgress
    ) {
        super.extractRenderState(blockEntity, state, partialTicks, cameraPosition, breakProgress);
        BlockState blockState = blockEntity.getBlockState();
        state.attachmentType = HangingSignBlock.getAttachmentPoint(blockState);
        if (blockState.getBlock() instanceof WallHangingSignBlock) {
            state.transformations = TRANSFORMATIONS.wallTransformation(blockState.getValue(WallHangingSignBlock.FACING));
        } else {
            state.transformations = TRANSFORMATIONS.freeTransformations(blockState.getValue(CeilingHangingSignBlock.ROTATION));
        }
    }

    public static Model.Simple createSignModel(EntityModelSet entityModelSet, WoodType woodType, HangingSignBlock.Attachment attachmentType) {
        return new Model.Simple(entityModelSet.bakeLayer(ModelLayers.createHangingSignModelName(woodType, attachmentType)), RenderTypes::entityCutout);
    }

    private static Matrix4f baseTransformation(float angle) {
        return new Matrix4f().translation(0.5F, 0.9375F, 0.5F).rotate(Axis.YP.rotationDegrees(-angle)).translate(0.0F, -0.3125F, 0.0F);
    }

    private static Transformation bodyTransformation(float angle) {
        return new Transformation(baseTransformation(angle).scale(1.0F, -1.0F, -1.0F));
    }

    private static Transformation textTransformation(float angle, boolean isFrontText) {
        Matrix4f result = baseTransformation(angle);
        if (!isFrontText) {
            result.rotate(Axis.YP.rotationDegrees(180.0F));
        }

        float s = 0.0140625F;
        result.translate(TEXT_OFFSET);
        result.scale(0.0140625F, -0.0140625F, 0.0140625F);
        return new Transformation(result);
    }

    private static SignRenderState.SignTransformations createTransformations(float angle) {
        return new SignRenderState.SignTransformations(bodyTransformation(angle), textTransformation(angle, true), textTransformation(angle, false));
    }

    private static SignRenderState.SignTransformations createGroundTransformation(int segment) {
        return createTransformations(RotationSegment.convertToDegrees(segment));
    }

    private static SignRenderState.SignTransformations createWallTransformation(Direction direction) {
        return createTransformations(direction.toYRot());
    }

    protected Model.Simple getSignModel(HangingSignRenderState state) {
        return this.signModels.get(state.woodType).get(state.attachmentType);
    }

    @Override
    protected SpriteId getSignSprite(WoodType type) {
        return Sheets.getHangingSignSprite(type);
    }

    public static void submitSpecial(
        SpriteGetter sprites,
        PoseStack poseStack,
        SubmitNodeCollector submitNodeCollector,
        int lightCoords,
        int overlayCoords,
        Model.Simple model,
        SpriteId sprite
    ) {
        submitNodeCollector.submitModel(model, Unit.INSTANCE, poseStack, lightCoords, overlayCoords, -1, sprite, sprites, 0, null);
    }

    public static LayerDefinition createHangingSignLayer(HangingSignBlock.Attachment type) {
        MeshDefinition mesh = new MeshDefinition();
        PartDefinition root = mesh.getRoot();
        root.addOrReplaceChild("board", CubeListBuilder.create().texOffs(0, 12).addBox(-7.0F, 0.0F, -1.0F, 14.0F, 10.0F, 2.0F), PartPose.ZERO);
        if (type == HangingSignBlock.Attachment.WALL) {
            root.addOrReplaceChild("plank", CubeListBuilder.create().texOffs(0, 0).addBox(-8.0F, -6.0F, -2.0F, 16.0F, 2.0F, 4.0F), PartPose.ZERO);
        }

        if (type == HangingSignBlock.Attachment.WALL || type == HangingSignBlock.Attachment.CEILING) {
            PartDefinition normalChains = root.addOrReplaceChild("normalChains", CubeListBuilder.create(), PartPose.ZERO);
            normalChains.addOrReplaceChild(
                "chainL1",
                CubeListBuilder.create().texOffs(0, 6).addBox(-1.5F, 0.0F, 0.0F, 3.0F, 6.0F, 0.0F),
                PartPose.offsetAndRotation(-5.0F, -6.0F, 0.0F, 0.0F, (float) (-Math.PI / 4), 0.0F)
            );
            normalChains.addOrReplaceChild(
                "chainL2",
                CubeListBuilder.create().texOffs(6, 6).addBox(-1.5F, 0.0F, 0.0F, 3.0F, 6.0F, 0.0F),
                PartPose.offsetAndRotation(-5.0F, -6.0F, 0.0F, 0.0F, (float) (Math.PI / 4), 0.0F)
            );
            normalChains.addOrReplaceChild(
                "chainR1",
                CubeListBuilder.create().texOffs(0, 6).addBox(-1.5F, 0.0F, 0.0F, 3.0F, 6.0F, 0.0F),
                PartPose.offsetAndRotation(5.0F, -6.0F, 0.0F, 0.0F, (float) (-Math.PI / 4), 0.0F)
            );
            normalChains.addOrReplaceChild(
                "chainR2",
                CubeListBuilder.create().texOffs(6, 6).addBox(-1.5F, 0.0F, 0.0F, 3.0F, 6.0F, 0.0F),
                PartPose.offsetAndRotation(5.0F, -6.0F, 0.0F, 0.0F, (float) (Math.PI / 4), 0.0F)
            );
        }

        if (type == HangingSignBlock.Attachment.CEILING_MIDDLE) {
            root.addOrReplaceChild("vChains", CubeListBuilder.create().texOffs(14, 6).addBox(-6.0F, -6.0F, 0.0F, 12.0F, 6.0F, 0.0F), PartPose.ZERO);
        }

        return LayerDefinition.create(mesh, 64, 32);
    }

    @OnlyIn(Dist.CLIENT)
    private record Models(Model.Simple ceiling, Model.Simple ceilingMiddle, Model.Simple wall) {
        public static HangingSignRenderer.Models create(BlockEntityRendererProvider.Context context, WoodType type) {
            return new HangingSignRenderer.Models(
                HangingSignRenderer.createSignModel(context.entityModelSet(), type, HangingSignBlock.Attachment.CEILING),
                HangingSignRenderer.createSignModel(context.entityModelSet(), type, HangingSignBlock.Attachment.CEILING_MIDDLE),
                HangingSignRenderer.createSignModel(context.entityModelSet(), type, HangingSignBlock.Attachment.WALL)
            );
        }

        public Model.Simple get(HangingSignBlock.Attachment attachmentType) {
            return switch (attachmentType) {
                case CEILING -> this.ceiling;
                case CEILING_MIDDLE -> this.ceilingMiddle;
                case WALL -> this.wall;
            };
        }
    }
}
