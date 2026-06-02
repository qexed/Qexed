package cn.qexed.integration;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.util.UUID;

final class PacketBuffer {
    private final ByteArrayOutputStream out = new ByteArrayOutputStream();
    private byte[] input;
    private int cursor;

    PacketBuffer() {
    }

    PacketBuffer(byte[] input) {
        this.input = input;
    }

    void writeByte(int value) {
        out.write(value & 0xff);
    }

    void writeBoolean(boolean value) {
        writeByte(value ? 1 : 0);
    }

    void writeShort(int value) {
        out.write((value >>> 8) & 0xff);
        out.write(value & 0xff);
    }

    void writeInt(int value) {
        out.write((value >>> 24) & 0xff);
        out.write((value >>> 16) & 0xff);
        out.write((value >>> 8) & 0xff);
        out.write(value & 0xff);
    }

    void writeLong(long value) {
        for (int shift = 56; shift >= 0; shift -= 8) {
            out.write((int) ((value >>> shift) & 0xff));
        }
    }

    void writeFloat(float value) {
        writeInt(Float.floatToIntBits(value));
    }

    void writeDouble(double value) {
        writeLong(Double.doubleToLongBits(value));
    }

    void writeVarInt(int value) {
        int remaining = value;
        do {
            int temp = remaining & 0x7f;
            remaining >>>= 7;
            if (remaining != 0) {
                temp |= 0x80;
            }
            writeByte(temp);
        } while (remaining != 0);
    }

    void writeString(String value) {
        byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
        writeVarInt(bytes.length);
        out.writeBytes(bytes);
    }

    void writeBytes(byte[] bytes) {
        out.writeBytes(bytes);
    }

    void writeUuid(UUID uuid) {
        writeLong(uuid.getMostSignificantBits());
        writeLong(uuid.getLeastSignificantBits());
    }

    void writePosition(int x, int y, int z) {
        long encoded = ((long) (x & 0x3ffffff) << 38)
                | ((long) (z & 0x3ffffff) << 12)
                | (long) (y & 0xfff);
        writeLong(encoded);
    }

    byte[] toByteArray() {
        return out.toByteArray();
    }

    int readUnsignedByte() {
        require(1);
        return input[cursor++] & 0xff;
    }

    boolean readBoolean() {
        return readUnsignedByte() != 0;
    }

    int readUnsignedShort() {
        require(2);
        int value = ByteBuffer.wrap(input, cursor, 2).getShort() & 0xffff;
        cursor += 2;
        return value;
    }

    int readInt() {
        require(4);
        int value = ByteBuffer.wrap(input, cursor, 4).getInt();
        cursor += 4;
        return value;
    }

    long readLong() {
        require(8);
        long value = ByteBuffer.wrap(input, cursor, 8).getLong();
        cursor += 8;
        return value;
    }

    float readFloat() {
        return Float.intBitsToFloat(readInt());
    }

    double readDouble() {
        return Double.longBitsToDouble(readLong());
    }

    int readVarInt() {
        int value = 0;
        int position = 0;
        int current;
        do {
            current = readUnsignedByte();
            value |= (current & 0x7f) << position;
            position += 7;
            if (position > 35) {
                throw new IllegalStateException("VarInt is too large");
            }
        } while ((current & 0x80) != 0);
        return value;
    }

    String readString() {
        int len = readVarInt();
        require(len);
        String value = new String(input, cursor, len, StandardCharsets.UTF_8);
        cursor += len;
        return value;
    }

    UUID readUuid() {
        return new UUID(readLong(), readLong());
    }

    int remaining() {
        return input.length - cursor;
    }

    byte[] readRemaining() {
        require(remaining());
        byte[] value = new byte[remaining()];
        System.arraycopy(input, cursor, value, 0, value.length);
        cursor = input.length;
        return value;
    }

    java.util.List<String> readNbtStrings() {
        java.util.List<String> strings = new java.util.ArrayList<>();
        int rootType = readUnsignedByte();
        readNbtPayload(rootType, strings);
        return strings;
    }

    private void readNbtPayload(int type, java.util.List<String> strings) {
        switch (type) {
            case 0 -> {
            }
            case 1 -> skip(1);
            case 2 -> skip(2);
            case 3, 5 -> skip(4);
            case 4, 6 -> skip(8);
            case 7 -> skip(readInt());
            case 8 -> strings.add(readShortString());
            case 9 -> {
                int itemType = readUnsignedByte();
                int len = readInt();
                for (int i = 0; i < len; i++) {
                    readNbtPayload(itemType, strings);
                }
            }
            case 10 -> {
                while (true) {
                    int itemType = readUnsignedByte();
                    if (itemType == 0) {
                        return;
                    }
                    readShortString();
                    readNbtPayload(itemType, strings);
                }
            }
            case 11 -> skip(Math.multiplyExact(readInt(), 4));
            case 12 -> skip(Math.multiplyExact(readInt(), 8));
            default -> throw new IllegalStateException("unknown NBT tag type: " + type);
        }
    }

    private String readShortString() {
        int len = readUnsignedShort();
        require(len);
        String value = new String(input, cursor, len, StandardCharsets.UTF_8);
        cursor += len;
        return value;
    }

    private void skip(int len) {
        require(len);
        cursor += len;
    }

    private void require(int len) {
        if (input == null || cursor + len > input.length) {
            throw new IllegalStateException("packet underflow: need " + len + ", remaining " + remaining());
        }
    }
}
