package net.minecraft.client.server;

import com.google.common.base.MoreObjects;
import com.google.common.collect.Lists;
import com.mojang.authlib.GameProfile;
import com.mojang.logging.LogUtils;
import java.io.IOException;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.Collection;
import java.util.List;
import java.util.Optional;
import java.util.UUID;
import java.util.function.BooleanSupplier;
import net.minecraft.CrashReport;
import net.minecraft.SharedConstants;
import net.minecraft.SystemReport;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.toasts.SystemToast;
import net.minecraft.core.BlockPos;
import net.minecraft.core.GlobalPos;
import net.minecraft.gizmos.Gizmos;
import net.minecraft.gizmos.SimpleGizmoCollector;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.Services;
import net.minecraft.server.WorldStem;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.level.progress.LevelLoadListener;
import net.minecraft.server.packs.repository.PackRepository;
import net.minecraft.server.permissions.LevelBasedPermissionSet;
import net.minecraft.server.permissions.PermissionSet;
import net.minecraft.server.players.NameAndId;
import net.minecraft.stats.Stats;
import net.minecraft.util.ModCheck;
import net.minecraft.util.ProblemReporter;
import net.minecraft.util.debugchart.LocalSampleLogger;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.ProfilerFiller;
import net.minecraft.util.thread.BlockableEventLoop;
import net.minecraft.world.level.ChunkPos;
import net.minecraft.world.level.GameType;
import net.minecraft.world.level.chunk.storage.RegionStorageInfo;
import net.minecraft.world.level.gamerules.GameRules;
import net.minecraft.world.level.storage.LevelStorageSource;
import net.minecraft.world.level.storage.TagValueInput;
import net.minecraft.world.level.storage.ValueInput;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.slf4j.Logger;

@OnlyIn(Dist.CLIENT)
public class IntegratedServer extends MinecraftServer {
    private static final Logger LOGGER = LogUtils.getLogger();
    private static final int MIN_SIM_DISTANCE = 2;
    public static final int MAX_PLAYERS = 8;
    private final Minecraft minecraft;
    private boolean paused = true;
    private int publishedPort = -1;
    private @Nullable GameType publishedGameType;
    private @Nullable LanServerPinger lanPinger;
    private @Nullable UUID uuid;
    private int previousSimulationDistance = 0;
    private volatile List<SimpleGizmoCollector.GizmoInstance> latestTicksGizmos = new ArrayList<>();
    private final SimpleGizmoCollector gizmoCollector = new SimpleGizmoCollector();

    public IntegratedServer(
        Thread serverThread,
        Minecraft minecraft,
        LevelStorageSource.LevelStorageAccess levelStorageAccess,
        PackRepository packRepository,
        WorldStem worldStem,
        Optional<GameRules> gameRules,
        Services services,
        LevelLoadListener levelLoadListener
    ) {
        super(
            serverThread,
            levelStorageAccess,
            packRepository,
            worldStem,
            gameRules,
            minecraft.getProxy(),
            minecraft.getFixerUpper(),
            services,
            levelLoadListener,
            false
        );
        this.setSingleplayerProfile(minecraft.getGameProfile());
        this.setDemo(minecraft.isDemo());
        this.setPlayerList(new IntegratedPlayerList(this, this.registries(), this.playerDataStorage));
        this.minecraft = minecraft;
    }

    @Override
    protected boolean initServer() {
        LOGGER.info("Starting integrated minecraft server version {}", SharedConstants.getCurrentVersion().name());
        this.setUsesAuthentication(true);
        this.initializeKeyPair();
        net.neoforged.neoforge.server.ServerLifecycleHooks.handleServerAboutToStart(this);
        this.loadLevel();
        GameProfile host = this.getSingleplayerProfile();
        String levelName = this.getWorldData().getLevelName();
        this.setMotd(host != null ? host.name() + " - " + levelName : levelName);
        this.saveEverything(false, true, true);
        net.neoforged.neoforge.server.ServerLifecycleHooks.handleServerStarting(this);
        return true;
    }

    @Override
    public boolean isPaused() {
        return this.paused;
    }

    @Override
    protected void processPacketsAndTick(boolean sprinting) {
        try (Gizmos.TemporaryCollection ignored = Gizmos.withCollector(this.gizmoCollector)) {
            super.processPacketsAndTick(sprinting);
        }

        if (this.tickRateManager().runsNormally()) {
            this.latestTicksGizmos = this.gizmoCollector.drainGizmos();
        }
    }

