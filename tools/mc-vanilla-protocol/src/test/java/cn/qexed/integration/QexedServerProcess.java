package cn.qexed.integration;

import static org.junit.jupiter.api.Assertions.fail;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Comparator;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;

public final class QexedServerProcess implements AutoCloseable {
    private final Path runDir;
    private final Process process;
    private final int port;
    private final List<String> logs = new CopyOnWriteArrayList<>();
    private boolean failed;

    private QexedServerProcess(Path runDir, Process process, int port) {
        this.runDir = runDir;
        this.process = process;
        this.port = port;
        capture(process.getInputStream());
        capture(process.getErrorStream());
    }

    public static QexedServerProcess start(String name) throws IOException, InterruptedException {
        Path repoRoot = Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
        Path binary = repoRoot.resolve("target/debug/qexed.exe");
        if (!Files.exists(binary)) {
            binary = repoRoot.resolve("target/debug/qexed");
        }
        if (!Files.exists(binary)) {
            fail("qexed binary not found: " + binary);
        }

        int port = Integer.parseInt(System.getProperty("qexed.port", "25565"));
        Path runDir = Files.createTempDirectory("qexed-protocol-" + name + "-");
        copyDirectory(repoRoot.resolve("config"), runDir.resolve("config"));
        copyDirectory(repoRoot.resolve("cache"), runDir.resolve("cache"));
        Files.writeString(
                runDir.resolve("config/qexed.toml"),
                """
                [server]
                bind = "127.0.0.1:%d"
                max_connections = 128
                motd = "Qexed Protocol Test"
                max_players = 20
                view_distance = 3
                simulation_distance = 3
                """.formatted(port),
                StandardCharsets.UTF_8);

        Process process = new ProcessBuilder(binary.toString())
                .directory(runDir.toFile())
                .redirectErrorStream(false)
                .start();
        QexedServerProcess server = new QexedServerProcess(runDir, process, port);
        server.waitForPort(Duration.ofSeconds(90));
        return server;
    }

    public int port() {
        return port;
    }

    public List<String> logs() {
        return List.copyOf(logs);
    }

    public void markFailed() {
        failed = true;
    }

    public void assertNoProtocolErrors() {
        List<String> failures = logs.stream()
                .filter(line -> line.contains("[ERROR]")
                        || line.contains("DecoderException")
                        || line.contains("Failed to decode packet")
                        || line.contains("connection closed: error="))
                .toList();
        if (!failures.isEmpty()) {
            failed = true;
            fail("qexed logged protocol errors:\n" + String.join("\n", failures));
        }
    }

    @Override
    public void close() throws Exception {
        if (process.isAlive()) {
            process.destroy();
            if (!process.waitFor(10, TimeUnit.SECONDS)) {
                process.destroyForcibly();
            }
        }
        if (failed) {
            System.err.println("qexed protocol run kept for diagnosis: " + runDir);
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
        fail("qexed did not open 127.0.0.1:" + port + ". logs:\n" + String.join("\n", logs));
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
        }, "qexed-protocol-log-capture");
        thread.setDaemon(true);
        thread.start();
    }

    private static void copyDirectory(Path from, Path to) throws IOException {
        if (!Files.exists(from)) {
            return;
        }
        Files.createDirectories(to);
        try (var paths = Files.walk(from)) {
            for (Path source : paths.toList()) {
                Path target = to.resolve(from.relativize(source));
                if (Files.isDirectory(source)) {
                    Files.createDirectories(target);
                } else {
                    Files.copy(source, target, java.nio.file.StandardCopyOption.REPLACE_EXISTING);
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
