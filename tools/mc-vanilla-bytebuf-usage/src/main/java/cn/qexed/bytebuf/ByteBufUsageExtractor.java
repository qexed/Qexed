package cn.qexed.bytebuf;

import com.fasterxml.jackson.databind.ObjectMapper;
import com.fasterxml.jackson.databind.SerializationFeature;
import com.fasterxml.jackson.databind.node.ArrayNode;
import com.fasterxml.jackson.databind.node.ObjectNode;
import org.objectweb.asm.ClassReader;
import org.objectweb.asm.Handle;
import org.objectweb.asm.Opcodes;
import org.objectweb.asm.tree.AbstractInsnNode;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.InvokeDynamicInsnNode;
import org.objectweb.asm.tree.LineNumberNode;
import org.objectweb.asm.tree.MethodInsnNode;
import org.objectweb.asm.tree.MethodNode;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Files;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.TreeMap;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;

/**
 * Extracts every method definition and every call site / method reference of a
 * target type (default {@code net.minecraft.network.FriendlyByteBuf}) and its
 * subtypes, from the jars Gradle resolved for the {@code minecraft}
 * configuration.
 *
 * <p>The jar list is never discovered by this tool itself; it is passed in as
 * {@code --jar <path>} arguments produced by the Gradle task, so switching
 * Minecraft versions only changes the Gradle property. No name-prefix filters:
 * every invocation is resolved through the class hierarchy index, so overrides
 * and inherited calls (e.g. RegistryFriendlyByteBuf -&gt; FriendlyByteBuf) are
 * attributed exactly.
 */
public final class ByteBufUsageExtractor {

    public static void main(String[] args) throws Exception {
        Args parsed = Args.parse(args);
        run(parsed);
    }

