package cn.qexed.protocol;

import static org.junit.jupiter.api.Assertions.assertDoesNotThrow;
import static org.junit.jupiter.api.Assertions.assertThrows;

import io.netty.buffer.Unpooled;
import java.lang.reflect.Constructor;
import java.lang.reflect.InvocationTargetException;
import net.minecraft.network.FriendlyByteBuf;
import net.minecraft.network.protocol.game.ClientboundEntityEventPacket;
import org.junit.jupiter.api.Test;

final class ClientboundPacketSourceContractTest {
    @Test
    void entityEventPacketUsesFixedIntEntityId() {
        assertDoesNotThrow(() -> newEntityEventPacket(new FriendlyByteBuf(
                Unpooled.buffer().writeInt(300).writeByte(3))));

        assertThrows(IndexOutOfBoundsException.class, () -> newEntityEventPacket(new FriendlyByteBuf(
                Unpooled.wrappedBuffer(new byte[] {(byte) 0xac, 0x02, 0x03}))));
    }

    private static ClientboundEntityEventPacket newEntityEventPacket(FriendlyByteBuf buffer) throws Throwable {
        Constructor<ClientboundEntityEventPacket> constructor =
                ClientboundEntityEventPacket.class.getDeclaredConstructor(FriendlyByteBuf.class);
        constructor.setAccessible(true);
        try {
            return constructor.newInstance(buffer);
        } catch (InvocationTargetException error) {
            throw error.getCause();
        }
    }
}
