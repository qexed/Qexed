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
    private static final String SET_DECORATION_SEED_DESC = "(JII)J";
    private static final String SET_FEATURE_SEED_DESC = "(JII)V";
    private static final String FEATURE_PLACE_DESC = "(Lnet/minecraft/world/level/levelgen/feature/configurations/FeatureConfiguration;Lnet/minecraft/world/level/WorldGenLevel;Lnet/minecraft/world/level/chunk/ChunkGenerator;Lnet/minecraft/util/RandomSource;Lnet/minecraft/core/BlockPos;)Z";
    private static final String ORE_PLACE_DESC = "(Lnet/minecraft/world/level/levelgen/feature/FeaturePlaceContext;)Z";
    private static final Map<Object, SectionInfo> SECTIONS =
            Collections.synchronizedMap(new IdentityHashMap<>());
    private static final Map<Object, FeatureSeedInfo> FEATURE_SEEDS =
            Collections.synchronizedMap(new IdentityHashMap<>());
    private static final ThreadLocal<FeatureSeedInfo> CURRENT_FEATURE_SEED = new ThreadLocal<>();

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

    public static void traceDecorationSeed(long decorationSeed, Object random, long worldSeed, int x, int z) {
        FEATURE_SEEDS.put(random, new FeatureSeedInfo(worldSeed, x, z, decorationSeed, -1, -1));
    }

    public static void traceFeatureSeed(Object random, long decorationSeed, int featureIndex, int stepIndex) {
        FeatureSeedInfo previous = FEATURE_SEEDS.get(random);
        FeatureSeedInfo info = previous == null
                ? new FeatureSeedInfo(0L, 0, 0, decorationSeed, featureIndex, stepIndex)
                : previous.withFeature(decorationSeed, featureIndex, stepIndex);
        FEATURE_SEEDS.put(random, info);
        innerRandom(random).ifPresent(inner -> FEATURE_SEEDS.put(inner, info));
        CURRENT_FEATURE_SEED.set(info);
        if (isRedstoneLowerFeature(featureIndex, stepIndex)) {
            System.out.printf(
                    "java feature seed trace: name=ore_redstone_lower source_origin=(%d,%d) source_chunk=(%d,%d) decoration_seed=%d step=%d index=%d%n",
                    info.originX,
                    info.originZ,
                    Math.floorDiv(info.originX, 16),
                    Math.floorDiv(info.originZ, 16),
                    decorationSeed,
                    stepIndex,
                    featureIndex);
        }
    }

    public static void traceOreFeaturePlace(Object context) {
        try {
            Object random = context.getClass().getMethod("random").invoke(context);
            Object origin = context.getClass().getMethod("origin").invoke(context);
            Object config = context.getClass().getMethod("config").invoke(context);
            String configText = String.valueOf(config);
            if (!configText.contains("redstone_ore")) {
                return;
            }
            Class<?> vec3i = Class.forName("net.minecraft.core.Vec3i", false, origin.getClass().getClassLoader());
            int x = (Integer) vec3i.getMethod("getX").invoke(origin);
            int y = (Integer) vec3i.getMethod("getY").invoke(origin);
            int z = (Integer) vec3i.getMethod("getZ").invoke(origin);
            FeatureSeedInfo info = FEATURE_SEEDS.get(random);
            System.out.printf(
                    "java ore feature place trace: origin=(%d,%d,%d) source_origin=%s source_chunk=%s decoration_seed=%s step=%s index=%s config=%s%n",
                    x,
                    y,
                    z,
                    info == null ? "unknown" : "(" + info.originX + "," + info.originZ + ")",
                    info == null
                            ? "unknown"
                            : "(" + Math.floorDiv(info.originX, 16) + "," + Math.floorDiv(info.originZ, 16) + ")",
                    info == null ? "unknown" : Long.toString(info.decorationSeed),
                    info == null ? "unknown" : Integer.toString(info.stepIndex),
                    info == null ? "unknown" : Integer.toString(info.featureIndex),
                    compactConfig(configText));
        } catch (ReflectiveOperationException error) {
            System.out.printf("java ore feature place trace failed: %s%n", error);
        }
    }

    public static void traceFeaturePlace(Object config, Object random, Object origin) {
        try {
            FeatureSeedInfo info = FEATURE_SEEDS.get(random);
            if (info == null) {
                info = CURRENT_FEATURE_SEED.get();
            }
            if (info == null || !isRedstoneLowerFeature(info.featureIndex, info.stepIndex)) {
                return;
            }
            String configClass = config.getClass().getName();
            if (!"net.minecraft.world.level.levelgen.feature.configurations.OreConfiguration".equals(configClass)) {
                return;
            }
            Class<?> vec3i = Class.forName("net.minecraft.core.Vec3i", false, origin.getClass().getClassLoader());
            int x = (Integer) vec3i.getMethod("getX").invoke(origin);
            int y = (Integer) vec3i.getMethod("getY").invoke(origin);
            int z = (Integer) vec3i.getMethod("getZ").invoke(origin);
            System.out.printf(
                    "java ore_redstone_lower real place: source_origin=(%d,%d) source_chunk=(%d,%d) decoration_seed=%d step=%d index=%d origin=(%d,%d,%d)%n",
                    info.originX,
                    info.originZ,
                    Math.floorDiv(info.originX, 16),
                    Math.floorDiv(info.originZ, 16),
                    info.decorationSeed,
                    info.stepIndex,
                    info.featureIndex,
                    x,
                    y,
                    z);
        } catch (ReflectiveOperationException error) {
            System.out.printf("java feature place trace failed: %s%n", error);
        }
    }

    private static boolean isRedstoneLowerFeature(int featureIndex, int stepIndex) {
        return stepIndex == 6 && featureIndex == 17;
    }

    private static String compactConfig(String configText) {
        return configText.length() <= 240 ? configText : configText.substring(0, 240) + "...";
    }

    private static java.util.Optional<Object> innerRandom(Object random) {
        try {
            java.lang.reflect.Field field = random.getClass().getDeclaredField("randomSource");
            field.setAccessible(true);
            return java.util.Optional.ofNullable(field.get(random));
        } catch (ReflectiveOperationException | RuntimeException ignored) {
            return java.util.Optional.empty();
        }
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
                    && !"net/minecraft/world/level/chunk/LevelChunkSection".equals(className)
                    && !"net/minecraft/world/level/levelgen/WorldgenRandom".equals(className)
                    && !"net/minecraft/world/level/levelgen/feature/Feature".equals(className)
                    && !"net/minecraft/world/level/levelgen/feature/OreFeature".equals(className)) {
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
            if ("net/minecraft/world/level/levelgen/WorldgenRandom".equals(className)
                    && "setDecorationSeed".equals(name)
                    && SET_DECORATION_SEED_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitInsn(int opcode) {
                        if (opcode == Opcodes.LRETURN) {
                            visitInsn(Opcodes.DUP2);
                            visitVarInsn(Opcodes.ALOAD, 0);
                            visitVarInsn(Opcodes.LLOAD, 1);
                            visitVarInsn(Opcodes.ILOAD, 3);
                            visitVarInsn(Opcodes.ILOAD, 4);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "traceDecorationSeed",
                                    "(JLjava/lang/Object;JII)V",
                                    false);
                        }
                        super.visitInsn(opcode);
                    }
                };
            }
            if ("net/minecraft/world/level/levelgen/WorldgenRandom".equals(className)
                    && "setFeatureSeed".equals(name)
                    && SET_FEATURE_SEED_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitCode() {
                        super.visitCode();
                        visitVarInsn(Opcodes.ALOAD, 0);
                        visitVarInsn(Opcodes.LLOAD, 1);
                        visitVarInsn(Opcodes.ILOAD, 3);
                        visitVarInsn(Opcodes.ILOAD, 4);
                        visitMethodInsn(
                                Opcodes.INVOKESTATIC,
                                "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                "traceFeatureSeed",
                                "(Ljava/lang/Object;JII)V",
                                false);
                    }
                };
            }
            if ("net/minecraft/world/level/levelgen/feature/OreFeature".equals(className)
                    && "place".equals(name)
                    && ORE_PLACE_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitCode() {
                        super.visitCode();
                        visitVarInsn(Opcodes.ALOAD, 1);
                        visitMethodInsn(
                                Opcodes.INVOKESTATIC,
                                "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                "traceOreFeaturePlace",
                                "(Ljava/lang/Object;)V",
                                false);
                    }
                };
            }
            if ("net/minecraft/world/level/levelgen/feature/Feature".equals(className)
                    && "place".equals(name)
                    && FEATURE_PLACE_DESC.equals(descriptor)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitCode() {
                        super.visitCode();
                        visitVarInsn(Opcodes.ALOAD, 1);
                        visitVarInsn(Opcodes.ALOAD, 4);
                        visitVarInsn(Opcodes.ALOAD, 5);
                        visitMethodInsn(
                                Opcodes.INVOKESTATIC,
                                "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                "traceFeaturePlace",
                                "(Ljava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
                                false);
                    }
                };
            }
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

    private record FeatureSeedInfo(long worldSeed, int originX, int originZ, long decorationSeed, int featureIndex, int stepIndex) {
        private FeatureSeedInfo withFeature(long nextDecorationSeed, int nextFeatureIndex, int nextStepIndex) {
            return new FeatureSeedInfo(worldSeed, originX, originZ, nextDecorationSeed, nextFeatureIndex, nextStepIndex);
        }
    }
}
