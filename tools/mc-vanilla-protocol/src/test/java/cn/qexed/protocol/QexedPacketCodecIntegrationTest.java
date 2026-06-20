package cn.qexed.protocol;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;
import static org.junit.jupiter.api.Assertions.fail;

import cn.qexed.integration.MinecraftServerboundPacketServer;
import cn.qexed.integration.QexedProtocolClient;
import cn.qexed.integration.QexedServerProcess;
import cn.qexed.integration.RustProtocolClientProcess;
import com.google.gson.JsonObject;
import com.google.gson.JsonParser;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.LinkedHashMap;
import java.util.Map;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

@Tag("integration")
final class QexedPacketCodecIntegrationTest {
    @Test
    void allPacketIdsMatchMinecraft262ProtocolRegistry() throws Exception {
        JsonObject reports = JsonParser.parseString(Files.readString(repoRoot().resolve("assets/reports/packets.json")))
                .getAsJsonObject();
        String[][] states = {
            {"handshake", "serverbound"},
            {"status", "serverbound"},
            {"status", "clientbound"},
            {"login", "serverbound"},
            {"login", "clientbound"},
            {"configuration", "serverbound"},
            {"configuration", "clientbound"},
            {"play", "serverbound"},
            {"play", "clientbound"},
        };

        for (String[] stateAndDirection : states) {
            String state = stateAndDirection[0];
            String direction = stateAndDirection[1];
            assertTrue(
                    MinecraftProtocolCatalog.packets(state, direction)
                            .equals(expectedPackets(reports, state, direction)),
                    state + "/" + direction + " packet catalog differs from Mojang 26.2");
        }
    }

    @Test
    void clientboundPacketsDecodeWithMinecraft262CodecsInNetworkOrder() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("clientbound-codec");
                QexedProtocolClient client = connect(server, "CodecClient")) {
            MinecraftPacketDecoder.ClientboundSession session = new MinecraftPacketDecoder.ClientboundSession();
            for (QexedProtocolClient.PacketRecord packet : client.loginPackets()) {
                if (packet.id() == QexedProtocolClient.CLIENTBOUND_LOGIN_FINISHED) {
                    session.decodeLogin(packet);
                }
            }

            int decodedConfiguration = 0;
            for (QexedProtocolClient.PacketRecord packet : client.configPackets()) {
                session.decodeConfiguration(packet);
                decodedConfiguration++;
            }
            assertTrue(decodedConfiguration >= 4, "expected several configuration packets to decode");

            client.waitForPlayPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SYSTEM_CHAT, Duration.ofSeconds(10));
            int decodedPlay = 0;
            for (QexedProtocolClient.PacketRecord packet : client.playPackets()) {
                session.decodePlay(packet);
                decodedPlay++;
            }
            assertTrue(decodedPlay >= 8, "expected initial play packets to decode, got " + decodedPlay);
            assertFalse(
                    client.playPackets().stream().map(QexedProtocolClient.PacketRecord::idHex).toList().isEmpty(),
                    "client should have seen play packets");
            server.assertNoProtocolErrors();
        }
    }

    @Test
    void serverboundPacketsSentByJavaAreDecodedByQexedInProtocolOrder() throws Exception {
        try (QexedServerProcess server = QexedServerProcess.start("serverbound-codec");
                QexedProtocolClient client = connect(server, "CodecServerbound")) {
            client.waitForPlayPacket(QexedProtocolClient.CLIENTBOUND_PLAY_SYSTEM_CHAT, Duration.ofSeconds(10));
            client.sendPlayerLoaded();
            client.sendMove(0.5, 64.0, 0.5, true);
            client.pump(Duration.ofSeconds(2));
            server.assertNoProtocolErrors();
        }
    }

    @Test
    void serverboundPacketsSentByRustDecodeWithMinecraft262CodecsInNetworkOrder() throws Exception {
        try (MinecraftServerboundPacketServer server = MinecraftServerboundPacketServer.start();
                RustProtocolClientProcess client = RustProtocolClientProcess.start(server.port())) {
            client.awaitSuccess(Duration.ofSeconds(60));
            server.awaitSuccess(Duration.ofSeconds(10));
        }
    }

    private static QexedProtocolClient connect(QexedServerProcess server, String username) throws Exception {
        try {
            return new QexedProtocolClient(username, server.port());
        } catch (Throwable error) {
            server.markFailed();
            fail("client failed to connect: " + error + "\nqexed logs:\n" + String.join("\n", server.logs()), error);
            throw error;
        }
    }

    private static Map<String, Integer> expectedPackets(JsonObject reports, String state, String direction) {
        JsonObject packets = reports.getAsJsonObject(state).getAsJsonObject(direction);
        Map<String, Integer> expected = new LinkedHashMap<>();
        packets.entrySet().stream()
                .sorted(Map.Entry.comparingByValue((left, right) -> Integer.compare(
                        left.getAsJsonObject().get("protocol_id").getAsInt(),
                        right.getAsJsonObject().get("protocol_id").getAsInt())))
                .forEach(entry -> expected.put(
                        entry.getKey(), entry.getValue().getAsJsonObject().get("protocol_id").getAsInt()));
        return expected;
    }

    private static Path repoRoot() {
        return Path.of(System.getProperty("qexed.repoRoot", "../..")).toAbsolutePath().normalize();
    }
}
