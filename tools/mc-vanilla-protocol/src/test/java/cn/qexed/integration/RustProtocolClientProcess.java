package cn.qexed.integration;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Path;
import java.time.Duration;
import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.concurrent.TimeUnit;

public final class RustProtocolClientProcess implements AutoCloseable {
    private final Process process;
    private final List<String> logs = new CopyOnWriteArrayList<>();

    private RustProtocolClientProcess(Process process) {
        this.process = process;
        capture(process.getInputStream());
        capture(process.getErrorStream());
    }

    public static RustProtocolClientProcess start(int port) throws IOException {
        Path repoRoot = Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
        Path manifest = repoRoot.resolve("tools/mc-vanilla-protocol/rust-client-smoke/Cargo.toml");
        Path targetDir = repoRoot.resolve("tools/mc-vanilla-protocol/build/rust-client-target");
        ProcessBuilder builder = new ProcessBuilder(
                        "cargo", "run", "--quiet", "--manifest-path", manifest.toString(), "--", Integer.toString(port))
                .directory(repoRoot.toFile())
                .redirectErrorStream(false);
        builder.environment().put("CARGO_TARGET_DIR", targetDir.toString());
        Process process = builder.start();
        return new RustProtocolClientProcess(process);
    }

    public void awaitSuccess(Duration timeout) throws InterruptedException {
        if (!process.waitFor(timeout.toMillis(), TimeUnit.MILLISECONDS)) {
            throw new AssertionError("Rust protocol client timed out. logs:\n" + String.join("\n", logs));
        }
        if (process.exitValue() != 0) {
            throw new AssertionError("Rust protocol client exited " + process.exitValue()
                    + ". logs:\n" + String.join("\n", logs));
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
    }

    private void capture(InputStream stream) {
        Thread thread = new Thread(() -> {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    logs.add(line);
                }
            } catch (IOException error) {
                logs.add("failed to read Rust protocol client log stream: " + error);
            }
        }, "rust-protocol-client-log-capture");
        thread.setDaemon(true);
        thread.start();
    }
}
