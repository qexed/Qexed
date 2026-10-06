package net.minecraft.client.renderer.state.level;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.blockentity.state.BlockEntityRenderState;
import net.minecraft.client.renderer.chunk.ChunkSectionsToRender;
import net.minecraft.client.renderer.entity.state.EntityRenderState;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class LevelRenderState extends net.neoforged.neoforge.client.renderstate.BaseRenderState {
    public CameraRenderState cameraRenderState = new CameraRenderState();
    public final List<EntityRenderState> entityRenderStates = new ArrayList<>();
    public final List<BlockEntityRenderState> blockEntityRenderStates = new ArrayList<>();
    public boolean haveGlowingEntities;
    public @Nullable BlockOutlineRenderState blockOutlineRenderState;
    public final List<BlockBreakingRenderState> blockBreakingRenderStates = new ArrayList<>();
    public final WeatherRenderState weatherRenderState = new WeatherRenderState();
    public final WorldBorderRenderState worldBorderRenderState = new WorldBorderRenderState();
    public final SkyRenderState skyRenderState = new SkyRenderState();
    public final ParticlesRenderState particlesRenderState = new ParticlesRenderState();
    public long gameTime;
    public int lastEntityRenderStateCount;
    public int cloudColor;
    public float cloudHeight;
    public @Nullable ChunkSectionsToRender chunkSectionsToRender;
    public net.neoforged.neoforge.client.@Nullable CustomSkyboxRenderer customSkyboxRenderer;
    public net.neoforged.neoforge.client.@Nullable CustomCloudsRenderer customCloudsRenderer;
    public net.neoforged.neoforge.client.@Nullable CustomWeatherEffectRenderer customWeatherEffectRenderer;

    public void reset() {
        this.entityRenderStates.clear();
        this.blockEntityRenderStates.clear();
        this.blockBreakingRenderStates.clear();
        this.haveGlowingEntities = false;
        this.blockOutlineRenderState = null;
        this.weatherRenderState.reset();
        this.worldBorderRenderState.reset();
        this.skyRenderState.reset();
        this.gameTime = 0L;
        this.customSkyboxRenderer = null;
        this.customCloudsRenderer = null;
        this.customWeatherEffectRenderer = null;
        this.resetRenderData();
    }
}
