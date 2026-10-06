package cn.qexed.protocol;

import java.io.InputStream;
import java.lang.classfile.ClassFile;
import java.lang.classfile.ClassModel;
import java.lang.classfile.MethodModel;
import java.lang.classfile.CodeModel;
import java.lang.classfile.Instruction;
import java.lang.classfile.instruction.FieldInstruction;
import java.lang.classfile.instruction.InvokeDynamicInstruction;
import java.lang.classfile.instruction.InvokeInstruction;
import java.lang.constant.ClassDesc;
import java.lang.constant.DirectMethodHandleDesc;
import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.lang.reflect.ParameterizedType;
import java.net.URL;
import java.util.ArrayList;
import java.util.Enumeration;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Extracts field-level packet wire structures by parsing the STREAM_CODEC
 * static initializer of every Minecraft packet class, using the JDK standard
 * Class-File API. Produces packet-structures.json next to Mojang's own
 * packets.json so names join via the packet ResourceLocation.
 */
public final class VanillaPacketStructExtractor {

    public static void main(String[] args) throws Exception {
        String outputPath = args.length > 0 ? args[0] : "build/vanilla-reports/packet-structures.json";
        URL jarUrl = net.minecraft.network.protocol.Packet.class
                .getProtectionDomain().getCodeSource().getLocation();
        String ssp = jarUrl.toURI().getSchemeSpecificPart();
        String jarPath = ssp.startsWith("/") && ssp.length() > 2 && ssp.charAt(2) == ':'
                ? ssp.substring(1) : ssp;
        System.out.println("[qexed] scanning " + jarPath);

        Map<String, Object> out = new LinkedHashMap<>();
        Map<String, String> typeFieldToLocation = gamePacketTypeFields();

        int packetCount = 0;
        int structCount = 0;
        try (JarFile jar = new JarFile(jarPath)) {
            Enumeration<JarEntry> entries = jar.entries();
            List<String> candidates = new ArrayList<>();
            while (entries.hasMoreElements()) {
                JarEntry e = entries.nextElement();
                String n = e.getName();
                if (n.startsWith("net/minecraft/network/protocol/") && n.endsWith(".class") && !n.contains("$")) {
                    candidates.add(n);
                }
            }
            for (String n : candidates) {
                byte[] bytes;
                try (InputStream in = jar.getInputStream(jar.getEntry(n))) {
                    bytes = in.readAllBytes();
                }
                ClassModel cm = ClassFile.of().parse(bytes);
                boolean hasCodec = cm.fields().stream()
                        .anyMatch(fld -> fld.fieldName().stringValue().equals("STREAM_CODEC"));
                if (!hasCodec) continue;
                boolean isPacket = cm.interfaces().stream()
                        .anyMatch(i -> i.asInternalName().equals("net/minecraft/network/protocol/Packet"));
                String className = n.substring(0, n.length() - 6).replace('/', '.');
                List<Map<String, Object>> fields = parseCodec(cm, className);
                boolean hasUnknown = fields.stream().anyMatch(f -> "unknown".equals(f.get("wire")));
                if (hasUnknown) {
                    List<Map<String, Object>> viaWrite = parseWriteMethod(cm);
                    if (!viaWrite.isEmpty()) fields = viaWrite;
                }
                String packetName = isPacket ? packetNameOf(cm, typeFieldToLocation) : null;
                Map<String, Object> entry = new LinkedHashMap<>();
                if (packetName != null) entry.put("packet", packetName);
                entry.put("class", className);
                entry.put("kind", isPacket ? "packet" : "struct");
                entry.put("fields", fields);
                out.put(className, entry);
                if (isPacket) packetCount++; else structCount++;
            }
        }

        String nl = String.valueOf((char) 10);
        StringBuilder json = new StringBuilder("{").append(nl);
        boolean[] first = {true};
        out.forEach((k, v) -> {
            if (!first[0]) json.append(",").append(nl);
            first[0] = false;
            json.append("  \"").append(k).append("\": ").append(toJson(v, 1));
        });
        json.append(nl).append("}").append(nl);
        java.nio.file.Path p = java.nio.file.Path.of(outputPath);
        java.nio.file.Files.createDirectories(p.getParent());
        java.nio.file.Files.writeString(p, json.toString());
        System.out.println("[qexed] " + packetCount + " packets, " + structCount
                + " structs -> " + outputPath);
    }

