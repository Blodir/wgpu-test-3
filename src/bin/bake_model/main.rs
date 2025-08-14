use cgmath::{Matrix4, SquareMatrix};
use cgmath::{One, Quaternion, Rotation3, Vector3};
use gltf::{Document, Gltf, Primitive};
use std::path::Path;
use std::{collections::HashMap, env, fs::File, io::Write};
use wgpu_test_3::render_engine::pipelines::model::vertex::Vertex;
use wgpu_test_3::render_engine::render_resources::dds::{create_dds, gltf_img_to_dxgi_format};
use wgpu_test_3::render_engine::render_resources::modelfile;

fn transform_to_matrix4(transform: gltf::scene::Transform) -> Matrix4<f32> {
    match transform {
        gltf::scene::Transform::Matrix { matrix } => {
            // glTF uses column-major 4x4 matrices; cgmath does too
            Matrix4::from(matrix)
        }
        gltf::scene::Transform::Decomposed {
            translation,
            rotation,
            scale,
        } => {
            let t = Matrix4::from_translation(Vector3::from(translation));
            let r = Matrix4::from(Quaternion::new(
                rotation[3],
                rotation[0],
                rotation[1],
                rotation[2],
            ));
            let s = Matrix4::from_nonuniform_scale(scale[0], scale[1], scale[2]);
            t * r * s
        }
    }
}

fn accumulate_primitive_instances(
    node: &gltf::Node,
    transform: &Matrix4<f32>,
    primitive_instances: &mut HashMap<(usize, usize), Vec<Matrix4<f32>>>,
) {
    let t = transform * transform_to_matrix4(node.transform());
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
    let elem_size = accessor.size(); // Total byte size of one element (e.g., a Vec3<f32> = 12)

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
        // No buffer view: all data must be provided via sparse
        assert!(
            accessor.sparse().is_some(),
            "Accessor has no buffer view and is not sparse"
        );
    }

    // Apply sparse if present
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

fn read2f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 2]> {
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

fn read3f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 3]> {
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

fn read4f32(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[f32; 4]> {
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

fn read4u8(accessor: &gltf::Accessor, buffers: &Vec<gltf::buffer::Data>) -> Vec<[u8; 4]> {
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

fn read_index_buffer(primitive: &gltf::Primitive, buffers: &Vec<gltf::buffer::Data>) -> Vec<u8> {
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
    let bytes: Vec<u8> = bytemuck::cast_slice(&output).to_vec();
    bytes
}

fn read_position_buffer(
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

fn read_normal_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 3]> {
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Normals)
        .expect("A primitive is missing the NORMALS attribute")
        .1;
    read3f32(&accessor, buffers)
}

fn read_tangents_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 4]> {
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Tangents)
        .expect("A primitive is missing the TANGENTS attribute")
        .1;
    read4f32(&accessor, buffers)
}

fn read_weights_buffer(
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

fn read_joints_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[u8; 4]> {
    if let Some(accessor) = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::Joints(0))
        .map(|e| e.1)
    {
        read4u8(&accessor, buffers)
    } else {
        let accessor = primitive
            .attributes()
            .find(|(s, _)| *s == gltf::Semantic::Positions)
            .expect("A primitive is missing the POSITION attribute")
            .1;
        vec![[0, 0, 0, 0]; accessor.count()]
    }
}

fn read_normals_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    let texcoord_idx = primitive
        .material()
        .normal_texture()
        .expect("A primitive is missing the normal texture")
        .tex_coord();
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
        .expect("A primitive is missing the normal TEXCOORDS attribute")
        .1;
    read2f32(&accessor, buffers)
}

fn read_base_color_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    let texcoord_idx = primitive
        .material()
        .pbr_metallic_roughness()
        .base_color_texture()
        .expect("A primitive is missing base color texture")
        .tex_coord();
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
        .expect("A primitive is missing the base color TEXCOORDS attribute")
        .1;
    read2f32(&accessor, buffers)
}

