use std::sync::{Mutex, mpsc};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::GpuDeviceSelector;
use wgpu::util::DeviceExt;

use super::{CHUNK_DAMPENING_LEN, LightDampeningNeighborhood};

const WORKGROUP_SIZE: u32 = 256;
const FAST_SHADER: &str = r#"
struct Params {
    center_index: u32,
};

@group(0) @binding(0)
var<storage, read> dampening: array<u32>;
@group(0) @binding(1)
var<storage, read_write> sky: array<atomic<u32>>;
@group(0) @binding(2)
var<uniform> params: Params;

fn chunk_index(offset_x: i32, offset_z: i32) -> i32 {
    if offset_x < -1 || offset_x > 1 || offset_z < -1 || offset_z > 1 {
        return -1;
    }
    return (offset_z + 1) * 3 + offset_x + 1;
}

fn value_index(chunk: u32, x: u32, y: i32, z: u32) -> u32 {
    return chunk * 98304u + u32(y + 64) * 256u + z * 16u + x;
}

fn dampening_at(x: i32, y: i32, z: i32) -> u32 {
    if y < -64 || y > 319 {
        return 15u;
    }
    let chunk_x = x / 16 - select(0, 1, x < 0 && x % 16 != 0);
    let chunk_z = z / 16 - select(0, 1, z < 0 && z % 16 != 0);
    let chunk = chunk_index(chunk_x, chunk_z);
    if chunk < 0 {
        return 15u;
    }
    let local_x = ((x % 16) + 16) % 16;
    let local_z = ((z % 16) + 16) % 16;
    return dampening[value_index(u32(chunk), u32(local_x), y, u32(local_z))];
}

fn center_index(x: u32, y: i32, z: u32) -> u32 {
    return value_index(params.center_index, x, y, z);
}

fn seed_column(x: u32, z: u32) {
    var light = 15u;
    for (var y = 319; y >= -64; y = y - 1) {
        let idx = center_index(x, y, z);
        atomicMax(&sky[idx], light);
        let loss = min(dampening[idx], 15u);
        light = select(light - loss, 0u, light <= loss);
        if y == -64 {
            break;
        }
    }
}

fn source_column_light(source_x: i32, y: i32, source_z: i32) -> u32 {
    var light = 15u;
    for (var scan_y = 319; scan_y >= y; scan_y = scan_y - 1) {
        let loss = min(dampening_at(source_x, scan_y, source_z), 15u);
        if scan_y == y {
            return light;
        }
        light = select(light - loss, 0u, light <= loss);
        if scan_y == -64 {
            break;
        }
    }
    return light;
}

fn seed_border(x: u32, z: u32, source_x: i32, source_z: i32) {
    for (var y = -64; y <= 319; y = y + 1) {
        let source_light = source_column_light(source_x, y, source_z);
        if source_light > 0u {
            let idx = center_index(x, y, z);
            let loss = max(min(dampening[idx], 15u), 1u);
            let candidate = select(source_light - loss, 0u, source_light <= loss);
            atomicMax(&sky[idx], candidate);
        }
    }
}

fn spread_from(x: u32, y: i32, z: u32, nx: u32, ny: i32, nz: u32) {
    let source = atomicLoad(&sky[center_index(x, y, z)]);
    if source <= 1u {
        return;
    }
    let target_idx = center_index(nx, ny, nz);
    let loss = max(min(dampening[target_idx], 15u), 1u);
    let candidate = select(source - loss, 0u, source <= loss);
    atomicMax(&sky[target_idx], candidate);
}

fn spread_cell(index: u32) {
    let x = index % 16u;
    let z = (index / 16u) % 16u;
    let y = i32(index / 256u) - 64;
    if x > 0u {
        spread_from(x, y, z, x - 1u, y, z);
    }
    if x < 15u {
        spread_from(x, y, z, x + 1u, y, z);
    }
    if z > 0u {
        spread_from(x, y, z, x, y, z - 1u);
    }
    if z < 15u {
        spread_from(x, y, z, x, y, z + 1u);
    }
    if y > -64 {
        spread_from(x, y, z, x, y - 1, z);
    }
    if y < 319 {
        spread_from(x, y, z, x, y + 1, z);
    }
}

@compute @workgroup_size(256)
fn seed(@builtin(global_invocation_id) id: vec3<u32>) {
    let column = id.x;
    if column >= 256u {
        return;
    }
    let x = column % 16u;
    let z = column / 16u;
    seed_column(x, z);
}

