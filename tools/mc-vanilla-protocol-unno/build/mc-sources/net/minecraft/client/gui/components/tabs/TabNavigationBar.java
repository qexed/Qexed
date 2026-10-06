package net.minecraft.client.gui.components.tabs;

import com.google.common.collect.ImmutableList;
import java.util.ArrayList;
import java.util.Collections;
import java.util.Comparator;
import java.util.List;
import java.util.Optional;
import net.minecraft.client.gui.ComponentPath;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractWidget;
import net.minecraft.client.gui.components.Renderable;
import net.minecraft.client.gui.components.TabButton;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.components.events.AbstractContainerEventHandler;
import net.minecraft.client.gui.components.events.GuiEventListener;
import net.minecraft.client.gui.layouts.LinearLayout;
import net.minecraft.client.gui.narration.NarratableEntry;
import net.minecraft.client.gui.narration.NarratedElementType;
import net.minecraft.client.gui.narration.NarrationElementOutput;
import net.minecraft.client.gui.navigation.FocusNavigationEvent;
import net.minecraft.client.gui.navigation.ScreenRectangle;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import net.minecraft.util.Mth;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;

@OnlyIn(Dist.CLIENT)
public class TabNavigationBar extends AbstractContainerEventHandler implements NarratableEntry, Renderable {
    private static final int NO_TAB = -1;
    private static final int MAX_WIDTH = 400;
    private static final int HEIGHT = 24;
    private static final int MARGIN = 14;
    private static final Component USAGE_NARRATION = Component.translatable("narration.tab_navigation.usage");
    private final LinearLayout layout = LinearLayout.horizontal();
    private int width;
    private final TabManager tabManager;
    private final ImmutableList<Tab> tabs;
    private final ImmutableList<TabButton> tabButtons;

    private TabNavigationBar(int width, TabManager tabManager, Iterable<Tab> tabs) {
        this.width = width;
        this.tabManager = tabManager;
        this.tabs = ImmutableList.copyOf(tabs);
        this.layout.defaultCellSetting().alignHorizontallyCenter();
        ImmutableList.Builder<TabButton> tabButtonsBuilder = ImmutableList.builder();

        for (Tab tab : tabs) {
            tabButtonsBuilder.add(this.layout.addChild(new TabButton(tabManager, tab, 0, 24)));
        }

        this.tabButtons = tabButtonsBuilder.build();
    }

    public static TabNavigationBar.Builder builder(TabManager tabManager, int width) {
        return new TabNavigationBar.Builder(tabManager, width);
    }

    public void updateWidth(int width) {
        this.width = width;
        this.arrangeElements();
    }

    @Override
    public boolean isMouseOver(double mouseX, double mouseY) {
        return mouseX >= this.layout.getX()
            && mouseY >= this.layout.getY()
            && mouseX < this.layout.getX() + this.layout.getWidth()
            && mouseY < this.layout.getY() + this.layout.getHeight();
    }

    @Override
    public void setFocused(boolean focused) {
        super.setFocused(focused);
        if (this.getFocused() != null) {
            this.setFocused(null);
        }
    }

    @Override
    public void setFocused(@Nullable GuiEventListener focused) {
        super.setFocused(focused);
        if (focused instanceof TabButton button && button.isActive()) {
            this.tabManager.setCurrentTab(button.tab(), true);
        }
    }

    @Override
    public @Nullable ComponentPath nextFocusPath(FocusNavigationEvent navigationEvent) {
        if (!this.isFocused()) {
            TabButton button = this.currentTabButton();
            if (button != null) {
                return ComponentPath.path(this, ComponentPath.leaf(button));
            }
        }

        return navigationEvent instanceof FocusNavigationEvent.TabNavigation ? null : super.nextFocusPath(navigationEvent);
    }

    @Override
    public List<? extends GuiEventListener> children() {
        return this.tabButtons;
    }

    public List<Tab> getTabs() {
        return this.tabs;
    }

    @Override
    public NarratableEntry.NarrationPriority narrationPriority() {
        return this.tabButtons.stream().map(AbstractWidget::narrationPriority).max(Comparator.naturalOrder()).orElse(NarratableEntry.NarrationPriority.NONE);
    }

    @Override
    public void updateNarration(NarrationElementOutput output) {
        Optional<TabButton> selected = this.tabButtons
            .stream()
            .filter(AbstractWidget::isHovered)
            .findFirst()
            .or(() -> Optional.ofNullable(this.currentTabButton()));
        selected.ifPresent(button -> {
            this.narrateListElementPosition(output.nest(), button);
            button.updateNarration(output);
        });
        if (this.isFocused()) {
            output.add(NarratedElementType.USAGE, USAGE_NARRATION);
        }
    }

