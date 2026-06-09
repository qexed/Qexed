package cn.qexed.integration;

import static org.junit.jupiter.api.Assertions.fail;

import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStreamWriter;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;

final class QexedServerProcess implements AutoCloseable {
    private final Path runDir;
    private final Process process;
    private final List<Process> shardProcesses;
    private final int port;
    private final List<String> logs = new CopyOnWriteArrayList<>();
    private boolean failed;
    private final boolean keepRunDir;

    private QexedServerProcess(Path repoRoot, Path runDir, Process process, boolean keepRunDir, List<Process> shardProcesses, int port) {
        this.runDir = runDir;
        this.process = process;
        this.keepRunDir = keepRunDir;
        this.shardProcesses = shardProcesses;
        this.port = port;
        capture(process.getInputStream());
        capture(process.getErrorStream());
        for (Process shard : shardProcesses) {
            capture(shard.getInputStream());
            capture(shard.getErrorStream());
        }
    }

    static QexedServerProcess start(String name, boolean creative) throws IOException, InterruptedException {
        return start(name, creative, false);
    }

    static QexedServerProcess start(String name, boolean creative, boolean allowAllCommands)
            throws IOException, InterruptedException {
        return start(name, creative, allowAllCommands, false);
    }

    static QexedServerProcess startWithPlugins(String name, boolean creative, boolean allowAllCommands, String... plugins)
            throws IOException, InterruptedException {
        return start(name, creative, allowAllCommands, false, plugins);
    }

    static QexedServerProcess startWithAiStress(String name) throws IOException, InterruptedException {
        return start(name, true, true, true);
    }

    static QexedServerProcess startClusterGateway(String name) throws IOException, InterruptedException {
        return start(name, false, true, false, true);
    }

    private static QexedServerProcess start(
            String name, boolean creative, boolean allowAllCommands, boolean aiStress)
            throws IOException, InterruptedException {
        return start(name, creative, allowAllCommands, aiStress, false, new String[0]);
    }

    private static QexedServerProcess start(
            String name, boolean creative, boolean allowAllCommands, boolean aiStress, String... plugins)
            throws IOException, InterruptedException {
        return start(name, creative, allowAllCommands, aiStress, false, plugins);
    }

    private static QexedServerProcess start(
            String name,
            boolean creative,
            boolean allowAllCommands,
            boolean aiStress,
            boolean clusterGateway,
            String... plugins)
            throws IOException, InterruptedException {
        Path repoRoot = Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
        Path runDir = Files.createTempDirectory("qexed-it-" + name + "-");
        copyDirectory(repoRoot.resolve("run/config"), runDir.resolve("config"));
        int port = Integer.parseInt(System.getProperty("qexed.port", "25565"));
        writeTestConfig(runDir.resolve("config"), creative, allowAllCommands, aiStress, clusterGateway, port);
        copyPlugins(repoRoot, runDir, plugins);
        Path binary = qexedBinary(repoRoot);
        if (!Files.exists(binary)) {
            fail("qexed binary not found, run buildQexedForIntegration first: " + binary);
        }

        List<Process> shards = clusterGateway ? startClusterShards(binary, runDir) : List.of();
        Process process = new ProcessBuilder(binary.toString(), "--language", "zh-CN")
                .directory(runDir.toFile())
                .redirectErrorStream(false)
                .start();
        QexedServerProcess server = new QexedServerProcess(repoRoot, runDir, process, false, shards, port);
        server.waitForPort(Duration.ofSeconds(90));
        return server;
    }

    private static Path qexedBinary(Path repoRoot) {
        String configured = System.getProperty("qexed.binary", "").trim();
        if (!configured.isEmpty()) {
            return Path.of(configured).toAbsolutePath().normalize();
        }
        Path binary = repoRoot.resolve("target/debug/qexed.exe");
        if (!Files.exists(binary)) {
            binary = repoRoot.resolve("target/debug/qexed");
        }
        return binary;
    }

