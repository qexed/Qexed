package net.minecraft.gametest.framework;

import org.jspecify.annotations.Nullable;

class GameTestEvent {
    public final @Nullable Long expectedDelay;
    public final @Nullable Long minimumDelay;
    public final Runnable assertion;

    private GameTestEvent(@Nullable Long expectedDelay, @Nullable Long minimumDelay, Runnable assertion) {
        this.expectedDelay = expectedDelay;
        this.minimumDelay = minimumDelay;
        this.assertion = assertion;
    }

    static GameTestEvent create(Runnable runnable) {
        return new GameTestEvent(null, null, runnable);
    }

    static GameTestEvent create(long expectedTick, Runnable runnable) {
        return new GameTestEvent(expectedTick, null, runnable);
    }

    static GameTestEvent createWithMinimumDelay(long minimumDelay, Runnable runnable) {
        return new GameTestEvent(null, minimumDelay, runnable);
    }
}
