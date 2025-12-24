use rand::Rng;

pub fn pos_join_spawn_area() -> [i64; 3] {
    let mut rng = rand::thread_rng();
    [
        rng.gen_range(-10_000..=10_000),  // x: ±10k
        i64::MAX,// 最大坐标:我们会在世界生成后重新计算高度,如果不是这个值就视为已生成
        rng.gen_range(-10_000..=10_000),  // z: ±10k
    ]
}