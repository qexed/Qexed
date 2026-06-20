package cn.qexed.protocol;

import java.util.LinkedHashMap;
import java.util.Map;
import net.minecraft.SharedConstants;
import net.minecraft.network.ProtocolInfo.Details;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.handshake.HandshakeProtocols;
import net.minecraft.network.protocol.login.LoginProtocols;
import net.minecraft.network.protocol.status.StatusProtocols;
import net.minecraft.server.Bootstrap;

final class MinecraftProtocolCatalog {
    static {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
    }

    private MinecraftProtocolCatalog() {
    }

    static Map<String, Integer> packets(String state, String direction) {
        Details details = details(state, direction);
        Map<String, Integer> packets = new LinkedHashMap<>();
        details.listPackets((type, id) -> packets.put(type.id().toString(), id));
        return packets;
    }

    private static Details details(String state, String direction) {
        boolean serverbound = "serverbound".equals(direction);
        return switch (state) {
            case "handshake" -> {
                requireServerbound(state, direction);
                yield HandshakeProtocols.SERVERBOUND_TEMPLATE.details();
            }
            case "status" -> serverbound
                    ? StatusProtocols.SERVERBOUND_TEMPLATE.details()
                    : StatusProtocols.CLIENTBOUND_TEMPLATE.details();
            case "login" -> serverbound
                    ? LoginProtocols.SERVERBOUND_TEMPLATE.details()
                    : LoginProtocols.CLIENTBOUND_TEMPLATE.details();
            case "configuration" -> serverbound
                    ? ConfigurationProtocols.SERVERBOUND_TEMPLATE.details()
                    : ConfigurationProtocols.CLIENTBOUND_TEMPLATE.details();
            case "play" -> serverbound
                    ? GameProtocols.SERVERBOUND_TEMPLATE.details()
                    : GameProtocols.CLIENTBOUND_TEMPLATE.details();
            default -> throw new IllegalArgumentException("unknown protocol state: " + state);
        };
    }

    private static void requireServerbound(String state, String direction) {
        if (!"serverbound".equals(direction)) {
            throw new IllegalArgumentException(state + " has no " + PacketFlow.CLIENTBOUND.id() + " protocol");
        }
    }
}