@compute @workgroup_size(256)
fn border(@builtin(global_invocation_id) id: vec3<u32>) {
    let line = id.x;
    if line >= 16u {
        return;
    }
    seed_border(0u, line, -1, i32(line));
    seed_border(15u, line, 16, i32(line));
    seed_border(line, 0u, i32(line), -1);
    seed_border(line, 15u, i32(line), 16);
}

@compute @workgroup_size(256)
fn spread(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x >= 98304u {
        return;
    }
    spread_cell(id.x);
}
"#;

#[derive(Debug)]
pub struct GpuLightEngine {
    inner: Mutex<GpuLightInner>,
}

#[derive(Debug)]
struct GpuLightInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    seed_pipeline: wgpu::ComputePipeline,
    border_pipeline: wgpu::ComputePipeline,
    spread_pipeline: wgpu::ComputePipeline,
    dampening_buffer: wgpu::Buffer,
    sky_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    _params_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    center_index: u32,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    center_index: u32,
}

impl GpuLightEngine {
    pub fn new(selector: &GpuDeviceSelector) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = select_adapter(&instance, selector)?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qexed-light-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("request GPU device")?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("qexed-fast-light-shader"),
            source: wgpu::ShaderSource::Wgsl(FAST_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qexed-fast-light-bind-group-layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("qexed-fast-light-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let seed_pipeline = compute_pipeline(&device, &pipeline_layout, &shader, "seed");
        let border_pipeline = compute_pipeline(&device, &pipeline_layout, &shader, "border");
        let spread_pipeline = compute_pipeline(&device, &pipeline_layout, &shader, "spread");
        let center_index =
            LightDampeningNeighborhood::chunk_index(0, 0).expect("center chunk index") as u32;
        let dampening_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-light-dampening"),
            size: neighbourhood_light_buffer_size(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let sky_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-light-sky"),
            size: neighbourhood_light_buffer_size(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-light-readback"),
            size: chunk_light_buffer_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("qexed-light-params"),
            contents: bytemuck::bytes_of(&Params { center_index }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("qexed-fast-light-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: dampening_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: sky_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        log::info!(
            "GPU 光照已启用: device={} type={:?} backend={:?}",
            info.name,
            info.device_type,
            info.backend
        );

        Ok(Self {
            inner: Mutex::new(GpuLightInner {
                device,
                queue,
                seed_pipeline,
                border_pipeline,
                spread_pipeline,
                dampening_buffer,
                sky_buffer,
                readback_buffer,
                _params_buffer: params_buffer,
                bind_group,
                center_index,
            }),
        })
    }

    pub fn fast_sky_light(&self, neighbourhood: &LightDampeningNeighborhood) -> Result<Vec<u8>> {
        let mut inner = self.inner.lock().expect("GPU light engine poisoned");
        inner.fast_sky_light(neighbourhood)
    }
}

impl GpuLightInner {
    fn fast_sky_light(&mut self, neighbourhood: &LightDampeningNeighborhood) -> Result<Vec<u8>> {
        let dampening = flatten_neighbourhood(neighbourhood)?;
        self.queue
            .write_buffer(&self.dampening_buffer, 0, bytemuck::cast_slice(&dampening));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("qexed-fast-light-encoder"),
            });
        encoder.clear_buffer(&self.sky_buffer, 0, None);
        self.dispatch(&mut encoder, &self.bind_group, &self.seed_pipeline, 256);
        self.dispatch(&mut encoder, &self.bind_group, &self.border_pipeline, 16);
        for _ in 0..30 {
            self.dispatch(
                &mut encoder,
                &self.bind_group,
                &self.spread_pipeline,
                CHUNK_DAMPENING_LEN,
            );
        }
        let center_offset = self.center_index as u64
            * CHUNK_DAMPENING_LEN as u64
            * std::mem::size_of::<u32>() as u64;
        encoder.copy_buffer_to_buffer(
            &self.sky_buffer,
            center_offset,
            &self.readback_buffer,
            0,
            self.readback_buffer.size(),
        );
        let submission = self.queue.submit([encoder.finish()]);
        self.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: None,
            })
            .context("wait for GPU light work")?;

        let slice = self.readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("poll GPU light readback")?;
        receiver
            .recv()
            .context("receive GPU light readback result")?
            .context("map GPU light readback")?;
        let mapped = slice.get_mapped_range();
        let values = bytemuck::cast_slice::<u8, u32>(&mapped)
            .iter()
            .map(|value| (*value).min(15) as u8)
            .collect::<Vec<_>>();
        drop(mapped);
        self.readback_buffer.unmap();

        Ok(values)
    }

