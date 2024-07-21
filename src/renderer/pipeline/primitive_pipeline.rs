use std::collections::BTreeSet;

use crate::renderer::{glb::{GLTFSceneRef, Primitive}, pipeline::pipeline::PipelineConfig};

use super::shader::ShaderCapability;

pub fn get_primitive_pipeline_config(scene: &GLTFSceneRef, primitive: &Primitive) -> PipelineConfig {
    let vertex_position_accessor_index = primitive.attributes.position as usize;
    let vertex_buffer_stride = scene.accessor_data[vertex_position_accessor_index].stride;

    let mut shader_capabilities = BTreeSet::new();
    let mut vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'static>> = vec![
        wgpu::VertexBufferLayout {
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
    ];
    if let Some(n) = primitive.attributes.normal {
        vertex_buffer_layouts.push(
            wgpu::VertexBufferLayout {
                array_stride: vertex_buffer_stride,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[
                    wgpu::VertexAttribute {
                        offset: 0,
                        shader_location: 1,
                        format: wgpu::VertexFormat::Float32x3,
                    },
                ]
            }
        );
        shader_capabilities.insert(ShaderCapability::Normals);
    }

    let bind_group_layouts: Vec<Vec<wgpu::BindGroupLayoutEntry>> = vec![];

    PipelineConfig {
        vertex_buffer_layouts,
        bind_group_layouts,
        shader_capabilities
    }
}

