package com.mojang.blaze3d.vertex;

import com.mojang.blaze3d.GraphicsWorkarounds;
import com.mojang.blaze3d.buffers.GpuBuffer;
import com.mojang.blaze3d.systems.CommandEncoder;
import com.mojang.blaze3d.systems.GpuDevice;
import com.mojang.datafixers.util.Pair;
import it.unimi.dsi.fastutil.objects.Object2ObjectOpenHashMap;
import it.unimi.dsi.fastutil.objects.ObjectOpenHashSet;
import java.nio.ByteBuffer;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Iterator;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Map.Entry;
import net.minecraft.client.renderer.MappableRingBuffer;
import net.minecraft.util.VisibleForDebug;
import net.minecraft.util.profiling.Profiler;
import net.minecraft.util.profiling.Zone;
import net.neoforged.api.distmarker.Dist;
import net.neoforged.api.distmarker.OnlyIn;
import org.jspecify.annotations.Nullable;
import org.lwjgl.system.MemoryUtil;

@OnlyIn(Dist.CLIENT)
public class UberGpuBuffer<T> implements AutoCloseable {
    private final int alignSize;
    private final UberGpuBuffer.UberGpuBufferStagingBuffer stagingBuffer;
    private int stagingBufferUsedSize;
    private final String name;
    private final List<Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap>> nodes;
    private final Object2ObjectOpenHashMap<T, UberGpuBuffer.StagedAllocationEntry<T>> stagedAllocations = new Object2ObjectOpenHashMap<>(32);
    private final ObjectOpenHashSet<T> skippedStagedAllocations = new ObjectOpenHashSet<>(32);
    private final Map<T, TlsfAllocator.Allocation> allocationMap = new HashMap<>(256);

    public UberGpuBuffer(String name, int usage, int heapSize, int alignSize, GpuDevice gpuDevice, int stagingBufferSize, GraphicsWorkarounds workarounds) {
        if (stagingBufferSize > heapSize) {
            throw new IllegalArgumentException("Staging buffer size cannot be bigger than heap size");
        } else {
            this.name = "UberBuffer " + name;
            this.stagingBuffer = UberGpuBuffer.UberGpuBufferStagingBuffer.create(this.name, gpuDevice, stagingBufferSize, workarounds);
            this.stagingBufferUsedSize = 0;
            this.nodes = new ArrayList<>();
            this.alignSize = alignSize;
            String initialHeapName = this.name + " 0";
            UberGpuBuffer.UberGpuBufferHeap initialHeap = new UberGpuBuffer.UberGpuBufferHeap(heapSize, gpuDevice, usage, initialHeapName);
            TlsfAllocator initialTlsfAllocator = new TlsfAllocator(initialHeap);
            this.nodes.add(new Pair<>(initialTlsfAllocator, initialHeap));
        }
    }

    public boolean addAllocation(T allocationKey, UberGpuBuffer.@Nullable UploadCallback<T> callback, ByteBuffer buffer) {
        int startOffset = this.stagingBufferUsedSize;
        ByteBuffer stagingBuffer = this.stagingBuffer.getStagingBuffer();
        if (buffer.remaining() > stagingBuffer.capacity()) {
            throw new IllegalArgumentException("UberGpuBuffer cannot have any allocations bigger than its staging buffer, increase the staging buffer size!");
        } else if (buffer.remaining() > stagingBuffer.capacity() - startOffset) {
            return false;
        } else {
            MemoryUtil.memCopy(buffer, stagingBuffer.position(startOffset));
            this.stagingBufferUsedSize = this.stagingBufferUsedSize + buffer.remaining();
            UberGpuBuffer.StagedAllocationEntry<T> entry = new UberGpuBuffer.StagedAllocationEntry<>(callback, (long)startOffset, (long)buffer.remaining());
            this.stagedAllocations.put(allocationKey, entry);
            return true;
        }
    }