fn read_metallic_roughness_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    let texcoord_idx = primitive
        .material()
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
        .expect("A primitive is missing metallic roughness texture")
        .tex_coord();
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
        .expect("A primitive is missing the metallic roughness TEXCOORDS attribute")
        .1;
    read2f32(&accessor, buffers)
}

fn read_occlusion_texcoord_buffer(
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

fn read_emissive_texcoord_buffer(
    primitive: &gltf::Primitive,
    buffers: &Vec<gltf::buffer::Data>,
) -> Vec<[f32; 2]> {
    let texcoord_idx = primitive
        .material()
        .emissive_texture()
        .expect("A primitive is missing emissive texture")
        .tex_coord();
    let accessor = primitive
        .attributes()
        .find(|(s, _)| *s == gltf::Semantic::TexCoords(texcoord_idx))
        .expect("A primitive is missing the emissive TEXCOORDS attribute")
        .1;
    read2f32(&accessor, buffers)
}

fn resolve_uri_to_path(uri: &str, model_name: &str, tex_name: &str) -> String {
    if uri.starts_with("data:") {
        // Embedded base64 image → map to local DDS path
        format!("assets/local/{}/{}.dds", model_name, tex_name)
    } else if uri.starts_with("http://") || uri.starts_with("https://") {
        // URL → convert to flat identifier
        let id = uri
            .trim_start_matches("http://")
            .trim_start_matches("https://")
            .replace(&['/', ':', '?', '=', '&', '%'][..], "_");
        format!("assets/remote/{}", id)
    } else {
        // Treat as relative/absolute path
        format!("assets/shared/{}", uri.to_string())
    }
}

fn ensure_parent_dir_exists(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn convert_rgb8_to_rgba8(image: &gltf::image::Data) -> Vec<u8> {
    assert_eq!(image.format, gltf::image::Format::R8G8B8);
    let rgb = &image.pixels;
    let mut rgba = Vec::with_capacity((rgb.len() / 3) * 4);

    for chunk in rgb.chunks_exact(3) {
        rgba.push(chunk[0]); // R
        rgba.push(chunk[1]); // G
        rgba.push(chunk[2]); // B
        rgba.push(255); // A
    }

    rgba
}

fn export_image_as_dds(
    image: &gltf::image::Data,
    path: &Path,
    srgb: bool,
    alpha_mode: gltf::material::AlphaMode,
) -> std::io::Result<()> {
    // create_dds expects RGBA
    let converted = match image.format {
        gltf::image::Format::R8G8B8 => gltf::image::Data {
            pixels: convert_rgb8_to_rgba8(image),
            format: gltf::image::Format::R8G8B8A8,
            width: image.width,
            height: image.height,
        },
        _ => image.clone(),
    };
    let format = gltf_img_to_dxgi_format(&converted, srgb);
    let dds = create_dds(
        vec![converted.pixels.clone()],
        &format,
        converted.width,
        converted.height,
        1,
        1,
        false,
        match alpha_mode {
            gltf::material::AlphaMode::Opaque => ddsfile::AlphaMode::Opaque,
            gltf::material::AlphaMode::Mask => ddsfile::AlphaMode::Straight,
            gltf::material::AlphaMode::Blend => ddsfile::AlphaMode::Straight,
        },
    );
    ensure_parent_dir_exists(path)?;
    let mut file = File::create(path)?;
    dds.write(&mut file).expect("Failed to write DDS");
    Ok(())
}

fn bake_placeholder_texture(
    data: gltf::image::Data,
    srgb: bool,
    tex_name: &str,
) -> modelfile::SampledTexture {
    let tex_output_path = format!("assets/shared/{}.placeholder.dds", tex_name);

    export_image_as_dds(
        &data,
        &Path::new(&tex_output_path),
        srgb,
        gltf::material::AlphaMode::Opaque,
    )
    .expect(&format!("Failed to write texture {}", tex_output_path));

    let sampler = modelfile::Sampler {
        mag_filter: modelfile::FilterMode::Linear,
        min_filter: modelfile::FilterMode::Linear,
        mipmap_filter: modelfile::MipmapFilterMode::None,
        wrap_u: modelfile::WrapMode::Repeat,
        wrap_v: modelfile::WrapMode::Repeat,
        wrap_w: modelfile::WrapMode::Repeat,
    };

    modelfile::SampledTexture {
        source: tex_output_path,
        sampler,
    }
}

fn bake_texture(
    texture: &gltf::texture::Texture,
    images: &Vec<gltf::image::Data>,
    srgb: bool,
    alpha_mode: gltf::material::AlphaMode,
    model_name: &str,
    tex_name: &str,
) -> modelfile::SampledTexture {
    let data = &images[texture.source().index()];
    let tex_output_path = match texture.source().source() {
        gltf::image::Source::View { view, mime_type } => {
            format!("assets/local/{}/{}.dds", model_name, tex_name)
        }
        gltf::image::Source::Uri { uri, mime_type } => {
            resolve_uri_to_path(uri, model_name, tex_name)
        }
    };

    export_image_as_dds(data, &Path::new(&tex_output_path), srgb, alpha_mode)
        .expect(&format!("Failed to write texture {}", tex_output_path));

    let gltf_sampler = texture.sampler();

    let mag_filter = match gltf_sampler
        .mag_filter()
        .unwrap_or(gltf::texture::MagFilter::Linear)
    {
        gltf::texture::MagFilter::Linear => modelfile::FilterMode::Linear,
        gltf::texture::MagFilter::Nearest => modelfile::FilterMode::Nearest,
    };

    let min_filter = match gltf_sampler
        .min_filter()
        .unwrap_or(gltf::texture::MinFilter::Linear)
    {
        gltf::texture::MinFilter::Nearest
        | gltf::texture::MinFilter::NearestMipmapNearest
        | gltf::texture::MinFilter::NearestMipmapLinear => modelfile::FilterMode::Nearest,

        gltf::texture::MinFilter::Linear
        | gltf::texture::MinFilter::LinearMipmapNearest
        | gltf::texture::MinFilter::LinearMipmapLinear => modelfile::FilterMode::Linear,
    };

    let mipmap_filter = match gltf_sampler.min_filter() {
        Some(
            gltf::texture::MinFilter::NearestMipmapNearest
            | gltf::texture::MinFilter::LinearMipmapNearest,
        ) => modelfile::MipmapFilterMode::Nearest,
        Some(
            gltf::texture::MinFilter::NearestMipmapLinear
            | gltf::texture::MinFilter::LinearMipmapLinear,
        ) => modelfile::MipmapFilterMode::Linear,
        Some(gltf::texture::MinFilter::Nearest | gltf::texture::MinFilter::Linear) | None => {
            modelfile::MipmapFilterMode::None
        }
    };

    let wrap_u = match gltf_sampler.wrap_s() {
        gltf::texture::WrappingMode::Repeat => modelfile::WrapMode::Repeat,
        gltf::texture::WrappingMode::ClampToEdge => modelfile::WrapMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => modelfile::WrapMode::MirroredRepeat,
    };

    let wrap_v = match gltf_sampler.wrap_t() {
        gltf::texture::WrappingMode::Repeat => modelfile::WrapMode::Repeat,
        gltf::texture::WrappingMode::ClampToEdge => modelfile::WrapMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => modelfile::WrapMode::MirroredRepeat,
    };

    // gltf has no wrap_w
    let wrap_w = modelfile::WrapMode::Repeat;

    modelfile::SampledTexture {
        source: tex_output_path,
        sampler: modelfile::Sampler {
            mag_filter,
            min_filter,
            mipmap_filter,
            wrap_u,
            wrap_v,
            wrap_w,
        },
    }
}

fn bake_base_color_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    let tex_info = material
        .pbr_metallic_roughness()
        .base_color_texture()
        .expect("A primitive is missing the base color texture");
    bake_texture(
        &tex_info.texture(),
        images,
        true,
        material.alpha_mode(),
        model_name,
        "base_color",
    )
}

fn bake_metallic_roughness_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    let tex_info = material
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
        .expect("A primitive is missing the metallic roughness texture");
    bake_texture(
        &tex_info.texture(),
        images,
        false,
        material.alpha_mode(),
        model_name,
        "metallic_roughness",
    )
}

