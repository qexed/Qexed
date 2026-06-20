package cn.qexed.integration;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.io.Closeable;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.net.SocketTimeoutException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.UUID;
import net.minecraft.SharedConstants;
import net.minecraft.core.RegistryAccess;
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.VarInt;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ServerboundClientInformationPacket;
import net.minecraft.network.protocol.common.ServerboundKeepAlivePacket;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket;
import net.minecraft.network.protocol.configuration.ServerboundSelectKnownPacks;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.game.ServerGamePacketListener;
import net.minecraft.network.protocol.game.ServerboundAcceptTeleportationPacket;
import net.minecraft.network.protocol.game.ServerboundChunkBatchReceivedPacket;
import net.minecraft.network.protocol.game.ServerboundMovePlayerPacket;
import net.minecraft.network.protocol.game.ServerboundPlayerLoadedPacket;
import net.minecraft.network.protocol.handshake.ClientIntent;
import net.minecraft.network.protocol.handshake.ClientIntentionPacket;
import net.minecraft.network.protocol.handshake.HandshakeProtocols;
import net.minecraft.network.protocol.login.LoginProtocols;
import net.minecraft.network.protocol.login.ServerLoginPacketListener;
import net.minecraft.network.protocol.login.ServerboundHelloPacket;
import net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.level.ClientInformation;

public final class QexedProtocolClient implements Closeable {
    static {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
    }

    public static final int PROTOCOL_VERSION = 776;

    public static final int CLIENTBOUND_LOGIN_FINISHED = 0x02;
    public static final int CLIENTBOUND_CONFIG_FINISH = 0x03;
    public static final int CLIENTBOUND_CONFIG_SELECT_KNOWN_PACKS = 0x0e;
    public static final int CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED = 0x0b;
    public static final int CLIENTBOUND_PLAY_KEEP_ALIVE = 0x2c;
    public static final int CLIENTBOUND_PLAY_LOGIN = 0x31;
    public static final int CLIENTBOUND_PLAY_PLAYER_POSITION = 0x48;
    public static final int CLIENTBOUND_PLAY_SYSTEM_CHAT = 0x79;

    private final Socket socket;
    private final InputStream input;
    private final OutputStream output;
    private final int port;
    private final ProtocolInfo<ServerGamePacketListener> playServerboundProtocol;
    private final List<PacketRecord> loginPackets = new ArrayList<>();
    private final List<PacketRecord> configPackets = new ArrayList<>();
    private final List<PacketRecord> playPackets = new ArrayList<>();
    private boolean play;
    private int teleportId = -1;

    public QexedProtocolClient(String username, int port) throws IOException {
        this.port = port;
        socket = new Socket();
        socket.connect(new InetSocketAddress("127.0.0.1", port), 5_000);
        socket.setSoTimeout(1_000);
        input = socket.getInputStream();
        output = socket.getOutputStream();
        playServerboundProtocol = GameProtocols.SERVERBOUND_TEMPLATE.bind(
                buffer -> new RegistryFriendlyByteBuf(buffer, RegistryAccess.EMPTY), () -> false);
        login(username);
    }

    public List<PacketRecord> loginPackets() {
        return loginPackets;
    }

    public List<PacketRecord> configPackets() {
        return configPackets;
    }

    public List<PacketRecord> playPackets() {
        return playPackets;
    }

    public PacketRecord waitForPlayPacket(int packetId, Duration timeout) throws IOException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            for (PacketRecord packet : playPackets) {
                if (packet.id == packetId) {
                    return packet;
                }
            }
            try {
                readPlayPacket();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new AssertionError("timed out waiting for play packet 0x" + Integer.toHexString(packetId)
                + ", seen=" + playPackets.stream().map(PacketRecord::idHex).toList());
    }

    public void pump(Duration duration) throws IOException {
        long deadline = System.nanoTime() + duration.toNanos();
        while (System.nanoTime() < deadline) {
            try {
                readPlayPacket();
            } catch (SocketTimeoutException ignored) {
            }
        }
    }

    public void sendPlayerLoaded() throws IOException {
        sendPacket(playServerboundProtocol, new ServerboundPlayerLoadedPacket());
    }

    public void sendMove(double x, double y, double z, boolean onGround) throws IOException {
        sendPacket(playServerboundProtocol, new ServerboundMovePlayerPacket.Pos(x, y, z, onGround, false));
    }

    private void login(String username) throws IOException {
        sendPacket(
                HandshakeProtocols.SERVERBOUND,
                new ClientIntentionPacket(PROTOCOL_VERSION, "127.0.0.1", port, ClientIntent.LOGIN));
        sendPacket(
                LoginProtocols.SERVERBOUND,
                new ServerboundHelloPacket(username, UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes())));

