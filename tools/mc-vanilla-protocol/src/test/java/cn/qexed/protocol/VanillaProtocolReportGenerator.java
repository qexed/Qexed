package cn.qexed.protocol;

import java.lang.reflect.Method;

/**
 * Runs Mojang's own data generator ("--reports") inside the VanillaGradle test
 * classpath, producing authoritative registry/packet reports without any
 * manual wiki lookup. Not a JUnit test: invoked reflectively by the Gradle
 * task 'generateVanillaReports' via a launcher main.
 */
public final class VanillaProtocolReportGenerator {

    public static void main(String[] args) throws Exception {
        String output = args.length > 0 ? args[0] : "build/vanilla-reports";
        String[] generatorArgs = {"--reports", "--output", output};
        Class<?> main = Class.forName("net.minecraft.data.Main");
        Method m = main.getMethod("main", String[].class);
        m.invoke(null, (Object) generatorArgs);
        System.out.println("[qexed] reports written to " + output);
    }

    private VanillaProtocolReportGenerator() {}
}