    static void run(Args args) throws IOException {
        long start = System.nanoTime();

        // ---- Pass 1: class hierarchy + declaration index over all provided jars ----
        ClassIndex index = new ClassIndex();
        for (File jar : args.jars) {
            index.indexJar(jar);
        }

        String targetInternal = args.targetType.replace('.', '/');
        if (!index.inUniverse(targetInternal)) {
            throw new IllegalStateException(
                "Target type " + args.targetType + " was not found in the provided jars. "
                + "Was the Minecraft dependency resolved by Gradle for this run?");
        }

        // Receiver universe: the target type itself plus every subtype found in the jars.
        Set<String> receiverUniverse = new LinkedHashSet<>();
        receiverUniverse.add(targetInternal);
        receiverUniverse.addAll(index.subtypesOf(targetInternal));

        // ---- Pass 2: scan all jars for call sites whose resolved declaration is in the universe ----
        CallSiteCollector collector = new CallSiteCollector(index, receiverUniverse);
        for (File jar : args.jars) {
            scanJar(jar, collector);
        }

        // ---- Assemble the report: every declared method of the universe (with calls) ----
        ObjectMapper mapper = new ObjectMapper();
        mapper.enable(SerializationFeature.INDENT_OUTPUT);
        ObjectNode root = mapper.createObjectNode();
        root.put("tool", "bytebuf-usage-extractor");
        root.put("mcVersion", args.mcVersion);
        root.put("targetType", args.targetType);
        root.put("source", "gradle-resolved minecraft configuration");
        root.put("generatedAt", java.time.Instant.now().toString());
        root.put("jarsScanned", args.jars.size());
        root.put("classesIndexed", index.parsedClasses);
        root.put("unparsedClasses", index.unparsed.size());

        ArrayNode universeArr = root.putArray("receiverUniverse");
        receiverUniverse.forEach(universeArr::add);

        // Definition universe: all methods declared on any universe type.
        Map<String, Map<String, Integer>> declaredByOwner = new TreeMap<>();
        for (String owner : receiverUniverse) {
            declaredByOwner.put(owner, index.declarations.getOrDefault(owner, Map.of()));
        }

        // Merge with invoked keys that resolved into the universe.
        Set<MethodKey> allKeys = new LinkedHashSet<>();
        for (String owner : declaredByOwner.keySet()) {
            for (String key : declaredByOwner.get(owner).keySet()) {
                int paren = key.indexOf('(');
                allKeys.add(new MethodKey(owner, key.substring(0, paren), key.substring(paren)));
            }
        }
        allKeys.addAll(collector.byKey.keySet());

        ObjectNode methods = root.putObject("methods");
        List<MethodKey> keys = new ArrayList<>(allKeys);
        keys.sort(Comparator.comparing((MethodKey k) -> k.owner).thenComparing(k -> k.name).thenComparing(k -> k.desc));
        int totalCalls = 0;
        int methodsWithCalls = 0;
        StringBuilder csv = new StringBuilder("owner,name,descriptor,callerClass,callerMethod,line,kind,invokedOwner,invokedName,invokedDesc\n");

        for (MethodKey key : keys) {
            ObjectNode m = methods.putObject(dotted(key.owner) + "#" + key.name + key.desc);
            m.put("owner", dotted(key.owner));
            m.put("name", key.name);
            m.put("descriptor", key.desc);
            m.put("signature", prettyDescriptor(key.desc));
            Integer access = index.declarations.getOrDefault(key.owner, Map.of()).get(key.name + key.desc);
            if (access != null) {
                m.put("access", access);
                m.put("modifiers", modifiersOf(access));
                m.put("declaredHere", true);
            } else {
                m.put("declaredHere", false);
            }
            Set<CallSite> siteSet = collector.byKey.getOrDefault(key, Set.of());
            List<CallSite> siteList = new ArrayList<>(siteSet);
            siteList.sort(Comparator.<CallSite, String>comparing(s -> s.callerClass)
                .thenComparing(s -> s.callerMethod())
                .thenComparing(s -> s.line));
            ArrayNode sites = m.putArray("callSites");
            for (CallSite site : siteList) {
                ObjectNode s = sites.addObject();
                s.put("callerClass", dotted(site.callerClass));
                s.put("callerMethod", site.callerMethodName + prettyDescriptor(site.callerMethodDesc));
                s.put("callerMethodDescriptor", site.callerMethodDesc);
                s.put("line", site.line);
                s.put("kind", site.kind);
                s.put("invokedOwner", dotted(site.invokedOwner));
                s.put("invokedName", site.invokedName);
                s.put("invokedDesc", site.invokedDesc);
                csv.append(dotted(key.owner)).append(',').append(key.name).append(',').append(key.desc).append(',')
                  .append(dotted(site.callerClass)).append(',')
                  .append(quoteCsv(site.callerMethodName + prettyDescriptor(site.callerMethodDesc))).append(',')
                  .append(site.line).append(',').append(site.kind).append(',')
                  .append(dotted(site.invokedOwner)).append(',').append(site.invokedName).append(',')
                  .append(quoteCsv(site.invokedDesc)).append('\n');
            }
            m.put("callSiteCount", siteList.size());
            totalCalls += siteList.size();
            if (!siteList.isEmpty()) {
                methodsWithCalls++;
            }
        }

        root.put("totalDistinctMethods", keys.size());
        root.put("methodsWithCallSites", methodsWithCalls);
        root.put("totalCallSites", totalCalls);

        File outDir = args.outDir;
        if (!outDir.exists() && !outDir.mkdirs()) {
            throw new IOException("Cannot create output directory " + outDir);
        }
        mapper.writerWithDefaultPrettyPrinter().writeValue(new File(outDir, "bytebuf-usage.json"), root);
        Files.writeString(new File(outDir, "bytebuf-usage.csv").toPath(), csv.toString());

        // ---- Markdown summary ----
        StringBuilder md = new StringBuilder();
        md.append("# FriendlyByteBuf usage report\n\n");
        md.append("- Minecraft version: ").append(args.mcVersion).append("\n");
        md.append("- Target: ").append(args.targetType).append(" (+ subtypes)\n");
        md.append("- Jars scanned: ").append(args.jars.size()).append("\n");
        md.append("- Classes indexed: ").append(index.parsedClasses);
        if (!index.unparsed.isEmpty()) {
            md.append(" (").append(index.unparsed.size()).append(" unparsed)");
        }
        md.append("\n");
        md.append("- Receiver universe: ").append(receiverUniverse.size()).append(" types\n");
        md.append("- Distinct methods: ").append(keys.size()).append("\n");
        md.append("- Total call sites: ").append(totalCalls).append("\n\n");
        md.append("| Owner | Method | Call sites |\n|---|---|---:|\n");
        for (MethodKey key : keys) {
            int n = collector.byKey.getOrDefault(key, Set.of()).size();
            md.append("| ").append(dotted(key.owner)).append(" | ").append(key.name).append(" | ").append(n).append(" |\n");
        }
        Files.writeString(new File(outDir, "bytebuf-usage.md").toPath(), md.toString());

        System.out.println("Receiver universe: " + receiverUniverse);
        System.out.println("Distinct methods: " + keys.size());
        System.out.println("Total call sites: " + totalCalls);
        System.out.println("Unparsed classes: " + index.unparsed.size());
        System.out.println("Elapsed: " + (System.nanoTime() - start) / 1_000_000 + " ms");
        System.out.println("Written: " + new File(outDir, "bytebuf-usage.json").getAbsolutePath());
        System.out.println("Written: " + new File(outDir, "bytebuf-usage.csv").getAbsolutePath());
        System.out.println("Written: " + new File(outDir, "bytebuf-usage.md").getAbsolutePath());
    }

