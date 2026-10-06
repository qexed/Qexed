package com.mojang.math;

import com.mojang.serialization.Codec;
import com.mojang.serialization.codecs.RecordCodecBuilder;
import java.util.Objects;
import java.util.Optional;
import net.minecraft.util.ExtraCodecs;
import net.minecraft.util.Util;
import org.apache.commons.lang3.tuple.Triple;
import org.joml.Matrix3f;
import org.joml.Matrix4f;
import org.joml.Matrix4fc;
import org.joml.Quaternionf;
import org.joml.Quaternionfc;
import org.joml.Vector3f;
import org.joml.Vector3fc;
import org.jspecify.annotations.Nullable;

public final class Transformation implements net.neoforged.neoforge.common.extensions.ITransformationExtension {
    private static final Vector3fc ZERO_TRANSLATION = new Vector3f();
    private static final Vector3fc UNIT_SCALE = new Vector3f(1.0F, 1.0F, 1.0F);
    private static final Quaternionfc ZERO_ROTATION = new Quaternionf();
    private final Matrix4fc matrix;
    public static final Codec<Transformation> CODEC = RecordCodecBuilder.create(
        i -> i.group(
                ExtraCodecs.VECTOR3F.fieldOf("translation").forGetter(Transformation::translation),
                ExtraCodecs.QUATERNIONF.fieldOf("left_rotation").forGetter(Transformation::leftRotation),
                ExtraCodecs.VECTOR3F.fieldOf("scale").forGetter(Transformation::scale),
                ExtraCodecs.QUATERNIONF.fieldOf("right_rotation").forGetter(Transformation::rightRotation)
            )
            .apply(i, Transformation::new)
    );
    public static final Codec<Transformation> EXTENDED_CODEC = Codec.withAlternative(
        CODEC, ExtraCodecs.MATRIX4F.xmap(Transformation::new, Transformation::getMatrix)
    );
    private boolean decomposed;
    private @Nullable Vector3fc translation;
    private @Nullable Quaternionfc leftRotation;
    private @Nullable Vector3fc scale;
    private @Nullable Quaternionfc rightRotation;
    public static final Transformation IDENTITY = Util.make(() -> {
        Transformation identity = new Transformation(new Matrix4f());
        identity.translation = ZERO_TRANSLATION;
        identity.leftRotation = ZERO_ROTATION;
        identity.scale = UNIT_SCALE;
        identity.rightRotation = ZERO_ROTATION;
        identity.decomposed = true;
        return identity;
    });

    public Transformation(Matrix4fc matrix) {
        this.matrix = matrix;
    }

    public Transformation(@Nullable Vector3fc translation, @Nullable Quaternionfc leftRotation, @Nullable Vector3fc scale, @Nullable Quaternionfc rightRotation) {
        this.matrix = compose(translation, leftRotation, scale, rightRotation);
        this.translation = Objects.requireNonNullElse(translation, ZERO_TRANSLATION);
        this.leftRotation = Objects.requireNonNullElse(leftRotation, ZERO_ROTATION);
        this.scale = Objects.requireNonNullElse(scale, UNIT_SCALE);
        this.rightRotation = Objects.requireNonNullElse(rightRotation, ZERO_ROTATION);
        this.decomposed = true;
    }

    public Transformation compose(Transformation that) {
        Matrix4f matrix = this.getMatrixCopy();
        matrix.mul(that.getMatrix());
        return new Transformation(matrix);
    }

    public @Nullable Transformation inverse() {
        if (this == IDENTITY) {
            return this;
        } else {
            Matrix4f matrix = this.getMatrixCopy().invertAffine();
            return matrix.isFinite() ? new Transformation(matrix) : null;
        }
    }

    private void ensureDecomposed() {
        if (!this.decomposed) {
            float scaleFactor = 1.0F / this.matrix.m33();
            Triple<Quaternionf, Vector3f, Quaternionf> triple = MatrixUtil.svdDecompose(new Matrix3f(this.matrix).scale(scaleFactor));
            this.translation = this.matrix.getTranslation(new Vector3f()).mul(scaleFactor);
            this.leftRotation = new Quaternionf(triple.getLeft());
            this.scale = new Vector3f(triple.getMiddle());
            this.rightRotation = new Quaternionf(triple.getRight());
            this.decomposed = true;
        }
    }

    private static Matrix4f compose(
        @Nullable Vector3fc translation, @Nullable Quaternionfc leftRotation, @Nullable Vector3fc scale, @Nullable Quaternionfc rightRotation
    ) {
        Matrix4f result = new Matrix4f();
        if (translation != null) {
            result.translation(translation);
        }

        if (leftRotation != null) {
            result.rotate(leftRotation);
        }

        if (scale != null) {
            result.scale(scale);
        }

        if (rightRotation != null) {
            result.rotate(rightRotation);
        }

        return result;
    }

    public Matrix4fc getMatrix() {
        return this.matrix;
    }

    public Matrix4f getMatrixCopy() {
        return new Matrix4f(this.matrix);
    }

    public Vector3fc translation() {
        this.ensureDecomposed();
        return this.translation;
    }

    public Quaternionfc leftRotation() {
        this.ensureDecomposed();
        return this.leftRotation;
    }

    public Vector3fc scale() {
        this.ensureDecomposed();
        return this.scale;
    }

    public Quaternionfc rightRotation() {
        this.ensureDecomposed();
        return this.rightRotation;
    }

    @Override
    public boolean equals(Object o) {
        if (this == o) {
            return true;
        } else if (o != null && this.getClass() == o.getClass()) {
            Transformation that = (Transformation)o;
            return Objects.equals(this.matrix, that.matrix);
        } else {
            return false;
        }
    }

    @Override
    public int hashCode() {
        return Objects.hash(this.matrix);
    }

    private Matrix3f normalTransform = null;
    public org.joml.Matrix3fc getNormalMatrix() {
        checkNormalTransform();
        return normalTransform;
    }
    private void checkNormalTransform() {
        if (normalTransform == null) {
            normalTransform = new Matrix3f(matrix);
            normalTransform.invert();
            normalTransform.transpose();
        }
    }

    public Transformation slerp(Transformation that, float progress) {
        return new Transformation(
            this.translation().lerp(that.translation(), progress, new Vector3f()),
            this.leftRotation().slerp(that.leftRotation(), progress, new Quaternionf()),
            this.scale().lerp(that.scale(), progress, new Vector3f()),
            this.rightRotation().slerp(that.rightRotation(), progress, new Quaternionf())
        );
    }

    public static Matrix4fc compose(Matrix4fc parent, Optional<Transformation> transform) {
        return (Matrix4fc)(transform.isPresent() ? parent.mul(transform.get().getMatrix(), new Matrix4f()) : parent);
    }
}
