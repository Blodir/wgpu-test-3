use std::{collections::BTreeSet, mem};

use crate::renderer::{glb::{GLTFSceneRef, Primitive}, pipeline::pipeline::PipelineConfig};

use super::shader::ShaderCapability;

pub fn get_primitive_pipeline_config(scene: &GLTFSceneRef, primitive: &Primitive) -> PipelineConfig {
    let vertex_position_accessor_index = primitive.attributes.position as usize;
    let vertex_buffer_stride = scene.accessor_data[vertex_position_accessor_index].stride;

    let mut shader_capabilities = BTreeSet::new();
    let mut bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>> = vec![];
    let mut vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'static>> = vec![
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<[f32; 16]>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ]
        },
        wgpu::VertexBufferLayout {
            array_stride: vertex_buffer_stride,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x3,
                },
            ]
        },
    ];
    if let Some(_) = primitive.attributes.normal {
        vertex_buffer_layouts.push(
            wgpu::VertexBufferLayout {
                array_stride: vertex_buffer_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 5,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                ]
            }
        );
        shader_capabilities.insert(ShaderCapability::VertexNormals);
    }

    if let Some(_) = primitive.material {
        bind_group_layouts.push(
            vec![
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }
            ]
        );
        shader_capabilities.insert(ShaderCapability::Material);
    }

    PipelineConfig {
        vertex_buffer_layouts,
        bind_group_layouts,
        shader_capabilities
    }
}