    private static void scanJar(File jarFile, CallSiteCollector collector) throws IOException {
        try (JarFile jar = new JarFile(jarFile)) {
            java.util.Enumeration<JarEntry> entries = jar.entries();
            while (entries.hasMoreElements()) {
                JarEntry entry = entries.nextElement();
                if (entry.isDirectory() || !entry.getName().endsWith(".class")) {
                    continue;
                }
                try (InputStream in = jar.getInputStream(entry)) {
                    ClassReader reader = new ClassReader(in);
                    ClassNode node = new ClassNode();
                    reader.accept(node, 0);
                    for (MethodNode mn : node.methods) {
                        collector.visitMethod(node.name, mn);
                    }
                } catch (RuntimeException ignored) {
                    // class file version or structure this ASM build cannot read; counted in pass 1
                }
            }
        }
    }

    static final class MethodKey {
        final String owner; final String name; final String desc;
        MethodKey(String owner, String name, String desc) {
            this.owner = owner; this.name = name; this.desc = desc;
        }
        @Override public boolean equals(Object o) {
            return o instanceof MethodKey k && owner.equals(k.owner) && name.equals(k.name) && desc.equals(k.desc);
        }
        @Override public int hashCode() {
            return owner.hashCode() * 31 + name.hashCode() * 7 + desc.hashCode();
        }
    }

    static final class CallSite {
        String callerClass; String callerMethodName; String callerMethodDesc; int line = -1; String kind;
        String invokedOwner; String invokedName; String invokedDesc;
        String callerMethod() { return callerMethodName + callerMethodDesc; }
        @Override public boolean equals(Object o) {
            if (!(o instanceof CallSite c)) return false;
            return line == c.line && kind.equals(c.kind) && callerClass.equals(c.callerClass)
                && callerMethod().equals(c.callerMethod()) && invokedName.equals(c.invokedName)
                && invokedDesc.equals(c.invokedDesc) && invokedOwner.equals(c.invokedOwner);
        }
        @Override public int hashCode() {
            return callerClass.hashCode() * 31 + callerMethod().hashCode() * 7 + invokedName.hashCode() + line;
        }
    }

    static final class CallSiteCollector {
        final ClassIndex index;
        final Set<String> receiverUniverse;
        final Map<MethodKey, Set<CallSite>> byKey = new LinkedHashMap<>();
        CallSiteCollector(ClassIndex index, Set<String> receiverUniverse) {
            this.index = index;
            this.receiverUniverse = receiverUniverse;
        }

        void visitMethod(String owner, MethodNode mn) {
            if (mn.instructions == null) {
                return;
            }
            int currentLine = -1;
            for (AbstractInsnNode insn : mn.instructions) {
                if (insn instanceof LineNumberNode l) {
                    currentLine = l.line;
                    continue;
                }
                if (insn instanceof MethodInsnNode min) {
                    String resolved = index.resolveDeclaration(min.owner, min.name + min.desc);
                    if (resolved != null && receiverUniverse.contains(resolved)) {
                        CallSite site = new CallSite();
                        site.callerClass = owner;
                        site.callerMethodName = mn.name;
                        site.callerMethodDesc = mn.desc;
                        site.line = currentLine;
                        site.kind = opcodeName(min.getOpcode());
                        site.invokedOwner = min.owner;
                        site.invokedName = min.name;
                        site.invokedDesc = min.desc;
                        add(resolved, site);
                    }
                } else if (insn instanceof InvokeDynamicInsnNode ind) {
                    Handle bsm = ind.bsm;
                    if (bsm != null) {
                        recordHandle(owner, mn, currentLine, bsm, "INVOKEDYNAMIC-bootstrap", ind);
                    }
                    if (ind.bsmArgs != null) {
                        for (Object carg : ind.bsmArgs) {
                            if (carg instanceof Handle h) {
                                recordHandle(owner, mn, currentLine, h, "INVOKEDYNAMIC-handle-arg", ind);
                            }
                        }
                    }
                }
            }
        }

        private void recordHandle(String owner, MethodNode mn, int line, Handle h, String kind, InvokeDynamicInsnNode ind) {
            String resolved = index.resolveDeclaration(h.getOwner(), h.getName() + h.getDesc());
            if (resolved == null || !receiverUniverse.contains(resolved)) {
                return;
            }
            CallSite site = new CallSite();
            site.callerClass = owner;
            site.callerMethodName = mn.name;
            site.callerMethodDesc = mn.desc;
            site.line = line;
            site.kind = kind;
            site.invokedOwner = h.getOwner();
            site.invokedName = h.getName();
            site.invokedDesc = h.getDesc();
            add(resolved, site);
        }