    /** Map "GamePacketTypes.CLIENTBOUND_LOGIN" static field name -> packet ResourceLocation string. */
    private static Map<String, String> gamePacketTypeFields() throws Exception {
        Map<String, String> map = new LinkedHashMap<>();
        for (Class<?> holder : new Class<?>[] {
                net.minecraft.network.protocol.game.GamePacketTypes.class,
                net.minecraft.network.protocol.common.CommonPacketTypes.class,
                net.minecraft.network.protocol.configuration.ConfigurationPacketTypes.class,
                net.minecraft.network.protocol.login.LoginPacketTypes.class,
                net.minecraft.network.protocol.status.StatusPacketTypes.class,
                net.minecraft.network.protocol.handshake.HandshakePacketTypes.class,
                net.minecraft.network.protocol.cookie.CookiePacketTypes.class,
                net.minecraft.network.protocol.ping.PingPacketTypes.class}) {
            for (Field f : holder.getDeclaredFields()) {
                if (f.getType() == net.minecraft.network.protocol.PacketType.class) {
                    f.setAccessible(true);
                    Object v = f.get(null);
                    map.put(holder.getSimpleName() + "." + f.getName(), String.valueOf(v));
                }
            }
        }
        return map;
    }

    /** Read the GETSTATIC GamePacketTypes.X inside type() to resolve the packet name. */
    private static String packetNameOf(ClassModel cm, Map<String, String> typeFields) {
        for (MethodModel m : cm.methods()) {
            if (!m.methodName().stringValue().equals("type")) continue;
            CodeModel body = m.code().orElse(null);
            if (body == null) continue;
            for (var elt : body.elementList()) {
                if (!(elt instanceof Instruction ins)) continue;
                if (ins instanceof FieldInstruction fi
                        && fi.opcode() == java.lang.classfile.Opcode.GETSTATIC
                        && fi.owner().asInternalName().endsWith("GamePacketTypes")) {
                    return typeFields.getOrDefault(fi.owner().asInternalName()
                            .substring(fi.owner().asInternalName().lastIndexOf('/') + 1)
                            + "." + fi.name().stringValue(), null);
                }
            }
        }
        return null;
    }

    /** Fallback for packets using Packet.codec(encoder, decoder): parse write(FriendlyByteBuf). */
    private static List<Map<String, Object>> parseWriteMethod(ClassModel cm) {
        List<Map<String, Object>> fields = new ArrayList<>();
        for (MethodModel m : cm.methods()) {
            if (!m.methodName().stringValue().equals("write")) continue;
            CodeModel body = m.code().orElse(null);
            if (body == null) continue;
            String pendingField = null;
            for (var elt : body.elementList()) {
                if (!(elt instanceof Instruction ins)) continue;
                if (ins instanceof FieldInstruction fi && fi.opcode() == java.lang.classfile.Opcode.GETFIELD
                        && fi.owner().asInternalName().equals(cm.thisClass().asInternalName())) {
                    pendingField = fi.name().stringValue();
                } else if (ins instanceof InvokeInstruction inv
                        && inv.name().stringValue().equals("write")
                        && pendingField != null
                        && !inv.owner().asInternalName().endsWith("FriendlyByteBuf")
                        && !inv.owner().asInternalName().endsWith("ByteBuf")) {
                    Map<String, Object> f = new LinkedHashMap<>();
                    f.put("name", pendingField);
                    f.put("wire", "delegated:" + inv.owner().asInternalName().substring(inv.owner().asInternalName().lastIndexOf('/') + 1));
                    fields.add(f);
                    pendingField = null;
                } else if (ins instanceof InvokeInstruction inv
                        && (inv.owner().asInternalName().endsWith("FriendlyByteBuf")
                            || inv.owner().asInternalName().endsWith("ByteBuf"))
                        && (inv.name().stringValue().startsWith("write") || inv.name().stringValue().startsWith("read"))) {
                    String mname = inv.name().stringValue();
                    boolean nullable = mname.equals("writeNullable") || mname.equals("readNullable")
                            || mname.equals("writeOptional") || mname.equals("writeOpt");
                    String wire = nullable ? "optional" : wireOfWrite(mname);
                    if (wire != null && pendingField != null) {
                        Map<String, Object> f = new LinkedHashMap<>();
                        f.put("name", pendingField);
                        f.put("wire", wire);
                        fields.add(f);
                        pendingField = null;
                    }
                }
            }
        }
        return fields;
    }

    private static String wireOfWrite(String method) {
        return switch (method) {
            case "writeVarInt", "readVarInt" -> "varint";
            case "writeVarLong", "readVarLong" -> "varlong";
            case "writeInt", "readInt" -> "i32";
            case "writeLong", "readLong" -> "i64";
            case "writeShort", "readShort" -> "i16";
            case "writeByte", "readByte", "readUnsignedByte" -> "u8";
            case "writeDouble", "readDouble" -> "f64";
            case "writeFloat", "readFloat" -> "f32";
            case "writeBoolean", "readBoolean" -> "bool";
            case "writeUtf", "readUtf" -> "string";
            case "writeUUID", "readUUID" -> "uuid";
            case "writeByteArray", "readByteArray" -> "bytes";
            case "writeBlockPos", "readBlockPos" -> "position";
            case "writeVec3", "readVec3" -> "vec3f64";
            case "writeEnum", "readEnum" -> "varint";
            case "writeResourceLocation", "readResourceLocation",
                 "writeIdentifier", "readIdentifier" -> "identifier";
            case "writeContainerId", "readContainerId" -> "varint";
            case "writeChunkPos", "readChunkPos" -> "chunk_pos";
            default -> null;
        };
    }