    public boolean uploadStagedAllocations(GpuDevice gpuDevice, CommandEncoder encoder) {
        for (T key : this.stagedAllocations.keySet()) {
            this.freeAllocation(key);
        }

        boolean newHeapCreatedOrDestroyed = false;

        try (Zone ignored = Profiler.get().zone("Upload staged allocations")) {
            for (Entry<T, UberGpuBuffer.StagedAllocationEntry<T>> entry : this.stagedAllocations.entrySet()) {
                long allocationSize = entry.getValue().size;
                if (!this.skippedStagedAllocations.contains(entry.getKey())) {
                    TlsfAllocator.Allocation allocation = null;

                    for (Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap> node : this.nodes) {
                        allocation = node.getFirst().allocate(allocationSize, this.alignSize);
                        if (allocation != null) {
                            break;
                        }
                    }

                    if (allocation == null) {
                        try (Zone ignored2 = Profiler.get().zone("Create new heap")) {
                            UberGpuBuffer.UberGpuBufferHeap firstHeap = this.nodes.getFirst().getSecond();
                            long heapSize = firstHeap.gpuBuffer.size();

                            assert allocationSize <= heapSize;

                            String heapName = String.format(Locale.ROOT, "%s %d", this.name, this.nodes.size());
                            UberGpuBuffer.UberGpuBufferHeap newHeap = new UberGpuBuffer.UberGpuBufferHeap(
                                heapSize, gpuDevice, firstHeap.gpuBuffer.usage(), heapName
                            );
                            TlsfAllocator newTlsfAllocator = new TlsfAllocator(newHeap);
                            this.nodes.add(new Pair<>(newTlsfAllocator, newHeap));
                            allocation = newTlsfAllocator.allocate(allocationSize, this.alignSize);
                            newHeapCreatedOrDestroyed = true;
                        }
                    }

                    if (allocation != null) {
                        TlsfAllocator.Heap allocationHeap = allocation.getHeap();
                        GpuBuffer allocationDestBuffer = ((UberGpuBuffer.UberGpuBufferHeap)allocationHeap).gpuBuffer;
                        this.stagingBuffer.copyToHeap(encoder, allocationDestBuffer, allocation.getOffsetFromHeap(), entry.getValue().offset, allocationSize);
                        this.allocationMap.put(entry.getKey(), allocation);
                        if (entry.getValue().callback != null) {
                            entry.getValue().callback.bufferHasBeenUploaded(entry.getKey());
                        }
                    }
                }
            }

            this.stagingBuffer.clearFrame(encoder);
            this.stagingBufferUsedSize = 0;
            this.stagedAllocations.clear();
            this.skippedStagedAllocations.clear();
        }

        Iterator<Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap>> iterator = this.nodes.iterator();

        while (iterator.hasNext() && this.nodes.size() > 1) {
            Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap> nodex = iterator.next();
            if (nodex.getFirst().isCompletelyFree()) {
                nodex.getSecond().gpuBuffer.close();
                iterator.remove();
                newHeapCreatedOrDestroyed = true;
                break;
            }
        }

        return newHeapCreatedOrDestroyed;
    }

    public TlsfAllocator.@Nullable Allocation getAllocation(T allocationKey) {
        return this.allocationMap.get(allocationKey);
    }

    public void removeAllocation(T allocationKey) {
        this.skippedStagedAllocations.add(allocationKey);
        this.freeAllocation(allocationKey);
    }

    private void freeAllocation(T allocationKey) {
        TlsfAllocator.Allocation allocation = this.allocationMap.remove(allocationKey);
        if (allocation != null) {
            for (Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap> node : this.nodes) {
                if (node.getSecond() == allocation.getHeap()) {
                    node.getFirst().free(allocation);
                    break;
                }
            }
        }
    }

    public GpuBuffer getGpuBuffer(TlsfAllocator.Allocation allocation) {
        return ((UberGpuBuffer.UberGpuBufferHeap)allocation.getHeap()).gpuBuffer;
    }

    @VisibleForDebug
    public void printStatistics() {
        for (int i = 0; i < this.nodes.size(); i++) {
            Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap> node = this.nodes.get(i);
            String heapName = String.format(Locale.ROOT, "%s %d", this.name, i);
            node.getFirst().printAllocatorStatistics(heapName);
        }
    }

    @Override
    public void close() {
        this.stagingBuffer.destroyBuffer();
        this.stagingBufferUsedSize = 0;
        this.stagedAllocations.clear();
        this.allocationMap.clear();

        for (Pair<TlsfAllocator, UberGpuBuffer.UberGpuBufferHeap> node : this.nodes) {
            node.getSecond().gpuBuffer.close();
        }

        this.nodes.clear();
    }

    @OnlyIn(Dist.CLIENT)
    private static class StagedAllocationEntry<T> {
        UberGpuBuffer.@Nullable UploadCallback<T> callback;
        long offset;
        long size;

        private StagedAllocationEntry(UberGpuBuffer.@Nullable UploadCallback<T> callback, long offset, long size) {
            this.offset = offset;
            this.size = size;
            this.callback = callback;
        }
    }

