package cn.qexed.bytebuf;

/** Method-name to wire-type mapping shared by constructor and write parsing. */
final class WireMap {

    private WireMap() {}

    /** FriendlyByteBuf readXxx -> wire type. */
    static String wireOfRead(String method) {
        String w = wireOfCommon(method, "read");
        if (w != null) {
            return w;
        }
        return switch (method) {
            case "readTrustedNbt", "readNbt" -> "nbt";
            case "readNullable", "readOptional" -> "optional";
            case "readEnum" -> "varint_enum";
            case "readEnumSet" -> "bitset_enum";
            case "readEnumCount" -> "varint_enum_count";
            case "readById" -> "varint_by_id";
            case "readRegistry" -> "registry_id";
            case "readItem", "readOptionalItem" -> "item_stack";
            case "readVarIntArray" -> "array_varint";
            case "readVarLongArray" -> "array_varlong";
            case "readLongArray" -> "array_i64";
            case "readFixedByteArray" -> "bytes_fixed";
            case "readChunkPos" -> "chunk_pos_i64";
            case "readContainerId" -> "container_id_varint";
            case "readQuaternionf" -> "quat_f32x4";
            case "readVector3f" -> "vec3_f32";
            case "readVector2f" -> "vec2_f32";
            default -> null;
        };
    }

    /** FriendlyByteBuf writeXxx -> wire type. */
    static String wireOfWrite(String method) {
        if (method.equals("writeNbt")) {
            return "nbt";
        }
        String w = wireOfCommon(method, "write");
        if (w != null) {
            return w;
        }
        return switch (method) {
            case "writeNullable", "writeOptional" -> "optional";
            case "writeEnum" -> "varint_enum";
            case "writeEnumCount" -> "varint_enum_count";
            case "writeById" -> "varint_by_id";
            case "writeItem", "writeOptionalItem" -> "item_stack";
            case "writeVarIntArray" -> "array_varint";
            case "writeVarLongArray" -> "array_varlong";
            case "writeLongArray" -> "array_i64";
            case "writeFixedByteArray" -> "bytes_fixed";
            case "writeChunkPos" -> "chunk_pos_i64";
            case "writeContainerId" -> "container_id_varint";
            case "writeQuaternionf" -> "quat_f32x4";
            case "writeVector3f" -> "vec3_f32";
            case "writeVector2f" -> "vec2_f32";
            default -> null;
        };
    }

    private static String wireOfCommon(String method, String prefix) {
        if (!method.startsWith(prefix)) {
            return null;
        }
        return switch (method) {
            case "readVarInt", "writeVarInt" -> "varint";
            case "readVarLong", "writeVarLong" -> "varlong";
            case "readInt", "writeInt" -> "i32_be";
            case "readLong", "writeLong" -> "i64_be";
            case "readShort", "writeShort" -> "i16_be";
            case "readUnsignedShort", "writeUnsignedShort" -> "u16_be";
            case "readByte", "writeByte" -> "i8";
            case "readUnsignedByte", "writeUnsignedByte" -> "u8";
            case "readDouble", "writeDouble" -> "f64_be";
            case "readFloat", "writeFloat" -> "f32_be";
            case "readBoolean", "writeBoolean" -> "bool";
            case "readUtf", "writeUtf", "readString", "writeString" -> "string_utf8";
            case "readUUID", "writeUUID" -> "uuid";
            case "readByteArray", "writeByteArray" -> "bytes_varint_len";
            case "readBlockPos", "writeBlockPos" -> "block_pos_i64";
            case "readVec3", "writeVec3" -> "vec3_f64";
            case "readResourceLocation", "writeResourceLocation",
                 "readIdentifier", "writeIdentifier" -> "identifier";
            case "readDate", "writeDate" -> "i64_date";
            default -> null;
        };
    }

    /** Known STREAM_CODEC static fields -> wire type (used for codec.decode/encode). */
    static String wireOfCodec(String owner, String field) {
        String o = owner.substring(owner.lastIndexOf('/') + 1);
        String key = o + "." + field;
        return switch (key) {
            case "Vec3.LP_STREAM_CODEC" -> "vec3_f64";
            case "Vec3i.STREAM_CODEC" -> "vec3_i32";
            case "ItemStack.OPTIONAL_STREAM_CODEC", "ItemStack.STREAM_CODEC" -> "item_stack";
            case "ByteBufCodecs.TRUSTED_NBT" -> "nbt";
            default -> "codec:" + key;
        };
    }

    /** FriendlyByteBuf method name of a lambda/method handle, if it is a reader. */
    static boolean isBufRead(String method) {
        return method.startsWith("read");
    }

    static boolean isBufWrite(String method) {
        return method.startsWith("write");
    }
}