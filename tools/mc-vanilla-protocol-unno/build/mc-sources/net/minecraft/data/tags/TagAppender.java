package net.minecraft.data.tags;

import java.util.Arrays;
import java.util.Collection;
import java.util.Objects;
import java.util.function.Function;
import java.util.stream.Stream;
import net.minecraft.resources.ResourceKey;
import net.minecraft.tags.TagBuilder;
import net.minecraft.tags.TagKey;

public interface TagAppender<E, T> extends net.neoforged.neoforge.common.extensions.ITagAppenderExtension<E, T> {
    TagAppender<E, T> add(E element);

    default TagAppender<E, T> add(E... elements) {
        return this.addAll(Arrays.stream(elements));
    }

    default TagAppender<E, T> addAll(Collection<E> elements) {
        elements.forEach(this::add);
        return this;
    }

    default TagAppender<E, T> addAll(Stream<E> elements) {
        elements.forEach(this::add);
        return this;
    }

    TagAppender<E, T> addOptional(E element);

    TagAppender<E, T> addTag(TagKey<T> tag);

    TagAppender<E, T> addOptionalTag(TagKey<T> tag);

    static <T> TagAppender<ResourceKey<T>, T> forBuilder(TagBuilder builder) {
        return new TagAppender<ResourceKey<T>, T>() {
            public TagAppender<ResourceKey<T>, T> add(ResourceKey<T> element) {
                builder.addElement(element.identifier());
                return this;
            }

            public TagAppender<ResourceKey<T>, T> addOptional(ResourceKey<T> element) {
                builder.addOptionalElement(element.identifier());
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> addTag(TagKey<T> tag) {
                builder.addTag(tag.location());
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> addOptionalTag(TagKey<T> tag) {
                builder.addOptionalTag(tag.location());
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> add(net.minecraft.tags.TagEntry entry) {
                builder.add(entry);
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> replace(boolean value) {
                builder.replace(value);
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> remove(final ResourceKey<T> resourceKey) {
                builder.removeElement(resourceKey.identifier());
                return this;
            }

            @Override
            public TagAppender<ResourceKey<T>, T> remove(TagKey<T> tag) {
                builder.removeTag(tag.location());
                return this;
            }
        };
    }

    default <U> TagAppender<U, T> map(Function<U, E> converter) {
        final TagAppender<E, T> original = this;
        return new TagAppender<U, T>() {
            {
                Objects.requireNonNull(TagAppender.this);
            }

            @Override
            public TagAppender<U, T> add(U element) {
                original.add(converter.apply(element));
                return this;
            }

            @Override
            public TagAppender<U, T> addOptional(U element) {
                original.addOptional(converter.apply(element));
                return this;
            }

            @Override
            public TagAppender<U, T> addTag(TagKey<T> tag) {
                original.addTag(tag);
                return this;
            }

            @Override
            public TagAppender<U, T> addOptionalTag(TagKey<T> tag) {
                original.addOptionalTag(tag);
                return this;
            }

            @Override
            public TagAppender<U, T> add(net.minecraft.tags.TagEntry entry) {
                original.add(entry);
                return this;
            }

            @Override
            public TagAppender<U, T> replace(boolean value) {
                original.replace(value);
                return this;
            }

            @Override
            public TagAppender<U, T> remove(final U u) {
                original.remove(converter.apply(u));
                return this;
            }

            @Override
            public TagAppender<U, T> remove(TagKey<T> tag) {
                original.remove(tag);
                return this;
            }
        };
    }
}