    fn dispatch(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        bind_group: &wgpu::BindGroup,
        pipeline: &wgpu::ComputePipeline,
        item_count: usize,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("qexed-fast-light-pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.dispatch_workgroups(div_ceil(item_count as u32, WORKGROUP_SIZE), 1, 1);
    }
}

fn select_adapter(
    instance: &wgpu::Instance,
    selector: &GpuDeviceSelector,
) -> Result<wgpu::Adapter> {
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    if adapters.is_empty() {
        anyhow::bail!("no GPU adapters found");
    }

    let selected = match selector {
        GpuDeviceSelector::Index(index) => adapters.get(*index).cloned(),
        GpuDeviceSelector::Discrete => adapter_by_type(&adapters, wgpu::DeviceType::DiscreteGpu),
        GpuDeviceSelector::Integrated => {
            adapter_by_type(&adapters, wgpu::DeviceType::IntegratedGpu)
        }
        GpuDeviceSelector::Cpu => adapter_by_type(&adapters, wgpu::DeviceType::Cpu),
        GpuDeviceSelector::Auto => adapters
            .iter()
            .find(|adapter| adapter.get_info().device_type != wgpu::DeviceType::Cpu)
            .cloned()
            .or_else(|| adapters.first().cloned()),
    };

    selected.with_context(|| format!("no GPU adapter matches selector {selector:?}"))
}

fn adapter_by_type(
    adapters: &[wgpu::Adapter],
    device_type: wgpu::DeviceType,
) -> Option<wgpu::Adapter> {
    adapters
        .iter()
        .find(|adapter| adapter.get_info().device_type == device_type)
        .cloned()
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn compute_pipeline(
    device: &wgpu::Device,
    layout: &wgpu::PipelineLayout,
    shader: &wgpu::ShaderModule,
    entry_point: &'static str,
) -> wgpu::ComputePipeline {
    device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some(entry_point),
        layout: Some(layout),
        module: shader,
        entry_point: Some(entry_point),
        compilation_options: Default::default(),
        cache: None,
    })
}

fn flatten_neighbourhood(neighbourhood: &LightDampeningNeighborhood) -> Result<Vec<u32>> {
    let mut values = vec![15_u32; CHUNK_DAMPENING_LEN * 9];
    for (chunk_index, chunk) in neighbourhood.chunks().iter().enumerate() {
        let Some(chunk) = chunk else {
            continue;
        };
        if chunk.len() != CHUNK_DAMPENING_LEN {
            anyhow::bail!("invalid light dampening chunk length: {}", chunk.len());
        }
        let start = chunk_index * CHUNK_DAMPENING_LEN;
        for (target, source) in values[start..start + CHUNK_DAMPENING_LEN]
            .iter_mut()
            .zip(chunk.iter())
        {
            *target = u32::from((*source).min(15));
        }
    }
    Ok(values)
}

fn neighbourhood_light_buffer_size() -> u64 {
    (CHUNK_DAMPENING_LEN * 9 * std::mem::size_of::<u32>()) as u64
}

fn chunk_light_buffer_size() -> u64 {
    (CHUNK_DAMPENING_LEN * std::mem::size_of::<u32>()) as u64
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor)
}

#[cfg(test)]
mod tests {
    use super::super::{WORLD_MAX_Y, WORLD_MIN_Y, block_light_dampening_index};
    use super::*;

    #[test]
    fn flatten_neighbourhood_fills_missing_chunks_as_solid() {
        let center = vec![0; CHUNK_DAMPENING_LEN];
        let neighbourhood = LightDampeningNeighborhood::single(&center);
        let values = flatten_neighbourhood(&neighbourhood).unwrap();
        let center_index = LightDampeningNeighborhood::chunk_index(0, 0).unwrap();

        assert_eq!(values[center_index * CHUNK_DAMPENING_LEN], 0);
        assert_eq!(values[0], 15);
    }

    #[test]
    fn gpu_light_matches_cpu_fast_for_empty_chunk_when_available() {
        let Ok(engine) = GpuLightEngine::new(&GpuDeviceSelector::Auto) else {
            return;
        };
        let center = vec![0; CHUNK_DAMPENING_LEN];
        let neighbourhood = LightDampeningNeighborhood::single(&center);
        let gpu = engine.fast_sky_light(&neighbourhood).unwrap();

        assert_eq!(gpu.len(), CHUNK_DAMPENING_LEN);
        for x in 0..16 {
            for z in 0..16 {
                for y in WORLD_MIN_Y..=WORLD_MAX_Y {
                    assert_eq!(gpu[block_light_dampening_index(x, y, z)], 15);
                }
            }
        }
    }
}
