use cgmath::{EuclideanSpace, Point3, Rotation, Rotation3, SquareMatrix, Vector4};
use wgpu::{BindGroup, BindGroupLayout, VertexBufferLayout};
use winit::window::Window;
use wgpu::util::DeviceExt;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;
use pollster::FutureExt as _;

use crate::glb::{get_accessor_component_count, get_accessor_component_size, DataBuffer, GLBObject};

#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

pub struct WgpuContext<'surface_lifetime> {
    window: Arc<Window>,
    surface: wgpu::Surface<'surface_lifetime>,
    surface_config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl WgpuContext<'_> {
    pub async fn new(window: Arc<Window>) -> Self {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false
            }
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: None,
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
            },
            None,
        ).await.unwrap();

        // device.push_error_scope(wgpu::ErrorFilter::Validation);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter()
            .copied()
            .filter(|f| f.is_srgb())
            .next()
            .unwrap_or(surface_caps.formats[0]);
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2
        };
        surface.configure(&device, &surface_config);

        Self {
            window,
            surface,
            device,
            queue,
            surface_config,
        }
    }
}

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

struct CameraBindGroups {
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

pub struct Renderer {
    wgpu_context: WgpuContext<'static>,
    render_pipeline: wgpu::RenderPipeline,
    camera: Camera,
    camera_bind_groups: CameraBindGroups,
    vertex_index_buffer: wgpu::Buffer,
    vertex_index_count: u32,
    vertex_position_buffer: wgpu::Buffer,
    vertex_normal_buffer: wgpu::Buffer,
    vertex_buffer_layouts: [VertexBufferLayout<'static>; 2],
}

impl Renderer {
    pub async fn new(window: Arc<Window>, glb_object: &GLBObject) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let camera = Camera::new(&wgpu_context);
        let camera_bind_groups = CameraBindGroups::new(&camera, &wgpu_context);

        let primitive = &glb_object.json_chunk.chunk_data.meshes.first().unwrap().primitives.first().unwrap();

        let vertex_position_accessor_index = primitive.attributes.position as usize;
        let vertex_normal_accessor_index = primitive.attributes.position as usize;
        let vertex_index_accessor_index = primitive.indices as usize;

        let vertex_position_accessor = &glb_object.json_chunk.chunk_data.accessors[vertex_position_accessor_index];
        let vertex_normal_accessor = &glb_object.json_chunk.chunk_data.accessors[vertex_normal_accessor_index];
        let vertex_index_accessor = &glb_object.json_chunk.chunk_data.accessors[vertex_index_accessor_index];

        let vertex_position_accessor_buffer_view = &glb_object.json_chunk.chunk_data.buffer_views[vertex_position_accessor.buffer_view as usize];
        let vertex_normal_accessor_buffer_view = &glb_object.json_chunk.chunk_data.buffer_views[vertex_normal_accessor.buffer_view as usize];
        let vertex_index_accessor_buffer_view = &glb_object.json_chunk.chunk_data.buffer_views[vertex_index_accessor.buffer_view as usize];

        let vertex_position_start_offset = (vertex_position_accessor_buffer_view.byte_offset.unwrap_or(0u32) + vertex_position_accessor.byte_offset.unwrap_or(0u32)) as usize;
        let vertex_normal_start_offset = (vertex_normal_accessor_buffer_view.byte_offset.unwrap_or(0u32) + vertex_normal_accessor.byte_offset.unwrap_or(0u32)) as usize;
        let vertex_index_start_offset = (vertex_index_accessor_buffer_view.byte_offset.unwrap_or(0u32) + vertex_index_accessor.byte_offset.unwrap_or(0u32)) as usize;
        let vertex_position_end_offset = vertex_position_accessor_buffer_view.byte_offset.unwrap_or(0u32) as usize + vertex_position_accessor_buffer_view.byte_length as usize;
        let vertex_normal_end_offset = vertex_normal_accessor_buffer_view.byte_offset.unwrap_or(0u32) as usize + vertex_normal_accessor_buffer_view.byte_length as usize;
        let vertex_index_end_offset = vertex_index_accessor_buffer_view.byte_offset.unwrap_or(0u32) as usize + vertex_index_accessor_buffer_view.byte_length as usize;

        let vertex_position_slice = &glb_object.binary_buffer[vertex_position_start_offset..vertex_position_end_offset];
        let vertex_normal_slice = &glb_object.binary_buffer[vertex_normal_start_offset..vertex_normal_end_offset];
        let vertex_index_slice = &glb_object.binary_buffer[vertex_index_start_offset..vertex_index_end_offset];

        let vertex_position_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: vertex_position_slice,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_normal_buffer = wgpu_context.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Vertex Buffer"),
            contents: vertex_normal_slice,
            usage: wgpu::BufferUsages::VERTEX,
        });
        let vertex_index_buffer = wgpu_context.device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: vertex_index_slice,
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        let vertex_index_count = vertex_index_accessor.count;

