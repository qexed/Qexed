use qexed_packet::{PacketCodec, net_types::*};

#[qexed_packet_macros::packet(id = 0x8C)]
#[derive(Debug, Default, PartialEq, Clone)]
pub struct TrackedWaypoint {
    // TODO: net.minecraft...TrackedWaypointPacket$Operation -> VarInt 占位（0=ADD/track 1=REMOVE/untrack 2=UPDATE）
    pub operation: VarInt,
    // TODO: net.minecraft.world.waypoints.TrackedWaypoint -> 复杂类型暂用 VarInt 占位，待展开
    pub waypoint: VarInt,
}