    @OnlyIn(Dist.CLIENT)
    public static class UberGpuBufferHeap extends TlsfAllocator.Heap {
        GpuBuffer gpuBuffer;

        UberGpuBufferHeap(long size, GpuDevice gpuDevice, int usage, String name) {
            super(size);
            this.gpuBuffer = gpuDevice.createBuffer(() -> name, usage | 8 | 16, size);
        }
    }

    @OnlyIn(Dist.CLIENT)
    private abstract static class UberGpuBufferStagingBuffer {
        public static UberGpuBuffer.UberGpuBufferStagingBuffer create(String name, GpuDevice gpuDevice, int stagingBufferSize, GraphicsWorkarounds workarounds) {
            return (UberGpuBuffer.UberGpuBufferStagingBuffer)(!workarounds.isGlOnDx12()
                ? new UberGpuBuffer.UberGpuBufferStagingBuffer.CPUStagingBuffer(name, gpuDevice, stagingBufferSize)
                : new UberGpuBuffer.UberGpuBufferStagingBuffer.MappedStagingBuffer(name, gpuDevice, stagingBufferSize));
        }

        abstract ByteBuffer getStagingBuffer();

        abstract void copyToHeap(final CommandEncoder encoder, final GpuBuffer heapBuffer, long heapOffset, long stagingBufferOffset, long copySize);

        abstract void clearFrame(final CommandEncoder encoder);

        abstract void destroyBuffer();

        @OnlyIn(Dist.CLIENT)
        private static class CPUStagingBuffer extends UberGpuBuffer.UberGpuBufferStagingBuffer {
            private final ByteBuffer stagingBuffer;

            private CPUStagingBuffer(String name, GpuDevice gpuDevice, int stagingBufferSize) {
                this.stagingBuffer = MemoryUtil.memAlloc(stagingBufferSize);
            }

            @Override
            ByteBuffer getStagingBuffer() {
                return this.stagingBuffer;
            }

            @Override
            void copyToHeap(CommandEncoder encoder, GpuBuffer heapBuffer, long heapOffset, long stagingBufferOffset, long copySize) {
                encoder.writeToBuffer(heapBuffer.slice(heapOffset, copySize), this.stagingBuffer.slice((int)stagingBufferOffset, (int)copySize));
            }

            @Override
            void clearFrame(CommandEncoder encoder) {
                this.stagingBuffer.clear();
            }

            @Override
            void destroyBuffer() {
                this.stagingBuffer.clear();
                MemoryUtil.memFree(this.stagingBuffer);
            }
        }

        @OnlyIn(Dist.CLIENT)
        private static class MappedStagingBuffer extends UberGpuBuffer.UberGpuBufferStagingBuffer {
            private final MappableRingBuffer mappableRingBuffer;
            private GpuBuffer.MappedView currentMappedView;
            private GpuBuffer currentGPUBuffer;
            private ByteBuffer currentBuffer;

            private MappedStagingBuffer(String name, GpuDevice gpuDevice, int stagingBufferSize) {
                String stagingBufferName = name + " staging buffer";
                this.mappableRingBuffer = new MappableRingBuffer(() -> stagingBufferName, 18, stagingBufferSize / 2);
                CommandEncoder encoder = gpuDevice.createCommandEncoder();
                this.currentGPUBuffer = this.mappableRingBuffer.currentBuffer();
                this.currentMappedView = encoder.mapBuffer(this.currentGPUBuffer, false, true);
                this.currentBuffer = this.currentMappedView.data();
            }

            @Override
            ByteBuffer getStagingBuffer() {
                return this.currentBuffer;
            }

            @Override
            void copyToHeap(CommandEncoder encoder, GpuBuffer heapBuffer, long heapOffset, long stagingBufferOffset, long copySize) {
                encoder.copyToBuffer(this.currentGPUBuffer.slice(stagingBufferOffset, copySize), heapBuffer.slice(heapOffset, copySize));
            }

            @Override
            void clearFrame(CommandEncoder encoder) {
                this.currentMappedView.close();
                this.mappableRingBuffer.rotate();
                this.currentGPUBuffer = this.mappableRingBuffer.currentBuffer();
                this.currentMappedView = encoder.mapBuffer(this.currentGPUBuffer, false, true);
                this.currentBuffer = this.currentMappedView.data();
            }

            @Override
            void destroyBuffer() {
                this.currentMappedView.close();
                this.mappableRingBuffer.close();
            }
        }
    }

    @OnlyIn(Dist.CLIENT)
    public interface UploadCallback<T> {
        void bufferHasBeenUploaded(T key);
    }
}