    @Override
    protected void tickServer(BooleanSupplier haveTime) {
        boolean wasPaused = this.paused;
        this.paused = Minecraft.getInstance().isPaused() || this.getPlayerList().getPlayers().isEmpty();
        ProfilerFiller profiler = Profiler.get();
        if (!wasPaused && this.paused) {
            profiler.push("autoSave");
            LOGGER.info("Saving and pausing game...");
            this.saveEverything(false, false, false);
            profiler.pop();
        }

        if (this.paused) {
            this.tickPaused();
        } else {
            if (wasPaused) {
                this.forceGameTimeSynchronization();
            }

            super.tickServer(haveTime);
            int serverViewDistance = Math.max(2, this.minecraft.options.renderDistance().get());
            if (serverViewDistance != this.getPlayerList().getViewDistance()) {
                LOGGER.info("Changing view distance to {}, from {}", serverViewDistance, this.getPlayerList().getViewDistance());
                this.getPlayerList().setViewDistance(serverViewDistance);
            }

            int serverSimulationDistance = Math.max(2, this.minecraft.options.simulationDistance().get());
            if (serverSimulationDistance != this.previousSimulationDistance) {
                LOGGER.info("Changing simulation distance to {}, from {}", serverSimulationDistance, this.previousSimulationDistance);
                this.getPlayerList().setSimulationDistance(serverSimulationDistance);
                this.previousSimulationDistance = serverSimulationDistance;
            }
        }
    }

    protected LocalSampleLogger getTickTimeLogger() {
        return this.minecraft.getDebugOverlay().getTickTimeLogger();
    }

    @Override
    public boolean isTickTimeLoggingEnabled() {
        return true;
    }

    private void tickPaused() {
        this.tickConnection();

        for (ServerPlayer player : this.getPlayerList().getPlayers()) {
            player.awardStat(Stats.TOTAL_WORLD_TIME);
        }
    }

    @Override
    public boolean shouldRconBroadcast() {
        return true;
    }

    @Override
    public boolean shouldInformAdmins() {
        return true;
    }

    @Override
    public Path getServerDirectory() {
        return this.minecraft.gameDirectory.toPath();
    }

    @Override
    public boolean isDedicatedServer() {
        return false;
    }

    @Override
    public int getRateLimitPacketsPerSecond() {
        return 0;
    }

    @Override
    public boolean useNativeTransport() {
        return this.minecraft.options.useNativeTransport();
    }

    @Override
    protected void onServerCrash(CrashReport report) {
        BlockableEventLoop.relayDelayCrash(report);
    }

    @Override
    public SystemReport fillServerSystemReport(SystemReport systemReport) {
        systemReport.setDetail("Type", "Integrated Server");
        systemReport.setDetail("Is Modded", () -> this.getModdedStatus().fullDescription());
        systemReport.setDetail("Launched Version", this.minecraft::getLaunchedVersion);
        return systemReport;
    }

    @Override
    public ModCheck getModdedStatus() {
        return Minecraft.checkModStatus().merge(super.getModdedStatus());
    }

    @Override
    public boolean publishServer(@Nullable GameType gameMode, boolean allowCommands, int port) {
        try {
            this.minecraft.prepareForMultiplayer();
            this.minecraft.getConnection().prepareKeyPair();
            this.getConnection().startTcpServerListener(null, port);
            LOGGER.info("Started serving on {}", port);
            this.publishedPort = port;
            this.lanPinger = new LanServerPinger(this.getMotd(), port + "");
            this.lanPinger.start();
            this.publishedGameType = gameMode;
            this.getPlayerList().setAllowCommandsForAllPlayers(allowCommands);
            PermissionSet newProfilePermissions = this.getProfilePermissions(this.minecraft.player.nameAndId());
            this.minecraft.player.setPermissions(newProfilePermissions);
            this.minecraft.player.refreshChatAbilities();

            for (ServerPlayer player : this.getPlayerList().getPlayers()) {
                this.getCommands().sendCommands(player);
            }

            return true;
        } catch (IOException var7) {
            return false;
        }
    }

    @Override
    public void stopServer() {
        super.stopServer();
        if (this.lanPinger != null) {
            this.lanPinger.interrupt();
            this.lanPinger = null;
        }
    }