    private static List<Process> startClusterShards(Path binary, Path runDir) throws IOException, InterruptedException {
        List<Process> shards = new ArrayList<>();
        String[][] specs = {
                {"far", "127.0.0.1:26006"},
                {"spawn", "127.0.0.1:26001"},
                {"east", "127.0.0.1:26002"},
                {"north", "127.0.0.1:26003"},
                {"west", "127.0.0.1:26004"},
                {"south", "127.0.0.1:26005"},
        };
        for (String[] spec : specs) {
            Process shard = new ProcessBuilder(
                            binary.toString(),
                            "--language", "zh-CN",
                            "--cluster-shard-id", spec[0],
                            "--cluster-shard-listen", spec[1])
                    .directory(runDir.toFile())
                    .redirectErrorStream(false)
                    .start();
            shards.add(shard);
            waitForShardPort(shard, spec[1], Duration.ofSeconds(30));
        }
        return shards;
    }

    private static void waitForShardPort(Process process, String address, Duration timeout) throws InterruptedException {
        String[] parts = address.split(":", 2);
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            if (!process.isAlive()) {
                fail("cluster shard exited before opening port: " + address);
            }
            try (Socket socket = new Socket()) {
                socket.connect(new InetSocketAddress(parts[0], Integer.parseInt(parts[1])), 250);
                return;
            } catch (IOException ignored) {
                Thread.sleep(100);
            }
        }
        fail("cluster shard did not open " + address);
    }

    private static void copyPlugins(Path repoRoot, Path runDir, String... plugins) throws IOException {
        if (plugins.length == 0) {
            return;
        }
        Path targetDir = runDir.resolve("plugins");
        Files.createDirectories(targetDir);
        for (String plugin : plugins) {
            Path source = repoRoot.resolve("plugins/examples/target/wasm32-unknown-unknown/release/" + plugin + ".wasm");
            if (!Files.exists(source)) {
                fail("plugin wasm not found, run buildWasmPluginsForIntegration first: " + source);
            }
            Files.copy(source, targetDir.resolve(plugin + ".wasm"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        }
    }

    void sendConsole(String command) throws IOException {
        BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
        writer.write(command);
        writer.newLine();
        writer.flush();
    }

    void assertNoConnectionErrors() {
        List<String> failures = logs.stream()
                .filter(line -> line.contains("[ERROR]") || line.contains("DecoderException")
                        || line.contains("connection handling error"))
                .toList();
        if (!failures.isEmpty()) {
            failed = true;
            fail("qexed logged errors:\n" + String.join("\n", failures));
        }
    }

    List<String> logs() {
        return new ArrayList<>(logs);
    }

    boolean waitForLogContaining(String text, Duration timeout) throws InterruptedException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            if (logs.stream().anyMatch(line -> line.contains(text))) {
                return true;
            }
            Thread.sleep(250);
        }
        return false;
    }

    Path runDir() {
        return runDir;
    }

    int port() {
        return port;
    }

    void markFailed() {
        failed = true;
    }

    @Override
    public void close() throws Exception {
        if (process.isAlive()) {
            try {
                sendConsole("stop");
            } catch (IOException ignored) {
            }
            if (!process.waitFor(10, TimeUnit.SECONDS)) {
                process.destroy();
            }
            if (!process.waitFor(5, TimeUnit.SECONDS)) {
                process.destroyForcibly();
            }
        }
        for (Process shard : shardProcesses) {
            if (shard.isAlive()) {
                shard.destroy();
                if (!shard.waitFor(5, TimeUnit.SECONDS)) {
                    shard.destroyForcibly();
                }
            }
        }
        if (failed || keepRunDir) {
            System.err.println("qexed integration run kept for diagnosis: " + runDir);
            System.err.println(String.join("\n", logs));
        } else {
            deleteDirectory(runDir);
        }
    }

    private void waitForPort(Duration timeout) throws InterruptedException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            if (!process.isAlive()) {
                failed = true;
                fail("qexed exited before opening port. logs:\n" + String.join("\n", logs));
            }
            try (Socket socket = new Socket()) {
                socket.connect(new InetSocketAddress("127.0.0.1", port), 250);
                return;
            } catch (IOException ignored) {
                Thread.sleep(100);
            }
        }
        failed = true;
        fail("qexed did not open 127.0.0.1:25565. logs:\n" + String.join("\n", logs));
    }

    private void capture(InputStream stream) {
        Thread thread = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    logs.add(line);
                }
            } catch (IOException error) {
                logs.add("failed to read qexed log stream: " + error);
            }
        }, "qexed-log-capture");
        thread.setDaemon(true);
        thread.start();
    }

    private static void writeTestConfig(
            Path configDir, boolean creative, boolean allowAllCommands, boolean aiStress, boolean clusterGateway, int port)
            throws IOException {
        Files.writeString(configDir.resolve("qexed_server.toml"), """
                [server]
                ip = "127.0.0.1:%d"
                online = false
                max_player = 40
                display_players = true
                online_mode = false
                log_level = "debug"
                mojang_cache_path = "cache/mojang"
                network_compression_threshold = -1
                max_port_connections = 65535
                rate_limit_window_secs = 60
                rate_limit_max_attempts = 100
                motd = ["Qexed integration test"]
                code_of_conduct = false
                favicon = ""

                [server.click_detection]
                enable = false
                window_ms = 1000
                max_clicks = 100
                cancel_actions = false

                [[server.economy.currencies]]
                id = "qexed:coin"
                name = "Coin"
                symbol = "Q"
                fractional_digits = 2
                storage = "redis"
                """.formatted(port), StandardCharsets.UTF_8);
        Files.writeString(configDir.resolve("proxy.toml"), """
                [proxy]
                enable = false
                protocol = "QTunnel"
                server_id = ""
                token = "integration"
                online_mode = false
                """, StandardCharsets.UTF_8);
        Files.writeString(configDir.resolve("world.toml"), """
                default_dimension = "minecraft:overworld"
                path = "world"
                read_only = false
                generator = "vanilla_flat"
                generator_preset = "minecraft:classic_flat"
                seed = 0
                game_mode = "%s"
                spawn_protection_radius = 0
                dimension = "minecraft:overworld"
                dimension_type = "minecraft:overworld"
                view_distance = 3
                chunk_load_parallelism = 4
                chunk_update_delay_ms = 50
                simulation_distance = 3
                light = "static"
                light_algorithm = "fast"

                [[worlds]]
                id = "overworld"
                dimension = "minecraft:overworld"
                dimension_type = "minecraft:overworld"
                path = "world"

                [precompiled_chunks]
                enable = false
                light = true
                max_cached_packets = 256
                max_cached_packet_bytes = 16777216
                block_state_cache_limit = 65536

                %s

                %s

                [spawn]
                x = 0.0
                y = %.1f
                z = 0.0
                yaw = 0.0
                pitch = 0.0
                """.formatted(
                        creative ? "creative" : "survival",
                        clusterGateway ? clusterGatewayConfig() : "",
                        clusterGateway ? spawnPlatformConfig() : "",
                        clusterGateway ? 255.0 : 64.0),
                StandardCharsets.UTF_8);
        if (aiStress) {
            writeAiStressEntityConfig(configDir);
        } else {
            Files.writeString(configDir.resolve("qexed_entity.toml"), """
                [entities]
                enable = true
                dimension = "minecraft:overworld"

                [entities.spawning]
                enable = false
                tick_interval_ms = 1000
                ai_tick_interval_ms = 200
                global_cap = 70
                per_dimension_cap = 70
                per_type_cap = 20
                max_spawn_per_tick = 4
                player_activation_range = 64.0

                [[entities.list]]
                id = "attack_target"
                kind = "entity"
                entity_type = "minecraft:zombie"
                name = "attack_target"
                display_name = "Attack Target"
                x = 2.0
                y = 64.0
                z = 0.0
                yaw = 180.0
                pitch = 0.0
                on_ground = true
                """, StandardCharsets.UTF_8);
        }
        if (aiStress) {
            Files.writeString(configDir.resolve("qexed_entity_rendering.toml"), """
                    [entity_rendering]
                    default_distance = 8.0
                    player_distance = 64.0
                    npc_distance = 16.0
                    hologram_distance = 16.0
                    item_distance = 16.0
                    item_merge_radius = 2.0
                    item_merge_max_stack = 64
                    stack_threshold = 999999
                    stack_radius = 4.0
                    """, StandardCharsets.UTF_8);
        }
        Files.writeString(configDir.resolve("qexed_lobby.toml"), """
                [lobby]
                enable = false
                protect_world = false
                menu_title = "Server Selector"
                menu_rows = 3

                [lobby.navigator]
                enable = false
                slot = 4
                item = "minecraft:compass"
                name = "Server Selector"

                [lobby.broadcast]
                enable = false
                interval_secs = 60

                [lobby.boss_bar]
                enable = false
                title = "Welcome to Qexed"
                color = "green"
                overlay = "progress"
                darken_screen = false
                play_music = false
                create_world_fog = false

                [lobby.health_check]
                interval_secs = 15
                timeout_ms = 600
                """, StandardCharsets.UTF_8);
        if (allowAllCommands) {
            Files.writeString(configDir.resolve("qexed_permissions.toml"), """
                    [permissions]
                    engine = "local"
                    local_path = "config/qexed_permissions.toml"
                    table_prefix = "luckperms_"
                    server = "global"
                    world = "global"
                    default_group = "default"
                    allow_by_default = true
                    denied_message = "You do not have permission to use this command."

                    [groups.default]
                    permissions = ["*"]
                    groups = []

                    [permissions.mysql]
                    ip = "127.0.0.1"
                    port = 3306
                    username = ""
                    password = ""
                    database = ""
                    pool_max_size = 10
                    pool_min_idle = 2
                    connection_timeout = "30s"
                    idle_timeout = "5m"
                    use_ssl = false
                    charset = "utf8mb4"
                    options = []
                    """, StandardCharsets.UTF_8);
        }
    }

    private static void writeAiStressEntityConfig(Path configDir) throws IOException {
        StringBuilder config = new StringBuilder();
        config.append("""
                [entities]
                enable = true
                dimension = "minecraft:overworld"

                [entities.spawning]
                enable = true
                tick_interval_ms = 50
                ai_tick_interval_ms = 50
                global_cap = 3000
                per_dimension_cap = 3000
                per_type_cap = 3000
                max_spawn_per_tick = 3000
                player_activation_range = 128.0

                [[entities.spawning.rules]]
                id = "stress_followers"
                dimension = "minecraft:overworld"
                entity_type = "minecraft:zombie"
                weight = 1
                cap = 3000
                display_name = ""
                ai = "follow_nearest_player"
                auto_jump = true
                require_ground = false
                require_air = false
                position_attempts = 1
                min_x = 48.0
                max_x = 104.0
                min_y = 64.0
                max_y = 64.0
                min_z = 48.0
                max_z = 104.0

                """);
        Files.writeString(configDir.resolve("qexed_entity.toml"), config.toString(), StandardCharsets.UTF_8);
    }

    private static String clusterGatewayConfig() {
        return """
                [cluster]
                enable = true
                mode = "regions"

                [[cluster.shards]]
                id = "far"
                endpoint = "tcp://127.0.0.1:26006"
                min_chunk_x = 9
                min_chunk_z = 9

                [[cluster.shards]]
                id = "spawn"
                endpoint = "tcp://127.0.0.1:26001"
                min_chunk_x = 0
                max_chunk_x = 1
                min_chunk_z = 0
                max_chunk_z = 1

                [[cluster.shards]]
                id = "east"
                endpoint = "tcp://127.0.0.1:26002"
                min_chunk_x = 2
                max_chunk_x = 8
                min_chunk_z = 0
                max_chunk_z = 8

                [[cluster.shards]]
                id = "north"
                endpoint = "tcp://127.0.0.1:26003"
                min_chunk_x = 0
                max_chunk_x = 1
                min_chunk_z = 2
                max_chunk_z = 8

                [[cluster.shards]]
                id = "west"
                endpoint = "tcp://127.0.0.1:26004"
                max_chunk_x = -1

                [[cluster.shards]]
                id = "south"
                endpoint = "tcp://127.0.0.1:26005"
                min_chunk_x = 0
                max_chunk_z = -1
                """;
    }

    private static String spawnPlatformConfig() {
        return """
                [spawn_platform]
                enable = true
                dimension = "minecraft:overworld"
                block = "minecraft:grass_block"
                y = 254
                min_x = -16
                max_x = 31
                min_z = -16
                max_z = 31
                """;
    }

    private static void copyDirectory(Path from, Path to) throws IOException {
        Files.createDirectories(to);
        try (var paths = Files.walk(from)) {
            for (Path source : paths.toList()) {
                Path target = to.resolve(from.relativize(source));
                if (Files.isDirectory(source)) {
                    Files.createDirectories(target);
                } else {
                    Files.copy(source, target);
                }
            }
        }
    }

    private static void deleteDirectory(Path root) throws IOException {
        if (!Files.exists(root)) {
            return;
        }
        try (var paths = Files.walk(root)) {
            for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) {
                Files.deleteIfExists(path);
            }
        }
    }
}
