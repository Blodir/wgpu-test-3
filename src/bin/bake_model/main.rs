use glam::Mat4;
use gltf::Document;
use std::fs;
use std::path::Path;
use std::{collections::HashMap, env, fs::File, io::Write};
use tangents::generate_tangents_for_mesh;
use gltf_utils::{
    accumulate_primitive_instances, filename_without_extension, read_base_color_texcoord_buffer,
    read_emissive_texcoord_buffer, read_index_buffer, read_joints_buffer,
    read_metallic_roughness_texcoord_buffer, read_normal_buffer, read_normals_texcoord_buffer,
    read_occlusion_texcoord_buffer, read_position_buffer, read_tangents_buffer, read_weights_buffer,
    readf32, JointsBuffer,
};
use wgpu_test_3::renderer::pipelines::model::vertex::Vertex;
use wgpu_test_3::renderer::render_resources::animationfile::{Target, Track};
use wgpu_test_3::renderer::render_resources::dds::{create_dds, gltf_img_to_dxgi_format};
use wgpu_test_3::renderer::render_resources::{animationfile, modelfile};

mod tangents;
mod gltf_utils;
mod skeletons;
mod utils;
use skeletons::bake_skeletonfile;
use utils::ensure_parent_dir_exists;

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
    if let Some(tex_info) = material.pbr_metallic_roughness().base_color_texture() {
        bake_texture(
            &tex_info.texture(),
            images,
            true,
            material.alpha_mode(),
            model_name,
            "base_color",
        )
    } else {
        println!("WARNING: material doesn't have a base color texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[1u16, 1u16, 1u16, 1u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, true, "base_color")
    }
}

fn bake_metallic_roughness_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    if let Some(tex_info) = material
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
    {
        bake_texture(
            &tex_info.texture(),
            images,
            false,
            material.alpha_mode(),
            model_name,
            "metallic_roughness",
        )
    } else {
        println!("WARNING: material doesn't have a metallic roughness texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[0x0000u16, 0x0000u16, 0x3C00u16, 0x3C00u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "metallic_roughness")
    }
}

fn bake_normals_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> modelfile::SampledTexture {
    if let Some(tex_info) = material.normal_texture() {
        bake_texture(
            &tex_info.texture(),
            images,
            false,
            material.alpha_mode(),
            model_name,
            "normals",
        )
    } else {
        println!("WARNING: material doesn't have a normals texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[0x0000u16, 0x0000u16, 0x3C00u16, 0x3C00u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "normals")
    }
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
    if let Some(tex_info) = material.emissive_texture() {
        bake_texture(
            &tex_info.texture(),
            images,
            false,
            material.alpha_mode(),
            model_name,
            "emissive",
        )
    } else {
        println!("WARNING: material doesn't have an emissive texture, using placeholder");
        let data = gltf::image::Data {
            pixels: vec![0u8],
            format: gltf::image::Format::R8,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "emissive")
    }
}

struct TempTargetSamplers<'a> {
    translation: Option<&'a gltf::animation::Sampler<'a>>,
    rotation: Option<&'a gltf::animation::Sampler<'a>>,
    scale: Option<&'a gltf::animation::Sampler<'a>>,
}

fn bake_animation(
    gltf: &Document,
    animation: &gltf::Animation,
    buffers: &Vec<gltf::buffer::Data>,
    joint_reindex: HashMap<u32, u32>,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let duration = animation
        .samplers()
        .into_iter()
        .map(|s| readf32(&s.input(), buffers).last().unwrap().clone())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0f32);
    let mut tracks: Vec<Track> = vec![];
    let mut binary_data: Vec<u8> = vec![];

    let mut targets = HashMap::<Target, TempTargetSamplers>::new();
    for channel in animation.channels() {
        // check if the target node is part of a skeleton
        //  use joint_reindex to get the joint index in the skeletonfile
        // if not, then need to find the primitive instance indices
        // collect the samplers in targets hashmap
    }

    // for each target
    for (target, samplers) in targets {
        // check if all target channels share the same time array

        // read binary times arrays (always scalar f32)
        // let shared_times: Option<Vec<u8>> = ...
        // let translation_times: Option<Vec<u8>> = ...

        // read binary data arrays
        // let translation_data: Option<Vec<u8>> = ... (vec3 f32)
        // let rotation_data: Option<Vec<u8>> = ... (vec4 f32)
        // let scale_data: Option<Vec<u8>> = ... (vec3 f32)

        // BIG TODO need to map the data because hierarchy gets flattened... so all data takes parents into account

        // construct binary refs

        let track = animationfile::Track {
            target,
            shared_times: todo!(),
            translation: todo!(),
            rotation: todo!(),
            scale: todo!(),
        };
        // append binary_data
        tracks.push(track);
    }

    // write binary file

    let animation_clip = animationfile::AnimationClip {
        duration,
        tracks,
        primitive_groups: todo!(),
        binary_path: todo!(),
    };

    let json = serde_json::to_string_pretty(&animation_clip)?;
    std::fs::write(output_path, json)?;

    Ok(())
}

