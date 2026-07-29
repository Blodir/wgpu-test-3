mod camera;
mod lights;
mod sun;

pub(crate) use camera::CameraBuffers;
pub(crate) use lights::LightsBuffers;
pub(crate) use sun::{SunBuffers, SunShadowUniform};

use glam::Mat4;
use wgpu::util::DeviceExt as _;

use crate::host::{
    renderer::{
        rw_buffer::{RWBuffer, RWBufferOptions},
        world::{
            gpu_context::HbgiSettingsUniform,
            prepare::mesh::{BoneMat34, SkinnedInstance, StaticInstance},
        },
    },
    wgpu_context::WgpuContext,
};

const FULLSCREEN_QUAD_INDICES: &[u16] = &[0, 2, 1, 3, 2, 0];
pub(crate) const HBGI_SVGF_PASS_COUNT: usize = 5;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiReprojectUniform {
    prev_inverse_view_proj: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct HbgiSvgfUniform {
    params: [u32; 4],
}

pub(crate) struct Buffers {
    pub bones: RWBuffer,
    pub camera: CameraBuffers,
    pub hbgi_settings: wgpu::Buffer,
    pub hbgi_reproject_settings: wgpu::Buffer,
    pub hbgi_svgf_settings: [wgpu::Buffer; HBGI_SVGF_PASS_COUNT],
    pub fullscreen_quad_indices: wgpu::Buffer,
    pub skinned_instances: RWBuffer,
    pub static_instances: wgpu::Buffer,
    pub lights: LightsBuffers,
}

impl Buffers {
    pub fn new(wgpu_context: &WgpuContext, hbgi_settings: Option<&HbgiSettingsUniform>) -> Self {
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

        let hbgi_settings = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("HBGI Settings Buffer"),
            contents: bytemuck::bytes_of(
                &hbgi_settings
                    .copied()
                    .unwrap_or_else(HbgiSettingsUniform::default),
            ),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let hbgi_reproject_settings =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HBGI Reproject Settings Buffer"),
                contents: bytemuck::bytes_of(&HbgiReprojectUniform {
                    prev_inverse_view_proj: Mat4::IDENTITY.to_cols_array_2d(),
                }),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let hbgi_svgf_settings = std::array::from_fn(|pass_index| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("HBGI SVGF Settings Buffer"),
                contents: bytemuck::bytes_of(&HbgiSvgfUniform {
                    params: [1u32 << pass_index, u32::from(pass_index == 0), 0, 0],
                }),
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
            hbgi_settings,
            hbgi_reproject_settings,
            hbgi_svgf_settings,
            fullscreen_quad_indices,
            skinned_instances,
            static_instances,
            lights,
        }
    }

    pub fn update_hbgi_settings(&self, hbgi_settings: &HbgiSettingsUniform, queue: &wgpu::Queue) {
        queue.write_buffer(&self.hbgi_settings, 0, bytemuck::bytes_of(hbgi_settings));
    }

    pub fn update_hbgi_reproject_settings(
        &self,
        prev_inverse_view_proj: &Mat4,
        queue: &wgpu::Queue,
    ) {
        let uniform = HbgiReprojectUniform {
            prev_inverse_view_proj: prev_inverse_view_proj.to_cols_array_2d(),
        };
        queue.write_buffer(
            &self.hbgi_reproject_settings,
            0,
            bytemuck::bytes_of(&uniform),
        );
    }
}
