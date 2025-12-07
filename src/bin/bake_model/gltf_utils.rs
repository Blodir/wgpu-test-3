use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;

pub fn transform_to_mat4(transform: gltf::scene::Transform) -> Mat4 {
    match transform {
        gltf::scene::Transform::Matrix { matrix } => Mat4::from_cols_array_2d(&matrix),
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => Mat4::from_scale_rotation_translation(
            Vec3::from(scale),
            Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
            Vec3::from(translation),
        ),
    }
}

pub fn accumulate_primitive_instances(
    node: &gltf::Node,
    transform: &Mat4,
    primitive_instances: &mut HashMap<(usize, usize), Vec<Mat4>>,
) {
    let t = transform * transform_to_mat4(node.transform());
    if let Some(mesh) = &node.mesh() {
        for primitive in mesh.primitives() {
            let arr = primitive_instances
                .entry((mesh.index(), primitive.index()))
                .or_insert_with(Vec::new);
            arr.push(t);
        }
    }
    for child in node.children() {
        accumulate_primitive_instances(&child, &t, primitive_instances);
    }
}

fn read_accessor_data(accessor: &gltf::Accessor, buffers: &[gltf::buffer::Data]) -> Vec<u8> {
    let view = accessor.view();
    let count = accessor.count();
    let elem_size = accessor.size();

    let mut data = vec![0u8; count * elem_size];

    if let Some(view) = view {
        let buffer = &buffers[view.buffer().index()].0;
        let stride = view.stride().unwrap_or(elem_size);
        let view_offset = view.offset();
        let accessor_offset = accessor.offset();

        for i in 0..count {
            let src = view_offset + accessor_offset + i * stride;
            let dst = i * elem_size;
            data[dst..dst + elem_size].copy_from_slice(&buffer[src..src + elem_size]);
        }
    } else {
        assert!(
            accessor.sparse().is_some(),
            "Accessor has no buffer view and is not sparse"
        );
    }

    if let Some(sparse) = accessor.sparse() {
        let indices_accessor = sparse.indices();
        let indices_view = indices_accessor.view();
        let indices_buffer = &buffers[indices_view.buffer().index()].0;
        let indices_offset = indices_view.offset() + indices_accessor.offset();
        let index_size = match indices_accessor.index_type() {
            gltf::accessor::sparse::IndexType::U8 => 1,
            gltf::accessor::sparse::IndexType::U16 => 2,
            gltf::accessor::sparse::IndexType::U32 => 4,
            _ => panic!("Unsupported index type for sparse accessor"),
        };
        let indices_stride = indices_view.stride().unwrap_or(index_size);

        let values_accessor = sparse.values();
        let values_view = values_accessor.view();
        let values_buffer = &buffers[values_view.buffer().index()].0;
        let values_offset = values_view.offset();
        let values_stride = values_view.stride().unwrap_or(elem_size);

        for i in 0..sparse.count() {
            let index = match indices_accessor.index_type() {
                gltf::accessor::sparse::IndexType::U8 => {
                    indices_buffer[indices_offset + i * indices_stride] as usize
                }
                gltf::accessor::sparse::IndexType::U16 => {
                    let start = indices_offset + i * indices_stride;
                    u16::from_le_bytes(indices_buffer[start..start + 2].try_into().unwrap())
                        as usize
                }
                gltf::accessor::sparse::IndexType::U32 => {
                    let start = indices_offset + i * indices_stride;
                    u32::from_le_bytes(indices_buffer[start..start + 4].try_into().unwrap())
                        as usize
                }
                _ => panic!("Unsupported sparse index type"),
            };

            let dst = index * elem_size;
            let src = values_offset + i * values_stride;
            data[dst..dst + elem_size].copy_from_slice(&values_buffer[src..src + elem_size]);
        }
    }

    data
}

pub fn readf32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<f32> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::F32);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Scalar);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![0f32; count];
    let stride = 4;

    for i in 0..count {
        let idx = i * stride;
        output[i] = bytemuck::cast::<[u8; 4], f32>(data[idx..idx + 4].try_into().unwrap());
    }

    output
}

