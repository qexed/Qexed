package net.minecraft.client.gui.components.debug;

import com.mojang.blaze3d.platform.GLX;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.blaze3d.systems.RenderSystem;
import java.util.List;
import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.resources.Identifier;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.LevelChunk;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class DebugEntrySystemSpecs implements DebugScreenEntry {
    public static final Identifier GROUP = Identifier.withDefaultNamespace("system");

    @Override
    public void display(DebugScreenDisplayer displayer, @Nullable Level serverOrClientLevel, @Nullable LevelChunk clientChunk, @Nullable LevelChunk serverChunk) {
        GpuDevice device = RenderSystem.getDevice();
        displayer.addToGroup(
            GROUP,
            List.of(
                String.format(Locale.ROOT, "Java: %s", System.getProperty("java.version")),
                String.format(Locale.ROOT, "CPU: %s", GLX._getCpuInfo()),
                String.format(
                    Locale.ROOT,
                    "Display: %dx%d (%s)",
                    Minecraft.getInstance().getWindow().getWidth(),
                    Minecraft.getInstance().getWindow().getHeight(),
                    device.getVendor()
                ),
                device.getRenderer(),
                String.format(Locale.ROOT, "%s %s", device.getBackendName(), device.getVersion())
            )
        );
    }

    @Override
    public boolean isAllowed(boolean reducedDebugInfo) {
        return true;
    }
}
