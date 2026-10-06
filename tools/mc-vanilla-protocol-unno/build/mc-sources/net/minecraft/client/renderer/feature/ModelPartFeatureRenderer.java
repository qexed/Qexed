package net.minecraft.client.renderer.feature;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.SheetedDecalTextureGenerator;
import com.mojang.blaze3d.vertex.VertexConsumer;
import it.unimi.dsi.fastutil.objects.ObjectOpenHashSet;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.Map.Entry;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.OutlineBufferSource;
import net.minecraft.client.renderer.SubmitNodeCollection;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.minecraft.client.resources.model.ModelBakery;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class ModelPartFeatureRenderer {
    private final PoseStack poseStack = new PoseStack();

    public void renderSolid(
        SubmitNodeCollection nodeCollection,
        MultiBufferSource.BufferSource bufferSource,
        OutlineBufferSource outlineBufferSource,
        MultiBufferSource.BufferSource crumblingBufferSource
    ) {
        ModelPartFeatureRenderer.Storage storage = nodeCollection.getModelPartSubmits();
        this.render(storage.solidModelPartSubmits, bufferSource, outlineBufferSource, crumblingBufferSource);
    }

    public void renderTranslucent(
        SubmitNodeCollection nodeCollection,
        MultiBufferSource.BufferSource bufferSource,
        OutlineBufferSource outlineBufferSource,
        MultiBufferSource.BufferSource crumblingBufferSource
    ) {
        ModelPartFeatureRenderer.Storage storage = nodeCollection.getModelPartSubmits();
        this.render(storage.translucentModelPartSubmits, bufferSource, outlineBufferSource, crumblingBufferSource);
    }

    private void render(
        Map<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> modelPartSubmitsMap,
        MultiBufferSource.BufferSource bufferSource,
        OutlineBufferSource outlineBufferSource,
        MultiBufferSource.BufferSource crumblingBufferSource
    ) {
        for (Entry<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> entry : modelPartSubmitsMap.entrySet()) {
            RenderType renderType = entry.getKey();
            List<SubmitNodeStorage.ModelPartSubmit> modelPartSubmits = entry.getValue();
            VertexConsumer buffer = bufferSource.getBuffer(renderType);

            for (SubmitNodeStorage.ModelPartSubmit modelPartSubmit : modelPartSubmits) {
                VertexConsumer actualBuffer;
                if (modelPartSubmit.sprite() != null) {
                    if (modelPartSubmit.hasFoil()) {
                        actualBuffer = modelPartSubmit.sprite()
                            .wrap(ItemFeatureRenderer.getFoilBuffer(bufferSource, renderType, modelPartSubmit.sheeted(), true));
                    } else {
                        actualBuffer = modelPartSubmit.sprite().wrap(buffer);
                    }
                } else if (modelPartSubmit.hasFoil()) {
                    actualBuffer = ItemFeatureRenderer.getFoilBuffer(bufferSource, renderType, modelPartSubmit.sheeted(), true);
                } else {
                    actualBuffer = buffer;
                }

                this.poseStack.last().set(modelPartSubmit.pose());
                modelPartSubmit.modelPart()
                    .render(this.poseStack, actualBuffer, modelPartSubmit.lightCoords(), modelPartSubmit.overlayCoords(), modelPartSubmit.tintedColor());
                if (modelPartSubmit.outlineColor() != 0 && (renderType.outline().isPresent() || renderType.isOutline())) {
                    outlineBufferSource.setColor(modelPartSubmit.outlineColor());
                    VertexConsumer outlineBuffer = outlineBufferSource.getBuffer(renderType);
                    modelPartSubmit.modelPart()
                        .render(
                            this.poseStack,
                            modelPartSubmit.sprite() == null ? outlineBuffer : modelPartSubmit.sprite().wrap(outlineBuffer),
                            modelPartSubmit.lightCoords(),
                            modelPartSubmit.overlayCoords(),
                            modelPartSubmit.tintedColor()
                        );
                }

                if (modelPartSubmit.crumblingOverlay() != null) {
                    VertexConsumer breakingBuffer = new SheetedDecalTextureGenerator(
                        crumblingBufferSource.getBuffer(ModelBakery.DESTROY_TYPES.get(modelPartSubmit.crumblingOverlay().progress())),
                        modelPartSubmit.crumblingOverlay().cameraPose(),
                        1.0F
                    );
                    modelPartSubmit.modelPart()
                        .render(this.poseStack, breakingBuffer, modelPartSubmit.lightCoords(), modelPartSubmit.overlayCoords(), modelPartSubmit.tintedColor());
                }
            }
        }
    }

    @OnlyIn(Dist.CLIENT)
    public static class Storage {
        private final Map<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> solidModelPartSubmits = new HashMap<>();
        private final Map<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> translucentModelPartSubmits = new HashMap<>();
        private final Set<RenderType> solidModelPartSubmitsUsage = new ObjectOpenHashSet<>();
        private final Set<RenderType> translucentModelPartSubmitsUsage = new ObjectOpenHashSet<>();

        public void add(RenderType renderType, SubmitNodeStorage.ModelPartSubmit submit) {
            if (!renderType.hasBlending()) {
                this.solidModelPartSubmits.computeIfAbsent(renderType, ignored -> new ArrayList<>()).add(submit);
            } else {
                this.translucentModelPartSubmits.computeIfAbsent(renderType, ignored -> new ArrayList<>()).add(submit);
            }
        }

        public void clear() {
            for (Entry<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> entry : this.solidModelPartSubmits.entrySet()) {
                if (!entry.getValue().isEmpty()) {
                    this.solidModelPartSubmitsUsage.add(entry.getKey());
                    entry.getValue().clear();
                }
            }

            for (Entry<RenderType, List<SubmitNodeStorage.ModelPartSubmit>> entryx : this.translucentModelPartSubmits.entrySet()) {
                if (!entryx.getValue().isEmpty()) {
                    this.translucentModelPartSubmitsUsage.add(entryx.getKey());
                    entryx.getValue().clear();
                }
            }
        }

        public void endFrame() {
            this.solidModelPartSubmits.keySet().removeIf(renderType -> !this.solidModelPartSubmitsUsage.contains(renderType));
            this.solidModelPartSubmitsUsage.clear();
            this.translucentModelPartSubmits.keySet().removeIf(renderType -> !this.translucentModelPartSubmitsUsage.contains(renderType));
            this.translucentModelPartSubmitsUsage.clear();
        }
    }
}