    @Override
    public void halt(boolean wait) {
        if (isRunning())
        this.executeBlocking(() -> {
            for (ServerPlayer player : Lists.newArrayList(this.getPlayerList().getPlayers())) {
                if (!player.getUUID().equals(this.uuid)) {
                    this.getPlayerList().remove(player);
                }
            }
        });
        super.halt(wait);
        if (this.lanPinger != null) {
            this.lanPinger.interrupt();
            this.lanPinger = null;
        }
    }

    @Override
    public boolean isPublished() {
        return this.publishedPort > -1;
    }

    @Override
    public int getPort() {
        return this.publishedPort;
    }

    @Override
    public void setDefaultGameType(GameType gameType) {
        super.setDefaultGameType(gameType);
        this.publishedGameType = null;
    }

    @Override
    public LevelBasedPermissionSet operatorUserPermissions() {
        return LevelBasedPermissionSet.GAMEMASTER;
    }

    public LevelBasedPermissionSet getFunctionCompilationPermissions() {
        return LevelBasedPermissionSet.GAMEMASTER;
    }

    public void setUUID(UUID uuid) {
        this.uuid = uuid;
    }

    @Override
    public boolean isSingleplayerOwner(NameAndId nameAndId) {
        return this.getSingleplayerProfile() != null && nameAndId.name().equalsIgnoreCase(this.getSingleplayerProfile().name());
    }

    @Override
    public int getScaledTrackingDistance(int baseRange) {
        return (int)(this.minecraft.options.entityDistanceScaling().get() * baseRange);
    }

    @Override
    public boolean forceSynchronousWrites() {
        return this.minecraft.options.syncWrites;
    }

    @Override
    public @Nullable GameType getForcedGameType() {
        return this.isPublished() && !this.isHardcore() ? MoreObjects.firstNonNull(this.publishedGameType, this.worldData.getGameType()) : null;
    }

    @Override
    protected GlobalPos selectLevelLoadFocusPos() {
        UUID lastSinglePlayerOwnerUUID = this.worldData.getSinglePlayerUUID();
        if (lastSinglePlayerOwnerUUID == null) {
            return super.selectLevelLoadFocusPos();
        } else {
            Optional<CompoundTag> playerData = this.playerDataStorage.load(new NameAndId(lastSinglePlayerOwnerUUID, "<single player owner>"));
            if (playerData.isEmpty()) {
                return super.selectLevelLoadFocusPos();
            } else {
                try (ProblemReporter.ScopedCollector reporter = new ProblemReporter.ScopedCollector(LOGGER)) {
                    ValueInput input = TagValueInput.create(reporter, this.registryAccess(), playerData.get());
                    ServerPlayer.SavedPosition loadedPosition = input.read(ServerPlayer.SavedPosition.MAP_CODEC).orElse(ServerPlayer.SavedPosition.EMPTY);
                    if (loadedPosition.dimension().isPresent() && loadedPosition.position().isPresent()) {
                        return new GlobalPos(loadedPosition.dimension().get(), BlockPos.containing(loadedPosition.position().get()));
                    }
                }

                return super.selectLevelLoadFocusPos();
            }
        }
    }

    @Override
    public void sendLowDiskSpaceWarning() {
        super.sendLowDiskSpaceWarning();
        this.minecraft.sendLowDiskSpaceWarning();
    }

    @Override
    public void reportChunkLoadFailure(Throwable throwable, RegionStorageInfo storageInfo, ChunkPos pos) {
        super.reportChunkLoadFailure(throwable, storageInfo, pos);
        this.warnOnLowDiskSpace();
        this.minecraft.execute(() -> SystemToast.onChunkLoadFailure(this.minecraft, pos));
    }

    @Override
    public void reportChunkSaveFailure(Throwable throwable, RegionStorageInfo storageInfo, ChunkPos pos) {
        super.reportChunkSaveFailure(throwable, storageInfo, pos);
        this.warnOnLowDiskSpace();
        this.minecraft.execute(() -> SystemToast.onChunkSaveFailure(this.minecraft, pos));
    }

    @Override
    public int getMaxPlayers() {
        return 8;
    }

    public Collection<SimpleGizmoCollector.GizmoInstance> getPerTickGizmos() {
        return this.latestTicksGizmos;
    }
}
