package cn.qexed.integration;

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
import java.util.List;
import java.util.UUID;
import java.util.stream.Collectors;
import java.util.zip.DataFormatException;
import java.util.zip.Inflater;

final class QexedProtocolClient implements Closeable {
    static final int PROTOCOL_VERSION = 775;

    static final int CLIENTBOUND_LOGIN_COMPRESS = 0x03;
    static final int CLIENTBOUND_LOGIN_SUCCESS = 0x02;
    static final int CLIENTBOUND_CONFIG_FINISH = 0x03;
    static final int CLIENTBOUND_CONFIG_SELECT_KNOWN_PACKS = 0x0e;
    static final int CLIENTBOUND_PLAY_ADD_ENTITY = 0x01;
    static final int CLIENTBOUND_PLAY_BLOCK_CHANGED_ACK = 0x04;
    static final int CLIENTBOUND_PLAY_BLOCK_UPDATE = 0x08;
    static final int CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED = 0x0b;
    static final int CLIENTBOUND_PLAY_COMMAND_SUGGESTIONS = 0x0f;
    static final int CLIENTBOUND_PLAY_COMMANDS = 0x10;
    static final int CLIENTBOUND_PLAY_CONTAINER_SET_CONTENT = 0x12;
    static final int CLIENTBOUND_PLAY_CONTAINER_SET_SLOT = 0x14;
    static final int CLIENTBOUND_PLAY_DAMAGE_EVENT = 0x19;
    static final int CLIENTBOUND_PLAY_ENTITY_EVENT = 0x22;
    static final int CLIENTBOUND_PLAY_LOGIN = 0x31;
    static final int CLIENTBOUND_PLAY_OPEN_SCREEN = 0x3b;
    static final int CLIENTBOUND_PLAY_POSITION = 0x48;
    static final int CLIENTBOUND_PLAY_SET_ENTITY_MOTION = 0x65;
    static final int CLIENTBOUND_PLAY_SET_PLAYER_INVENTORY = 0x6c;
    static final int CLIENTBOUND_PLAY_SYSTEM_CHAT = 0x79;
    static final int CLIENTBOUND_PLAY_TAKE_ITEM_ENTITY = 0x7c;
    static final int CLIENTBOUND_PLAY_UPDATE_RECIPES = 0x85;
    static final int PLAYER_INPUT_LEFT = 0x04;
    static final int PLAYER_INPUT_RIGHT = 0x08;

    private final Socket socket;
    private final InputStream input;
    private final OutputStream output;
    private final int port;
    private final List<PacketRecord> seenPackets = new ArrayList<>();
    private final List<String> loginPackets = new ArrayList<>();

    private boolean play;
    private boolean compressed;
    private int entityId = -1;
    private int teleportId = -1;
    private boolean initialPositionAccepted;

    QexedProtocolClient(String username) throws IOException {
        this(username, 25565);
    }

    QexedProtocolClient(String username, int port) throws IOException {
        this.port = port;
        socket = new Socket();
        socket.connect(new InetSocketAddress("127.0.0.1", port), 5_000);
        socket.setSoTimeout(1_000);
        input = socket.getInputStream();
        output = socket.getOutputStream();
        login(username);
    }

    int entityId() {
        return entityId;
    }

    List<PacketRecord> seenPackets() {
        return seenPackets;
    }

    boolean sawPacket(int packetId) {
        return seenPackets.stream().anyMatch(packet -> packet.id == packetId);
    }

