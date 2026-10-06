package cn.qexed.bytebuf;

import org.objectweb.asm.ClassReader;
import org.objectweb.asm.tree.ClassNode;
import org.objectweb.asm.tree.MethodNode;

import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.jar.JarEntry;
import java.util.jar.JarFile;

/**
 * Pass 1 index over every jar provided by Gradle dependency resolution:
 * for each class we keep its supertype edges and the set of declared method
 * keys (name + descriptor). Nothing is downloaded or looked up outside the
 * jars handed to us -- the tool stays generic over Minecraft versions.
 */
final class ClassIndex {
    /** internal name -> (super internal name or null, interfaces). */
    final Map<String, SuperInfo> supers = new HashMap<>();
    /** internal name -> declared method keys (name+desc), with access flags. */
    final Map<String, Map<String, Integer>> declarations = new HashMap<>();
    /** internal name -> originating jar file name (first jar wins). */
    final Map<String, String> classJar = new HashMap<>();
    /** classes that failed to parse (unsupported class file version etc.). */
    final List<String> unparsed = new ArrayList<>();

    int parsedClasses;
    int parsedJars;

    static final class SuperInfo {
        final String superName;
        final List<String> interfaces;
        SuperInfo(String superName, List<String> interfaces) {
            this.superName = superName;
            this.interfaces = interfaces;
        }
    }

    void indexJar(java.io.File jarFile) throws IOException {
        try (JarFile jar = new JarFile(jarFile)) {
            parsedJars++;
            java.util.Enumeration<JarEntry> entries = jar.entries();
            while (entries.hasMoreElements()) {
                JarEntry entry = entries.nextElement();
                if (entry.isDirectory() || !entry.getName().endsWith(".class")) {
                    continue;
                }
                String className = entry.getName().substring(0, entry.getName().length() - ".class".length());
                if (supers.containsKey(className)) {
                    continue; // first jar on the classpath wins
                }
                try (InputStream in = jar.getInputStream(entry)) {
                    ClassReader reader = new ClassReader(in);
                    ClassNode node = new ClassNode();
                    reader.accept(node, ClassReader.SKIP_CODE | ClassReader.SKIP_DEBUG);
                    supers.put(node.name, new SuperInfo(node.superName, node.interfaces == null ? List.of() : node.interfaces));
                    Map<String, Integer> decl = new LinkedHashMap<>();
                    if (node.methods != null) {
                        for (MethodNode mn : node.methods) {
                            decl.put(mn.name + mn.desc, mn.access);
                        }
                    }
                    declarations.put(node.name, decl);
                    classJar.put(node.name, jarFile.getName());
                    parsedClasses++;
                } catch (RuntimeException e) {
                    unparsed.add(className + " (" + e.getMessage() + ")");
                }
            }
        }
    }

    /** All transitive supertypes (classes + interfaces) of the given class, itself excluded. */
    Set<String> ancestorsOf(String internalName) {
        Set<String> out = new HashSet<>();
        collectAncestors(internalName, out);
        return out;
    }

    private void collectAncestors(String name, Set<String> seen) {
        SuperInfo info = supers.get(name);
        while (info != null) {
            String superName = info.superName;
            for (String itf : info.interfaces) {
                if (itf != null && seen.add(itf)) {
                    collectAncestors(itf, seen);
                }
            }
            if (superName == null || superName.equals("java/lang/Object") || !seen.add(superName)) {
                break;
            }
            info = supers.get(superName);
        }
    }

    /**
     * Resolves the declaring class for a method key starting at {@code owner}:
     * walks the inheritance chain (owner first) and returns the first class
     * that declares the key, or {@code null} when the chain leaves the indexed
     * universe (a library method) or the key is unknown.
     */
    String resolveDeclaration(String owner, String key) {
        String current = owner;
        Set<String> visited = new HashSet<>();
        while (current != null && visited.add(current)) {
            Map<String, Integer> decl = declarations.get(current);
            if (decl != null && decl.containsKey(key)) {
                return current;
            }
            SuperInfo info = supers.get(current);
            if (info == null) {
                return null;
            }
            for (String itf : info.interfaces) {
                String viaItf = resolveDeclaration(itf, key);
                if (viaItf != null) {
                    return viaItf;
                }
            }
            current = info.superName;
        }
        return null;
    }

    /** Classes whose transitive supertype set contains {@code type}. */
    List<String> subtypesOf(String type) {
        List<String> out = new ArrayList<>();
        for (String candidate : supers.keySet()) {
            if (!candidate.equals(type) && ancestorsOf(candidate).contains(type)) {
                out.add(candidate);
            }
        }
        java.util.Collections.sort(out);
        return out;
    }

    boolean inUniverse(String internalName) {
        return supers.containsKey(internalName);
    }
}
