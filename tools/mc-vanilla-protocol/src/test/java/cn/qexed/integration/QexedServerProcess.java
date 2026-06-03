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
    private final List<String> logs = new CopyOnWriteArrayList<>();
    private boolean failed;
    private final boolean keepRunDir;

    private QexedServerProcess(Path repoRoot, Path runDir, Process process, boolean keepRunDir) {
        this.runDir = runDir;
        this.process = process;
        this.keepRunDir = keepRunDir;
        capture(process.getInputStream());
        capture(process.getErrorStream());
    }

    static QexedServerProcess start(String name, boolean creative) throws IOException, InterruptedException {
        return start(name, creative, false);
    }

    static QexedServerProcess start(String name, boolean creative, boolean allowAllCommands)
            throws IOException, InterruptedException {
        return start(name, creative, allowAllCommands, false);
    }

    static QexedServerProcess startWithAiStress(String name) throws IOException, InterruptedException {
        return start(name, true, true, true);
    }

    private static QexedServerProcess start(
            String name, boolean creative, boolean allowAllCommands, boolean aiStress)
            throws IOException, InterruptedException {
        Path repoRoot = Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
        Path runDir = Files.createTempDirectory("qexed-it-" + name + "-");
        copyDirectory(repoRoot.resolve("run/config"), runDir.resolve("config"));
        writeTestConfig(runDir.resolve("config"), creative, allowAllCommands, aiStress);
        Path binary = repoRoot.resolve("target/debug/qexed.exe");
        if (!Files.exists(binary)) {
            binary = repoRoot.resolve("target/debug/qexed");
        }
        if (!Files.exists(binary)) {
            fail("qexed binary not found, run buildQexedForIntegration first: " + binary);
        }

        Process process = new ProcessBuilder(binary.toString(), "--language", "zh-CN")
                .directory(runDir.toFile())
                .redirectErrorStream(false)
                .start();
        QexedServerProcess server = new QexedServerProcess(repoRoot, runDir, process, false);
        server.waitForPort(Duration.ofSeconds(30));
        return server;
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
                socket.connect(new InetSocketAddress("127.0.0.1", 25565), 250);
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
            Path configDir, boolean creative, boolean allowAllCommands, boolean aiStress) throws IOException {
        Files.writeString(configDir.resolve("qexed_server.toml"), """
                [server]
                ip = "127.0.0.1:25565"
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
                """, StandardCharsets.UTF_8);
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

                [spawn]
                x = 0.0
                y = 64.0
                z = 0.0
                yaw = 0.0
                pitch = 0.0
                """.formatted(creative ? "creative" : "survival"), StandardCharsets.UTF_8);
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
