package net.minecraft.client.renderer.feature;

import com.mojang.blaze3d.vertex.PoseStack;
import com.mojang.blaze3d.vertex.VertexConsumer;
import it.unimi.dsi.fastutil.objects.ObjectOpenHashSet;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.Map.Entry;
import net.minecraft.client.renderer.MultiBufferSource;
import net.minecraft.client.renderer.SubmitNodeCollection;
import net.minecraft.client.renderer.SubmitNodeCollector;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.minecraft.client.renderer.rendertype.RenderType;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class CustomFeatureRenderer {
    public void renderSolid(SubmitNodeCollection nodeCollection, MultiBufferSource.BufferSource bufferSource) {
        CustomFeatureRenderer.Storage storage = nodeCollection.getCustomGeometrySubmits();

        for (Entry<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> entry : storage.solidCustomGeometrySubmits.entrySet()) {
            VertexConsumer buffer = bufferSource.getBuffer(entry.getKey());

            for (SubmitNodeStorage.CustomGeometrySubmit customGeometrySubmit : entry.getValue()) {
                customGeometrySubmit.customGeometryRenderer().render(customGeometrySubmit.pose(), buffer);
            }
        }
    }

    public void renderTranslucent(SubmitNodeCollection nodeCollection, MultiBufferSource.BufferSource bufferSource) {
        CustomFeatureRenderer.Storage storage = nodeCollection.getCustomGeometrySubmits();

        for (Entry<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> entry : storage.translucentCustomGeometrySubmits.entrySet()) {
            VertexConsumer buffer = bufferSource.getBuffer(entry.getKey());

            for (SubmitNodeStorage.CustomGeometrySubmit customGeometrySubmit : entry.getValue()) {
                customGeometrySubmit.customGeometryRenderer().render(customGeometrySubmit.pose(), buffer);
            }
        }
    }

    @OnlyIn(Dist.CLIENT)
    public static class Storage {
        private final Map<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> solidCustomGeometrySubmits = new HashMap<>();
        private final Map<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> translucentCustomGeometrySubmits = new HashMap<>();
        private final Set<RenderType> solidCustomGeometrySubmitsUsage = new ObjectOpenHashSet<>();
        private final Set<RenderType> translucentCustomGeometrySubmitsUsage = new ObjectOpenHashSet<>();

        public void add(PoseStack poseStack, RenderType renderType, SubmitNodeCollector.CustomGeometryRenderer customGeometryRenderer) {
            SubmitNodeStorage.CustomGeometrySubmit submit = new SubmitNodeStorage.CustomGeometrySubmit(poseStack.last().copy(), customGeometryRenderer);
            if (!renderType.hasBlending()) {
                this.solidCustomGeometrySubmits.computeIfAbsent(renderType, rt -> new ArrayList<>()).add(submit);
            } else {
                this.translucentCustomGeometrySubmits.computeIfAbsent(renderType, rt -> new ArrayList<>()).add(submit);
            }
        }

        public void clear() {
            for (Entry<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> entry : this.solidCustomGeometrySubmits.entrySet()) {
                if (!entry.getValue().isEmpty()) {
                    this.solidCustomGeometrySubmitsUsage.add(entry.getKey());
                    entry.getValue().clear();
                }
            }

            for (Entry<RenderType, List<SubmitNodeStorage.CustomGeometrySubmit>> entryx : this.translucentCustomGeometrySubmits.entrySet()) {
                if (!entryx.getValue().isEmpty()) {
                    this.translucentCustomGeometrySubmitsUsage.add(entryx.getKey());
                    entryx.getValue().clear();
                }
            }
        }

        public void endFrame() {
            this.solidCustomGeometrySubmits.keySet().removeIf(renderType -> !this.solidCustomGeometrySubmitsUsage.contains(renderType));
            this.solidCustomGeometrySubmitsUsage.clear();
            this.translucentCustomGeometrySubmits.keySet().removeIf(renderType -> !this.translucentCustomGeometrySubmitsUsage.contains(renderType));
            this.translucentCustomGeometrySubmitsUsage.clear();
        }
    }
}
