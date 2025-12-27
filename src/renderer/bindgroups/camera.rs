use glam::{Mat3, Mat4, Quat, Vec3, Vec4};
use wgpu::util::DeviceExt as _;

use crate::{render_snapshot::CameraSnapshot, renderer::wgpu_context};

pub struct CameraMatrices {
    pub view_proj: [[f32; 4]; 4],
    pub position: [f32; 3],
    pub inverse_view_proj_rot: [[f32; 4]; 4],
}

pub struct CameraBinding {
    view_proj_buffer: wgpu::Buffer,
    position_buffer: wgpu::Buffer,
    inverse_view_proj_rot_buffer: wgpu::Buffer,
    pub bind_group: wgpu::BindGroup,
}
impl CameraBinding {
    pub fn new(
        camera: &CameraSnapshot,
        device: &wgpu::Device,
        surface_config: &wgpu::SurfaceConfiguration,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        let view_proj = matrices.view_proj;
        let position = matrices.position;
        let inverse_view_proj_rot = matrices.inverse_view_proj_rot;
        let view_proj_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("View Projection Buffer"),
            contents: bytemuck::cast_slice(&view_proj),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let position_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Position Buffer"),
            contents: bytemuck::cast_slice(&position),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let inverse_view_proj_rot_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Inverse View Projection Buffer"),
                contents: bytemuck::cast_slice(&inverse_view_proj_rot),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: view_proj_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: position_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: inverse_view_proj_rot_buffer.as_entire_binding(),
                },
            ],
            label: Some("Camera Bind Group"),
        });

        CameraBinding {
            bind_group,
            view_proj_buffer,
            position_buffer,
            inverse_view_proj_rot_buffer,
        }
    }

    pub fn desc() -> wgpu::BindGroupLayoutDescriptor<'static> {
        wgpu::BindGroupLayoutDescriptor {
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
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
            label: Some("Camera Bind Group Layout"),
        }
    }

    fn camera_to_matrices(
        cam: &CameraSnapshot,
        surface_config: &wgpu::SurfaceConfiguration,
    ) -> CameraMatrices {
        let rot_inv = cam.rotation.conjugate();
        let view = Mat4::from_rotation_translation(rot_inv, -(rot_inv * cam.position));
        let aspect = if surface_config.height > 0 {
            surface_config.width as f32 / surface_config.height as f32
        } else {
            16.0 / 9.0
        };
        let proj = Mat4::perspective_rh(cam.fovy, aspect, cam.znear, cam.zfar);
        let view_proj: Mat4 = wgpu_context::OPENGL_TO_WGPU_MATRIX * proj * view;
        let m3 = Mat3::from_mat4(view_proj).inverse();
        let inverse_view_proj_rot = Mat4::from_cols(
            Vec4::new(m3.x_axis.x, m3.x_axis.y, m3.x_axis.z, 0.0),
            Vec4::new(m3.y_axis.x, m3.y_axis.y, m3.y_axis.z, 0.0),
            Vec4::new(m3.z_axis.x, m3.z_axis.y, m3.z_axis.z, 0.0),
            Vec4::ZERO,
        );

        CameraMatrices {
            view_proj: view_proj.to_cols_array_2d(),
            position: cam.position.to_array(),
            inverse_view_proj_rot: inverse_view_proj_rot.to_cols_array_2d(),
        }
    }

    pub fn update(
        &self,
        camera: &CameraSnapshot,
        queue: &wgpu::Queue,
        surface_config: &wgpu::SurfaceConfiguration,
    ) {
        let matrices = CameraBinding::camera_to_matrices(camera, surface_config);
        queue.write_buffer(
            &self.view_proj_buffer,
            0,
            bytemuck::cast_slice(&matrices.view_proj),
        );
        queue.write_buffer(
            &self.position_buffer,
            0,
            bytemuck::cast_slice(&matrices.position),
        );
        queue.write_buffer(
            &self.inverse_view_proj_rot_buffer,
            0,
            bytemuck::cast_slice(&matrices.inverse_view_proj_rot),
        );
    }
}
