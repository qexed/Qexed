package net.minecraft.client.renderer.state.level;

import java.util.ArrayList;
import java.util.List;
import net.minecraft.client.renderer.SubmitNodeStorage;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;

@OnlyIn(Dist.CLIENT)
public class ParticlesRenderState {
    public final List<ParticleGroupRenderState> particles = new ArrayList<>();

    public void reset() {
        this.particles.forEach(ParticleGroupRenderState::clear);
        this.particles.clear();
    }

    public void add(ParticleGroupRenderState state) {
        this.particles.add(state);
    }

    public void submit(SubmitNodeStorage submitNodeStorage, CameraRenderState camera) {
        for (ParticleGroupRenderState particle : this.particles) {
            particle.submit(submitNodeStorage, camera);
        }
    }
}
