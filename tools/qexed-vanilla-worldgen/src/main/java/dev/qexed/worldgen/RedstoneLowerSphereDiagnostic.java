package dev.qexed.worldgen;

import net.minecraft.util.Mth;
import net.minecraft.world.level.levelgen.XoroshiroRandomSource;

public final class RedstoneLowerSphereDiagnostic {
    private static final int WORLD_MIN_Y = -64;
    private static final int WORLD_HEIGHT = 384;
    private static final int FEATURE_INDEX = 17;
    private static final int STEP_INDEX = 6;
    private static final int SIZE = 8;

    private RedstoneLowerSphereDiagnostic() {
    }

    public static void main(String[] args) {
        long seed = args.length > 0 ? Long.parseLong(args[0]) : 0L;
        int chunkX = args.length > 1 ? Integer.parseInt(args[1]) : 0;
        int chunkZ = args.length > 2 ? Integer.parseInt(args[2]) : 0;
        int targetAttempt = args.length > 3 ? Integer.parseInt(args[3]) : 3;

        int originX = chunkX * 16;
        int originZ = chunkZ * 16;
        XoroshiroRandomSource random = new XoroshiroRandomSource(seed);
        long decorationSeed = setDecorationSeed(random, seed, originX, originZ);
        setFeatureSeed(random, decorationSeed, FEATURE_INDEX, STEP_INDEX);

        int count = 8;
        System.out.printf(
                "java seed=%d chunk=(%d,%d) ore_redstone_lower count=%d step=%d index=%d decoration_seed=%d%n",
                seed,
                chunkX,
                chunkZ,
                count,
                STEP_INDEX,
                FEATURE_INDEX,
                decorationSeed);

        for (int attempt = 0; attempt < count; attempt++) {
            int x = originX + random.nextInt(16);
            int z = originZ + random.nextInt(16);
            int y = sampleRedstoneLowerY(random);
            BlobPrefix prefix = samplePrefix(random, x, y, z);

            if (attempt != targetAttempt) {
                consumeShape(random);
                continue;
            }

            System.out.printf("java ore_redstone_lower attempt=%d origin=(%d,%d,%d)%n", attempt, x, y, z);
            System.out.printf(
                    "java ore_redstone_lower attempt=%d prefix x0=%.17f x1=%.17f y0=%.17f y1=%.17f z0=%.17f z1=%.17f min_box=(%d,%d,%d) tested_dims=(%d, %d, %d) tested_strides=(%d, %d)%n",
                    attempt,
                    prefix.x0,
                    prefix.x1,
                    prefix.y0,
                    prefix.y1,
                    prefix.z0,
                    prefix.z1,
                    prefix.minBoxX,
                    prefix.minBoxY,
                    prefix.minBoxZ,
                    prefix.testedSizeX + 1,
                    prefix.testedSizeY + 1,
                    prefix.testedSizeZ + 1,
                    prefix.testedStrideX,
                    prefix.testedStrideY);
            printSpheres(random, prefix);
            return;
        }

        throw new IllegalStateException("attempt not reached: " + targetAttempt);
    }

    private static long setDecorationSeed(XoroshiroRandomSource random, long worldSeed, int x, int z) {
        random.setSeed(worldSeed);
        long xSeed = random.nextLong() | 1L;
        long zSeed = random.nextLong() | 1L;
        long decorationSeed = (long) x * xSeed + (long) z * zSeed ^ worldSeed;
        random.setSeed(decorationSeed);
        return decorationSeed;
    }

    private static void setFeatureSeed(XoroshiroRandomSource random, long decorationSeed, int featureIndex, int stepIndex) {
        random.setSeed(decorationSeed + (long) featureIndex + (long) (10_000 * stepIndex));
    }

    private static int sampleRedstoneLowerY(XoroshiroRandomSource random) {
        int min = WORLD_MIN_Y - 32;
        int max = WORLD_MIN_Y + 32;
        int range = max - min;
        int plateauStart = range / 2;
        int plateauEnd = range - plateauStart;
        return min + random.nextInt(plateauEnd + 1) + random.nextInt(plateauStart + 1);
    }

