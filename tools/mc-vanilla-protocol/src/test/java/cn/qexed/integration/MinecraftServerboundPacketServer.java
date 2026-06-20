package cn.qexed.integration;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.io.Closeable;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.ServerSocket;
import java.net.Socket;
import java.time.Duration;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicReference;
import net.minecraft.SharedConstants;
import net.minecraft.core.RegistryAccess;
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.game.ServerGamePacketListener;
import net.minecraft.network.protocol.game.ServerboundChunkBatchReceivedPacket;
import net.minecraft.network.protocol.game.ServerboundMovePlayerPacket;
import net.minecraft.network.protocol.game.ServerboundPlayerLoadedPacket;
import net.minecraft.network.protocol.handshake.HandshakeProtocols;
import net.minecraft.network.protocol.handshake.ClientIntentionPacket;
import net.minecraft.network.protocol.login.LoginProtocols;
import net.minecraft.network.protocol.login.ServerboundHelloPacket;
import net.minecraft.network.protocol.login.ServerboundLoginAcknowledgedPacket;
import net.minecraft.network.protocol.common.ServerboundClientInformationPacket;
import net.minecraft.network.protocol.common.ServerboundKeepAlivePacket;
import net.minecraft.network.protocol.configuration.ServerboundFinishConfigurationPacket;
import net.minecraft.network.protocol.configuration.ServerboundSelectKnownPacks;
import net.minecraft.server.Bootstrap;

public final class MinecraftServerboundPacketServer implements Closeable {
    static {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
    }

    private final ServerSocket serverSocket;
    private final Thread thread;
    private final CountDownLatch finished = new CountDownLatch(1);
    private final AtomicReference<Throwable> failure = new AtomicReference<>();
    private final List<Class<?>> decodedPackets = new ArrayList<>();

    private MinecraftServerboundPacketServer(ServerSocket serverSocket) {
        this.serverSocket = serverSocket;
        this.thread = new Thread(this::run, "minecraft-serverbound-codec-server");
        this.thread.setDaemon(true);
        this.thread.start();
    }

    public static MinecraftServerboundPacketServer start() throws IOException {
        return new MinecraftServerboundPacketServer(new ServerSocket(0));
    }

    public int port() {
        return serverSocket.getLocalPort();
    }

    public void awaitSuccess(Duration timeout) throws Exception {
        if (!finished.await(timeout.toMillis(), TimeUnit.MILLISECONDS)) {
            throw new AssertionError("timed out waiting for Rust client packets, decoded=" + decodedPackets);
        }
        if (failure.get() != null) {
            throw new AssertionError("Java Mojang server failed after decoding " + decodedPackets, failure.get());
        }
    }

    @Override
    public void close() throws IOException {
        serverSocket.close();
    }

    private void run() {
        try (Socket socket = serverSocket.accept()) {
            socket.setSoTimeout(10_000);
            InputStream input = socket.getInputStream();

            decodeExpect(HandshakeProtocols.SERVERBOUND, input, ClientIntentionPacket.class);
            decodeExpect(LoginProtocols.SERVERBOUND, input, ServerboundHelloPacket.class);
            decodeExpect(LoginProtocols.SERVERBOUND, input, ServerboundLoginAcknowledgedPacket.class);

            decodeExpect(ConfigurationProtocols.SERVERBOUND, input, ServerboundClientInformationPacket.class);
            decodeExpect(ConfigurationProtocols.SERVERBOUND, input, ServerboundSelectKnownPacks.class);
            decodeExpect(ConfigurationProtocols.SERVERBOUND, input, ServerboundFinishConfigurationPacket.class);

            ProtocolInfo<ServerGamePacketListener> playProtocol = GameProtocols.SERVERBOUND_TEMPLATE.bind(
                    buffer -> new RegistryFriendlyByteBuf(buffer, RegistryAccess.EMPTY), () -> false);
            decodeExpect(playProtocol, input, ServerboundPlayerLoadedPacket.class);
            decodeExpect(playProtocol, input, ServerboundMovePlayerPacket.Pos.class);
            decodeExpect(playProtocol, input, ServerboundChunkBatchReceivedPacket.class);
            decodeExpect(playProtocol, input, ServerboundKeepAlivePacket.class);

            OutputStream output = socket.getOutputStream();
            output.write(1);
            output.flush();
        } catch (Throwable error) {
            failure.set(error);
        } finally {
            finished.countDown();
        }
    }

    private <T extends net.minecraft.network.PacketListener> void decodeExpect(
            ProtocolInfo<T> protocol, InputStream input, Class<?> packetClass) throws IOException {
        Packet<?> packet = decode(protocol, input);
        if (!packetClass.isInstance(packet)) {
            throw new AssertionError("expected " + packetClass.getName() + ", got " + packet.getClass().getName());
        }
        decodedPackets.add(packet.getClass());
    }

    private <T extends net.minecraft.network.PacketListener> Packet<?> decode(ProtocolInfo<T> protocol, InputStream input)
            throws IOException {
        int length = readVarInt(input);
        byte[] frame = input.readNBytes(length);
        if (frame.length != length) {
            throw new EOFException("connection closed inside packet");
        }

        ByteBuf buffer = Unpooled.wrappedBuffer(frame);
        Packet<?> packet = protocol.codec().decode(buffer);
        if (buffer.readableBytes() != 0) {
            throw new AssertionError(packet.getClass().getSimpleName() + " left "
                    + buffer.readableBytes() + " unread bytes");
        }
        return packet;
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
}
