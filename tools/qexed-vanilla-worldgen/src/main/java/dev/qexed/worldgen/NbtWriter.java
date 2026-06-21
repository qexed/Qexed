package dev.qexed.worldgen;

import java.io.ByteArrayOutputStream;
import java.io.DataOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Map;

final class NbtWriter {
    private final ByteArrayOutputStream bytes = new ByteArrayOutputStream();
    private final DataOutputStream output = new DataOutputStream(bytes);

    void writeRoot(Map<String, Object> root) throws Exception {
        output.writeByte(Nbt.COMPOUND);
        writeString("");
        writeCompound(root);
    }

    byte[] toByteArray() {
        return bytes.toByteArray();
    }

    @SuppressWarnings("unchecked")
    private void writeNamedTag(String name, Nbt.Tag tag) throws Exception {
        output.writeByte(tag.type());
        writeString(name);
        writePayload(tag);
    }

    @SuppressWarnings("unchecked")
    private void writePayload(Nbt.Tag tag) throws Exception {
        switch (tag.type()) {
            case Nbt.BYTE -> output.writeByte((Byte) tag.value());
            case Nbt.INT -> output.writeInt((Integer) tag.value());
            case Nbt.LONG -> output.writeLong((Long) tag.value());
            case Nbt.STRING -> writeString((String) tag.value());
            case Nbt.LIST -> writeList((Nbt.ListValue) tag.value());
            case Nbt.COMPOUND -> writeCompound((Map<String, Object>) tag.value());
            case Nbt.LONG_ARRAY -> writeLongArray((long[]) tag.value());
            default -> throw new IllegalArgumentException("unsupported NBT tag type: " + tag.type());
        }
    }

    private void writeCompound(Map<String, Object> compound) throws Exception {
        for (Map.Entry<String, Object> entry : compound.entrySet()) {
            writeNamedTag(entry.getKey(), (Nbt.Tag) entry.getValue());
        }
        output.writeByte(Nbt.END);
    }

    private void writeList(Nbt.ListValue list) throws Exception {
        output.writeByte(list.elementType());
        output.writeInt(list.values().size());
        for (Nbt.Tag item : list.values()) {
            writePayload(item);
        }
    }

    private void writeLongArray(long[] values) throws Exception {
        output.writeInt(values.length);
        for (long value : values) {
            output.writeLong(value);
        }
    }

    private void writeString(String value) throws Exception {
        byte[] encoded = value.getBytes(StandardCharsets.UTF_8);
        output.writeShort(encoded.length);
        output.write(encoded);
    }
}