fn bake(
    gltf: &Document,
    buffers: &Vec<gltf::buffer::Data>,
    images: &Vec<gltf::image::Data>,
    input_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let model_name = filename_without_extension(&input_path).unwrap();
    let directory_path = format!("assets/local/{}", model_name);
    let binary_path = format!("{}/{}.bin", directory_path, model_name);
    let json_path = format!("{}/{}.json", directory_path, model_name);

    let skeletonfile_path = format!("assets/local/{}/{}.skeleton.json", model_name, model_name);
    let joint_reindex = bake_skeletonfile(gltf, buffers, &skeletonfile_path)?;

    // list of pairs (mesh index, primitive)
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

    let mut primitive_instances_map = HashMap::<(usize, usize), Vec<Mat4>>::new();
    // TODO multi scene support
    for node in gltf.scenes().next().unwrap().nodes() {
        accumulate_primitive_instances(&node, &Mat4::IDENTITY, &mut primitive_instances_map);
    }

    let mut output_primitives = vec![];
    let mut output_vertex_buffers = vec![];
    let mut output_index_buffers = vec![];
    let mut current_vertex_offset = 0usize;
    let mut current_index_byte_offset = 0usize;
    let mut base_vertex = 0u32;

    for (mesh_idx, primitive) in primitives {
        if primitive.mode() != gltf::mesh::Mode::Triangles {
            println!("Warning: skipping non-triangle topology!");
            continue;
        }
        let indices = read_index_buffer(&primitive, buffers);
        if indices.len() % 3 != 0 {
            println!(
                "Warning: primitive {} has a non-triangular index count, skipping!",
                primitive.index()
            );
            continue;
        }
        let index_bytes: Vec<u8> = bytemuck::cast_slice(&indices).to_vec();
        let index_bytes_count = index_bytes.len();

        let mut verts: Vec<Vertex> = vec![];
        let pos_buffer = read_position_buffer(&primitive, buffers);
        let normals_buffer = match read_normal_buffer(&primitive, buffers) {
            Some(t) => t,
            None => {
                println!("Warning: a primitive is missing the NORMALS attribute, skipping!");
                continue;
            }
        };
        let base_color_texcoord_buffer = read_base_color_texcoord_buffer(&primitive, buffers);
        let normals_texcoord_buffer = read_normals_texcoord_buffer(&primitive, buffers);
        let metallic_roughness_texcoord_buffer =
            read_metallic_roughness_texcoord_buffer(&primitive, buffers);
        let occlusion_texcoord_buffer = read_occlusion_texcoord_buffer(&primitive, buffers);
        let emissive_texcoord_buffer = read_emissive_texcoord_buffer(&primitive, buffers);
        let tangents_buffer = match read_tangents_buffer(&primitive, buffers) {
            Some(t) => t,
            None => {
                match generate_tangents_for_mesh(
                    &pos_buffer,
                    &normals_buffer,
                    &normals_texcoord_buffer,
                    &indices,
                ) {
                    Ok(t) => t,
                    Err(e) => {
                        println!(
                            "Warning: failed to generate tangents for primitive {} of mesh {}: {}",
                            primitive.index(),
                            mesh_idx,
                            e
                        );
                        continue;
                    }
                }
            }
        };
        let weights_buffer = read_weights_buffer(&primitive, buffers);
        let joints_buffer = read_joints_buffer(&primitive, buffers);
        for i in 0..pos_buffer.len() {
            let vert = Vertex {
                position: pos_buffer[i],
                normal: normals_buffer[i],
                tangent: tangents_buffer[i],
                weights: weights_buffer[i],
                joints: match &joints_buffer {
                    JointsBuffer::U8(buffer) => buffer[i]
                        .map(|idx| *joint_reindex.get(&(idx as u32)).unwrap_or(&0u32) as u8),
                    JointsBuffer::U16(buffer) => buffer[i]
                        .map(|idx| *joint_reindex.get(&(idx as u32)).unwrap_or(&0u32) as u8),
                },
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
                .map(|m| (*m).to_cols_array_2d())
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
        skeletonfile_path: skeletonfile_path.to_string(),
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

    fs::create_dir_all(directory_path)?;
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
