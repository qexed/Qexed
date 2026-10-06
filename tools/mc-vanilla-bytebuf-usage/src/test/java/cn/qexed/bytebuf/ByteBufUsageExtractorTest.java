package cn.qexed.bytebuf;

import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

import java.io.FileOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.jar.JarEntry;
import java.util.jar.JarOutputStream;

import static org.junit.jupiter.api.Assertions.*;

/**
 * Self-contained tests: the extractor runs against a tiny synthetic jar that
 * mimics the FriendlyByteBuf / RegistryFriendlyByteBuf / packet-class pattern,
 * so the tests never depend on a Minecraft download.
 */
class ByteBufUsageExtractorTest {

    @TempDir
    Path temp;

    /** Mirrors the AddEntity write() pattern: a packet writes through a subtype parameter, methods live on the base. */
    @Test
    void resolvesInheritedCallsToBaseClassDefinition() throws Exception {
        byte[] buf = declaringClass("synth/FriendlyByteBuf", null,
                new String[][] {{"writeVarInt", "(I)Lsynth/FriendlyByteBuf;"}});
        byte[] sub = declaringClass("synth/RegistryFriendlyByteBuf", "synth/FriendlyByteBuf", new String[0][]);
        byte[] packet = packetClass("synth/Packet", "synth/RegistryFriendlyByteBuf");
        Path jar = writeJar(buf, sub, packet);

        Path out = temp.resolve("out");
        ByteBufUsageExtractor.Args args = ByteBufUsageExtractor.Args.parse(new String[] {
                "--out", out.toString(), "--target", "synth.FriendlyByteBuf",
                "--mc-version", "test", "--jar", jar.toString()});
        ByteBufUsageExtractor.run(args);

        String json = Files.readString(out.resolve("bytebuf-usage.json"));
        assertTrue(json.contains("synth.FriendlyByteBuf#writeVarInt"),
                "definition must be attributed to the base class, json=" + json);
        assertTrue(json.contains("synth.Packet"), "packet caller must appear");
        assertTrue(java.util.regex.Pattern.compile("\"callerClass\"\\s*:\\s*\"synth.Packet\"").matcher(json).find(),
                "caller field must use the dotted name, json=" + json);

        String csv = Files.readString(out.resolve("bytebuf-usage.csv"));
        assertTrue(csv.contains("synth.Packet"));
        assertTrue(csv.contains("writeVarInt"));
        // class-name columns must use dotted names (descriptors keep their internal form)
        assertTrue(csv.lines().skip(1).allMatch(l -> l.startsWith("synth.FriendlyByteBuf,")),
                "owner column must be dotted: " + csv);
        assertTrue(csv.lines().skip(1).noneMatch(l -> l.split(",")[3].contains("/")),
                "callerClass column must be dotted: " + csv);
    }

    @Test
    void handlesInvokeDynamicMethodReferences() throws Exception {
        byte[] buf = declaringClass("synth/FriendlyByteBuf", null,
                new String[][] {{"writeVarInt", "(I)Lsynth/FriendlyByteBuf;"}});
        byte[] caller = indyCaller("synth/LambdaUser", "synth/FriendlyByteBuf");
        Path jar = writeJar(new String[]{"synth/FriendlyByteBuf.class", "synth/LambdaUser.class"}, buf, caller);

        Path out = temp.resolve("out");
        ByteBufUsageExtractor.run(ByteBufUsageExtractor.Args.parse(new String[] {
                "--out", out.toString(), "--target", "synth.FriendlyByteBuf",
                "--mc-version", "test", "--jar", jar.toString()}));

        String json = Files.readString(out.resolve("bytebuf-usage.json"));
        assertTrue(json.contains("INVOKEDYNAMIC-handle-arg") || json.contains("INVOKEDYNAMIC-bootstrap"),
                "indy handle reference must be recorded: " + json);
    }

    @Test
    void unknownTargetTypeFailsLoudly() throws Exception {
        byte[] buf = declaringClass("synth/FriendlyByteBuf", null, new String[0][]);
        Path jar = writeJar(new String[]{"synth/FriendlyByteBuf.class"}, buf);
        Path out = temp.resolve("out");
        ByteBufUsageExtractor.Args args = ByteBufUsageExtractor.Args.parse(new String[] {
                "--out", out.toString(), "--target", "synth.Missing",
                "--mc-version", "test", "--jar", jar.toString()});
        IllegalStateException ex = assertThrows(IllegalStateException.class, () -> ByteBufUsageExtractor.run(args));
        assertTrue(ex.getMessage().contains("synth.Missing"));
    }

    @Test
    void missingJarArgumentsFails() {
        assertThrows(IllegalArgumentException.class,
                () -> ByteBufUsageExtractor.Args.parse(new String[] {"--out", "x"}));
    }