    private static BlobPrefix samplePrefix(XoroshiroRandomSource random, int x, int y, int z) {
        float angle = random.nextFloat() * (float) Math.PI;
        float horizontalRadius = (float) SIZE / 8.0F;
        int paddedRadius = Mth.ceil(((float) SIZE / 16.0F * 2.0F + 1.0F) / 2.0F);
        double sin = Math.sin(angle);
        double cos = Math.cos(angle);
        double x0 = (double) x + sin * (double) horizontalRadius;
        double x1 = (double) x - sin * (double) horizontalRadius;
        double z0 = (double) z + cos * (double) horizontalRadius;
        double z1 = (double) z - cos * (double) horizontalRadius;
        double y0 = (double) (y + random.nextInt(3) - 2);
        double y1 = (double) (y + random.nextInt(3) - 2);
        int minBoxX = x - Mth.ceil(horizontalRadius) - paddedRadius;
        int minBoxY = y - 2 - paddedRadius;
        int minBoxZ = z - Mth.ceil(horizontalRadius) - paddedRadius;
        int testedSizeX = 2 * (Mth.ceil(horizontalRadius) + paddedRadius);
        int testedSizeY = 2 * (2 + paddedRadius);

        return new BlobPrefix(
                x0,
                x1,
                y0,
                y1,
                z0,
                z1,
                minBoxX,
                minBoxY,
                minBoxZ,
                testedSizeX,
                testedSizeY,
                testedSizeX,
                testedSizeX,
                testedSizeY);
    }

    private static void consumeShape(XoroshiroRandomSource random) {
        for (int index = 0; index < SIZE; index++) {
            random.nextDouble();
        }
    }

    private static void printSpheres(XoroshiroRandomSource random, BlobPrefix prefix) {
        Sphere[] spheres = new Sphere[SIZE];
        for (int index = 0; index < SIZE; index++) {
            float step = (float) index / (float) SIZE;
            double centerX = Mth.lerp((double) step, prefix.x0, prefix.x1);
            double centerY = Mth.lerp((double) step, prefix.y0, prefix.y1);
            double centerZ = Mth.lerp((double) step, prefix.z0, prefix.z1);
            double radiusNoise = random.nextDouble() * (double) SIZE / 16.0D;
            float sin = Mth.sin((float) Math.PI * step);
            double radius = ((double) (sin + 1.0F) * radiusNoise + 1.0D) / 2.0D;
            spheres[index] = new Sphere(index, step, sin, radiusNoise, centerX, centerY, centerZ, radius);
        }

        for (int left = 0; left < SIZE - 1; left++) {
            if (spheres[left].radiusAfterCull <= 0.0D) {
                continue;
            }
            for (int right = left + 1; right < SIZE; right++) {
                if (spheres[right].radiusAfterCull <= 0.0D) {
                    continue;
                }
                double dx = spheres[left].x - spheres[right].x;
                double dy = spheres[left].y - spheres[right].y;
                double dz = spheres[left].z - spheres[right].z;
                double dr = spheres[left].radiusAfterCull - spheres[right].radiusAfterCull;
                if (dr * dr > dx * dx + dy * dy + dz * dz) {
                    if (dr > 0.0D) {
                        spheres[right].radiusAfterCull = -1.0D;
                    } else {
                        spheres[left].radiusAfterCull = -1.0D;
                    }
                }
            }
        }

        for (Sphere sphere : spheres) {
            System.out.printf(
                    "java ore_redstone_lower attempt=3 sphere=%d step=%.9f sin=%.9f radius_noise=%.17f center=(%.17f,%.17f,%.17f) radius_before_cull=%.17f radius_after_cull=%.17f%n",
                    sphere.index,
                    sphere.step,
                    sphere.sin,
                    sphere.radiusNoise,
                    sphere.x,
                    sphere.y,
                    sphere.z,
                    sphere.radiusBeforeCull,
                    sphere.radiusAfterCull);
        }
    }

    private record BlobPrefix(
            double x0,
            double x1,
            double y0,
            double y1,
            double z0,
            double z1,
            int minBoxX,
            int minBoxY,
            int minBoxZ,
            int testedSizeX,
            int testedSizeY,
            int testedSizeZ,
            int testedStrideX,
            int testedStrideY) {
    }

    private static final class Sphere {
        final int index;
        final float step;
        final float sin;
        final double radiusNoise;
        final double x;
        final double y;
        final double z;
        final double radiusBeforeCull;
        double radiusAfterCull;

        Sphere(
                int index,
                float step,
                float sin,
                double radiusNoise,
                double x,
                double y,
                double z,
                double radius) {
            this.index = index;
            this.step = step;
            this.sin = sin;
            this.radiusNoise = radiusNoise;
            this.x = x;
            this.y = y;
            this.z = z;
            this.radiusBeforeCull = radius;
            this.radiusAfterCull = radius;
        }
    }
}