        let data_element_size = get_accessor_component_count(vertex_position_accessor) as u64 * get_accessor_component_size(vertex_position_accessor) as u64;

        let vertex_buffer_stride = match vertex_position_accessor_buffer_view.byte_stride {
            Some(stride) => if stride > 0 { stride as u64 } else { data_element_size },
            None => data_element_size,
        };

        let vertex_buffer_layouts = [
            VertexBufferLayout {
                array_stride: vertex_buffer_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 0,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                ]
            },
            VertexBufferLayout {
                array_stride: vertex_buffer_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                ]
            },
        ];

        let render_pipeline = Renderer::create_render_pipeline(
            &wgpu_context, &vertex_buffer_layouts,
            &camera_bind_groups.camera_bind_group_layout,
            &camera_bind_groups.view_invert_transpose_bind_group_layout
        );

        Self {
            wgpu_context,
            camera,
            camera_bind_groups,
            render_pipeline,
            vertex_index_buffer,
            vertex_position_buffer,
            vertex_normal_buffer,
            vertex_index_count,
            vertex_buffer_layouts,
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.wgpu_context.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self.wgpu_context.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.camera_bind_groups.camera_bind_group, &[]);
            render_pass.set_bind_group(1, &self.camera_bind_groups.view_invert_transpose_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_position_buffer.slice(..));
            render_pass.set_vertex_buffer(1, self.vertex_normal_buffer.slice(..));
            render_pass.set_index_buffer(self.vertex_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            render_pass.draw_indexed(0..self.vertex_index_count, 0, 0..1);
        }

        self.wgpu_context.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }

    pub fn reload_shaders(&mut self) {
        self.render_pipeline = Renderer::create_render_pipeline(
            &self.wgpu_context, &self.vertex_buffer_layouts,
            &self.camera_bind_groups.camera_bind_group_layout,
            &self.camera_bind_groups.view_invert_transpose_bind_group_layout
        );
        self.render().unwrap();
    }

    pub fn resize(&mut self, new_size: Option<winit::dpi::PhysicalSize<u32>>) {
        let new_size = new_size.unwrap_or(self.wgpu_context.window.inner_size());
        if new_size.width > 0 && new_size.height > 0 {
            self.wgpu_context.surface_config.width = new_size.width;
            self.wgpu_context.surface_config.height = new_size.height;
            self.wgpu_context.surface.configure(&self.wgpu_context.device, &self.wgpu_context.surface_config);
            self.camera.aspect = self.wgpu_context.surface_config.width as f32 / self.wgpu_context.surface_config.height as f32;
        }
    }

    // update_camera_bindings should be called after mutating the camera
    pub fn get_camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }

    pub fn update_camera_bindings(&mut self) {
        self.camera_bind_groups = CameraBindGroups::new(&self.camera, &self.wgpu_context)
    }

    fn read_shaders() -> io::Result<String> {
        let mut file = fs::File::open("src/shaders/shader.wgsl")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn read_fallback_shaders() -> io::Result<String> {
        let mut file = fs::File::open("src/shaders/fallback.wgsl")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn create_shader_module(wgpu_context: &WgpuContext) -> wgpu::ShaderModule {
        wgpu_context.device.push_error_scope(wgpu::ErrorFilter::Validation);

        {
            let source = wgpu::ShaderSource::Wgsl(Self::read_shaders().unwrap_or_else(|e| {
                println!("Error reading shader: {}", e);
                Self::read_fallback_shaders().unwrap()
            }).into());
            let shader = wgpu_context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source
            });

            // Poll the device to process any pending errors
            wgpu_context.device.poll(wgpu::Maintain::Wait);

            // Check for errors
            let error = wgpu_context.device.pop_error_scope().block_on();

            match error {
                Some(e) => Err(e),
                None => Ok(shader),
            }
        }.unwrap_or_else(|e| {
            println!("Shader compilation failed: {}", e);
            let source = wgpu::ShaderSource::Wgsl(Self::read_fallback_shaders().unwrap().into());
            wgpu_context.device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source
            })
        })
    }

    fn create_render_pipeline(wgpu_context: &WgpuContext, vertex_buffer_layouts: &[VertexBufferLayout], camera_bind_group_layout: &BindGroupLayout, view_invert_transpose_bind_group_layout: &BindGroupLayout) -> wgpu::RenderPipeline {
        let shader_module = Renderer::create_shader_module(wgpu_context);
        let render_pipeline_layout =
            wgpu_context.device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[camera_bind_group_layout, view_invert_transpose_bind_group_layout],
                push_constant_ranges: &[],
            });

        wgpu_context.device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: "vs_main",
                buffers: vertex_buffer_layouts.clone(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu_context.surface_config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLIP_CONTROL
                unclipped_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }
}

