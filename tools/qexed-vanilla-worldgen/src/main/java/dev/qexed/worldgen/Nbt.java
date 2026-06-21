package dev.qexed.worldgen;

import java.util.Map;

final class Nbt {
    static final byte END = 0;
    static final byte BYTE = 1;
    static final byte INT = 3;
    static final byte LONG = 4;
    static final byte STRING = 8;
    static final byte LIST = 9;
    static final byte COMPOUND = 10;
    static final byte LONG_ARRAY = 12;

    private Nbt() {}

    record Tag(byte type, Object value) {}
    record ListValue(byte elementType, java.util.List<Tag> values) {}

    static Tag byteTag(byte value) {
        return new Tag(BYTE, value);
    }

    static Tag intTag(int value) {
        return new Tag(INT, value);
    }

    static Tag longTag(long value) {
        return new Tag(LONG, value);
    }

    static Tag stringTag(String value) {
        return new Tag(STRING, value);
    }

    static Tag listTag(byte elementType, java.util.List<Tag> values) {
        return new Tag(LIST, new ListValue(elementType, values));
    }

    static Tag compoundTag(Map<String, Object> fields) {
        return new Tag(COMPOUND, fields);
    }

    static Tag longArrayTag(long[] values) {
        return new Tag(LONG_ARRAY, values);
    }
}