    protected void narrateListElementPosition(NarrationElementOutput output, TabButton widget) {
        if (this.tabs.size() > 1) {
            int index = this.tabButtons.indexOf(widget);
            if (index != -1) {
                output.add(NarratedElementType.POSITION, Component.translatable("narrator.position.tab", index + 1, this.tabs.size()));
            }
        }
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float a) {
        graphics.blit(
            RenderPipelines.GUI_TEXTURED,
            Screen.HEADER_SEPARATOR,
            0,
            this.layout.getY() + this.layout.getHeight() - 2,
            0.0F,
            0.0F,
            this.tabButtons.get(0).getX(),
            2,
            32,
            2
        );
        int afterLastTab = this.tabButtons.get(this.tabButtons.size() - 1).getRight();
        graphics.blit(
            RenderPipelines.GUI_TEXTURED,
            Screen.HEADER_SEPARATOR,
            afterLastTab,
            this.layout.getY() + this.layout.getHeight() - 2,
            0.0F,
            0.0F,
            this.width,
            2,
            32,
            2
        );

        for (TabButton value : this.tabButtons) {
            value.extractRenderState(graphics, mouseX, mouseY, a);
        }
    }

    @Override
    public ScreenRectangle getRectangle() {
        return this.layout.getRectangle();
    }

    public void arrangeElements() {
        int tabsWidth = Math.min(400, this.width) - 28;
        int tabWidth = Mth.roundToward(tabsWidth / this.tabs.size(), 2);

        for (TabButton button : this.tabButtons) {
            button.setWidth(tabWidth);
        }

        this.layout.arrangeElements();
        this.layout.setX(Mth.roundToward((this.width - tabsWidth) / 2, 2));
        this.layout.setY(0);
    }

    public void selectTab(int index, boolean playSound) {
        if (this.isFocused()) {
            this.setFocused(this.tabButtons.get(index));
        } else if (this.tabButtons.get(index).isActive()) {
            this.tabManager.setCurrentTab(this.tabs.get(index), playSound);
        }
    }

    public void setTabActiveState(int index, boolean active) {
        if (index >= 0 && index < this.tabButtons.size()) {
            this.tabButtons.get(index).active = active;
        }
    }

    public void setTabTooltip(int index, @Nullable Tooltip hint) {
        if (index >= 0 && index < this.tabButtons.size()) {
            this.tabButtons.get(index).setTooltip(hint);
        }
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        if (event.hasControlDownWithQuirk()) {
            int tabIndex = this.getNextTabIndex(event);
            if (tabIndex != -1) {
                this.selectTab(Mth.clamp(tabIndex, 0, this.tabs.size() - 1), true);
                return true;
            }
        }

        return false;
    }

    private int getNextTabIndex(KeyEvent event) {
        return this.getNextTabIndex(this.currentTabIndex(), event);
    }

    private int getNextTabIndex(int currentTab, KeyEvent event) {
        int digit = event.getDigit();
        if (digit != -1) {
            return Math.floorMod(digit - 1, 10);
        } else if (event.isCycleFocus() && currentTab != -1) {
            int nextTabIndex = event.hasShiftDown() ? currentTab - 1 : currentTab + 1;
            int index = Math.floorMod(nextTabIndex, this.tabs.size());
            return this.tabButtons.get(index).active ? index : this.getNextTabIndex(index, event);
        } else {
            return -1;
        }
    }

    private int currentTabIndex() {
        Tab currentTab = this.tabManager.getCurrentTab();
        int index = this.tabs.indexOf(currentTab);
        return index != -1 ? index : -1;
    }

    private @Nullable TabButton currentTabButton() {
        int index = this.currentTabIndex();
        return index != -1 ? this.tabButtons.get(index) : null;
    }

    @OnlyIn(Dist.CLIENT)
    public static class Builder {
        private final int width;
        private final TabManager tabManager;
        private final List<Tab> tabs = new ArrayList<>();

        private Builder(TabManager tabManager, int width) {
            this.tabManager = tabManager;
            this.width = width;
        }

        public TabNavigationBar.Builder addTabs(Tab... tabs) {
            Collections.addAll(this.tabs, tabs);
            return this;
        }

        public TabNavigationBar build() {
            return new TabNavigationBar(this.width, this.tabManager, this.tabs);
        }
    }
}
