package cn.qexed.integration;

final class PacketBuffer {
    private final byte[] input;
    private int cursor;

    PacketBuffer(byte[] input) {
        this.input = input;
    }

    int readUnsignedByte() {
        require(1);
        return input[cursor++] & 0xff;
    }

    long readLong() {
        require(8);
        long value = 0;
        for (int i = 0; i < Long.BYTES; i++) {
            value = (value << 8) | readUnsignedByte();
        }
        return value;
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

    byte[] readRemaining() {
        require(remaining());
        byte[] value = new byte[remaining()];
        System.arraycopy(input, cursor, value, 0, value.length);
        cursor = input.length;
        return value;
    }

    int remaining() {
        return input.length - cursor;
    }

    private void require(int len) {
        if (cursor + len > input.length) {
            throw new IllegalStateException("packet underflow: need " + len + ", remaining " + remaining());
        }
    }
}
