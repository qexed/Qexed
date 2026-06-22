package dev.qexed.worldgen;

import java.lang.instrument.ClassFileTransformer;
import java.lang.instrument.Instrumentation;
import java.security.ProtectionDomain;
import java.util.Collections;
import java.util.IdentityHashMap;
import java.util.Map;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.ClassVisitor;
import org.objectweb.asm.ClassWriter;
import org.objectweb.asm.MethodVisitor;
import org.objectweb.asm.Opcodes;

public final class TargetBlockWriteTraceAgent {
    private static final String METHOD_NAME = "setBlockState";
    private static final String METHOD_DESC = "(Lnet/minecraft/core/BlockPos;Lnet/minecraft/world/level/block/state/BlockState;I)Lnet/minecraft/world/level/block/state/BlockState;";
    private static final String SECTION_METHOD_DESC = "(IIILnet/minecraft/world/level/block/state/BlockState;Z)Lnet/minecraft/world/level/block/state/BlockState;";
    private static final String GET_SECTION_DESC = "(I)Lnet/minecraft/world/level/chunk/LevelChunkSection;";
    private static final Map<Object, SectionInfo> SECTIONS =
            Collections.synchronizedMap(new IdentityHashMap<>());

    private TargetBlockWriteTraceAgent() {
    }

    public static void premain(String args, Instrumentation instrumentation) {
        instrumentation.addTransformer(new Transformer(args));
    }

    public static void trace(String owner, Object chunk, Object pos, Object replacement) {
        try {
            Class<?> vec3i = Class.forName("net.minecraft.core.Vec3i", false, pos.getClass().getClassLoader());
            int x = (Integer) vec3i.getMethod("getX").invoke(pos);
            int y = (Integer) vec3i.getMethod("getY").invoke(pos);
            int z = (Integer) vec3i.getMethod("getZ").invoke(pos);
            if (x != targetX() || y != targetY() || z != targetZ()) {
                return;
            }

            Object chunkPos = chunk.getClass().getMethod("getPos").invoke(chunk);
            System.out.printf(
                    "java target write trace: owner=%s chunk=%s coord=(%d,%d,%d) replacement=%s%n",
                    owner,
                    chunkPos,
                    x,
                    y,
                    z,
                    replacement);
            for (StackTraceElement frame : Thread.currentThread().getStackTrace()) {
                String className = frame.getClassName();
                if (className.startsWith("java.")
                        || className.startsWith("jdk.")
                        || className.equals(TargetBlockWriteTraceAgent.class.getName())) {
                    continue;
                }
                System.out.printf("java target write trace frame: %s%n", frame);
            }
        } catch (ReflectiveOperationException error) {
            System.out.printf("java target write trace failed: %s%n", error);
        }
    }

    public static void traceSection(String owner, int localX, int localY, int localZ, Object replacement) {
        traceSectionWithObject(owner, null, localX, localY, localZ, replacement);
    }

    public static void traceSectionWithObject(String owner, Object section, int localX, int localY, int localZ, Object replacement) {
        SectionInfo sectionInfo = section == null ? null : SECTIONS.get(section);
        if (sectionInfo == null && localX == (targetX() & 15) && localY == (targetY() & 15) && localZ == (targetZ() & 15)) {
            System.out.printf(
                    "java target section write trace unregistered: owner=%s local=(%d,%d,%d) replacement=%s%n",
                    owner,
                    localX,
                    localY,
                    localZ,
                    replacement);
        }
        int worldX = sectionInfo == null ? localX : sectionInfo.chunkX * 16 + localX;
        int worldY = sectionInfo == null ? localY : sectionInfo.sectionY * 16 + localY;
        int worldZ = sectionInfo == null ? localZ : sectionInfo.chunkZ * 16 + localZ;
        if (worldX != targetX() || worldY != targetY() || worldZ != targetZ()) {
            return;
        }
        String replacementText = String.valueOf(replacement);
        if (!replacementText.contains("redstone")
                && !replacementText.contains("deepslate")
                && !replacementText.contains("air")) {
            return;
        }

        System.out.printf(
                "java target section write trace: owner=%s coord=(%d,%d,%d) local=(%d,%d,%d) replacement=%s%n",
                owner,
                worldX,
                worldY,
                worldZ,
                localX,
                localY,
                localZ,
                replacement);
        for (StackTraceElement frame : Thread.currentThread().getStackTrace()) {
            String className = frame.getClassName();
            if (className.startsWith("java.")
                    || className.startsWith("jdk.")
                    || className.equals(TargetBlockWriteTraceAgent.class.getName())) {
                continue;
            }
            System.out.printf("java target section write trace frame: %s%n", frame);
        }
    }

