use cgmath::{EuclideanSpace, Point3, Rotation3, SquareMatrix, Vector4};
use wgpu::{BindGroup, BindGroupLayout, VertexBufferLayout};
use winit::window::Window;
use wgpu::util::DeviceExt;
use std::fs;
use std::io::{self, Read};
use std::sync::Arc;
use pollster::FutureExt as _;
use crate::camera::{Camera, CameraBindGroups};
use crate::wgpu_context::{WgpuContext, OPENGL_TO_WGPU_MATRIX};

use crate::glb::{get_accessor_component_count, get_accessor_component_size, GLBObject};

struct DepthTexture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
}

impl DepthTexture {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float; // 1.
    
    pub fn create_depth_texture(wgpu_context: &WgpuContext) -> Self {
        let size = wgpu::Extent3d {
            width: wgpu_context.surface_config.width,
            height: wgpu_context.surface_config.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = wgpu_context.device.create_texture(&desc);

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = wgpu_context.device.create_sampler(
            &wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::LessEqual),
                lod_min_clamp: 0.0,
                lod_max_clamp: 100.0,
                ..Default::default()
            }
        );

        Self { 
            texture, view, sampler
        }
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
    depth_texture: DepthTexture,
}

impl Renderer {
    pub async fn new(window: Arc<Window>, glb_object: &GLBObject) -> Self {
        let wgpu_context = WgpuContext::new(window).await;
        let camera = Camera::new(&wgpu_context);
        let camera_bind_groups = CameraBindGroups::new(&camera, &wgpu_context);

        let primitive = &glb_object.json_chunk.chunk_data.meshes.first().unwrap().primitives.first().unwrap();

        let vertex_position_accessor_index = primitive.attributes.position as usize;
        let vertex_normal_accessor_index = primitive.attributes.normal as usize;
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
            label: Some("Normal Buffer"),
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

        let depth_texture = DepthTexture::create_depth_texture(&wgpu_context);

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
            depth_texture,
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
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
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
            self.depth_texture = DepthTexture::create_depth_texture(&self.wgpu_context);
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
                buffers: vertex_buffer_layouts,
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: DepthTexture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        })
    }
}

