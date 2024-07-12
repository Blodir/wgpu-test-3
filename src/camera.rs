use cgmath::{EuclideanSpace, Point3, Rotation3, SquareMatrix, Vector4};
use wgpu::{util::DeviceExt, BindGroup, BindGroupLayout};

use crate::wgpu_context::{WgpuContext, OPENGL_TO_WGPU_MATRIX};


pub struct Camera {
    pub eye: cgmath::Point3<f32>,
    pub target: cgmath::Point3<f32>,
    pub up: cgmath::Vector3<f32>,
    pub aspect: f32,
    pub fovy: f32,
    pub znear: f32,
    pub zfar: f32,
    pub rotation: cgmath::Quaternion<f32>,
}

impl Camera {
    pub fn new(wgpu_context: &WgpuContext) -> Self {
        let eye: cgmath::Point3<f32> = (0.0, 0.0, 2.0).into();
        let target: cgmath::Point3<f32> = (0.0, 0.0, 0.0).into();
        let up: cgmath::Vector3<f32> = cgmath::Vector3::unit_y();
        let aspect = wgpu_context.surface_config.width as f32 / wgpu_context.surface_config.height as f32;
        let fovy = 45.0f32;
        let znear = 0.1f32;
        let zfar = 100.0f32;
        let rotation = cgmath::Quaternion::from_angle_y(cgmath::Deg(0f32));

        Self {
            eye, target, up, aspect, fovy, znear, zfar, rotation
        }
    }
}

pub struct CameraBindGroups {
    pub camera_bind_group: BindGroup,
    pub camera_bind_group_layout: BindGroupLayout,
    pub view_invert_transpose_bind_group: BindGroup,
    pub view_invert_transpose_bind_group_layout: BindGroupLayout,
}

impl CameraBindGroups {
    pub fn new(camera: &Camera, wgpu_context: &WgpuContext) -> CameraBindGroups {
        let eye_rotated = cgmath::Matrix4::from(camera.rotation) * Vector4::new(camera.eye.x, camera.eye.y, camera.eye.z, 1.0);
        let view = cgmath::Matrix4::look_at_rh(Point3::from_vec(eye_rotated.truncate()), camera.target, camera.up);
        let proj = cgmath::perspective(cgmath::Deg(camera.fovy), camera.aspect, camera.znear, camera.zfar);
        let view_proj = OPENGL_TO_WGPU_MATRIX * proj * view;
        let view_proj_m: [[f32; 4]; 4] = view_proj.into();
        let mut view_invert_transpose = view.invert().unwrap();
        view_invert_transpose.transpose_self();
        let view_invert_transpose_m: [[f32; 4]; 4] = view_invert_transpose.into();

        let camera_buffer = wgpu_context.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[view_proj_m]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let view_invert_transpose_buffer = wgpu_context.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[view_invert_transpose_m]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        let camera_bind_group_layout = wgpu_context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("camera_bind_group_layout"),
        });

        let view_invert_transpose_bind_group_layout = wgpu_context.device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ],
            label: Some("view_invert_transpose_bind_group_layout"),
        });

        let camera_bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: camera_buffer.as_entire_binding(),
                }
            ],
            label: Some("camera_bind_group"),
        });

        let view_invert_transpose_bind_group = wgpu_context.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &view_invert_transpose_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_invert_transpose_buffer.as_entire_binding(),
                }
            ],
            label: Some("view_invert_transpose_bind_group"),
        });

        Self { camera_bind_group, camera_bind_group_layout, view_invert_transpose_bind_group, view_invert_transpose_bind_group_layout }
    }
}