        private void add(String resolvedOwner, CallSite site) {
            MethodKey key = new MethodKey(resolvedOwner, site.invokedName, site.invokedDesc);
            byKey.computeIfAbsent(key, k -> new LinkedHashSet<>()).add(site);
        }
    }

    static String opcodeName(int opcode) {
        return switch (opcode) {
            case Opcodes.INVOKEVIRTUAL -> "INVOKEVIRTUAL";
            case Opcodes.INVOKESPECIAL -> "INVOKESPECIAL";
            case Opcodes.INVOKESTATIC -> "INVOKESTATIC";
            case Opcodes.INVOKEINTERFACE -> "INVOKEINTERFACE";
            default -> "INVOKE" + opcode;
        };
    }

    static String dotted(String internalName) {
        return internalName == null ? null : internalName.replace('/', '.');
    }

    static String modifiersOf(int access) {
        StringBuilder sb = new StringBuilder();
        if ((access & Opcodes.ACC_PUBLIC) != 0) sb.append("public ");
        if ((access & Opcodes.ACC_PRIVATE) != 0) sb.append("private ");
        if ((access & Opcodes.ACC_PROTECTED) != 0) sb.append("protected ");
        if ((access & Opcodes.ACC_STATIC) != 0) sb.append("static ");
        if ((access & Opcodes.ACC_FINAL) != 0) sb.append("final ");
        if ((access & Opcodes.ACC_SYNCHRONIZED) != 0) sb.append("synchronized ");
        if ((access & Opcodes.ACC_BRIDGE) != 0) sb.append("bridge ");
        if ((access & Opcodes.ACC_SYNTHETIC) != 0) sb.append("synthetic ");
        if ((access & Opcodes.ACC_ABSTRACT) != 0) sb.append("abstract ");
        if ((access & Opcodes.ACC_NATIVE) != 0) sb.append("native ");
        return sb.toString().trim();
    }

    /** Human-readable signature, e.g. "(ILjava/lang/String;)V" -&gt; "(int, java.lang.String) void". */
    static String prettyDescriptor(String desc) {
        try {
            org.objectweb.asm.Type[] args = org.objectweb.asm.Type.getArgumentTypes(desc);
            org.objectweb.asm.Type ret = org.objectweb.asm.Type.getReturnType(desc);
            StringBuilder sb = new StringBuilder("(");
            for (int i = 0; i < args.length; i++) {
                if (i > 0) {
                    sb.append(", ");
                }
                sb.append(prettyType(args[i]));
            }
            return sb.append(") ").append(prettyType(ret)).toString();
        } catch (RuntimeException e) {
            return desc;
        }
    }

    static String prettyType(org.objectweb.asm.Type t) {
        return switch (t.getSort()) {
            case org.objectweb.asm.Type.VOID -> "void";
            case org.objectweb.asm.Type.BOOLEAN -> "boolean";
            case org.objectweb.asm.Type.CHAR -> "char";
            case org.objectweb.asm.Type.BYTE -> "byte";
            case org.objectweb.asm.Type.SHORT -> "short";
            case org.objectweb.asm.Type.INT -> "int";
            case org.objectweb.asm.Type.FLOAT -> "float";
            case org.objectweb.asm.Type.LONG -> "long";
            case org.objectweb.asm.Type.DOUBLE -> "double";
            case org.objectweb.asm.Type.ARRAY -> prettyType(t.getElementType()) + "[]";
            default -> t.getClassName();
        };
    }

    static String quoteCsv(String s) {
        if (s.contains(",") || s.contains("\"")) {
            return "\"" + s.replace("\"", "\"\"") + "\"";
        }
        return s;
    }

    static final class Args {
        File outDir = new File("build/bytebuf-usage");
        String targetType = "net.minecraft.network.FriendlyByteBuf";
        String mcVersion = "unknown";
        final List<File> jars = new ArrayList<>();

        static Args parse(String[] argv) {
            Args a = new Args();
            for (int i = 0; i < argv.length; i++) {
                switch (argv[i]) {
                    case "--out" -> a.outDir = new File(argv[++i]);
                    case "--target" -> a.targetType = argv[++i];
                    case "--mc-version" -> a.mcVersion = argv[++i];
                    case "--jar" -> a.jars.add(new File(argv[++i]));
                    default -> throw new IllegalArgumentException("Unknown argument: " + argv[i]);
                }
            }
            if (a.jars.isEmpty()) {
                throw new IllegalArgumentException(
                    "No --jar arguments. The Gradle task must pass the resolved Minecraft artifacts.");
            }
            return a;
        }
    }
}