    public static Object registerSection(String owner, Object chunk, int sectionIndex, Object section) {
        try {
            Object chunkPos = chunk.getClass().getMethod("getPos").invoke(chunk);
            int chunkX = (Integer) chunkPos.getClass().getMethod("x").invoke(chunkPos);
            int chunkZ = (Integer) chunkPos.getClass().getMethod("z").invoke(chunkPos);
            int sectionY = (Integer) chunk.getClass().getMethod("getSectionYFromSectionIndex", int.class)
                    .invoke(chunk, sectionIndex);
            SECTIONS.put(section, new SectionInfo(chunkX, chunkZ, sectionY));
        } catch (ReflectiveOperationException error) {
            System.out.printf("java target section registration failed: owner=%s error=%s%n", owner, error);
        }
        return section;
    }

    private static int targetX() {
        return Integer.getInteger("qexed.traceBlock.x", 6);
    }

    private static int targetY() {
        return Integer.getInteger("qexed.traceBlock.y", -61);
    }

    private static int targetZ() {
        return Integer.getInteger("qexed.traceBlock.z", 0);
    }

    private static final class Transformer implements ClassFileTransformer {
        private final String agentArgs;

        private Transformer(String agentArgs) {
            this.agentArgs = agentArgs == null ? "" : agentArgs;
        }

        @Override
        public byte[] transform(
                Module module,
                ClassLoader loader,
                String className,
                Class<?> classBeingRedefined,
                ProtectionDomain protectionDomain,
                byte[] classfileBuffer) {
            if (!"net/minecraft/world/level/chunk/ProtoChunk".equals(className)
                    && !"net/minecraft/world/level/chunk/LevelChunk".equals(className)
                    && !"net/minecraft/world/level/chunk/ChunkAccess".equals(className)
                    && !"net/minecraft/world/level/chunk/LevelChunkSection".equals(className)) {
                return null;
            }

            ClassReader reader = new ClassReader(classfileBuffer);
            ClassWriter writer = new ClassWriter(reader, ClassWriter.COMPUTE_MAXS);
            reader.accept(new Visitor(writer, className, agentArgs), 0);
            return writer.toByteArray();
        }
    }

    private static final class Visitor extends ClassVisitor {
        private final String className;
        private final String agentArgs;

        private Visitor(ClassVisitor delegate, String className, String agentArgs) {
            super(Opcodes.ASM9, delegate);
            this.className = className;
            this.agentArgs = agentArgs;
        }

        @Override
        public MethodVisitor visitMethod(int access, String name, String descriptor, String signature, String[] exceptions) {
            MethodVisitor method = super.visitMethod(access, name, descriptor, signature, exceptions);
            if (METHOD_NAME.equals(name) && SECTION_METHOD_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitCode() {
                        super.visitCode();
                        visitLdcInsn(agentArgs.isBlank() ? className : agentArgs + ":" + className);
                        visitVarInsn(Opcodes.ALOAD, 0);
                        visitVarInsn(Opcodes.ILOAD, 1);
                        visitVarInsn(Opcodes.ILOAD, 2);
                        visitVarInsn(Opcodes.ILOAD, 3);
                        visitVarInsn(Opcodes.ALOAD, 4);
                        visitMethodInsn(
                                Opcodes.INVOKESTATIC,
                                "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                "traceSectionWithObject",
                                "(Ljava/lang/String;Ljava/lang/Object;IIILjava/lang/Object;)V",
                                false);
                    }
                };
            }
            if ("getSection".equals(name) && GET_SECTION_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitInsn(int opcode) {
                        if (opcode == Opcodes.ARETURN) {
                            visitVarInsn(Opcodes.ASTORE, 2);
                            visitLdcInsn(agentArgs.isBlank() ? className : agentArgs + ":" + className);
                            visitVarInsn(Opcodes.ALOAD, 0);
                            visitVarInsn(Opcodes.ILOAD, 1);
                            visitVarInsn(Opcodes.ALOAD, 2);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "registerSection",
                                    "(Ljava/lang/String;Ljava/lang/Object;ILjava/lang/Object;)Ljava/lang/Object;",
                                    false);
                            visitTypeInsn(Opcodes.CHECKCAST, "net/minecraft/world/level/chunk/LevelChunkSection");
                        }
                        super.visitInsn(opcode);
                    }
                };
            }
            if (!METHOD_NAME.equals(name) || !METHOD_DESC.equals(descriptor)) {
                return method;
            }
            return new MethodVisitor(Opcodes.ASM9, method) {
                @Override
                public void visitCode() {
                    super.visitCode();
                    visitLdcInsn(agentArgs.isBlank() ? className : agentArgs + ":" + className);
                    visitVarInsn(Opcodes.ALOAD, 0);
                    visitVarInsn(Opcodes.ALOAD, 1);
                    visitVarInsn(Opcodes.ALOAD, 2);
                    visitMethodInsn(
                            Opcodes.INVOKESTATIC,
                            "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                            "trace",
                            "(Ljava/lang/String;Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
                            false);
                }
            };
        }
    }

    private record SectionInfo(int chunkX, int chunkZ, int sectionY) {
    }
}