pub fn read2f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 2]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::F32);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Vec2);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![[0f32, 0f32]; count];
    let stride = 8;

    for i in 0..count {
        let idx = i * stride;
        output[i] = [
            bytemuck::cast::<[u8; 4], f32>(data[idx..idx + 4].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 4..idx + 8].try_into().unwrap()),
        ];
    }

    output
}

pub fn read3f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 3]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::F32);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Vec3);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![[0f32, 0f32, 0f32]; count];
    let stride = 12;

    for i in 0..count {
        let idx = i * stride;
        output[i] = [
            bytemuck::cast::<[u8; 4], f32>(data[idx..idx + 4].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 4..idx + 8].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 8..idx + 12].try_into().unwrap()),
        ];
    }

    output
}

pub fn read4f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 4]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::F32);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Vec4);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![[0f32, 0f32, 0f32, 0f32]; count];
    let stride = 16;

    for i in 0..count {
        let idx = i * stride;
        output[i] = [
            bytemuck::cast::<[u8; 4], f32>(data[idx..idx + 4].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 4..idx + 8].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 8..idx + 12].try_into().unwrap()),
            bytemuck::cast::<[u8; 4], f32>(data[idx + 12..idx + 16].try_into().unwrap()),
        ];
    }

    output
}

pub fn read_mat4(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[[f32; 4]; 4]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::F32);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Mat4);

    let data = read_accessor_data(accessor, buffers);

    let count = accessor.count();
    let mut output = Vec::<[[f32; 4]; 4]>::with_capacity(count);
    let stride = 4 * 4 * 4;

    for i in 0..count {
        let idx = i * stride;

        let mut mat = [
            [0f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 0f32],
            [0f32, 0f32, 0f32, 0f32],
        ];
        for col in 0..4 {
            for row in 0..4 {
                let offset = idx + col * 16 + row * 4;
                mat[col][row] =
                    bytemuck::cast::<[u8; 4], f32>(data[offset..offset + 4].try_into().unwrap());
            }
        }
        output.push(mat);
    }

    output
}

pub fn read4u8(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[u8; 4]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::U8);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Vec4);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![[0u8, 0u8, 0u8, 0u8]; count];
    let stride = 4;

    for i in 0..count {
        let idx = i * stride;
        output[i] = [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]];
    }

    output
}

pub fn read4u16(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[u16; 4]> {
    assert_eq!(accessor.data_type(), gltf::accessor::DataType::U16);
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Vec4);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![[0u16, 0u16, 0u16, 0u16]; count];
    let stride = 8;

    for i in 0..count {
        let idx = i * stride;
        output[i] = [
            bytemuck::cast::<[u8; 2], u16>(data[idx..idx + 2].try_into().unwrap()),
            bytemuck::cast::<[u8; 2], u16>(data[idx + 2..idx + 4].try_into().unwrap()),
            bytemuck::cast::<[u8; 2], u16>(data[idx + 4..idx + 6].try_into().unwrap()),
            bytemuck::cast::<[u8; 2], u16>(data[idx + 6..idx + 8].try_into().unwrap()),
        ];
    }

    output
}

pub fn read_index_buffer(primitive: &gltf::Primitive, buffers: &Vec<gltf::buffer::Data>) -> Vec<u32> {
    if primitive.indices().is_none() {
        println!(
            "WARNING: primitive {} has no index buffer, so we generate it",
            primitive.index()
        );
        let position_accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        let count = position_accessor.count();
        let indices: Vec<u32> = (0..count as u32).collect();
        return bytemuck::cast_slice(&indices).to_vec();
    }

    let accessor = primitive.indices().unwrap();
    assert_eq!(accessor.dimensions(), gltf::accessor::Dimensions::Scalar);

    let data = read_accessor_data(&accessor, buffers);

    let count = accessor.count();
    let mut output = vec![0u32; count];
    let stride = match accessor.data_type() {
        gltf::accessor::DataType::U8 => 1,
        gltf::accessor::DataType::U16 => 2,
        gltf::accessor::DataType::U32 => 4,
        _ => panic!(),
    };

    for i in 0..count {
        let idx = i * stride;
        output[i] = match accessor.data_type() {
            gltf::accessor::DataType::U8 => data[idx] as u32,
            gltf::accessor::DataType::U16 => {
                bytemuck::cast::<[u8; 2], u16>(data[idx..idx + 2].try_into().unwrap()) as u32
            }
            gltf::accessor::DataType::U32 => {
                bytemuck::cast::<[u8; 4], u32>(data[idx..idx + 4].try_into().unwrap())
            }
            _ => panic!(),
        };
    }
    output
}

