package cn.qexed.protocol;

import io.netty.buffer.ByteBuf;
import io.netty.buffer.ByteBufUtil;
import io.netty.buffer.Unpooled;
import net.minecraft.network.protocol.game.ServerboundAttackPacket;
import net.minecraft.network.protocol.game.ServerboundInteractPacket;
import net.minecraft.world.InteractionHand;
import net.minecraft.world.phys.Vec3;
import org.junit.jupiter.api.Test;

/** Mojang's own STREAM_CODEC as a byte-level oracle: golden vectors for Rust tests. */
final class GoldenBytesOracleTest {

    @Test
    void attackPacketGoldenBytes() {
        ByteBuf buf = Unpooled.buffer();
        ServerboundAttackPacket.STREAM_CODEC.encode(buf, new ServerboundAttackPacket(300));
        System.out.println("ATTACK(entityId=300)      = " + ByteBufUtil.hexDump(buf));
    }

    @Test
    void interactPacketGoldenBytes() {
        ByteBuf buf = Unpooled.buffer();
        ServerboundInteractPacket packet = new ServerboundInteractPacket(
                42, InteractionHand.MAIN_HAND, new Vec3(1.5, 64.0, 2.5), false);
        ServerboundInteractPacket.STREAM_CODEC.encode(buf, packet);
        System.out.println("INTERACT(id=42,mainHand) = " + ByteBufUtil.hexDump(buf));
    }
}