fn bake_normals_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    let tex_info = material
        .normal_texture()
        .expect("A primitive is missing the normals texture");
    bake_texture(
        &tex_info.texture(),
        images,
        false,
        material.alpha_mode(),
        model_name,
        "normals",
    )
}

fn bake_occlusion_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    if let Some(tex_info) = material.occlusion_texture() {
        bake_texture(
            &tex_info.texture(),
            images,
            false,
            material.alpha_mode(),
            model_name,
            "occlusion",
        )
    } else {
        println!("WARNING: material doesn't have an occlusion texture, using placeholder");
        let data = gltf::image::Data {
            pixels: vec![u8::MAX],
            format: gltf::image::Format::R8,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "occlusion")
    }
}

fn bake_emissive_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    let tex_info = material
        .emissive_texture()
        .expect("A primitive is missing the emissive texture");
    bake_texture(
        &tex_info.texture(),
        images,
        false,
        material.alpha_mode(),
        model_name,
        "emissive",
    )
}

fn filename_without_extension(path: &str) -> Option<&str> {
    Path::new(path).file_stem().and_then(|s| s.to_str())
}

fn bake(
    gltf: &Document,
    buffers: &Vec<gltf::buffer::Data>,
    images: &Vec<gltf::image::Data>,
    input_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let model_name = filename_without_extension(&input_path).unwrap();
    let binary_path = format!("assets/local/{}/{}.bin", model_name, model_name);
    let json_path = format!("assets/local/{}/{}.json", model_name, model_name);

    /// list of pairs (mesh index, primitive)
    let mut primitives = vec![];
    for mesh in gltf.meshes() {
        primitives.extend(mesh.primitives().map(|p| (mesh.index(), p)));
    }
    primitives.sort_by(|(_, a), (_, b)| {
        a.material()
            .index()
            .unwrap_or(0)
            .cmp(&b.material().index().unwrap_or(0))
    });

    let mut primitive_instances_map = HashMap::<(usize, usize), Vec<Matrix4<f32>>>::new();
    // TODO multi scene support
    for node in gltf.scenes().next().unwrap().nodes() {
        accumulate_primitive_instances(&node, &Matrix4::identity(), &mut primitive_instances_map);
    }

    let mut output_primitives = vec![];
    let mut output_vertex_buffers = vec![];
    let mut output_index_buffers = vec![];
    let mut current_vertex_offset = 0usize;
    let mut current_index_byte_offset = 0usize;
    let mut base_vertex = 0u32;

    for (mesh_idx, primitive) in primitives {
        let index_bytes = read_index_buffer(&primitive, buffers);
        let index_bytes_count = index_bytes.len();

        let mut verts: Vec<Vertex> = vec![];
        let pos_buffer = read_position_buffer(&primitive, buffers);
        let normals_buffer = read_normal_buffer(&primitive, buffers);
        let tangents_buffer = read_tangents_buffer(&primitive, buffers);
        let weights_buffer = read_weights_buffer(&primitive, buffers);
        let joints_buffer = read_joints_buffer(&primitive, buffers);
        let base_color_texcoord_buffer = read_base_color_texcoord_buffer(&primitive, buffers);
        let normals_texcoord_buffer = read_normals_texcoord_buffer(&primitive, buffers);
        let metallic_roughness_texcoord_buffer =
            read_metallic_roughness_texcoord_buffer(&primitive, buffers);
        let occlusion_texcoord_buffer = read_occlusion_texcoord_buffer(&primitive, buffers);
        let emissive_texcoord_buffer = read_emissive_texcoord_buffer(&primitive, buffers);
        for i in 0..pos_buffer.len() {
            let vert = Vertex {
                position: pos_buffer[i],
                normal: normals_buffer[i],
                tangent: tangents_buffer[i],
                weights: weights_buffer[i],
                joints: joints_buffer[i],
                base_color_tex_coords: base_color_texcoord_buffer[i],
                normal_tex_coords: normals_texcoord_buffer[i],
                metallic_roughness_tex_coords: metallic_roughness_texcoord_buffer[i],
                occlusion_tex_coords: occlusion_texcoord_buffer[i],
                emissive_tex_coords: emissive_texcoord_buffer[i],
            };
            verts.push(vert);
        }
        let vertex_count = verts.len();
        let vertex_bytes: Vec<u8> = bytemuck::cast_slice(&verts).to_vec();
        let vertex_bytes_count = vertex_bytes.len();

        output_index_buffers.push(index_bytes);
        output_vertex_buffers.push(vertex_bytes);
        output_primitives.push(modelfile::Primitive {
            instances: primitive_instances_map
                .get(&(mesh_idx, primitive.index()))
                .unwrap()
                .iter()
                .map(|m| (*m).into())
                .collect(),
            // TODO default material!
            material: primitive.material().index().unwrap() as u32,
            index_byte_length: index_bytes_count as u32,
            index_byte_offset: current_index_byte_offset as u32,
            vertex_byte_length: vertex_bytes_count as u32,
            vertex_byte_offset: current_vertex_offset as u32,
            base_vertex,
        });

        current_vertex_offset += vertex_bytes_count;
        current_index_byte_offset += index_bytes_count;
        base_vertex += vertex_count as u32;
    }

    let mut materials: Vec<modelfile::Material> = vec![];
    for material in gltf.materials() {
        let base_color_texture = bake_base_color_tex(&material, images, model_name);
        let normal_texture = bake_normals_tex(&material, images, model_name);
        let metallic_roughness_texture = bake_metallic_roughness_tex(&material, images, model_name);
        let occlusion_texture = bake_occlusion_tex(&material, images, model_name);
        let emissive_texture = bake_emissive_tex(&material, images, model_name);
        materials.push(modelfile::Material {
            base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
            metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
            roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
            emissive_factor: material.emissive_factor(),
            // TODO placeholder normal texture
            normal_texture_scale: material.normal_texture().map(|n| n.scale()).unwrap_or(1f32),
            occlusion_strength: material
                .occlusion_texture()
                .map(|o| o.strength())
                .unwrap_or(1f32),
            alpha_mode: match material.alpha_mode() {
                gltf::material::AlphaMode::Blend => modelfile::AlphaMode::Blend,
                gltf::material::AlphaMode::Mask => modelfile::AlphaMode::Mask,
                gltf::material::AlphaMode::Opaque => modelfile::AlphaMode::Opaque,
            },
            alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5f32),
            double_sided: material.double_sided(),
            base_color_texture,
            normal_texture,
            metallic_roughness_texture,
            occlusion_texture,
            emissive_texture,
        });
    }

    let model = modelfile::Model {
        buffer_path: binary_path.to_string(),
        // vertex buffer starts immediately after indices

        // note: vertex buffer requires alignment to 4 bytes, but since indices are u32, it's already aligned!
        vertex_buffer_start_offset: current_index_byte_offset as u32,
        primitives: output_primitives,
        materials,
        // TODO bounding box
        aabb: modelfile::Aabb {
            min: [0f32, 0f32, 0f32],
            max: [0f32, 0f32, 0f32],
        },
    };

    let mut binary_file = File::create(binary_path)?;
    let final_binary_buffer: Vec<u8> = output_index_buffers
        .into_iter()
        .flatten()
        .chain(output_vertex_buffers.into_iter().flatten())
        .collect();
    binary_file.write_all(final_binary_buffer.as_ref())?;

    let json = serde_json::to_string_pretty(&model)?;
    std::fs::write(json_path, json)?;

    Ok(())
}

pub fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let path = args
        .get(1)
        .map(String::as_str)
        .unwrap_or("assets/Lantern.glb");
    let (gltf, buffers, images) = gltf::import(path)?;
    bake(&gltf, &buffers, &images, path.to_string())?;

    Ok(())
}
