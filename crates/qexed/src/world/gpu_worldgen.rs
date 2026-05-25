use std::sync::{Mutex, mpsc};

use anyhow::{Context, Result};
use qexed_config::app::qexed::server::GpuDeviceSelector;
use wgpu::util::DeviceExt;

use super::{CHUNK_DAMPENING_LEN, gpu_light};

const WORKGROUP_SIZE: u32 = 256;
const COLUMN_COUNT: usize = 16 * 16;
const HEIGHTMAP_SHADER: &str = r#"
struct Params {
    height: u32,
};

@group(0) @binding(0)
var<storage, read> solid: array<u32>;
@group(0) @binding(1)
var<storage, read_write> heights: array<u32>;
@group(0) @binding(2)
var<uniform> params: Params;

@compute @workgroup_size(256)
fn first_available_height(@builtin(global_invocation_id) id: vec3<u32>) {
    let column = id.x;
    if column >= 256u {
        return;
    }

    let x = column % 16u;
    let z = column / 16u;
    var y = params.height;
    loop {
        if y == 0u {
            break;
        }
        y = y - 1u;
        let index = y * 256u + z * 16u + x;
        if solid[index] != 0u {
            heights[column] = y + 1u;
            return;
        }
    }

    heights[column] = 0u;
}
"#;

#[derive(Debug)]
pub(crate) struct GpuWorldgenEngine {
    inner: Mutex<GpuWorldgenInner>,
}

#[derive(Debug)]
struct GpuWorldgenInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    solid_buffer: wgpu::Buffer,
    height_buffer: wgpu::Buffer,
    readback_buffer: wgpu::Buffer,
    _params_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Params {
    height: u32,
}

impl GpuWorldgenEngine {
    pub(crate) fn new(selector: &GpuDeviceSelector, height: i32) -> Result<Self> {
        let height = u32::try_from(height).context("world height must be non-negative")?;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = gpu_light::select_adapter(&instance, selector)?;
        let info = adapter.get_info();
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("qexed-worldgen-device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..Default::default()
        }))
        .context("request GPU worldgen device")?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("qexed-worldgen-heightmap-shader"),
            source: wgpu::ShaderSource::Wgsl(HEIGHTMAP_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("qexed-worldgen-heightmap-bind-group-layout"),
            entries: &[
                gpu_light::storage_entry(0, true),
                gpu_light::storage_entry(1, false),
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
            label: Some("qexed-worldgen-heightmap-pipeline-layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = gpu_light::compute_pipeline(
            &device,
            &pipeline_layout,
            &shader,
            "first_available_height",
        );
        let solid_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-worldgen-solid-mask"),
            size: chunk_u32_buffer_size(),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let height_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-worldgen-heights"),
            size: heights_u32_buffer_size(),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("qexed-worldgen-height-readback"),
            size: heights_u32_buffer_size(),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let params_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("qexed-worldgen-params"),
            contents: bytemuck::bytes_of(&Params { height }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("qexed-worldgen-heightmap-bind-group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: solid_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: height_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        log::info!(
            "GPU 世界生成后处理已启用: device={} type={:?} backend={:?}",
            info.name,
            info.device_type,
            info.backend
        );

        Ok(Self {
            inner: Mutex::new(GpuWorldgenInner {
                device,
                queue,
                pipeline,
                solid_buffer,
                height_buffer,
                readback_buffer,
                _params_buffer: params_buffer,
                bind_group,
            }),
        })
    }

    pub(crate) fn first_available_heights(&self, solid_mask: &[u32]) -> Result<Vec<i32>> {
        self.inner
            .lock()
            .expect("GPU worldgen engine poisoned")
            .first_available_heights(solid_mask)
    }
}

impl GpuWorldgenInner {
    fn first_available_heights(&mut self, solid_mask: &[u32]) -> Result<Vec<i32>> {
        anyhow::ensure!(
            solid_mask.len() == CHUNK_DAMPENING_LEN,
            "solid mask length mismatch: expected {}, got {}",
            CHUNK_DAMPENING_LEN,
            solid_mask.len()
        );

        self.queue
            .write_buffer(&self.solid_buffer, 0, bytemuck::cast_slice(solid_mask));

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("qexed-worldgen-heightmap-encoder"),
            });
        encoder.clear_buffer(&self.height_buffer, 0, None);
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("qexed-worldgen-heightmap-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(div_ceil(COLUMN_COUNT as u32, WORKGROUP_SIZE), 1, 1);
        }
        encoder.copy_buffer_to_buffer(
            &self.height_buffer,
            0,
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
            .context("wait for GPU worldgen work")?;

        let slice = self.readback_buffer.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .context("poll GPU worldgen readback")?;
        receiver
            .recv()
            .context("receive GPU worldgen readback result")?
            .context("map GPU worldgen readback")?;
        let mapped = slice.get_mapped_range();
        let heights = bytemuck::cast_slice::<u8, u32>(&mapped)
            .iter()
            .map(|height| *height as i32)
            .collect::<Vec<_>>();
        drop(mapped);
        self.readback_buffer.unmap();

        Ok(heights)
    }
}

fn chunk_u32_buffer_size() -> u64 {
    (CHUNK_DAMPENING_LEN * std::mem::size_of::<u32>()) as u64
}

fn heights_u32_buffer_size() -> u64 {
    (COLUMN_COUNT * std::mem::size_of::<u32>()) as u64
}

fn div_ceil(value: u32, divisor: u32) -> u32 {
    value.div_ceil(divisor)
}

#[cfg(test)]
mod tests {
    use qexed_config::app::qexed::server::GpuDeviceSelector;

    use super::*;

    #[test]
    fn gpu_worldgen_heightmap_matches_cpu_when_available() {
        let Ok(engine) = GpuWorldgenEngine::new(&GpuDeviceSelector::Auto, 384) else {
            return;
        };
        let mut mask = vec![0_u32; CHUNK_DAMPENING_LEN];
        mask[(4 * 256) + (7 * 16) + 3] = 1;
        mask[(120 * 256) + (7 * 16) + 3] = 1;

        let heights = engine.first_available_heights(&mask).unwrap();

        assert_eq!(heights[7 * 16 + 3], 121);
    }
}
