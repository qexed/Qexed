package com.mojang.blaze3d.platform;

import com.mojang.blaze3d.systems.RenderSystem;
import com.mojang.logging.LogUtils;
import it.unimi.dsi.fastutil.longs.Long2ObjectMap;
import it.unimi.dsi.fastutil.longs.Long2ObjectOpenHashMap;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.lwjgl.PointerBuffer;
import org.lwjgl.glfw.GLFW;
import org.lwjgl.glfw.GLFWMonitorCallback;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
public class ScreenManager {
    private static final Logger LOGGER = LogUtils.getLogger();
    private final Long2ObjectMap<Monitor> monitors = new Long2ObjectOpenHashMap<>();
    private final MonitorCreator monitorCreator;

    public ScreenManager(MonitorCreator monitorCreator) {
        this.monitorCreator = monitorCreator;
        GLFW.glfwSetMonitorCallback(this::onMonitorChange);
        PointerBuffer buffer = GLFW.glfwGetMonitors();
        if (buffer != null) {
            for (int i = 0; i < buffer.limit(); i++) {
                long monitor = buffer.get(i);
                this.monitors.put(monitor, monitorCreator.createMonitor(monitor));
            }
        }
    }

    private void onMonitorChange(long monitor, int event) {
        RenderSystem.assertOnRenderThread();
        if (event == 262145) {
            this.monitors.put(monitor, this.monitorCreator.createMonitor(monitor));
            LOGGER.debug("Monitor {} connected. Current monitors: {}", monitor, this.monitors);
        } else if (event == 262146) {
            this.monitors.remove(monitor);
            LOGGER.debug("Monitor {} disconnected. Current monitors: {}", monitor, this.monitors);
        }
    }

    public @Nullable Monitor getMonitor(long monitor) {
        return this.monitors.get(monitor);
    }

    public @Nullable Monitor findBestMonitor(Window window) {
        long windowMonitor = GLFW.glfwGetWindowMonitor(window.handle());
        if (windowMonitor != 0L) {
            return this.getMonitor(windowMonitor);
        } else {
            int winMinX = window.getX();
            int winMaxX = winMinX + window.getScreenWidth();
            int winMinY = window.getY();
            int winMaxY = winMinY + window.getScreenHeight();
            int maxArea = -1;
            Monitor result = null;
            long primaryMonitor = GLFW.glfwGetPrimaryMonitor();
            LOGGER.debug("Selecting monitor - primary: {}, current monitors: {}", primaryMonitor, this.monitors);

            for (Monitor monitor : this.monitors.values()) {
                int monMinX = monitor.getX();
                int monMaxX = monMinX + monitor.getCurrentMode().getWidth();
                int monMinY = monitor.getY();
                int monMaxY = monMinY + monitor.getCurrentMode().getHeight();
                int minX = clamp(winMinX, monMinX, monMaxX);
                int maxX = clamp(winMaxX, monMinX, monMaxX);
                int minY = clamp(winMinY, monMinY, monMaxY);
                int maxY = clamp(winMaxY, monMinY, monMaxY);
                int sx = Math.max(0, maxX - minX);
                int sy = Math.max(0, maxY - minY);
                int area = sx * sy;
                if (area > maxArea) {
                    result = monitor;
                    maxArea = area;
                } else if (area == maxArea && primaryMonitor == monitor.getMonitor()) {
                    LOGGER.debug("Primary monitor {} is preferred to monitor {}", monitor, result);
                    result = monitor;
                }
            }

            LOGGER.debug("Selected monitor: {}", result);
            return result;
        }
    }

    public static int clamp(int value, int min, int max) {
        if (value < min) {
            return min;
        } else {
            return value > max ? max : value;
        }
    }

    public void shutdown() {
        RenderSystem.assertOnRenderThread();
        GLFWMonitorCallback callback = GLFW.glfwSetMonitorCallback(null);
        if (callback != null) {
            callback.free();
        }
    }
}
