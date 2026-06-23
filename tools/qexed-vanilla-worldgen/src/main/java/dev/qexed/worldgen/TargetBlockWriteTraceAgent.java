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
    private static final Map<Object, Integer> FEATURE_ATTEMPTS =
            Collections.synchronizedMap(new IdentityHashMap<>());
    private static final ThreadLocal<FeatureSeedInfo> CURRENT_FEATURE_SEED = new ThreadLocal<>();
    private static final ThreadLocal<FeaturePlaceInfo> CURRENT_FEATURE_PLACE = new ThreadLocal<>();
    private static final ThreadLocal<OreDoPlaceCount> CURRENT_ORE_COUNT = new ThreadLocal<>();
    private static int traceTargetX = 6;
    private static int traceTargetY = -61;
    private static int traceTargetZ = 0;

    private TargetBlockWriteTraceAgent() {
    }

    public static void premain(String args, Instrumentation instrumentation) {
        parseTarget(args);
        instrumentation.addTransformer(new Transformer(args));
    }

    private static void parseTarget(String args) {
        if (args == null) {
            return;
        }
        int start = args.indexOf("target(");
        if (start < 0) {
            return;
        }
        int end = args.indexOf(')', start);
        if (end < 0) {
            return;
        }
        String[] parts = args.substring(start + "target(".length(), end).split(",");
        if (parts.length != 3) {
            return;
        }
        try {
            traceTargetX = Integer.parseInt(parts[0].trim());
            traceTargetY = Integer.parseInt(parts[1].trim());
            traceTargetZ = Integer.parseInt(parts[2].trim());
        } catch (NumberFormatException ignored) {
        }
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
                && !replacementText.contains("gravel")
                && !replacementText.contains("iron")
                && !replacementText.contains("tuff")
                && !replacementText.contains("air")) {
            return;
        }

        FeaturePlaceInfo place = CURRENT_FEATURE_PLACE.get();
        System.out.printf(
                "java target section write trace: owner=%s coord=(%d,%d,%d) local=(%d,%d,%d) replacement=%s feature=%s%n",
                owner,
                worldX,
                worldY,
                worldZ,
                localX,
                localY,
                localZ,
                replacement,
                place == null ? "unknown" : place.describe());
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
        FEATURE_ATTEMPTS.put(random, 0);
        innerRandom(random).ifPresent(inner -> FEATURE_ATTEMPTS.put(inner, 0));
        CURRENT_FEATURE_SEED.set(info);
        if (isTracedOreFeature(featureIndex, stepIndex)) {
            System.out.printf(
                    "java feature seed trace: name=%s source_origin=(%d,%d) source_chunk=(%d,%d) decoration_seed=%d step=%d index=%d%n",
                    oreFeatureName(featureIndex, stepIndex),
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
            if (!configText.contains("redstone_ore") && !configText.contains("diamond_ore")) {
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

    public static void traceTargetPredicateResult(boolean matches) {
        OreDoPlaceCount count = CURRENT_ORE_COUNT.get();
        if (count != null && matches) {
            count.targetPredicateMatches++;
        }
    }

    public static void traceCanPlaceOreTargetPredicate(boolean matches, Object blockState, Object targetState, Object orePos) {
        OreDoPlaceCount count = CURRENT_ORE_COUNT.get();
        if (count == null) {
            return;
        }
        int x;
        int y;
        int z;
        try {
            Class<?> vec3i = Class.forName("net.minecraft.core.Vec3i", false, orePos.getClass().getClassLoader());
            x = (Integer) vec3i.getMethod("getX").invoke(orePos);
            y = (Integer) vec3i.getMethod("getY").invoke(orePos);
            z = (Integer) vec3i.getMethod("getZ").invoke(orePos);
        } catch (ReflectiveOperationException error) {
            System.out.printf("java ore_diamond_medium canPlaceOre coord trace failed: %s%n", error);
            return;
        }
        if (matches) {
            count.targetPredicateMatches++;
        }
        if (matches || isRequestedRustCoord(x, y, z)) {
            System.out.printf(
                    "java ore_diamond_medium canPlaceOre predicate: source_origin=(%d,%d) source_chunk=(%d,%d) step=%d index=%d attempt=%d origin=(%d,%d,%d) coord=(%d,%d,%d) current=%s targetState=%s predicate=%s%n",
                    count.info.originX,
                    count.info.originZ,
                    Math.floorDiv(count.info.originX, 16),
                    Math.floorDiv(count.info.originZ, 16),
                    count.info.stepIndex,
                    count.info.featureIndex,
                    count.attempt,
                    count.originX,
                    count.originY,
                    count.originZ,
                    x,
                    y,
                    z,
                    blockState,
                    targetState,
                    matches);
        }
    }

    public static void traceShouldSkipAirCheckNextFloat(float value) {
        OreDoPlaceCount count = CURRENT_ORE_COUNT.get();
        if (count != null) {
            count.shouldSkipAirCheckNextFloatCalls++;
        }
    }

    public static void traceOreSetBlockState(Object ignoredPreviousState) {
        OreDoPlaceCount count = CURRENT_ORE_COUNT.get();
        if (count != null) {
            count.setBlockStateWrites++;
        }
    }

    public static void finishOreDoPlace(boolean placedAny) {
        OreDoPlaceCount count = CURRENT_ORE_COUNT.get();
        if (count == null) {
            return;
        }
        System.out.printf(
                "java %s count summary: source_origin=(%d,%d) source_chunk=(%d,%d) step=%d index=%d attempt=%d origin=(%d,%d,%d) entered_doPlace=true target_predicate_matches=%d shouldSkipAirCheck_nextFloat_calls=%d setBlockState_writes=%d doPlace_result=%s%n",
                featureLabel(count.info),
                count.info.originX,
                count.info.originZ,
                Math.floorDiv(count.info.originX, 16),
                Math.floorDiv(count.info.originZ, 16),
                count.info.stepIndex,
                count.info.featureIndex,
                count.attempt,
                count.originX,
                count.originY,
                count.originZ,
                count.targetPredicateMatches,
                count.shouldSkipAirCheckNextFloatCalls,
                count.setBlockStateWrites,
                placedAny);
        CURRENT_ORE_COUNT.remove();
    }

    public static void traceFeaturePlace(Object config, Object random, Object origin) {
        try {
            FeatureSeedInfo info = FEATURE_SEEDS.get(random);
            if (info == null) {
                info = CURRENT_FEATURE_SEED.get();
            }
            if (info == null) {
                return;
            }
            String configClass = config.getClass().getName();
            Class<?> vec3i = Class.forName("net.minecraft.core.Vec3i", false, origin.getClass().getClassLoader());
            int x = (Integer) vec3i.getMethod("getX").invoke(origin);
            int y = (Integer) vec3i.getMethod("getY").invoke(origin);
            int z = (Integer) vec3i.getMethod("getZ").invoke(origin);
            int attempt = nextFeatureAttempt(random);
            CURRENT_FEATURE_PLACE.set(new FeaturePlaceInfo(info, attempt, x, y, z, configClass, compactConfig(String.valueOf(config))));
            if (!isTracedOreFeature(info.featureIndex, info.stepIndex)) {
                return;
            }
            if (!"net.minecraft.world.level.levelgen.feature.configurations.OreConfiguration".equals(configClass)) {
                return;
            }
            if (isTrackedCountAttempt(info, attempt)) {
                OreDoPlaceCount count = new OreDoPlaceCount(info, attempt, x, y, z);
                CURRENT_ORE_COUNT.set(count);
                System.out.printf(
                        "java %s count start: source_origin=(%d,%d) source_chunk=(%d,%d) step=%d index=%d attempt=%d origin=(%d,%d,%d) precheck=Feature.place_entered%n",
                        featureLabel(info),
                        info.originX,
                        info.originZ,
                        Math.floorDiv(info.originX, 16),
                        Math.floorDiv(info.originZ, 16),
                        info.stepIndex,
                        info.featureIndex,
                        attempt,
                        x,
                        y,
                        z);
            }
            System.out.printf(
                    "java %s real place: source_origin=(%d,%d) source_chunk=(%d,%d) decoration_seed=%d step=%d index=%d attempt=%d origin=(%d,%d,%d)%n",
                    oreFeatureName(info.featureIndex, info.stepIndex),
                    info.originX,
                    info.originZ,
                    Math.floorDiv(info.originX, 16),
                    Math.floorDiv(info.originZ, 16),
                    info.decorationSeed,
                    info.stepIndex,
                    info.featureIndex,
                    attempt,
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

    private static boolean isTracedOreFeature(int featureIndex, int stepIndex) {
        return isRedstoneLowerFeature(featureIndex, stepIndex)
                || isGlowLichenFeature(featureIndex, stepIndex)
                || (stepIndex == 6 && featureIndex == 1)
                || (stepIndex == 6 && featureIndex == 8)
                || (stepIndex == 6 && featureIndex >= 11 && featureIndex <= 13)
                || (stepIndex == 6 && featureIndex >= 18 && featureIndex <= 21);
    }

    private static String oreFeatureName(int featureIndex, int stepIndex) {
        if (stepIndex == 6 && featureIndex == 18) {
            return "ore_diamond";
        }
        if (stepIndex == 6 && featureIndex == 1) {
            return "ore_gravel";
        }
        if (stepIndex == 6 && featureIndex == 8) {
            return "ore_tuff";
        }
        if (stepIndex == 6 && featureIndex == 11) {
            return "ore_iron_upper";
        }
        if (stepIndex == 6 && featureIndex == 12) {
            return "ore_iron_middle";
        }
        if (stepIndex == 6 && featureIndex == 13) {
            return "ore_iron_small";
        }
        if (stepIndex == 6 && featureIndex == 19) {
            return "ore_diamond_medium";
        }
        if (stepIndex == 6 && featureIndex == 20) {
            return "ore_diamond_large";
        }
        if (stepIndex == 6 && featureIndex == 21) {
            return "ore_diamond_buried";
        }
        if (isRedstoneLowerFeature(featureIndex, stepIndex)) {
            return "ore_redstone_lower";
        }
        if (isGlowLichenFeature(featureIndex, stepIndex)) {
            return "glow_lichen";
        }
        return "ore_unknown";
    }

    private static boolean isGlowLichenFeature(int featureIndex, int stepIndex) {
        return stepIndex == 9 && featureIndex == 0;
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

    private static int nextFeatureAttempt(Object random) {
        if (random == null) {
            return -1;
        }
        int attempt = FEATURE_ATTEMPTS.getOrDefault(random, 0);
        FEATURE_ATTEMPTS.put(random, attempt + 1);
        innerRandom(random).ifPresent(inner -> FEATURE_ATTEMPTS.put(inner, attempt + 1));
        return attempt;
    }

    private static boolean isTargetDiamondMediumAttempt(FeatureSeedInfo info, int attempt) {
        return info != null
                && info.stepIndex == 6
                && info.featureIndex >= 18
                && info.featureIndex <= 21
                && attempt >= 0;
    }

    private static boolean isTargetOreGravelAttempt(FeatureSeedInfo info, int attempt) {
        return info != null
                && info.stepIndex == 6
                && info.featureIndex == 1
                && attempt >= 0
                && attempt <= 3
                && Math.floorDiv(info.originX, 16) == 0
                && Math.floorDiv(info.originZ, 16) == 0;
    }

    private static boolean isTargetOreIronSmallAttempt(FeatureSeedInfo info, int attempt) {
        return info != null
                && info.stepIndex == 6
                && info.featureIndex == 13
                && attempt >= 0
                && attempt <= 9
                && Math.floorDiv(info.originX, 16) == 0
                && Math.floorDiv(info.originZ, 16) == 0;
    }

    private static boolean isTrackedCountAttempt(FeatureSeedInfo info, int attempt) {
        return isTargetDiamondMediumAttempt(info, attempt)
                || isTargetOreGravelAttempt(info, attempt)
                || isTargetOreIronSmallAttempt(info, attempt);
    }

    private static String featureLabel(FeatureSeedInfo info) {
        if (info.stepIndex == 6 && info.featureIndex == 1) {
            return "ore_gravel";
        }
        return oreFeatureName(info.featureIndex, info.stepIndex);
    }

    private static boolean isRequestedRustCoord(int x, int y, int z) {
        return x == targetX() && y == targetY() && z == targetZ();
    }

    private static int targetX() {
        return Integer.getInteger("qexed.traceBlock.x", traceTargetX);
    }

    private static int targetY() {
        return Integer.getInteger("qexed.traceBlock.y", traceTargetY);
    }

    private static int targetZ() {
        return Integer.getInteger("qexed.traceBlock.z", traceTargetZ);
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
            if ("net/minecraft/world/level/levelgen/feature/OreFeature".equals(className)
                    && "doPlace".equals(name)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitInsn(int opcode) {
                        if (opcode == Opcodes.IRETURN) {
                            visitInsn(Opcodes.DUP);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "finishOreDoPlace",
                                    "(Z)V",
                                    false);
                        }
                        super.visitInsn(opcode);
                    }

                    @Override
                    public void visitMethodInsn(int opcode, String owner, String methodName, String methodDescriptor, boolean isInterface) {
                        super.visitMethodInsn(opcode, owner, methodName, methodDescriptor, isInterface);
                        if ("net/minecraft/world/level/levelgen/structure/templatesystem/RuleTest".equals(owner)
                                && "test".equals(methodName)
                                && "(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/util/RandomSource;)Z".equals(methodDescriptor)) {
                            visitInsn(Opcodes.DUP);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "traceTargetPredicateResult",
                                    "(Z)V",
                                    false);
                        }
                        if ("net/minecraft/world/level/chunk/LevelChunkSection".equals(owner)
                                && METHOD_NAME.equals(methodName)
                                && SECTION_METHOD_DESC.equals(methodDescriptor)) {
                            visitInsn(Opcodes.DUP);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "traceOreSetBlockState",
                                    "(Ljava/lang/Object;)V",
                                    false);
                        }
                    }
                };
            }
            if ("net/minecraft/world/level/levelgen/feature/OreFeature".equals(className)
                    && "shouldSkipAirCheck".equals(name)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitMethodInsn(int opcode, String owner, String methodName, String methodDescriptor, boolean isInterface) {
                        super.visitMethodInsn(opcode, owner, methodName, methodDescriptor, isInterface);
                        if ("net/minecraft/util/RandomSource".equals(owner)
                                && "nextFloat".equals(methodName)
                                && "()F".equals(methodDescriptor)) {
                            visitInsn(Opcodes.DUP);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "traceShouldSkipAirCheckNextFloat",
                                    "(F)V",
                                    false);
                        }
                    }
                };
            }
            if ("net/minecraft/world/level/levelgen/feature/OreFeature".equals(className)
                    && "canPlaceOre".equals(name)) {
                return new MethodVisitor(Opcodes.ASM9, method) {
                    @Override
                    public void visitMethodInsn(int opcode, String owner, String methodName, String methodDescriptor, boolean isInterface) {
                        super.visitMethodInsn(opcode, owner, methodName, methodDescriptor, isInterface);
                        if ("net/minecraft/world/level/levelgen/structure/templatesystem/RuleTest".equals(owner)
                                && "test".equals(methodName)
                                && "(Lnet/minecraft/world/level/block/state/BlockState;Lnet/minecraft/util/RandomSource;)Z".equals(methodDescriptor)) {
                            visitInsn(Opcodes.DUP);
                            visitVarInsn(Opcodes.ALOAD, 0);
                            visitVarInsn(Opcodes.ALOAD, 4);
                            visitVarInsn(Opcodes.ALOAD, 5);
                            visitMethodInsn(
                                    Opcodes.INVOKESTATIC,
                                    "dev/qexed/worldgen/TargetBlockWriteTraceAgent",
                                    "traceCanPlaceOreTargetPredicate",
                                    "(ZLjava/lang/Object;Ljava/lang/Object;Ljava/lang/Object;)V",
                                    false);
                        }
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

    private record FeaturePlaceInfo(
            FeatureSeedInfo info,
            int attempt,
            int originX,
            int originY,
            int originZ,
            String configClass,
            String configText) {
        private String describe() {
            return String.format(
                    "source_origin=(%d,%d) source_chunk=(%d,%d) decoration_seed=%d step=%d index=%d attempt=%d origin=(%d,%d,%d) configClass=%s config=%s",
                    info.originX,
                    info.originZ,
                    Math.floorDiv(info.originX, 16),
                    Math.floorDiv(info.originZ, 16),
                    info.decorationSeed,
                    info.stepIndex,
                    info.featureIndex,
                    attempt,
                    originX,
                    originY,
                    originZ,
                    configClass,
                    configText);
        }
    }

    private static final class OreDoPlaceCount {
        private final FeatureSeedInfo info;
        private final int attempt;
        private final int originX;
        private final int originY;
        private final int originZ;
        private int targetPredicateMatches;
        private int shouldSkipAirCheckNextFloatCalls;
        private int setBlockStateWrites;

        private OreDoPlaceCount(FeatureSeedInfo info, int attempt, int originX, int originY, int originZ) {
            this.info = info;
            this.attempt = attempt;
            this.originX = originX;
            this.originY = originY;
            this.originZ = originZ;
        }
    }
}
