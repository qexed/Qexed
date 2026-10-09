//! 调试：两次 set_block_state 的 NBT 差异定位。
use qexed_anvil::chunk::{self, BlockStateRef};

fn main() {
    let bytes = std::fs::read(".tmp/anvil_fixtures/etho.chunk").unwrap();
    let root = qexed_nbt::from_slice_lossy(&bytes).unwrap().1;
    let target = BlockStateRef::new("minecraft:gold_block");
    let once = chunk::set_block_state(&root, 2, 70, 2, &target,
        &BlockStateRef::new("minecraft:stone"), "minecraft:plains").unwrap();
    let twice = chunk::set_block_state(&once, 2, 70, 2, &target,
        &BlockStateRef::new("minecraft:stone"), "minecraft:plains").unwrap();

    let b1 = qexed_nbt::to_vec("", &once).unwrap();
    let b2 = qexed_nbt::to_vec("", &twice).unwrap();
    eprintln!("lens: {} vs {}", b1.len(), b2.len());
    for (i, (x, y)) in b1.iter().zip(b2.iter()).enumerate() {
        if x != y {
            eprintln!("first diff at byte {i}: {x:02X} vs {y:02X}");
            eprintln!("context once: {:?}", &b1[i.saturating_sub(8)..(i + 24).min(b1.len())]);
            eprintln!("context twice: {:?}", &b2[i.saturating_sub(8)..(i + 24).min(b2.len())]);
            break;
        }
    }
}