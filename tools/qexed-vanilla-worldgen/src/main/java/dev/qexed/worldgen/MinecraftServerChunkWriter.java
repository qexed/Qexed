package dev.qexed.worldgen;

import java.io.BufferedWriter;
import java.io.IOException;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.net.ServerSocket;
import java.net.Socket;
import java.util.Properties;
import net.minecraft.nbt.CompoundTag;
import net.minecraft.server.Main;
import net.minecraft.world.level.ChunkPos;

final class MinecraftServerChunkWriter implements VanillaChunkWriter, AutoCloseable {
    private static final long CHUNK_GENERATION_TIMEOUT_NANOS = java.time.Duration.ofSeconds(30).toNanos();

    private final Object lock = new Object();
    private final Path workRoot;
    private final long seed;
    private Process process;
    private BufferedWriter consoleInput;

    MinecraftServerChunkWriter(long seed) throws IOException {
        this.workRoot = Files.createTempDirectory("qexed-vanilla-worldgen-");
        this.seed = seed;
    }

    @Override
    public GenerateChunkResult generateChunk(GenerateChunkRequest request) throws Exception {
        if (!"minecraft".equals(request.dimension().namespace())
                || !"overworld".equals(request.dimension().value())) {
            throw new IllegalArgumentException("only minecraft:overworld vanilla worldgen is supported");
        }

        synchronized (lock) {
            ensureServer();
            ChunkPos chunk = new ChunkPos(request.chunkX(), request.chunkZ());
            forceGenerate(chunk);
            byte[] compressedNbt = readGeneratedCompressedChunk(chunk);
            AnvilRegionWriter.writeChunk(Path.of(request.regionPath()), request.chunkX(), request.chunkZ(), compressedNbt);
            return new GenerateChunkResult(true, request.regionPath());
        }
    }

    private void ensureServer() throws Exception {
        if (process != null && process.isAlive()) {
            return;
        }

        Path serverRoot = workRoot.resolve("server");
        Files.createDirectories(serverRoot);
        Files.writeString(serverRoot.resolve("eula.txt"), "eula=true\n");
        writeServerProperties(serverRoot.resolve("server.properties"), seed);

        ProcessBuilder builder = new ProcessBuilder(
                javaExecutable(),
                "-cp",
                absoluteClassPath(),
                Main.class.getName(),
                "--nogui",
                "--universe",
                serverRoot.toAbsolutePath().toString(),
                "--world",
                "world",
                "--port",
                "0");
        builder.directory(serverRoot.toFile());
        builder.redirectErrorStream(true);
        builder.redirectOutput(ProcessBuilder.Redirect.INHERIT);

        process = builder.start();
        consoleInput = new BufferedWriter(new OutputStreamWriter(process.getOutputStream(), StandardCharsets.UTF_8));
        waitForServerReady(serverRoot.resolve("logs").resolve("latest.log"));
    }

    private static void writeServerProperties(Path path, long seed) throws IOException {
        Properties properties = new Properties();
        properties.setProperty("online-mode", "false");
        properties.setProperty("enable-rcon", "false");
        properties.setProperty("enable-query", "false");
        properties.setProperty("spawn-protection", "0");
        properties.setProperty("view-distance", "4");
        properties.setProperty("simulation-distance", "4");
        properties.setProperty("level-name", "world");
        properties.setProperty("level-seed", Long.toString(seed));
        properties.setProperty("server-port", "0");
        properties.setProperty("motd", "qexed vanilla worldgen");
        try (var output = Files.newOutputStream(path)) {
            properties.store(output, "qexed vanilla worldgen");
        }
    }

    private void forceGenerate(ChunkPos chunk) throws Exception {
        int blockX = chunk.getMinBlockX();
        int blockZ = chunk.getMinBlockZ();
        sendCommand("execute in minecraft:overworld run forceload add " + blockX + " " + blockZ);
        waitForChunkFile(chunk);
        sendCommand("execute in minecraft:overworld run forceload remove " + blockX + " " + blockZ);
    }

