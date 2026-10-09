mod camera;
mod lights;
mod sun;

pub(crate) use camera::CameraBuffers;
pub(crate) use lights::LightsBuffers;
pub(crate) use sun::{SunBuffers, SunShadowUniform};

use wgpu::util::DeviceExt as _;

use crate::host::{
    renderer::{
        rw_buffer::{RWBuffer, RWBufferOptions},
        world::{
            gpu_context::SsrtSettingsUniform,
            prepare::mesh::{BoneMat34, SkinnedInstance, StaticInstance},
        },
    },
    wgpu_context::WgpuContext,
};

const FULLSCREEN_QUAD_INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];

pub(crate) struct Buffers {
    pub bones: RWBuffer,
    pub camera: CameraBuffers,
    pub ssrt_settings: wgpu::Buffer,
    pub a_trous_settings: [wgpu::Buffer; super::textures::SSRT_A_TROUS_PASS_COUNT],
    pub fullscreen_quad_indices: wgpu::Buffer,
    pub skinned_instances: RWBuffer,
    pub static_instances: wgpu::Buffer,
    pub lights: LightsBuffers,
}

impl Buffers {
    pub fn new(wgpu_context: &WgpuContext, ssrt_settings: &SsrtSettingsUniform) -> Self {
        let device = &wgpu_context.device;
        let storage_usage = wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST;

        let mut bones = RWBuffer::new(
            RWBufferOptions {
                label: Some("Bones SSBO".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        let initial_bones = vec![BoneMat34::default(); 2048];
        let initial_bones_bytes = bytemuck::cast_slice(initial_bones.as_slice());
        bones.write(initial_bones_bytes, wgpu_context);
        bones.swap();
        bones.write(initial_bones_bytes, wgpu_context);
        bones.swap();

        let camera = CameraBuffers::new(device);

        let ssrt_settings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("SSRT Settings Buffer"),
            contents: bytemuck::bytes_of(ssrt_settings),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let a_trous_settings = std::array::from_fn(|pass_index| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("SSRT A-Trous Settings Buffer"),
                contents: bytemuck::cast_slice(&[1u32 << pass_index, 0, 0, 0]),
                usage: wgpu::BufferUsages::UNIFORM,
            })
        });
        let fullscreen_quad_indices =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Fullscreen Quad Index Buffer"),
                contents: bytemuck::cast_slice(FULLSCREEN_QUAD_INDICES),
                usage: wgpu::BufferUsages::INDEX,
            });

        let mut skinned_instances = RWBuffer::new(
            RWBufferOptions {
                label: Some("Skinned Instance Buffer".to_string()),
                usage: storage_usage,
            },
            wgpu_context,
        );
        let initial_skinned_instances = [SkinnedInstance::default()];
        let initial_skinned_instance_bytes = bytemuck::cast_slice(&initial_skinned_instances);
        skinned_instances.write(initial_skinned_instance_bytes, wgpu_context);
        skinned_instances.swap();
        skinned_instances.write(initial_skinned_instance_bytes, wgpu_context);
        skinned_instances.swap();

        let static_instances = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance buffer"),
            contents: bytemuck::cast_slice(&[StaticInstance::default()]),
            usage: storage_usage,
        });

        let lights = LightsBuffers::new(device);

        Self {
            bones,
            camera,
            ssrt_settings,
            a_trous_settings,
            fullscreen_quad_indices,
            skinned_instances,
            static_instances,
            lights,
        }
    }

    pub fn update_ssrt_settings(&self, settings: &SsrtSettingsUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.ssrt_settings, 0, bytemuck::bytes_of(settings));
    }
}
