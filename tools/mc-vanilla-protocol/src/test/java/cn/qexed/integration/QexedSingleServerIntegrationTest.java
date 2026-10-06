package cn.qexed.integration;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.Callable;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.Executors;
import java.util.concurrent.TimeUnit;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

@Tag("integration")
final class QexedSingleServerIntegrationTest {
    private static final Duration READY_TIMEOUT = Duration.ofSeconds(20);

    private static QexedProtocolClient connect(QexedServerProcess server, String username) throws Exception {
        try {
            return new QexedProtocolClient(username, server.port());
        } catch (Throwable error) {
            server.markFailed();
            fail("client failed to connect: " + error + "\nqexed logs:\n" + String.join("\n", server.logs()), error);
            throw error;
        }
    }

    @Test
    void concurrentLoginAndChunkLoadDoesNotBlockPlayPackets() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("login", false)) {
            var pool = Executors.newFixedThreadPool(6);
            List<Callable<Void>> tasks = new ArrayList<>();
            for (int i = 0; i < 6; i++) {
                int index = i;
                tasks.add(() -> {
                    try (QexedProtocolClient client = connect(server, "Load" + index)) {
                        client.waitForPlayReady(READY_TIMEOUT);
                        client.sendMove(index + 0.5, 64.0, 0.5, true);
                        client.pump(Duration.ofMillis(500));
                    }
                    return null;
                });
            }
            for (var result : pool.invokeAll(tasks, 40, TimeUnit.SECONDS)) {
                result.get();
            }
            pool.shutdownNow();
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void miningDuringInitialChunkLoadReturnsAcknowledgement() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("mining", false);
                QexedProtocolClient client = connect(server, "Miner")) {
            client.sendBreakBlock(0, 63, 0, 10);
            client.waitForPlayReady(READY_TIMEOUT);
            client.waitForPacket(QexedProtocolClient.CLIENTBOUND_PLAY_BLOCK_CHANGED_ACK, Duration.ofSeconds(10));
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void creativePlacementUseAndContainerPacketsStayDecodable() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("creative", true);
                QexedProtocolClient client = connect(server, "Builder")) {
            client.waitForPlayReady(READY_TIMEOUT);
            client.sendCreativeSlot(36, 1029, 64);
            client.sendSetCarriedItem(0);
            client.sendUseItemOn(0, 63, 0, 1, 20);
            client.sendUseItem(21);
            client.sendContainerClick(0, 36);
            client.pump(Duration.ofSeconds(2));
            assertTrue(
                    client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_BLOCK_UPDATE)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_SLOT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_CONTENT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SET_PLAYER_INVENTORY)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_BLOCK_CHANGED_ACK),
                    "expected block or inventory feedback, seen=" + client.seenPackets().stream()
                            .map(QexedProtocolClient.PacketRecord::idHex)
                            .toList());
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void recipesAreSentBeforeInventoryCraftingSmoke() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("recipes", true);
                QexedProtocolClient client = connect(server, "Crafter")) {
            client.waitForPlayReady(READY_TIMEOUT);
            client.waitForPacket(QexedProtocolClient.CLIENTBOUND_PLAY_UPDATE_RECIPES, Duration.ofSeconds(10));
            client.waitForPacket(0x4a, Duration.ofSeconds(10));
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void attackingConfiguredEntitySendsDamageMotionAndDeathPackets() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("combat", true);
                QexedProtocolClient client = connect(server, "Fighter")) {
            client.waitForPlayReady(READY_TIMEOUT);
            QexedProtocolClient.PacketRecord addEntity =
                    client.firstSeenPacket(QexedProtocolClient.CLIENTBOUND_PLAY_ADD_ENTITY);
            int targetEntityId = new PacketBuffer(addEntity.payload()).readVarInt();
            client.sendMove(0.5, 64.0, 0.5, true);
            for (int i = 0; i < 30; i++) {
                client.sendAttack(targetEntityId);
                client.pump(Duration.ofMillis(100));
                if (client.seenPackets().stream().anyMatch(packet ->
                        packet.id() == QexedProtocolClient.CLIENTBOUND_PLAY_ENTITY_EVENT
                                && packet.payload().length >= 5
                                && packet.payload()[4] == 3)) {
                    break;
                }
            }
            assertTrue(client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_DAMAGE_EVENT), "missing damage event");
            assertTrue(client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SET_ENTITY_MOTION), "missing motion packet");
            assertTrue(client.seenPackets().stream().anyMatch(packet ->
                            packet.id() == QexedProtocolClient.CLIENTBOUND_PLAY_ENTITY_EVENT
                                    && packet.payload().length >= 5
                                    && packet.payload()[4] == 3),
                    "missing death animation entity event");
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void droppingAndPickingUpItemsKeepsInventoryMergePathAlive() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("pickup", true);
                QexedProtocolClient client = connect(server, "Picker")) {
            client.waitForPlayReady(READY_TIMEOUT);
            client.sendCreativeSlot(36, 1, 64);
            client.sendCreativeSlot(37, 1, 16);
            client.sendSetCarriedItem(0);
            client.sendDropSelectedStack();
            client.pump(Duration.ofMillis(700));
            client.sendMove(0.4, 64.0, 0.4, true);
            client.pump(Duration.ofSeconds(3));
            assertTrue(
                    client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_TAKE_ITEM_ENTITY)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_SLOT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_CONTENT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SET_PLAYER_INVENTORY),
                    "expected pickup or inventory feedback");
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void defaultPlayerCommandsExposeHelpAndListButDenyTeleport() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("commands-default", false);
                QexedProtocolClient client = connect(server, "CmdUser")) {
            client.waitForPlayReady(READY_TIMEOUT);
            assertTrue(client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_COMMANDS), "missing command tree");

            client.sendCommandSuggestion(41, "/");
            QexedProtocolClient.CommandSuggestions suggestions =
                    client.waitForCommandSuggestions(41, Duration.ofSeconds(10));
            assertTrue(suggestions.matches().contains("help"), "missing /help suggestion: " + suggestions.matches());
            assertTrue(suggestions.matches().contains("list"), "missing /list suggestion: " + suggestions.matches());

            client.sendChatCommand("help");
            QexedProtocolClient.SystemChatMessage help =
                    client.waitForSystemChatContaining("/help", Duration.ofSeconds(10));
            assertTrue(help.text().contains("/list"), "help should include /list: " + help.text());
            assertFalse(help.text().contains("/tp"), "default help must not expose denied /tp: " + help.text());

            client.sendChatCommand("list");
            client.waitForSystemChatContaining("CmdUser", Duration.ofSeconds(10));

            client.sendChatCommand("tp 1 65 1");
            client.waitForSystemChatContaining("You do not have permission", Duration.ofSeconds(10));
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void allowedPlayerCommandsExecuteTeleportTimeGameruleAndScoreboard() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("commands-allowed", true, true);
                QexedProtocolClient client = connect(server, "Operator")) {
            client.waitForPlayReady(READY_TIMEOUT);

            client.sendCommandSuggestion(42, "/t");
            QexedProtocolClient.CommandSuggestions suggestions =
                    client.waitForCommandSuggestions(42, Duration.ofSeconds(10));
            assertTrue(suggestions.matches().contains("tp"), "missing /tp suggestion: " + suggestions.matches());
            assertTrue(suggestions.matches().contains("time"), "missing /time suggestion: " + suggestions.matches());

            client.sendChatCommand("time query minecraft:overworld");
            client.waitForSystemChatContaining("commands.time.query", Duration.ofSeconds(10));

            client.sendChatCommand("gamerule minecraft:overworld doDaylightCycle");
            client.waitForSystemChatContaining("doDaylightCycle", Duration.ofSeconds(10));

            client.sendChatCommand("scoreboard objectives list");
            client.waitForSystemChatContaining("commands.scoreboard.objectives.list.success", Duration.ofSeconds(10));

            client.sendChatCommand("tp 3 65 4");
            QexedProtocolClient.PositionPacket position =
                    client.waitForTeleportAfterCommand(Duration.ofSeconds(10));
            assertTrue(Math.abs(position.x() - 3.0) < 0.001, "unexpected teleport x: " + position);
            assertTrue(Math.abs(position.y() - 65.0) < 0.001, "unexpected teleport y: " + position);
            assertTrue(Math.abs(position.z() - 4.0) < 0.001, "unexpected teleport z: " + position);
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void pluginDependencyApiCallAndEconomyStorageWorkAfterLogin() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.startWithPlugins(
                        "plugin-api", false, true, "api_provider_demo", "api_consumer_demo");
                QexedProtocolClient client = connect(server, "PluginUser")) {
            client.waitForPlayReady(READY_TIMEOUT);
            client.sendChatCommand("apitest");
            QexedProtocolClient.SystemChatMessage message =
                    client.waitForSystemChatContaining("api=provider:ok;exists=true;storage=redis", Duration.ofSeconds(10));
            assertTrue(message.text().contains("provider:ok"), "unexpected plugin API response: " + message.text());
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void clusteredRegionGatewayKeepsSingleSessionAcrossMovementEntityAndItems() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.startClusterGateway("cluster-regions");
                QexedProtocolClient client = connect(server, "ClusterUser")) {
            client.waitForPlayReady(READY_TIMEOUT);
            String worldConfig = java.nio.file.Files.readString(server.runDir().resolve("config/world.toml"));
            assertTrue(worldConfig.contains("mode = \"regions\""), "cluster must use arbitrary regions mode");
            assertTrue(worldConfig.contains("id = \"spawn\""), "missing spawn shard");
            assertTrue(worldConfig.contains("id = \"east\""), "missing east shard");
            assertTrue(worldConfig.contains("id = \"north\""), "missing north shard");
            assertTrue(worldConfig.contains("id = \"west\""), "missing west shard");
            assertTrue(worldConfig.contains("id = \"south\""), "missing south shard");
            assertTrue(worldConfig.contains("id = \"far\""), "missing far shard");
            assertTrue(worldConfig.contains("endpoint = \"tcp://127.0.0.1:26001\""),
                    "cluster test must use real TCP shard endpoints");
            assertTrue(worldConfig.contains("y = 255.0"), "spawn must be configured at y=255");
            assertTrue(
                    client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED),
                    "cluster gateway did not stream chunks in the existing play session");
            client.sendPlayerLoaded();
            assertTrue(server.waitForLogContaining("client reported player loaded", Duration.ofSeconds(10)),
                    "server did not handle player_loaded packet");

            QexedProtocolClient.PacketRecord addEntity =
                    client.firstSeenPacket(QexedProtocolClient.CLIENTBOUND_PLAY_ADD_ENTITY);
            int targetEntityId = new PacketBuffer(addEntity.payload()).readVarInt();
            client.sendMove(0.5, 255.0, 0.5, true);
            client.pump(Duration.ofMillis(500));
            client.sendMove(40.5, 255.0, 0.5, true);
            client.pump(Duration.ofMillis(500));
            client.sendMove(160.5, 255.0, 160.5, true);
            client.pump(Duration.ofMillis(500));
            client.sendAttack(targetEntityId);
            client.pump(Duration.ofMillis(800));
            assertTrue(
                    client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_DAMAGE_EVENT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SET_ENTITY_MOTION)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_ENTITY_EVENT),
                    "entity interaction path did not produce combat/AI feedback");

            client.sendCreativeSlot(36, 1, 64);
            client.sendSetCarriedItem(0);
            client.sendDropSelectedStack();
            client.pump(Duration.ofMillis(700));
            client.sendMove(160.5, 255.0, 160.5, true);
            client.pump(Duration.ofSeconds(2));
            assertTrue(
                    client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_TAKE_ITEM_ENTITY)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_SLOT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_CONTAINER_SET_CONTENT)
                            || client.sawPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SET_PLAYER_INVENTORY),
                    "drop/pickup path did not produce item or inventory feedback");
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void threeThousandAiEntitiesDoNotBlockProtocolInteractions() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.startWithAiStress("ai-stress-3000");
                QexedProtocolClient client = connect(server, "StressClient")) {
            client.waitForPlayReady(READY_TIMEOUT);
            Thread.sleep(2_000);
            long started = System.nanoTime();
            for (int i = 0; i < 12; i++) {
                client.sendMove(0.5 + i * 0.15, 64.0, 0.5, true);
                client.sendBreakBlock(i % 2, 63, i % 2, 100 + i * 2);
                client.waitForPacket(QexedProtocolClient.CLIENTBOUND_PLAY_BLOCK_CHANGED_ACK, Duration.ofSeconds(3));
            }
            client.sendChatCommand("list");
            client.waitForSystemChatContaining("StressClient", Duration.ofSeconds(3));
            client.pump(Duration.ofSeconds(2));
            long elapsedMs = TimeUnit.NANOSECONDS.toMillis(System.nanoTime() - started);
            assertTrue(elapsedMs < 15_000, "3000 AI entity interaction loop was too slow: " + elapsedMs + "ms");
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void consoleCommandsBypassPlayerPermissionPolicy() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("commands-console", false)) {
            server.sendConsole("help");
            server.sendConsole("status");
            server.sendConsole("list");
            server.sendConsole("say console smoke");
            server.sendConsole("tp 1 2 3");
            assertTrue(server.waitForLogContaining("[Console]", Duration.ofSeconds(10)),
                    "console did not print command output: " + String.join("\n", server.logs()));
            assertFalse(server.logs().stream().anyMatch(line -> line.contains("You do not have permission")),
                    "console command was denied:\n" + String.join("\n", server.logs()));
            server.assertNoConnectionErrors();
        }
    }

    @Test
    void realVanillaClientConnectsWithoutDecoderException() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("real-client", false)) {
            java.nio.file.Path projectDir = java.nio.file.Path.of(System.getProperty("user.dir"))
                    .toAbsolutePath()
                    .normalize();
            java.nio.file.Path smokeBuildDir = java.nio.file.Files.createTempDirectory("qexed-smoke-build-");
            terminateStaleVanillaGradleClients(projectDir);
            List<String> clientLogs = new CopyOnWriteArrayList<>();
            Process client = new ProcessBuilder(
                            "C:/gradle/gradle-9.5.1/bin/gradle.bat",
                            "-PqexedSmokeBuildDir=" + smokeBuildDir,
                            "-PqexedQuickPlay=127.0.0.1:" + server.port(),
                            "runClient",
                            "--no-daemon",
                            "--console=plain")
                    .directory(projectDir.toFile())
                    .redirectErrorStream(true)
                    .start();
            Thread output = new Thread(() -> {
                try (var reader = new java.io.BufferedReader(new java.io.InputStreamReader(
                        client.getInputStream(), java.nio.charset.StandardCharsets.UTF_8))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        clientLogs.add(line);
                    }
                } catch (Exception error) {
                    clientLogs.add("failed to read client output: " + error);
                }
            }, "minecraft-client-smoke-log");
            output.setDaemon(true);
            output.start();
            boolean passed = false;
            try {
                boolean enteredPlay = waitForClientGameActivity(clientLogs, Duration.ofSeconds(300));
                assertTrue(enteredPlay, "vanilla client did not enter play state. qexed logs=\n"
                        + String.join("\n", server.logs()) + "\nclient logs=\n" + String.join("\n", clientLogs));
                Thread.sleep(5_000);
                server.assertNoConnectionErrors();
                assertTrue(clientLogs.stream().noneMatch(QexedSingleServerIntegrationTest::isClientProtocolFailure),
                        "client protocol failure:\n" + String.join("\n", clientLogs));
                passed = true;
            } finally {
                destroyProcessTree(client.toHandle());
                if (!client.waitFor(10, TimeUnit.SECONDS)) {
                    client.destroyForcibly();
                }
                output.join(5_000);
                if (passed) {
                    deleteDirectoryIfExists(smokeBuildDir);
                } else {
                    System.err.println("vanilla smoke build kept for diagnosis: " + smokeBuildDir);
                }
            }
        }
    }

    @Test
    void realVanillaClientConnectsToClusterRegionGateway() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.startClusterGateway("real-client-cluster")) {
            java.nio.file.Path projectDir = java.nio.file.Path.of(System.getProperty("user.dir"))
                    .toAbsolutePath()
                    .normalize();
            java.nio.file.Path smokeBuildDir = java.nio.file.Files.createTempDirectory("qexed-cluster-smoke-build-");
            terminateStaleVanillaGradleClients(projectDir);
            List<String> clientLogs = new CopyOnWriteArrayList<>();
            Process client = new ProcessBuilder(
                            "C:/gradle/gradle-9.5.1/bin/gradle.bat",
                            "-PqexedSmokeBuildDir=" + smokeBuildDir,
                            "-PqexedQuickPlay=127.0.0.1:" + server.port(),
                            "runClient",
                            "--no-daemon",
                            "--console=plain")
                    .directory(projectDir.toFile())
                    .redirectErrorStream(true)
                    .start();
            Thread output = new Thread(() -> {
                try (var reader = new java.io.BufferedReader(new java.io.InputStreamReader(
                        client.getInputStream(), java.nio.charset.StandardCharsets.UTF_8))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        clientLogs.add(line);
                    }
                } catch (Exception error) {
                    clientLogs.add("failed to read client output: " + error);
                }
            }, "minecraft-cluster-client-smoke-log");
            output.setDaemon(true);
            output.start();
            boolean passed = false;
            try {
                boolean enteredPlay = waitForClientGameActivity(clientLogs, Duration.ofSeconds(300));
                assertTrue(enteredPlay, "vanilla client did not enter cluster gateway play state. qexed logs=\n"
                        + String.join("\n", server.logs()) + "\nclient logs=\n" + String.join("\n", clientLogs));
                Thread.sleep(5_000);
                String worldConfig = java.nio.file.Files.readString(server.runDir().resolve("config/world.toml"));
                assertTrue(worldConfig.contains("mode = \"regions\""), "cluster gateway must use regions mode");
                assertTrue(worldConfig.contains("id = \"far\""), "cluster gateway must not be limited to ABCD shards");
                assertTrue(worldConfig.contains("endpoint = \"tcp://127.0.0.1:26001\""),
                        "cluster gateway must use real TCP shard endpoints");
                server.assertNoConnectionErrors();
                assertTrue(clientLogs.stream().noneMatch(QexedSingleServerIntegrationTest::isClientProtocolFailure),
                        "client protocol failure:\n" + String.join("\n", clientLogs));
                passed = true;
            } finally {
                destroyProcessTree(client.toHandle());
                if (!client.waitFor(10, TimeUnit.SECONDS)) {
                    client.destroyForcibly();
                }
                output.join(5_000);
                if (passed) {
                    deleteDirectoryIfExists(smokeBuildDir);
                } else {
                    System.err.println("cluster vanilla smoke build kept for diagnosis: " + smokeBuildDir);
                }
            }
        }
    }

    private static boolean waitForClientGameActivity(List<String> clientLogs, Duration timeout)
            throws InterruptedException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            if (clientLogs.stream().anyMatch(QexedSingleServerIntegrationTest::isClientProtocolFailure)) {
                return false;
            }
            if (clientLogs.stream().anyMatch(line ->
                    line.contains("Resizing Chunk Sections UBO")
                            || line.contains("[System] [CHAT]")
                            || line.contains("ClientLevel"))) {
                return true;
            }
            Thread.sleep(250);
        }
        return false;
    }

    private static boolean isClientProtocolFailure(String line) {
        return line.contains("DecoderException")
                || line.contains("Failed to decode packet")
                || line.contains("recipe_book_add")
                || line.contains("Can't create item stack with properties");
    }

    private static void terminateStaleVanillaGradleClients(java.nio.file.Path projectDir) {
        String project = projectDir.toString().toLowerCase();
        String runNatives = projectDir.resolve("build").resolve("run-natives")
                .toAbsolutePath()
                .normalize()
                .toString()
                .toLowerCase();
        ProcessHandle.allProcesses()
                .filter(handle -> {
                    String commandLine = handle.info().commandLine().orElse("").toLowerCase();
                    return commandLine.contains(runNatives)
                            || (commandLine.contains(project)
                                    && commandLine.contains("net.minecraft.client.main.main"));
                })
                .forEach(QexedSingleServerIntegrationTest::destroyProcessTree);
    }

    private static void destroyProcessTree(ProcessHandle handle) {
        handle.descendants().forEach(QexedSingleServerIntegrationTest::destroyProcessTree);
        if (handle.isAlive()) {
            handle.destroy();
            try {
                handle.onExit().get(5, TimeUnit.SECONDS);
            } catch (Exception ignored) {
                if (handle.isAlive()) {
                    handle.destroyForcibly();
                }
            }
        }
    }

    private static void deleteDirectoryIfExists(java.nio.file.Path root) throws java.io.IOException {
        if (!java.nio.file.Files.exists(root)) {
            return;
        }
        try (var paths = java.nio.file.Files.walk(root)) {
            for (java.nio.file.Path path : paths.sorted(java.util.Comparator.reverseOrder()).toList()) {
                java.nio.file.Files.deleteIfExists(path);
            }
        }
    }
}