    private void sendCommand(String command) throws IOException {
        if (consoleInput == null) {
            throw new IOException("vanilla server console is not available");
        }
        consoleInput.write(command);
        consoleInput.newLine();
        consoleInput.flush();
        System.out.printf("CONSOLE %s%n", command);
    }

    private void waitForServerReady(Path logPath) throws Exception {
        long deadline = System.nanoTime() + java.time.Duration.ofSeconds(90).toNanos();
        while (System.nanoTime() < deadline) {
            if (process == null || !process.isAlive()) {
                throw new IllegalStateException("vanilla worldgen server exited during startup");
            }
            if (serverLogContains(logPath, "Done (")) {
                return;
            }
            Thread.sleep(250);
        }
        throw new IllegalStateException("timed out waiting for vanilla server startup");
    }

    private static boolean serverLogContains(Path path, String needle) throws IOException {
        if (!Files.exists(path)) {
            return false;
        }
        return Files.readString(path).contains(needle);
    }

    private void waitForChunkFile(ChunkPos chunk) throws Exception {
        Path region = vanillaRegionPath(chunk);
        long deadline = System.nanoTime() + CHUNK_GENERATION_TIMEOUT_NANOS;
        sendCommand("save-all flush");
        while (System.nanoTime() < deadline) {
            if (Files.exists(region)) {
                String status = AnvilRegionWriter.readChunkStatus(region, chunk.x(), chunk.z());
                if ("minecraft:full".equals(status)) {
                    return;
                }
            }
            if (process == null || !process.isAlive()) {
                throw new IllegalStateException("vanilla worldgen server exited");
            }
            Thread.sleep(100);
        }
        throw new IllegalStateException("timed out waiting for vanilla chunk " + chunk);
    }

    private byte[] readGeneratedCompressedChunk(ChunkPos chunk) throws Exception {
        byte[] compressedNbt = readGeneratedCompressedChunkOrNull(chunk);
        if (compressedNbt == null) {
            throw new IllegalStateException("vanilla server did not write chunk " + chunk);
        }
        return compressedNbt;
    }

    private byte[] readGeneratedCompressedChunkOrNull(ChunkPos chunk) throws Exception {
        return AnvilRegionWriter.readCompressedChunk(vanillaRegionPath(chunk), chunk.x(), chunk.z());
    }

    private Path vanillaRegionPath(ChunkPos chunk) {
        return vanillaRegionDir().resolve("r." + chunk.getRegionX() + "." + chunk.getRegionZ() + ".mca");
    }

    private Path vanillaRegionDir() {
        return workRoot
                .resolve("server")
                .resolve("world")
                .resolve("dimensions")
                .resolve("minecraft")
                .resolve("overworld")
                .resolve("region");
    }

    private static String javaExecutable() {
        String home = System.getProperty("java.home");
        String executable = System.getProperty("os.name").toLowerCase().contains("win")
                ? "java.exe"
                : "java";
        return Path.of(home, "bin", executable).toString();
    }

    private static int freeLocalPort() throws IOException {
        try (ServerSocket socket = new ServerSocket(0)) {
            socket.setReuseAddress(true);
            return socket.getLocalPort();
        }
    }

    private static String absoluteClassPath() {
        String separator = System.getProperty("path.separator");
        String[] entries = System.getProperty("java.class.path").split(java.util.regex.Pattern.quote(separator));
        StringBuilder classPath = new StringBuilder();
        for (String entry : entries) {
            if (classPath.length() > 0) {
                classPath.append(separator);
            }
            classPath.append(Path.of(entry).toAbsolutePath());
        }
        return classPath.toString();
    }

    @Override
    public void close() throws Exception {
        synchronized (lock) {
            if (process != null && process.isAlive()) {
                sendCommand("stop");
            }
            if (process != null) {
                if (!process.waitFor(30, java.util.concurrent.TimeUnit.SECONDS)) {
                    process.destroyForcibly();
                    process.waitFor();
                }
                process = null;
            }
            consoleInput = null;
        }
    }
}
