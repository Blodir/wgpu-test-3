use aabb::{calculate_aabb, fold_aabb};
use animations::bake_animation;
use glam::Mat4;
use gltf::Document;
use materials::bake_material;
use engine::renderer::buffers::skinned_vertex::SkinnedVertex;
use engine::renderer::buffers::static_vertex::StaticVertex;
use std::fs;
use std::{collections::HashMap, env, fs::File, io::Write};
use tangents::generate_tangents_for_mesh;
use gltf_utils::{
    accumulate_primitive_instances, filename_without_extension, read_base_color_texcoord_buffer,
    read_emissive_texcoord_buffer, read_index_buffer, read_joints_buffer,
    read_metallic_roughness_texcoord_buffer, read_normal_buffer, read_normals_texcoord_buffer,
    read_occlusion_texcoord_buffer, read_position_buffer, read_tangents_buffer, read_weights_buffer,
    JointsBuffer,
};
use engine::resource_system::file_formats::modelfile::{self, Deformation};

mod aabb;
mod tangents;
mod normals;
mod gltf_utils;
mod skeletons;
mod utils;
mod animations;
mod materials;
use skeletons::bake_skeletonfile;
use normals::generate_flat_normals_for_mesh;

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
    let maybe_joint_reindex = if gltf.skins().len() > 0 {
        Some(bake_skeletonfile(gltf, buffers, &skeletonfile_path)?)
    } else {
        None
    };

    // (mesh, skin)
    let mut meshes_and_skins = vec![];
    for node in gltf.nodes() {
        if let Some(mesh) = node.mesh() {
            meshes_and_skins.push((mesh, node.skin()))
        }
    }
    // (mesh index, skin index, primitive)
    let mut primitives = vec![];
    for (mesh, skin) in meshes_and_skins {
        primitives.extend(mesh.primitives().map(|p| (mesh.index(), skin.as_ref().map(|s| s.index()), p)));
    }
    primitives.sort_by(|(_, _, a), (_, _, b)| {
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
    let skins: Vec<_> = gltf.skins().collect();
    let mut aabbs = vec![];

    let mut output_primitives = vec![];
    let mut output_vertex_buffers = vec![];
    let mut output_index_buffers = vec![];
    let mut current_vertex_offset = 0usize;
    let mut current_index_byte_offset = 0usize;
    let mut base_vertex = 0u32;

    for (mesh_idx, maybe_skin_idx, primitive) in primitives {
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

        let pos_buffer = read_position_buffer(&primitive, buffers);
        let normals_buffer = match read_normal_buffer(&primitive, buffers) {
            Some(t) => t,
            None => match generate_flat_normals_for_mesh(&pos_buffer, &indices) {
                Ok(n) => n,
                Err(e) => {
                    println!(
                        "Warning: failed to generate flat normals for primitive {} of mesh {}: {}",
                        primitive.index(),
                        mesh_idx,
                        e
                    );
                    continue;
                }
            },
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
        aabbs.push(calculate_aabb(&pos_buffer));
        let (vertex_bytes, vertex_count) = match maybe_joint_reindex {
            Some(ref joint_reindex) => {
                let mut verts: Vec<SkinnedVertex> = vec![];
                for i in 0..pos_buffer.len() {
                    let vert = SkinnedVertex {
                        position: pos_buffer[i],
                        normal: normals_buffer[i],
                        tangent: tangents_buffer[i],
                        weights: weights_buffer[i],
                        joints: match &joints_buffer {
                            JointsBuffer::U8(buffer) => buffer[i].map(|skin_joint_idx| {
                                let maybe_node_idx = maybe_skin_idx.map(|i| {
                                    let joints: Vec<_> = skins[i].joints().collect();
                                    joints[skin_joint_idx as usize].index()
                                });
                                *maybe_node_idx.and_then(|node_idx| joint_reindex.get(&(node_idx as u32))).unwrap_or(&0u32) as u8
                            }),
                            JointsBuffer::U16(buffer) => buffer[i].map(|skin_joint_idx| {
                                let maybe_node_idx = maybe_skin_idx.map(|i| {
                                    let joints: Vec<_> = skins[i].joints().collect();
                                    joints[skin_joint_idx as usize].index()
                                });
                                *maybe_node_idx.and_then(|node_idx| joint_reindex.get(&(node_idx as u32))).unwrap_or(&0u32) as u8
                            }),
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
                (vertex_bytes, vertex_count)
            },
            None => {
                let mut verts: Vec<StaticVertex> = vec![];
                for i in 0..pos_buffer.len() {
                    let vert = StaticVertex {
                        position: pos_buffer[i],
                        normal: normals_buffer[i],
                        tangent: tangents_buffer[i],
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
                (vertex_bytes, vertex_count)
            },
        };
        let vertex_bytes_count = vertex_bytes.len();

        output_index_buffers.push(index_bytes);
        output_vertex_buffers.push(vertex_bytes);
        output_primitives.push(modelfile::Submesh {
            instances: primitive_instances_map
                .get(&(mesh_idx, primitive.index()))
                .unwrap()
                .iter()
                .map(|m| (*m).to_cols_array_2d())
                .collect(),
            material: primitive.material().index().map(|i| i as u32),
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

    let mut material_paths: Vec<String> = vec![];
    for (idx, material) in gltf.materials().enumerate() {
        let name = material.name().map(|n| n.to_string()).unwrap_or(model_name.to_string() + &idx.to_string());
        let json_path = format!("assets/local/{}/{}.material.json", model_name, name);
        bake_material(&material, buffers, images, model_name, &json_path)?;
        material_paths.push(json_path);
    }

    let aabb = fold_aabb(&aabbs);

    let deformation = match maybe_joint_reindex {
        Some(joint_reindex) => {
            let animations: Result<Vec<_>, _> = gltf.animations()
                .enumerate()
                .map(|(idx, anim)| -> Result<String, Box<dyn std::error::Error>> {
                    let anim_json_path = format!("assets/local/{}/{}_{}.animation.json", model_name, model_name, idx);
                    let anim_bin_path  = format!("assets/local/{}/{}_{}.animation.bin",  model_name, model_name, idx);
                    bake_animation(&anim, buffers, &joint_reindex, &anim_json_path, &anim_bin_path)?;
                    Ok(anim_json_path)
                })
                .collect();
            let animations = animations?;

            Deformation::Skinned {
                skeleton: skeletonfile_path.to_string(),
                animations
            }
        },
        None => Deformation::None,
    };

    let model = modelfile::Model {
        buffer: binary_path.to_string(),
        // vertex buffer starts immediately after indices
        // note: vertex buffer requires alignment to 4 bytes, but since indices are u32, it's already aligned!
        vertex_buffer_start_offset: current_index_byte_offset as u32,
        submeshes: output_primitives,
        material_paths,
        aabb,
        deformation,
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