pub fn read_position_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 3]> {
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Positions)
        .expect("A primitive is missing the POSITION attribute")
        .1;
    read3f32(&accessor, buffers)
}

pub fn read_normal_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Option<Vec<[f32; 3]>> {
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Normals);
    if let Some(accessor) = accessor {
        Some(read3f32(&accessor.1, buffers))
    } else {
        None
    }
}

pub fn read_tangents_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Option<Vec<[f32; 4]>> {
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Tangents);
    if let Some(accessor) = accessor {
        Some(read4f32(&accessor.1, buffers))
    } else {
        None
    }
}

pub fn read_weights_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 4]> {
    if let Some(accessor) = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Weights(0))
        .map(|e| e.1)
    {
        read4f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0.0, 0.0, 0.0, 0.0]; accessor.count()]
    }
}

pub enum JointsBuffer {
    U8(Vec<[u8; 4]>),
    U16(Vec<[u16; 4]>),
}

pub fn read_joints_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> JointsBuffer {
    if let Some(accessor) = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Joints(0))
        .map(|e| e.1)
    {
        match accessor.data_type() {
            gltf::accessor::DataType::U8 => JointsBuffer::U8(read4u8(&accessor, buffers)),
            gltf::accessor::DataType::U16 => JointsBuffer::U16(read4u16(&accessor, buffers)),
            _ => panic!("Joints buffer has an unrecognized data type!"),
        }
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        JointsBuffer::U8(vec![[0u8, 0u8, 0u8, 0u8]; accessor.count()])
    }
}

pub fn read_normals_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    if let Some(normal_texture) = primitive.material().normal_texture() {
        let texcoord_idx = normal_texture.tex_coord();
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
            .expect("A primitive is missing the normal TEXCOORDS attribute")
            .1;
        read2f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0f32, 0f32]; accessor.count()]
    }
}

pub fn read_base_color_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    if let Some(base_color) = primitive
        .material()
        .pbr_metallic_roughness()
        .base_color_texture()
    {
        let texcoord_idx = base_color.tex_coord();
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
            .expect("A primitive is missing the base color TEXCOORDS attribute")
            .1;
        read2f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0f32, 0f32]; accessor.count()]
    }
}

pub fn read_metallic_roughness_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    if let Some(metallic_roughness) = primitive
        .material()
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
    {
        let texcoord_idx = metallic_roughness.tex_coord();
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
            .expect("A primitive is missing the metallic roughness TEXCOORDS attribute")
            .1;
        read2f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0f32, 0f32]; accessor.count()]
    }
}

pub fn read_occlusion_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    if let Some(texcoord_idx) = primitive
        .material()
        .occlusion_texture()
        .map(|e| e.tex_coord())
    {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
            .expect("A primitive is missing the occlusion TEXCOORDS attribute")
            .1;
        read2f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0.0, 0.0]; accessor.count()]
    }
}

pub fn read_emissive_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    if let Some(texcoord_idx) = primitive
        .material()
        .emissive_texture()
        .map(|e| e.tex_coord())
    {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
            .expect("A primitive is missing the emissive TEXCOORDS attribute")
            .1;
        read2f32(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0.0, 0.0]; accessor.count()]
    }
}

pub fn filename_without_extension(path: &str) -> Option<&str> {
    std::path::Path::new(path).file_stem().and_then(|s| s.to_str())
}

pub fn col_major_to_row_major(m: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for r in 0..4 {
        for c in 0..4 {
            out[r][c] = m[c][r];
        }
    }
    out
}