        while (true) {
            PacketRecord packet = readRawPacket();
            loginPackets.add(packet);
            if (packet.id == CLIENTBOUND_LOGIN_FINISHED) {
                break;
            }
        }

        sendPacket(LoginProtocols.SERVERBOUND, ServerboundLoginAcknowledgedPacket.INSTANCE);
        sendClientInformation();

        boolean selectedPacks = false;
        while (true) {
            PacketRecord packet = readRawPacket();
            configPackets.add(packet);
            if (packet.id == CLIENTBOUND_CONFIG_SELECT_KNOWN_PACKS && !selectedPacks) {
                sendPacket(ConfigurationProtocols.SERVERBOUND, new ServerboundSelectKnownPacks(Collections.emptyList()));
                selectedPacks = true;
                continue;
            }
            if (packet.id == CLIENTBOUND_CONFIG_FINISH) {
                sendPacket(ConfigurationProtocols.SERVERBOUND, ServerboundFinishConfigurationPacket.INSTANCE);
                break;
            }
        }

        play = true;
        while (true) {
            PacketRecord packet = readPlayPacket();
            if (packet.id == CLIENTBOUND_PLAY_PLAYER_POSITION) {
                sendAcceptTeleportation(teleportId);
                return;
            }
        }
    }

    private void sendClientInformation() throws IOException {
        sendPacket(
                ConfigurationProtocols.SERVERBOUND,
                new ServerboundClientInformationPacket(ClientInformation.createDefault()));
    }

    private void sendAcceptTeleportation(int id) throws IOException {
        sendPacket(playServerboundProtocol, new ServerboundAcceptTeleportationPacket(id));
    }

    private void sendChunkBatchReceived(float desiredChunksPerTick) throws IOException {
        sendPacket(playServerboundProtocol, new ServerboundChunkBatchReceivedPacket(desiredChunksPerTick));
    }

    private PacketRecord readPlayPacket() throws IOException {
        PacketRecord packet = readRawPacket();
        playPackets.add(packet);
        if (packet.id == CLIENTBOUND_PLAY_PLAYER_POSITION) {
            PacketBuffer body = new PacketBuffer(packet.payload);
            teleportId = body.readVarInt();
        }
        if (packet.id == CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED) {
            sendChunkBatchReceived(20.0f);
        }
        if (packet.id == CLIENTBOUND_PLAY_KEEP_ALIVE) {
            PacketBuffer in = new PacketBuffer(packet.payload);
            sendPacket(playServerboundProtocol, new ServerboundKeepAlivePacket(in.readLong()));
        }
        return packet;
    }

    private PacketRecord readRawPacket() throws IOException {
        int length = readVarInt(input);
        if (length < 0 || length > 8 * 1024 * 1024) {
            throw new IOException("invalid packet length: " + length);
        }
        byte[] payload = input.readNBytes(length);
        if (payload.length != length) {
            throw new EOFException("connection closed inside packet");
        }
        PacketBuffer buffer = new PacketBuffer(payload);
        int id = buffer.readVarInt();
        return new PacketRecord(id, buffer.readRemaining(), play ? "play" : "pre-play");
    }

    private <T extends net.minecraft.network.PacketListener> void sendPacket(
            ProtocolInfo<T> protocol, Packet<? super T> packet) throws IOException {
        ByteBuf payload = Unpooled.buffer();
        ByteBuf frame = Unpooled.buffer();
        try {
            protocol.codec().encode(payload, packet);
            VarInt.write(frame, payload.readableBytes());
            frame.writeBytes(payload, payload.readerIndex(), payload.readableBytes());

            byte[] bytes = new byte[frame.readableBytes()];
            frame.readBytes(bytes);
            output.write(bytes);
        } finally {
            payload.release();
            frame.release();
        }
        output.flush();
    }

    private static int readVarInt(InputStream input) throws IOException {
        int value = 0;
        int position = 0;
        int current;
        do {
            current = input.read();
            if (current < 0) {
                throw new EOFException("connection closed while reading VarInt");
            }
            value |= (current & 0x7f) << position;
            position += 7;
            if (position > 35) {
                throw new IOException("VarInt is too large");
            }
        } while ((current & 0x80) != 0);
        return value;
    }

    @Override
    public void close() throws IOException {
        socket.close();
    }

    public record PacketRecord(int id, byte[] payload, String state) {
        public String idHex() {
            return "0x" + Integer.toHexString(id);
        }
    }
}