    @Test
    void prettyDescriptorRendersTypes() {
        assertEquals("(int, java.lang.String) byte[]",
                ByteBufUsageExtractor.prettyDescriptor("(ILjava/lang/String;)[B"));
        assertEquals("() void", ByteBufUsageExtractor.prettyDescriptor("()V"));
        assertEquals("(java.util.UUID) void",
                ByteBufUsageExtractor.prettyDescriptor("(Ljava/util/UUID;)V"));
    }

    @Test
    void modifiersOfCoversFlags() {
        String mods = ByteBufUsageExtractor.modifiersOf(Opcodes.ACC_PUBLIC | Opcodes.ACC_FINAL | Opcodes.ACC_ABSTRACT);
        assertTrue(mods.contains("public") && mods.contains("final") && mods.contains("abstract"));
    }

    // ------------------------------------------------------------------
    // synthetic fixtures (ASM-generated bytecode)
    // ------------------------------------------------------------------

    private byte[] declaringClass(String name, String superName, String[][] methods) {
        ClassWriter cw = new ClassWriter(0);
        cw.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, name, null,
                superName == null ? "java/lang/Object" : superName, null);
        for (String[] m : methods) {
            MethodVisitor mv = cw.visitMethod(Opcodes.ACC_PUBLIC, m[0], m[1], null, null);
            mv.visitCode();
            mv.visitInsn(m[1].endsWith("V") ? Opcodes.RETURN : Opcodes.ARETURN);
            mv.visitMaxs(1, 1);
            mv.visitEnd();
        }
        cw.visitEnd();
        return cw.toByteArray();
    }

    private byte[] packetClass(String name, String bufType) {
        ClassWriter cw = new ClassWriter(0);
        cw.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, name, null, "java/lang/Object", null);
        MethodVisitor mv = cw.visitMethod(Opcodes.ACC_PUBLIC, "write", "(L" + bufType + ";)V", null, null);
        mv.visitCode();
        mv.visitVarInsn(Opcodes.ALOAD, 1);
        mv.visitIntInsn(Opcodes.BIPUSH, 42);
        mv.visitMethodInsn(Opcodes.INVOKEVIRTUAL, bufType, "writeVarInt", "(I)Lsynth/FriendlyByteBuf;", false);
        mv.visitInsn(Opcodes.POP);
        mv.visitInsn(Opcodes.RETURN);
        mv.visitMaxs(2, 2);
        mv.visitEnd();
        cw.visitEnd();
        return cw.toByteArray();
    }

    /**
     * A method whose body contains an invokedynamic whose bootstrap args carry a
     * method handle referencing the buffer class (the lambda/method-ref pattern).
     */
    private byte[] indyCaller(String name, String bufType) {
        ClassWriter cw = new ClassWriter(0);
        cw.visit(Opcodes.V1_8, Opcodes.ACC_PUBLIC, name, null, "java/lang/Object", null);
        MethodVisitor mv = cw.visitMethod(Opcodes.ACC_PUBLIC, "make", "()Ljava/util/function/ToIntFunction;", null, null);
        mv.visitCode();
        org.objectweb.asm.Handle handle = new org.objectweb.asm.Handle(
                Opcodes.H_INVOKEVIRTUAL, bufType, "writeVarInt", "(I)Lsynth/FriendlyByteBuf;", false);
        mv.visitInvokeDynamicInsn("applyAsInt", "()Ljava/util/function/ToIntFunction;",
                new org.objectweb.asm.Handle(Opcodes.H_INVOKESTATIC, "synth/LambdaMeta", "metafactory",
                        "(Ljava/lang/invoke/MethodHandles$Lookup;Ljava/lang/String;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodType;Ljava/lang/invoke/MethodHandle;Ljava/lang/invoke/MethodType;)Ljava/lang/invoke/CallSite;",
                        false),
                org.objectweb.asm.Type.getType("(Ljava/lang/Object;)I"),
                handle,
                org.objectweb.asm.Type.getType("(L" + bufType + ";)I"));
        mv.visitInsn(Opcodes.ARETURN);
        mv.visitMaxs(1, 1);
        mv.visitEnd();
        cw.visitEnd();
        return cw.toByteArray();
    }

    private Path writeJar(byte[]... classes) throws IOException {
        return writeJar(new String[] {
                "synth/FriendlyByteBuf.class",
                "synth/RegistryFriendlyByteBuf.class",
                "synth/Packet.class"}, classes);
    }

    private Path writeJar(String[] names, byte[]... classes) throws IOException {
        Path jar = temp.resolve("fixture-" + System.nanoTime() + ".jar");
        try (JarOutputStream jos = new JarOutputStream(new FileOutputStream(jar.toFile()))) {
            for (int i = 0; i < classes.length; i++) {
                jos.putNextEntry(new JarEntry(names[i]));
                jos.write(classes[i]);
                jos.closeEntry();
            }
        }
        return jar;
    }
}