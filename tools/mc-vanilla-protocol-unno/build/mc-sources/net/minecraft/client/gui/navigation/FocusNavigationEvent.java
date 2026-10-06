package net.minecraft.client.gui.navigation;

import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public interface FocusNavigationEvent {
    ScreenDirection getVerticalDirectionForInitialFocus();

    @OnlyIn(Dist.CLIENT)
    public record ArrowNavigation(ScreenDirection direction, @Nullable ScreenRectangle previousFocus) implements FocusNavigationEvent {
        public ArrowNavigation(ScreenDirection direction) {
            this(direction, null);
        }

        @Override
        public ScreenDirection getVerticalDirectionForInitialFocus() {
            return this.direction.getAxis() == ScreenAxis.VERTICAL ? this.direction : ScreenDirection.DOWN;
        }

        public FocusNavigationEvent.ArrowNavigation with(ScreenRectangle previousFocus) {
            return new FocusNavigationEvent.ArrowNavigation(this.direction(), previousFocus);
        }
    }

    @OnlyIn(Dist.CLIENT)
    public static class InitialFocus implements FocusNavigationEvent {
        @Override
        public ScreenDirection getVerticalDirectionForInitialFocus() {
            return ScreenDirection.DOWN;
        }
    }

    @OnlyIn(Dist.CLIENT)
    public record TabNavigation(boolean forward) implements FocusNavigationEvent {
        @Override
        public ScreenDirection getVerticalDirectionForInitialFocus() {
            return this.forward ? ScreenDirection.DOWN : ScreenDirection.UP;
        }
    }
}
