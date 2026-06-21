package dev.qexed.worldgen;

import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.PropertyNamingStrategies;
import com.sun.net.httpserver.HttpServer;
import java.io.IOException;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;

public final class WorldgenServer {
    private static final ObjectMapper JSON = new ObjectMapper()
            .setPropertyNamingStrategy(PropertyNamingStrategies.SNAKE_CASE);

    private final VanillaChunkWriter chunkWriter;

    private WorldgenServer(VanillaChunkWriter chunkWriter) {
        this.chunkWriter = chunkWriter;
    }

    public static void main(String[] args) throws Exception {
        int port = args.length > 0
                ? Integer.parseInt(args[0])
                : Integer.parseInt(System.getProperty("qexed.worldgen.port", "39170"));
        HttpServer server = HttpServer.create(new InetSocketAddress("127.0.0.1", port), 0);
        MinecraftServerChunkWriter chunkWriter = new MinecraftServerChunkWriter();
        WorldgenServer handler = new WorldgenServer(chunkWriter);
        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            try {
                chunkWriter.close();
            } catch (Exception ignored) {
            }
        }, "qexed-vanilla-worldgen-shutdown"));
        server.createContext("/", exchange -> {
            if (!"POST".equals(exchange.getRequestMethod())) {
                exchange.sendResponseHeaders(405, -1);
                return;
            }

            byte[] body = exchange.getRequestBody().readAllBytes();
            byte[] response = handler.handle(body);
            exchange.getResponseHeaders().set("content-type", "application/json; charset=utf-8");
            exchange.sendResponseHeaders(200, response.length);
            try (OutputStream output = exchange.getResponseBody()) {
                output.write(response);
            }
        });
        server.start();
        System.out.printf("qexed vanilla worldgen JSON-RPC listening on 127.0.0.1:%d%n", port);
    }

    private byte[] handle(byte[] body) {
        Object id = null;
        try {
            JsonNode root = JSON.readTree(body);
            id = rpcId(root);
            String method = root.path("method").asText();
            if (!"worldgen.generateChunk".equals(method)) {
                return response(id, null, new RpcError(-32601, "method not found: " + method));
            }

            GenerateChunkRequest request =
                    JSON.treeToValue(root.path("params"), GenerateChunkRequest.class);
            System.out.printf(
                    "worldgen.generateChunk dimension=%s:%s chunk=(%d,%d) region=%s%n",
                    request.dimension().namespace(),
                    request.dimension().value(),
                    request.chunkX(),
                    request.chunkZ(),
                    request.regionPath());
            GenerateChunkResult result = chunkWriter.generateChunk(request);
            return response(id, result, null);
        } catch (Exception error) {
            error.printStackTrace(System.out);
            return response(id, null, new RpcError(-32000, error.getMessage()));
        }
    }

    private static Object rpcId(JsonNode root) {
        JsonNode id = root.get("id");
        if (id == null || id.isNull()) {
            return null;
        }
        if (id.isNumber()) {
            return id.longValue();
        }
        return id.asText();
    }

    private static byte[] response(Object id, Object result, RpcError error) {
        try {
            Map<String, Object> payload = new LinkedHashMap<>();
            payload.put("jsonrpc", "2.0");
            payload.put("id", id);
            if (error == null) {
                payload.put("result", result);
            } else {
                payload.put("error", error);
            }
            return JSON.writeValueAsBytes(payload);
        } catch (Exception serializationError) {
            String fallback = "{\"jsonrpc\":\"2.0\",\"id\":null,\"error\":{\"code\":-32603,\"message\":\"internal error\"}}";
            return fallback.getBytes(StandardCharsets.UTF_8);
        }
    }
}
