package cn.qexed.protocol;

import cn.qexed.integration.QexedProtocolClient;
import io.netty.buffer.ByteBuf;
import io.netty.buffer.Unpooled;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Executor;
import net.minecraft.SharedConstants;
import net.minecraft.core.HolderLookup;
import net.minecraft.core.Registry;
import net.minecraft.core.RegistryAccess;
import net.minecraft.core.RegistrySynchronization;
import net.minecraft.network.ProtocolInfo;
import net.minecraft.network.RegistryFriendlyByteBuf;
import net.minecraft.network.VarInt;
import net.minecraft.network.protocol.Packet;
import net.minecraft.network.protocol.common.ClientboundUpdateTagsPacket;
import net.minecraft.network.protocol.configuration.ClientboundRegistryDataPacket;
import net.minecraft.network.protocol.configuration.ClientConfigurationPacketListener;
import net.minecraft.network.protocol.configuration.ConfigurationProtocols;
import net.minecraft.network.protocol.game.GameProtocols;
import net.minecraft.network.protocol.login.ClientLoginPacketListener;
import net.minecraft.network.protocol.login.LoginProtocols;
import net.minecraft.resources.RegistryDataLoader;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.Bootstrap;
import net.minecraft.server.RegistryLayer;
import net.minecraft.server.packs.resources.ResourceProvider;
import net.minecraft.tags.TagLoader;
import net.minecraft.tags.TagNetworkSerialization;

final class MinecraftPacketDecoder {
    static {
        SharedConstants.tryDetectVersion();
        Bootstrap.bootStrap();
    }

    private MinecraftPacketDecoder() {
    }

    private static Packet<?> decode(ProtocolInfo<?> protocol, QexedProtocolClient.PacketRecord packet) {
        ByteBuf buffer = Unpooled.buffer(VarInt.getByteSize(packet.id()) + packet.payload().length);
        VarInt.write(buffer, packet.id());
        buffer.writeBytes(packet.payload());

        Packet<?> decoded = protocol.codec().decode(buffer);
        if (buffer.readableBytes() != 0) {
            throw new AssertionError(protocol.id().id() + "/" + packet.idHex() + " left "
                    + buffer.readableBytes() + " unread bytes");
        }
        return decoded;
    }

    static final class ClientboundSession {
        private final Map<ResourceKey<? extends Registry<?>>, List<RegistrySynchronization.PackedRegistryEntry>>
                registryEntries = new HashMap<>();
        private final Map<ResourceKey<? extends Registry<?>>, TagNetworkSerialization.NetworkPayload> registryTags =
                new HashMap<>();
        private RegistryAccess.Frozen playRegistryAccess;
        private ProtocolInfo<?> playProtocol;

        Object decodeLogin(QexedProtocolClient.PacketRecord packet) {
            ProtocolInfo<ClientLoginPacketListener> protocol = LoginProtocols.CLIENTBOUND;
            return decode(protocol, packet);
        }

        Object decodeConfiguration(QexedProtocolClient.PacketRecord packet) {
            ProtocolInfo<ClientConfigurationPacketListener> protocol = ConfigurationProtocols.CLIENTBOUND;
            Object decoded = decode(protocol, packet);
            if (decoded instanceof ClientboundRegistryDataPacket registryData) {
                registryEntries.put(registryData.registry(), List.copyOf(registryData.entries()));
                playRegistryAccess = null;
                playProtocol = null;
            }
            if (decoded instanceof ClientboundUpdateTagsPacket updateTags) {
                registryTags.putAll(updateTags.getTags());
                playRegistryAccess = null;
                playProtocol = null;
            }
            return decoded;
        }

        Object decodePlay(QexedProtocolClient.PacketRecord packet) {
            return decode(playProtocol(), packet);
        }

        private ProtocolInfo<?> playProtocol() {
            if (playProtocol == null) {
                playProtocol = GameProtocols.CLIENTBOUND_TEMPLATE.bind(
                        buffer -> new RegistryFriendlyByteBuf(buffer, playRegistryAccess()));
            }
            return playProtocol;
        }

        private RegistryAccess.Frozen playRegistryAccess() {
            if (playRegistryAccess == null) {
                playRegistryAccess = buildPlayRegistryAccess();
            }
            return playRegistryAccess;
        }

        private RegistryAccess.Frozen buildPlayRegistryAccess() {
            Map<ResourceKey<? extends Registry<?>>, RegistryDataLoader.NetworkedRegistryData> networkData =
                    new HashMap<>();
            for (Map.Entry<ResourceKey<? extends Registry<?>>, List<RegistrySynchronization.PackedRegistryEntry>> entry :
                    registryEntries.entrySet()) {
                TagNetworkSerialization.NetworkPayload tags =
                        registryTags.getOrDefault(entry.getKey(), TagNetworkSerialization.NetworkPayload.EMPTY);
                networkData.put(entry.getKey(), new RegistryDataLoader.NetworkedRegistryData(entry.getValue(), tags));
            }

            List<HolderLookup.RegistryLookup<?>> staticRegistries = RegistryLayer.createRegistryAccess()
                    .getLayer(RegistryLayer.STATIC)
                    .listRegistries()
                    .toList();
            List<Registry.PendingTags<?>> pendingStaticTags = pendingTags(
                    RegistryLayer.createRegistryAccess().getLayer(RegistryLayer.STATIC));
            if (!pendingStaticTags.isEmpty()) {
                staticRegistries = TagLoader.buildUpdatedLookups(
                        RegistryLayer.createRegistryAccess().getLayer(RegistryLayer.STATIC), pendingStaticTags);
            }
            Executor directExecutor = Runnable::run;
            return RegistryDataLoader.load(
                            networkData,
                            ResourceProvider.EMPTY,
                            staticRegistries,
                            RegistryDataLoader.SYNCHRONIZED_REGISTRIES,
                            directExecutor)
                    .join();
        }

        private List<Registry.PendingTags<?>> pendingTags(RegistryAccess.Frozen registryAccess) {
            List<Registry.PendingTags<?>> pending = new ArrayList<>();
            registryAccess.registries().forEach(entry -> pendingTag(entry).ifPresent(pending::add));
            return pending;
        }

        private <T> java.util.Optional<Registry.PendingTags<T>> pendingTag(RegistryAccess.RegistryEntry<T> entry) {
            @SuppressWarnings("unchecked")
            TagNetworkSerialization.NetworkPayload payload =
                    registryTags.get((ResourceKey<? extends Registry<?>>) entry.key());
            if (payload == null || payload.isEmpty()) {
                return java.util.Optional.empty();
            }
            return java.util.Optional.of(entry.value().prepareTagReload(payload.resolve(entry.value())));
        }
    }
}
