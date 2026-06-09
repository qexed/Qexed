package cn.qexed.integration;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
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
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

@Tag("integration")
final class QexedMazeRunnerIntegrationTest {
    @Test
    void mazeGenerationShowsLoadingBeforeTeleportingToFinishedInstance() throws Exception {
        try (MazeServer server = MazeServer.start()) {
            try (QexedProtocolClient client = new QexedProtocolClient("MazeSmoke", 25565)) {
                client.waitForPlayReady(Duration.ofSeconds(30));
                client.sendChatCommand("maze start hard");
                QexedProtocolClient.SystemChatMessage started = client.waitForAnySystemChat(Duration.ofSeconds(10));
                assertTrue(started.text().contains("生成"), "unexpected command response: " + started.text());
                client.pump(Duration.ofSeconds(2));
                assertFalse(client.seenPackets().stream()
                                .filter(packet -> packet.id() == QexedProtocolClient.CLIENTBOUND_PLAY_POSITION)
                                .skip(1)
                                .findAny()
                                .isPresent(),
                        "maze plugin teleported before the instance finished generating");

                QexedProtocolClient.PositionPacket position =
                        server.waitForTeleportAfterCommand(client, Duration.ofSeconds(60));
                assertTrue(position.x() > 1000.0, "expected generated maze instance x, got " + position);
                assertTrue(position.z() > 1000.0, "expected generated maze instance z, got " + position);
            }
            server.assertNoPluginOrConnectionErrors();
        }
    }

    private static final class MazeServer implements AutoCloseable {
        private final Path runDir;
        private final Process process;
        private final List<String> logs = new CopyOnWriteArrayList<>();
        private boolean failed;

        private MazeServer(Path runDir, Process process) {
            this.runDir = runDir;
            this.process = process;
            capture(process.getInputStream());
            capture(process.getErrorStream());
        }

        static MazeServer start() throws IOException, InterruptedException {
            Path repoRoot = Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
            Path runDir = Files.createTempDirectory("qexed-maze-it-");
            copyDirectory(repoRoot.resolve("run/maze"), runDir);
            Files.copy(
                    repoRoot.resolve("plugins/examples/target/wasm32-unknown-unknown/release/maze_runner.wasm"),
                    runDir.resolve("plugins/maze_runner.wasm"),
                    java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            patchServerConfig(runDir.resolve("config/qexed_server.toml"));
            deleteDirectory(runDir.resolve("logs"));
            deleteDirectory(runDir.resolve("cache"));

            Path binary = repoRoot.resolve("target/debug/qexed.exe");
            if (!Files.exists(binary)) {
                binary = repoRoot.resolve("target/debug/qexed");
            }
            if (!Files.exists(binary)) {
                fail("qexed binary not found: " + binary);
            }
            Process process = new ProcessBuilder(binary.toString(), "--language", "zh-CN")
                    .directory(runDir.toFile())
                    .start();
            MazeServer server = new MazeServer(runDir, process);
            server.waitForPort(Duration.ofSeconds(90));
            return server;
        }

        void assertNoPluginOrConnectionErrors() {
            List<String> failures = logs.stream()
                    .filter(line -> line.contains("[ERROR]")
                            || line.contains("connection handling error")
                            || line.contains("payload too large")
                            || line.contains("DecoderException"))
                    .toList();
            if (!failures.isEmpty()) {
                failed = true;
                fail("maze server logged errors:\n" + String.join("\n", failures)
                        + "\nrunDir=" + runDir);
            }
        }

        QexedProtocolClient.PositionPacket waitForTeleportAfterCommand(
                QexedProtocolClient client, Duration timeout) throws IOException {
            try {
                return client.waitForTeleportAfterCommand(timeout);
            } catch (AssertionError error) {
                failed = true;
                throw new AssertionError(error.getMessage()
                        + "\nrunDir=" + runDir
                        + "\nqexed logs:\n" + String.join("\n", logs), error);
            }
        }

        @Override
        public void close() throws Exception {
            if (process.isAlive()) {
                try {
                    BufferedWriter writer = new BufferedWriter(
                            new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
                    writer.write("stop");
                    writer.newLine();
                    writer.flush();
                } catch (IOException ignored) {
                }
                if (!process.waitFor(10, TimeUnit.SECONDS)) {
                    process.destroy();
                }
                if (!process.waitFor(5, TimeUnit.SECONDS)) {
                    process.destroyForcibly();
                }
            }
            if (failed) {
                System.err.println("qexed maze integration run kept for diagnosis: " + runDir);
                System.err.println(String.join("\n", logs));
            } else {
                deleteDirectory(runDir);
            }
        }

        private void waitForPort(Duration timeout) throws InterruptedException {
            long deadline = System.nanoTime() + timeout.toNanos();
            while (System.nanoTime() < deadline) {
                if (!process.isAlive()) {
                    fail("qexed exited before opening maze test port. logs:\n" + String.join("\n", logs));
                }
                try (Socket socket = new Socket()) {
                    socket.connect(new InetSocketAddress("127.0.0.1", 25565), 250);
                    return;
                } catch (IOException ignored) {
                    Thread.sleep(100);
                }
            }
            fail("qexed did not open 127.0.0.1:25565. logs:\n" + String.join("\n", logs));
        }

        private void capture(InputStream stream) {
            Thread thread = new Thread(() -> {
                try (BufferedReader reader =
                        new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
                    String line;
                    while ((line = reader.readLine()) != null) {
                        logs.add(line);
                    }
                } catch (IOException error) {
                    logs.add("failed to read qexed log stream: " + error);
                }
            }, "qexed-maze-log-capture");
            thread.setDaemon(true);
            thread.start();
        }

        private static void patchServerConfig(Path config) throws IOException {
            String contents = Files.readString(config, StandardCharsets.UTF_8)
                    .replace("ip = \"127.0.0.1:25569\"", "ip = \"127.0.0.1:25565\"")
                    .replace("network_compression_threshold = 256", "network_compression_threshold = -1")
                    .replace("log_level = \"info\"", "log_level = \"debug\"");
            Files.writeString(config, contents, StandardCharsets.UTF_8);
        }

        private static void copyDirectory(Path source, Path target) throws IOException {
            try (var stream = Files.walk(source)) {
                for (Path path : stream.toList()) {
                    Path relative = source.relativize(path);
                    Path destination = target.resolve(relative);
                    if (Files.isDirectory(path)) {
                        Files.createDirectories(destination);
                    } else {
                        Files.createDirectories(destination.getParent());
                        Files.copy(path, destination, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
                    }
                }
            }
        }

        private static void deleteDirectory(Path path) throws IOException {
            if (!Files.exists(path)) {
                return;
            }
            try (var stream = Files.walk(path)) {
                List<Path> paths = new ArrayList<>(stream.sorted(Comparator.reverseOrder()).toList());
                for (Path item : paths) {
                    Files.deleteIfExists(item);
                }
            }
        }
    }
}
