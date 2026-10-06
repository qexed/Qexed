package net.minecraft.client.gui.components.debug;

import java.util.Locale;
import net.minecraft.client.Minecraft;
import net.minecraft.client.Options;
import net.minecraft.world.level.Level;
import net.minecraft.world.level.chunk.LevelChunk;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class DebugEntryFps implements DebugScreenEntry {
    @Override
    public void display(DebugScreenDisplayer displayer, @Nullable Level serverOrClientLevel, @Nullable LevelChunk clientChunk, @Nullable LevelChunk serverChunk) {
        Minecraft minecraft = Minecraft.getInstance();
        int framerateLimit = minecraft.getFramerateLimitTracker().getFramerateLimit();
        Options options = minecraft.options;
        displayer.addPriorityLine(
            String.format(
                Locale.ROOT, "%d fps T: %s%s", minecraft.getFps(), framerateLimit == 260 ? "inf" : framerateLimit, options.enableVsync().get() ? " vsync" : ""
            )
        );
    }

    @Override
    public boolean isAllowed(boolean reducedDebugInfo) {
        return true;
    }
}