    private static List<Map<String, Object>> parseCodec(ClassModel cm, String className) {
        List<Map<String, Object>> fields = new ArrayList<>();
        for (MethodModel m : cm.methods()) {
            if (!m.methodName().stringValue().equals("<clinit>")) continue;
            CodeModel body = m.code().orElse(null);
            if (body == null) continue;
            String pendingWire = null;
            boolean pendingCollection = false;
            for (var elt : body.elementList()) {
                if (!(elt instanceof Instruction ins)) continue;
                if (ins instanceof FieldInstruction fi && fi.opcode() == java.lang.classfile.Opcode.GETSTATIC) {
                    String owner = fi.owner().asInternalName();
                    String name = fi.name().stringValue();
                    String desc = fi.type().stringValue();
                    if (desc.contains("StreamCodec")) {
                        pendingWire = wireOf(owner, name);
                    } else if (owner.endsWith("Registries") || desc.contains("ResourceKey")) {
                        pendingWire = "resource_key:" + name.toLowerCase();
                    }
                } else if (ins instanceof InvokeInstruction inv) {
                    String owner = inv.owner().asInternalName();
                    String name = inv.name().stringValue();
                    String desc = inv.type().stringValue();
                    if (name.equals("streamCodec") && owner.endsWith("ResourceKey")) {
                        // keep pendingWire set by the Registries GETSTATIC above
                    } else if (owner.endsWith("ByteBufCodecs") && name.equals("collection")) {
                        pendingCollection = true;
                    } else if (desc.contains("StreamCodec") && desc.endsWith(";")
                            && inv.opcode() == java.lang.classfile.Opcode.INVOKESTATIC
                            && pendingWire == null) {
                        pendingWire = wireOf(owner, name);
                    }
                } else if (ins instanceof InvokeDynamicInstruction indy) {
                    for (Object arg : indy.bootstrapArgs()) {
                        if (arg instanceof DirectMethodHandleDesc mh && mh.refKind() == 5 /* REF_invokeVirtual */
                                && mh.owner().descriptorString().contains(className.replace('.', '/'))) {
                            String getter = mh.methodName();
                            String wire = pendingWire == null ? "unknown" : pendingWire;
                            if (pendingCollection) { wire = "array:" + wire; pendingCollection = false; }
                            Map<String, Object> f = new LinkedHashMap<>();
                            f.put("name", getter);
                            f.put("wire", wire);
                            fields.add(f);
                            pendingWire = null;
                        }
                    }
                }
            }
        }
        return fields;
    }

    private static String wireOf(String owner, String staticName) {
        String o = owner.substring(owner.lastIndexOf('/') + 1);
        return switch (staticName) {
            case "VAR_INT" -> "varint";
            case "VAR_LONG" -> "varlong";
            case "INT" -> "i32";
            case "LONG" -> "i64";
            case "SHORT" -> "i16";
            case "BYTE", "BOOLEAN_BYTE" -> "i8";
            case "FLOAT" -> "f32";
            case "DOUBLE" -> "f64";
            case "BOOL" -> "bool";
            case "STRING_UTF8", "STRING" -> "string";
            case "NBT", "ANY_NBT", "OPTIONAL_NBT", "TRUSTED_NBT" -> "nbt";
            case "VECTOR3F" -> "vec3f";
            case "VECTOR2F" -> "vec2f";
            case "QUATERNIONF" -> "quatf";
            default -> o + "." + staticName;
        };
    }

    private static String toJson(Object v, int depth) {
        StringBuilder sb = new StringBuilder();
        String pad = "  ".repeat(depth + 1);
        String nl = String.valueOf((char) 10);
        if (v instanceof Map<?, ?> m) {
            sb.append('{').append(nl);
            boolean[] f = {true};
            m.forEach((k, val) -> {
                if (!f[0]) sb.append(',').append(nl);
                f[0] = false;
                sb.append(pad).append('"').append(k).append("\": ").append(toJson(val, depth + 1));
            });
            sb.append(nl).append("  ".repeat(depth)).append('}');
        } else if (v instanceof String s) {
            sb.append('"').append(s).append('"');
        } else if (v instanceof List<?> list) {
            sb.append('[');
            boolean[] f = {true};
            for (Object item : list) {
                if (!f[0]) sb.append(", ");
                f[0] = false;
                sb.append(toJson(item, depth));
            }
            sb.append(']');
        } else {
            sb.append(v);
        }
        return sb.toString();
    }

    private VanillaPacketStructExtractor() {}
}