    PacketRecord waitForPacket(int packetId, Duration timeout) throws IOException {
        for (PacketRecord packet : seenPackets) {
            if (packet.id == packetId) {
                return packet;
            }
        }
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            PacketRecord packet;
            try {
                packet = readOne();
            } catch (SocketTimeoutException ignored) {
                continue;
            }
            if (packet.id == packetId) {
                return packet;
            }
        }
        throw new AssertionError("timed out waiting for packet 0x" + Integer.toHexString(packetId)
                + ", seen=" + seenPackets.stream().map(PacketRecord::idHex).toList());
    }

    PacketRecord firstSeenPacket(int packetId) {
        return seenPackets.stream()
                .filter(packet -> packet.id == packetId)
                .findFirst()
                .orElseThrow(() -> new AssertionError("packet not seen: 0x" + Integer.toHexString(packetId)));
    }

    void waitForPlayReady(Duration timeout) throws IOException {
        if (teleportId < 0) {
            waitForPacket(CLIENTBOUND_PLAY_POSITION, timeout);
        }
        if (!initialPositionAccepted) {
            sendAcceptTeleportation(teleportId);
            initialPositionAccepted = true;
        }
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            PacketRecord packet;
            try {
                packet = readOne();
            } catch (SocketTimeoutException ignored) {
                continue;
            }
            if (packet.id == CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED) {
                PacketBuffer body = new PacketBuffer(packet.payload);
                body.readVarInt();
                sendChunkBatchReceived(20.0f);
                return;
            }
        }
        throw new AssertionError("play did not finish an initial chunk batch");
    }

    void pump(Duration duration) throws IOException {
        long deadline = System.nanoTime() + duration.toNanos();
        while (System.nanoTime() < deadline) {
            try {
                readOne();
            } catch (SocketTimeoutException ignored) {
            }
        }
    }

    void sendMove(double x, double y, double z, boolean onGround) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeDouble(x);
        body.writeDouble(y);
        body.writeDouble(z);
        body.writeByte(onGround ? 1 : 0);
        sendPacket(0x1e, body);
    }

    void sendPlayerInput(int flags) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeByte(flags);
        sendPacket(0x2b, body);
    }

    void sendPlayerLoaded() throws IOException {
        sendPacket(0x2c, new PacketBuffer());
    }

    void sendAttack(int targetEntityId) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(targetEntityId);
        sendPacket(0x01, body);
    }

    void sendCommandSuggestion(int id, String text) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(id);
        body.writeString(text);
        sendPacket(0x0f, body);
    }

    void sendChatCommand(String command) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeString(command.startsWith("/") ? command.substring(1) : command);
        sendPacket(0x07, body);
    }

    void sendBreakBlock(int x, int y, int z, int sequence) throws IOException {
        sendPlayerAction(0, x, y, z, 1, sequence);
        sendPlayerAction(2, x, y, z, 1, sequence + 1);
    }

    void sendDropSelectedStack() throws IOException {
        sendPlayerAction(3, 0, 0, 0, 0, 0);
    }

    void sendCreativeSlot(int slot, int itemId, int count) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeShort(slot);
        writeSlot(body, itemId, count);
        sendPacket(0x38, body);
    }

    void sendSetCarriedItem(int slot) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeShort(slot);
        sendPacket(0x35, body);
    }

    void sendUseItemOn(int x, int y, int z, int face, int sequence) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(0);
        body.writePosition(x, y, z);
        body.writeVarInt(face);
        body.writeFloat(0.5f);
        body.writeFloat(0.5f);
        body.writeFloat(0.5f);
        body.writeBoolean(false);
        body.writeBoolean(false);
        body.writeVarInt(sequence);
        sendPacket(0x42, body);
    }

    void sendUseItem(int sequence) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(0);
        body.writeVarInt(sequence);
        body.writeFloat(0.0f);
        body.writeFloat(0.0f);
        sendPacket(0x43, body);
    }

    void sendContainerClick(int windowId, int slot) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(windowId);
        body.writeVarInt(0);
        body.writeShort(slot);
        body.writeByte(0);
        body.writeVarInt(0);
        body.writeVarInt(0);
        body.writeVarInt(0);
        sendPacket(0x12, body);
    }

    SystemChatMessage waitForSystemChatContaining(String text, Duration timeout) throws IOException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            for (PacketRecord packet : seenPackets) {
                if (packet.id == CLIENTBOUND_PLAY_SYSTEM_CHAT) {
                    SystemChatMessage message = parseSystemChat(packet);
                    if (message.text().contains(text)) {
                        return message;
                    }
                }
            }
            try {
                readOne();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new AssertionError("timed out waiting for system chat containing '" + text + "', messages="
                + seenPackets.stream()
                        .filter(packet -> packet.id == CLIENTBOUND_PLAY_SYSTEM_CHAT)
                        .map(QexedProtocolClient::parseSystemChat)
                        .map(SystemChatMessage::text)
                        .toList());
    }

    SystemChatMessage waitForAnySystemChat(Duration timeout) throws IOException {
        long deadline = System.nanoTime() + timeout.toNanos();
        int consumed = 0;
        while (System.nanoTime() < deadline) {
            for (; consumed < seenPackets.size(); consumed++) {
                PacketRecord packet = seenPackets.get(consumed);
                if (packet.id == CLIENTBOUND_PLAY_SYSTEM_CHAT) {
                    return parseSystemChat(packet);
                }
            }
            try {
                readOne();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new AssertionError("timed out waiting for any system chat, seen="
                + seenPackets.stream().map(PacketRecord::idHex).toList());
    }

    CommandSuggestions waitForCommandSuggestions(int requestId, Duration timeout) throws IOException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            for (PacketRecord packet : seenPackets) {
                if (packet.id == CLIENTBOUND_PLAY_COMMAND_SUGGESTIONS) {
                    CommandSuggestions suggestions = parseCommandSuggestions(packet);
                    if (suggestions.id() == requestId) {
                        return suggestions;
                    }
                }
            }
            try {
                readOne();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new AssertionError("timed out waiting for command suggestions id=" + requestId);
    }

    PositionPacket waitForTeleportAfterCommand(Duration timeout) throws IOException {
        long deadline = System.nanoTime() + timeout.toNanos();
        while (System.nanoTime() < deadline) {
            try {
                PacketRecord packet = readOne();
                if (packet.id == CLIENTBOUND_PLAY_POSITION) {
                    PositionPacket position = parsePosition(packet);
                    sendAcceptTeleportation(position.teleportId());
                    return position;
                }
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new AssertionError("timed out waiting for teleport position packet");
    }

    private void login(String username) throws IOException {
        PacketBuffer handshake = new PacketBuffer();
        handshake.writeVarInt(PROTOCOL_VERSION);
        handshake.writeString("127.0.0.1");
        handshake.writeShort(port);
        handshake.writeVarInt(2);
        sendPacket(0x00, handshake);

        PacketBuffer loginStart = new PacketBuffer();
        loginStart.writeString(username);
        loginStart.writeUuid(UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes()));
        sendPacket(0x00, loginStart);

        long loginDeadline = System.nanoTime() + Duration.ofSeconds(30).toNanos();
        while (true) {
            PacketRecord packet = readRawPacketUntil(loginDeadline, "login success");
            loginPackets.add("login:" + packet.idHex());
            if (packet.id == CLIENTBOUND_LOGIN_COMPRESS) {
                compressed = true;
                continue;
            }
            if (packet.id == CLIENTBOUND_LOGIN_SUCCESS) {
                break;
            }
        }

        sendPacket(0x03, new PacketBuffer());
        sendClientSettings();

        boolean selectedPacks = false;
        long configDeadline = System.nanoTime() + Duration.ofSeconds(90).toNanos();
        while (true) {
            PacketRecord packet = readRawPacketUntil(configDeadline, "configuration finish");
            loginPackets.add("config:" + packet.idHex());
            if (packet.id == CLIENTBOUND_CONFIG_SELECT_KNOWN_PACKS && !selectedPacks) {
                PacketBuffer selected = new PacketBuffer();
                selected.writeVarInt(0);
                sendPacket(0x07, selected);
                selectedPacks = true;
                continue;
            }
            if (packet.id == CLIENTBOUND_CONFIG_FINISH) {
                sendPacket(0x03, new PacketBuffer());
                break;
            }
        }

        play = true;
        long playDeadline = System.nanoTime() + Duration.ofSeconds(60).toNanos();
        while (true) {
            PacketRecord packet = readOneUntil(playDeadline, "initial play position");
            if (packet.id == CLIENTBOUND_PLAY_LOGIN) {
                PacketBuffer body = new PacketBuffer(packet.payload);
                entityId = body.readInt();
            }
            if (packet.id == CLIENTBOUND_PLAY_POSITION) {
                sendAcceptTeleportation(teleportId);
                initialPositionAccepted = true;
                return;
            }
        }
    }

    private void sendClientSettings() throws IOException {
        PacketBuffer settings = new PacketBuffer();
        settings.writeString("zh_CN");
        settings.writeByte(8);
        settings.writeVarInt(0);
        settings.writeBoolean(true);
        settings.writeByte(0x7f);
        settings.writeVarInt(1);
        settings.writeBoolean(false);
        settings.writeBoolean(true);
        settings.writeVarInt(0);
        sendPacket(0x00, settings);
    }

    private void sendAcceptTeleportation(int id) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(id);
        sendPacket(0x00, body);
    }

    private void sendChunkBatchReceived(float desiredChunksPerTick) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeFloat(desiredChunksPerTick);
        sendPacket(0x0b, body);
    }

    private void sendPlayerAction(int status, int x, int y, int z, int face, int sequence) throws IOException {
        PacketBuffer body = new PacketBuffer();
        body.writeVarInt(status);
        body.writePosition(x, y, z);
        body.writeByte(face);
        body.writeVarInt(sequence);
        sendPacket(0x29, body);
    }

    private PacketRecord readOne() throws IOException {
        PacketRecord packet = readRawPacket();
        seenPackets.add(packet);
        if (play && packet.id == CLIENTBOUND_PLAY_POSITION) {
            PacketBuffer body = new PacketBuffer(packet.payload);
            teleportId = body.readVarInt();
        }
        if (play && packet.id == CLIENTBOUND_PLAY_CHUNK_BATCH_FINISHED) {
            sendChunkBatchReceived(20.0f);
        }
        if (play && packet.id == 0x2c) {
            PacketBuffer in = new PacketBuffer(packet.payload);
            PacketBuffer out = new PacketBuffer();
            out.writeLong(in.readLong());
            sendPacket(0x1c, out);
        }
        return packet;
    }

    private PacketRecord readOneUntil(long deadlineNanos, String phase) throws IOException {
        while (System.nanoTime() < deadlineNanos) {
            try {
                return readOne();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new SocketTimeoutException("timed out waiting for " + phase + "; loginPackets=" + loginPackets);
    }

    private static SystemChatMessage parseSystemChat(PacketRecord packet) {
        PacketBuffer body = new PacketBuffer(packet.payload);
        String text = body.readNbtStrings().stream().collect(Collectors.joining("\n"));
        boolean overlay = body.readBoolean();
        return new SystemChatMessage(text, overlay);
    }

    private static CommandSuggestions parseCommandSuggestions(PacketRecord packet) {
        PacketBuffer body = new PacketBuffer(packet.payload);
        int id = body.readVarInt();
        int start = body.readVarInt();
        int length = body.readVarInt();
        int count = body.readVarInt();
        List<String> matches = new ArrayList<>();
        for (int i = 0; i < count; i++) {
            matches.add(body.readString());
            if (body.readBoolean()) {
                body.readNbtStrings();
            }
        }
        return new CommandSuggestions(id, start, length, matches);
    }

    private static PositionPacket parsePosition(PacketRecord packet) {
        PacketBuffer body = new PacketBuffer(packet.payload);
        int teleportId = body.readVarInt();
        double x = body.readDouble();
        double y = body.readDouble();
        double z = body.readDouble();
        body.readDouble();
        body.readDouble();
        body.readDouble();
        float yaw = body.readFloat();
        float pitch = body.readFloat();
        int flags = body.readInt();
        return new PositionPacket(teleportId, x, y, z, yaw, pitch, flags);
    }

    private PacketRecord readRawPacket() throws IOException {
        int length;
        try {
            length = readVarInt(input);
        } catch (SocketTimeoutException error) {
            throw new SocketTimeoutException(error.getMessage() + "; loginPackets=" + loginPackets);
        }
        if (length < 0 || length > 8 * 1024 * 1024) {
            throw new IOException("invalid packet length: " + length);
        }
        byte[] payload = input.readNBytes(length);
        if (payload.length != length) {
            throw new EOFException("connection closed inside packet");
        }
        if (compressed) {
            PacketBuffer compressedFrame = new PacketBuffer(payload);
            int uncompressedLength = compressedFrame.readVarInt();
            byte[] compressedPayload = compressedFrame.readRemaining();
            if (uncompressedLength > 0) {
                payload = inflate(compressedPayload, uncompressedLength);
            } else {
                payload = compressedPayload;
            }
        }
        PacketBuffer buffer = new PacketBuffer(payload);
        int id = buffer.readVarInt();
        return new PacketRecord(id, buffer.readRemaining());
    }

    private PacketRecord readRawPacketUntil(long deadlineNanos, String phase) throws IOException {
        while (System.nanoTime() < deadlineNanos) {
            try {
                return readRawPacket();
            } catch (SocketTimeoutException ignored) {
            }
        }
        throw new SocketTimeoutException("timed out waiting for " + phase + "; loginPackets=" + loginPackets);
    }

    private void sendPacket(int packetId, PacketBuffer body) throws IOException {
        PacketBuffer payload = new PacketBuffer();
        payload.writeVarInt(packetId);
        payload.writeBytes(body.toByteArray());
        byte[] payloadBytes = payload.toByteArray();
        PacketBuffer frame = new PacketBuffer();
        if (compressed) {
            PacketBuffer compressedFrame = new PacketBuffer();
            compressedFrame.writeVarInt(0);
            compressedFrame.writeBytes(payloadBytes);
            byte[] compressedPayload = compressedFrame.toByteArray();
            frame.writeVarInt(compressedPayload.length);
            frame.writeBytes(compressedPayload);
        } else {
            frame.writeVarInt(payloadBytes.length);
            frame.writeBytes(payloadBytes);
        }
        output.write(frame.toByteArray());
        output.flush();
    }

    private static byte[] inflate(byte[] payload, int expectedLength) throws IOException {
        if (expectedLength < 0 || expectedLength > 16 * 1024 * 1024) {
            throw new IOException("invalid uncompressed packet length: " + expectedLength);
        }
        Inflater inflater = new Inflater();
        inflater.setInput(payload);
        byte[] out = new byte[expectedLength];
        try {
            int written = inflater.inflate(out);
            if (written != expectedLength || !inflater.finished()) {
                throw new IOException("compressed packet length mismatch: expected="
                        + expectedLength + ", actual=" + written);
            }
            return out;
        } catch (DataFormatException error) {
            throw new IOException("failed to inflate packet", error);
        } finally {
            inflater.end();
        }
    }

    private static void writeSlot(PacketBuffer body, int itemId, int count) {
        if (count <= 0) {
            body.writeVarInt(0);
            return;
        }
        body.writeVarInt(count);
        body.writeVarInt(itemId);
        body.writeVarInt(0);
        body.writeVarInt(0);
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

    record PacketRecord(int id, byte[] payload) {
        String idHex() {
            return "0x" + Integer.toHexString(id);
        }
    }

    record SystemChatMessage(String text, boolean overlay) {
    }

    record CommandSuggestions(int id, int start, int length, List<String> matches) {
    }

    record PositionPacket(int teleportId, double x, double y, double z, float yaw, float pitch, int flags) {
    }
}
