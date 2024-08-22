use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use bytemuck::{cast_slice, try_cast_slice, Pod, Zeroable};
use cgmath::{Matrix4, Quaternion, SquareMatrix};
use serde_json::Result;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

fn buffer_to_ascii(buffer: &[u8]) -> String {
    buffer.iter().map(|&x| x as char).collect()
}

fn default_tex_coord() -> usize { 0 }
fn default_scale() -> u64 { 1 }
fn default_strength() -> u64 { 1 }

#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u16)]
pub enum ComponentType {
    SignedByte = 5120,
    UnsignedByte = 5121,
    SignedShort = 5122,
    UnsignedShort = 5123,
    UnsignedInt = 5125,
    Float = 5126,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum AccessorType {
    #[serde(rename = "SCALAR")]
    Scalar,
    #[serde(rename = "VEC2")]
    Vec2,
    #[serde(rename = "VEC3")]
    Vec3,
    #[serde(rename = "VEC4")]
    Vec4,
    #[serde(rename = "MAT2")]
    Mat2,
    #[serde(rename = "MAT3")]
    Mat3,
    #[serde(rename = "MAT4")]
    Mat4,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MimeType {
    #[serde(rename = "image/png")]
    PNG,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Accessor {
    #[serde(rename = "bufferView")]
    pub buffer_view: u8,
    #[serde(rename = "byteOffset")]
    pub byte_offset: Option<u32>,
    #[serde(rename = "componentType")]
    pub component_type: ComponentType,
    pub count: u32,
    // pub max: Option<[f64; 3]>,
    // pub min: Option<[f64; 3]>,
    #[serde(rename = "type")]
    pub accessor_type: AccessorType
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Asset {
    pub generator: String,
    pub version: String,
}

#[derive(Serialize_repr, Deserialize_repr, Debug)]
#[repr(u16)]
pub enum BufferViewTarget {
    ArrayBuffer = 34962,
    ElementArrayBuffer = 34963,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BufferView {
    pub buffer: u8,
    #[serde(rename = "byteLength")]
    pub byte_length : u32,
    #[serde(rename = "byteOffset")]
    pub byte_offset : Option<u32>,
    #[serde(rename = "byteStride")]
    pub byte_stride : Option<u32>,
    pub target: Option<BufferViewTarget>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Buffer {
    #[serde(rename = "byteLength")]
    pub byte_length : u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PrimitiveAttributes {
    #[serde(rename = "POSITION")]
    pub position: usize,
    #[serde(rename = "NORMAL")]
    pub normal: Option<usize>,
    #[serde(rename = "TANGENT")]
    pub tangent: Option<usize>,

    /* The rest of the fields can't be mapped so they're collected in a hashmap:
    * TEXCOORD_n
    * COLOR_n
    * JOINTS_n
    * WEIGHTS_n
    */
    #[serde(flatten)]
    pub additional_fields: HashMap<String, usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub indices: usize,
    pub attributes: PrimitiveAttributes,
    pub material: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mesh {
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BaseColorTexture {
    pub index: usize,
    #[serde(rename = "texCoord", default = "default_tex_coord")]
    pub tex_coord: usize,
}

/*
* Green channel = roughness
* Blue channel = metalness
*/
#[derive(Serialize, Deserialize, Debug)]
pub struct MetallicRoughnessTexture {
    pub index: usize,
    #[serde(rename = "texCoord", default = "default_tex_coord")]
    pub tex_coord: usize,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PbrMetallicRoughness {
    #[serde(rename = "baseColorFactor")]
    pub base_color_factor: Option<[f64; 4]>,
    #[serde(rename = "metallicFactor")]
    pub metallic_factor: Option<f64>,
    #[serde(rename = "roughnessFactor")]
    pub roughness_factor: Option<f64>,

    #[serde(rename = "baseColorTexture")]
    pub base_color_texture: Option<BaseColorTexture>,
    #[serde(rename = "metallicRoughnessTexture")]
    pub metallic_roughness_texture: Option<MetallicRoughnessTexture>,
    // ..
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NormalTextureInfo {
    pub index: usize,
    #[serde(rename = "texCoord", default = "default_tex_coord")]
    pub tex_coord: usize,
    #[serde(default = "default_scale")]
    pub scale: u64,
    //extensions, extras ..
}

#[derive(Serialize, Deserialize, Debug)]
pub struct OcclusionTextureInfo {
    pub index: usize,
    #[serde(rename = "texCoord", default = "default_tex_coord")]
    pub tex_coord: usize,
    #[serde(default = "default_strength")]
    pub strength: u64,
    //extensions, extras ..
}

#[derive(Serialize, Deserialize, Debug)]
pub struct EmissiveTextureInfo {
    pub index: usize,
    #[serde(rename = "texCoord", default = "default_tex_coord")]
    pub tex_coord: usize,
    //extensions, extras ..
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Material {
    pub name: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr_metallic_roughness: Option<PbrMetallicRoughness>,
    #[serde(rename = "normalTexture")]
    pub normal_texture: Option<NormalTextureInfo>,
    #[serde(rename = "occlusionTexture")]
    pub occlusion_texture: Option<OcclusionTextureInfo>,
    #[serde(rename = "emissiveTexture")]
    pub emissive_texture: Option<EmissiveTextureInfo>,
    #[serde(rename = "emissiveFactor")]
    pub emissive_factor: Option<[f64; 3]>,
    // .. alpha cutoff, double sided, name, extension, extras
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Node {
    pub name: Option<String>,
    pub mesh: Option<usize>,
    pub translation: Option<[f64; 3]>,
    pub rotation: Option<[f64; 4]>,
    pub scale: Option<[f64; 3]>,
    pub matrix: Option<[f64; 16]>,
    pub children: Option<Vec<usize>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Scene {
    pub name: Option<String>,
    pub nodes: Vec<usize>,
}


#[derive(Serialize, Deserialize, Debug)]
pub struct Texture {
    pub source: usize,
    pub sampler: Option<usize>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Sampler {
    #[serde(rename = "magFilter")]
    pub mag_filter: u32,
    #[serde(rename = "minFilter")]
    pub min_filter: u32,
    #[serde(rename = "wrapS")]
    pub wrap_s: u32,
    #[serde(rename = "wrapT")]
    pub wrap_t: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Image {
    pub uri: Option<String>,
    #[serde(rename = "bufferView")]
    pub buffer_view: Option<usize>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<MimeType>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SceneDescription {
    pub accessors: Vec<Accessor>,
    pub asset: Asset,
    #[serde(rename = "bufferViews")]
    pub buffer_views: Vec<BufferView>,
    pub buffers: Vec<Buffer>,
    pub meshes: Vec<Mesh>,
    pub nodes: Vec<Node>,
    pub scene: usize,
    pub scenes: Vec<Scene>,
    pub materials: Option<Vec<Material>>,
    pub textures: Option<Vec<Texture>>,
    pub images: Option<Vec<Image>>,
    pub samplers: Option<Vec<Sampler>>,
}

pub struct JSONChunk {
    pub chunk_length: u32,
    pub chunk_type: String,
    pub chunk_data: String,
}

pub struct AccessorDataElement<'a> {
    pub buffer_slice: &'a [u8],
    pub stride: u64,
}

pub struct GLTF {
    pub magic: String,
    pub version: u32,
    pub length: u32,
    pub scene: SceneDescription,
    pub json_chunk: JSONChunk,
    pub binary_buffer: Vec<u8>,
}

pub fn get_accessor_component_count(accessor: &Accessor) -> u8 {
    match accessor.accessor_type {
        AccessorType::Scalar => 1,
        AccessorType::Vec2 => 2,
        AccessorType::Vec3 => 3,
        AccessorType::Vec4 => 4,
        AccessorType::Mat2 => 4,
        AccessorType::Mat3 => 9,
        AccessorType::Mat4 => 16,
    }
}

// size in bytes
pub fn get_accessor_component_size(accessor: &Accessor) -> u8 {
    match accessor.component_type {
        ComponentType::SignedByte => 1,
        ComponentType::UnsignedByte => 1,
        ComponentType::SignedShort => 2,
        ComponentType::UnsignedShort => 2,
        ComponentType::UnsignedInt => 4,
        ComponentType::Float => 4,
    }
}

fn construct_mesh_instances_map(scene: &SceneDescription, node_idx: usize, mut transform: Matrix4<f32>, acc: &mut HashMap<usize, Vec<super::pbr::Instance>>) {
    let node = &scene.nodes[node_idx];

    if let Some(v) = node.scale {
        transform = transform * Matrix4::from_nonuniform_scale(v[0] as f32, v[1] as f32, v[2] as f32);
    }
    if let Some(v) = node.rotation {
        transform = transform * Matrix4::from(Quaternion::new(v[3] as f32, v[0] as f32, v[1] as f32, v[2] as f32));
    }
    if let Some(v) = node.translation {
        transform = transform * Matrix4::from_translation(cgmath::Vector3::from(v.map(|x| x as f32)));
    }
    if let Some(m) = node.matrix {
        let m: [f32; 16] = m.map(|x| x as f32);
        let m: Matrix4<f32> = Matrix4::new(
            m[0],  m[1],  m[2],  m[3],
            m[4],  m[5],  m[6],  m[7],
            m[8],  m[9],  m[10], m[11],
            m[12], m[13], m[14], m[15]
        );
        transform = transform * m;
    }
    if let Some(mesh) = node.mesh {
        acc.entry(mesh as usize).or_insert(Vec::new()).push(super::pbr::Instance::from(transform.clone()));
    }
    if let Some(children) = &node.children {
        for child_idx in children {
            construct_mesh_instances_map(scene, *child_idx, transform.clone(), acc);
        }
    }
}

fn scene_to_mesh_instances(scene: &SceneDescription) -> HashMap<usize, Vec<super::pbr::Instance>> {
    let mut map: HashMap<usize, Vec<super::pbr::Instance>> = HashMap::new();
    let transform = Matrix4::identity();

    // Only rendering the main scene for now
    let scene_nodes = &scene.scenes[scene.scene].nodes;
    for node_idx in scene_nodes {
        construct_mesh_instances_map(scene, *node_idx, transform, &mut map);
    }

    map
}

impl GLTF {
    pub fn new(file: &mut File) -> io::Result<Self> {
        let mut magic_buffer = [0u8; 4];
        file.read_exact(&mut magic_buffer)?;
        let magic = buffer_to_ascii(&magic_buffer);

        let mut version_buffer = [0u8; 4];
        file.read_exact(&mut version_buffer)?;
        let version = u32::from_le_bytes(version_buffer);

        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        let json_chunk = GLTF::parse_json_chunk(file)?;
        let binary_buffer = GLTF::parse_binary_buffer(file)?;
        let scene = serde_json::from_str(&json_chunk.chunk_data)?;
        println!("{:#?}", scene);

        Ok(
            Self {
                magic, version, length, json_chunk, binary_buffer, scene
            }
        )
    }

    fn parse_json_chunk(file: &mut File) -> io::Result<JSONChunk> {
        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let chunk_length = u32::from_le_bytes(length_buffer);

        let mut type_buffer = [0u8; 4];
        file.read_exact(&mut type_buffer)?;
        let chunk_type = buffer_to_ascii(&type_buffer);

        let mut data_buffer = vec![0u8; chunk_length.try_into().unwrap()];
        file.read_exact(&mut data_buffer)?;
        let chunk_data = buffer_to_ascii(&data_buffer);

        Ok(JSONChunk { chunk_length, chunk_type, chunk_data })
    }

    fn parse_binary_buffer(file: &mut File) -> io::Result<Vec<u8>> {
        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let chunk_length = u32::from_le_bytes(length_buffer);

        let mut type_buffer = [0u8; 4];
        file.read_exact(&mut type_buffer)?;
        let chunk_type = buffer_to_ascii(&type_buffer);

        let mut binary_buffer = vec![0u8; chunk_length as usize];
        file.read_exact(&mut binary_buffer)?;

        Ok(binary_buffer)
    }

    fn accessor_to_contiguous_array<F, T>(&self, accessor_idx: usize, f: F) -> Vec<T>
    where
        F: Fn(&[u8]) -> T,
    {
        let accessor = &self.scene.accessors[accessor_idx];
        let buffer_view = &self.scene.buffer_views[accessor.buffer_view as usize];
        let start_offset =
            buffer_view.byte_offset.unwrap_or(0u32) as usize
            + accessor.byte_offset.unwrap_or(0u32) as usize;
        let end_offset =
            buffer_view.byte_offset.unwrap_or(0u32) as usize
            + buffer_view.byte_length as usize;
        let slice = &self.binary_buffer[start_offset..end_offset];

        let data_element_size =
            get_accessor_component_count(accessor) as usize
            * get_accessor_component_size(accessor) as usize;
        let stride = {
            match buffer_view.byte_stride {
                Some(s) => s as usize,
                None => data_element_size
            }
        };

        let mut data: Vec<T> = vec![];
        let mut current_index = 0usize;
        let mut i = 0u32;
        while i < accessor.count {
            let a = f(&slice[current_index..current_index+data_element_size]);
            data.push(a);
            current_index += stride;
            i += 1;
        }
        data
    }

    fn accessor_to_pbr_indices(&self, accessor_idx: usize) -> super::pbr::VertexIndices {
        let accessor = &self.scene.accessors[accessor_idx];
        match accessor.component_type {
            ComponentType::UnsignedByte => {
                super::pbr::VertexIndices::U16(
                    self.accessor_to_contiguous_array(accessor_idx, |buf| {
                        buf[0] as u16
                    })
                )
            },
            ComponentType::UnsignedShort => {
                super::pbr::VertexIndices::U16(
                    self.accessor_to_contiguous_array(accessor_idx, |buf| {
                        bytemuck::cast::<[u8; 2], u16>(buf[0..2].try_into().unwrap())
                    })
                )
            },
            ComponentType::UnsignedInt => {
                super::pbr::VertexIndices::U32(
                    self.accessor_to_contiguous_array(accessor_idx, |buf| {
                        bytemuck::cast::<[u8; 4], u32>(buf[0..4].try_into().unwrap())
                    })
                )
            },
            _ => { panic!("GLTF: Illegal vertex index component type.") },
        }
    }

    fn primitive_to_pbr_vertices(&self, primitive: &Primitive) -> Vec<super::pbr::Vertex> {
        let positions =
            self.accessor_to_contiguous_array(primitive.attributes.position, |buf| {
                let s: &[u8; 12] = buf[0..12].try_into().unwrap();
                let res: [f32; 3] = bytemuck::cast(*s);
                res
            });

        let normals = primitive.attributes.normal.map(|n| {
            self.accessor_to_contiguous_array(n, |buf| {
                let s: &[u8; 12] = buf[0..12].try_into().unwrap();
                // TODO normalize? there's a property in gltf for this
                let res: [f32; 3] = bytemuck::cast(*s);
                res
            })
        });

        let tangents = primitive.attributes.tangent.map(|n| {
            self.accessor_to_contiguous_array(n, |buf| {
                let s: &[u8; 16] = buf[0..16].try_into().unwrap();
                let res: [f32; 4] = bytemuck::cast(*s);
                res
            })
        });

        let weights = primitive.attributes.additional_fields.get("WEIGHTS_0").map(|n| {
            self.accessor_to_contiguous_array(*n, |buf| {
                let s: &[u8; 16] = buf[0..16].try_into().unwrap();
                let res: [f32; 4] = bytemuck::cast(*s);
                res
            })
        });

        let joints = primitive.attributes.additional_fields.get("JOINTS_0").map(|n| {
            self.accessor_to_contiguous_array(*n, |buf| {
                let s: &[u8; 4] = buf[0..4].try_into().unwrap();
                let res: [u8; 4] = bytemuck::cast(*s);
                res
            })
        });

        let maybe_material: Option<&Material> = match (primitive.material, &self.scene.materials) {
            (Some(i), Some(mats)) => Some(&mats[i]),
            _ => None
        };

        let normal_tex_coords = maybe_material
            .and_then(|mat| mat.normal_texture.as_ref())
            .and_then(|nt| primitive.attributes.additional_fields.get(&format!("TEXCOORD_{}", nt.tex_coord)))
            .map(|n| {
                self.accessor_to_contiguous_array(*n, |buf| {
                    let s: &[u8; 8] = buf[0..8].try_into().unwrap();
                    let res: [f32; 2] = bytemuck::cast(*s);
                    res
                })
            });

        let occlusion_tex_coords = maybe_material
            .and_then(|mat| mat.occlusion_texture.as_ref())
            .and_then(|ot| primitive.attributes.additional_fields.get(&format!("TEXCOORD_{}", ot.tex_coord)))
            .map(|n| {
                self.accessor_to_contiguous_array(*n, |buf| {
                    let s: &[u8; 8] = buf[0..8].try_into().unwrap();
                    let res: [f32; 2] = bytemuck::cast(*s);
                    res
                })
            });

        let emissive_tex_coords = maybe_material
            .and_then(|mat| mat.emissive_texture.as_ref())
            .and_then(|et| primitive.attributes.additional_fields.get(&format!("TEXCOORD_{}", et.tex_coord)))
            .map(|n| {
                self.accessor_to_contiguous_array(*n, |buf| {
                    let s: &[u8; 8] = buf[0..8].try_into().unwrap();
                    let res: [f32; 2] = bytemuck::cast(*s);
                    res
                })
            });

        let base_color_tex_coords = maybe_material
            .and_then(|mat| mat.pbr_metallic_roughness.as_ref())
            .and_then(|pmr| pmr.base_color_texture.as_ref())
            .and_then(|bct| primitive.attributes.additional_fields.get(&format!("TEXCOORD_{}", bct.tex_coord)))
            .map(|n| {
                self.accessor_to_contiguous_array(*n, |buf| {
                    let s: &[u8; 8] = buf[0..8].try_into().unwrap();
                    let res: [f32; 2] = bytemuck::cast(*s);
                    res
                })
            });

        let metallic_roughness_tex_coords = maybe_material
            .and_then(|mat| mat.pbr_metallic_roughness.as_ref())
            .and_then(|pmr| pmr.metallic_roughness_texture.as_ref())
            .and_then(|mrt| primitive.attributes.additional_fields.get(&format!("TEXCOORD_{}", mrt.tex_coord)))
            .map(|n| {
                self.accessor_to_contiguous_array(*n, |buf| {
                    let s: &[u8; 8] = buf[0..8].try_into().unwrap();
                    let res: [f32; 2] = bytemuck::cast(*s);
                    res
                })
            });

        let mut vertices = vec![];
        for i in 0..positions.len() {
            let mut vert = super::pbr::Vertex::default();
            vert.position = positions[i];
            if let Some(ref n) = normals { vert.normal = n[i]; }
            if let Some(ref n) = tangents { vert.tangent = n[i]; }
            if let Some(ref n) = weights { vert.weights = n[i]; }
            if let Some(ref n) = joints { vert.joints = n[i]; }
            if let Some(ref n) = normal_tex_coords { vert.normal_tex_coords = n[i]; }
            if let Some(ref n) = occlusion_tex_coords { vert.occlusion_tex_coords = n[i]; }
            if let Some(ref n) = emissive_tex_coords { vert.emissive_tex_coords = n[i]; }
            if let Some(ref n) = base_color_tex_coords { vert.base_color_tex_coords = n[i]; }
            if let Some(ref n) = metallic_roughness_tex_coords { vert.metallic_roughness_tex_coords = n[i]; }
            vertices.push(vert);
        }
        vertices
    }

    fn texture_to_dynamic_image(&self, texture_idx: usize) -> image::DynamicImage {
        let image_idx = self.scene.textures.as_ref().unwrap()[texture_idx].source;
        let image = &self.scene.images.as_ref().unwrap()[image_idx];
        let image_format = match image.mime_type {
            Some(MimeType::PNG) => { image::ImageFormat::Png },
            _ => panic!("Expecting PNG")
        };
        let bv = &self.scene.buffer_views[image.buffer_view.unwrap()];
        let start_offset = bv.byte_offset.unwrap_or(0u32) as usize;
        let end_offset = bv.byte_offset.unwrap_or(0u32) as usize + bv.byte_length as usize;
        let slice = &&self.binary_buffer[start_offset..end_offset];
        image::load_from_memory_with_format(slice, image_format).unwrap()
    }

    fn material_to_pbr(&self, maybe_material_idx: Option<usize>) -> super::pbr::Material {
        let mut pbr_material = super::pbr::Material::default();
        let maybe_material: Option<&Material> = match (maybe_material_idx, &self.scene.materials) {
            (Some(i), Some(mats)) => Some(&mats[i]),
            _ => None
        };
        if let Some(material) = maybe_material {
            if let Some(factor) = material.pbr_metallic_roughness.as_ref()
                .and_then(|pmr| pmr.base_color_factor)
            {
                pbr_material.base_color_factor = factor.map(|f| f as f32);
            }

            if let Some(factor) = material.pbr_metallic_roughness.as_ref()
                .and_then(|pmr| pmr.metallic_factor)
            {
                pbr_material.metallic_factor = factor as f32;
            }

            if let Some(factor) = material.pbr_metallic_roughness.as_ref()
                .and_then(|pmr| pmr.roughness_factor)
            {
                pbr_material.roughness_factor = factor as f32;
            }

            if let Some(factor) = material.emissive_factor {
                pbr_material.emissive_factor = factor.map(|f| f as f32);
            }

            // TODO figure out samplers ..
            // TODO figure out normal texture scale property

            if let Some(texture) = material.pbr_metallic_roughness.as_ref()
                .and_then(|pmr| pmr.base_color_texture.as_ref())
                .map(|t| self.texture_to_dynamic_image(t.index))
            {
                pbr_material.base_color_texture = texture;
            }

            if let Some(texture) = material.pbr_metallic_roughness.as_ref()
                .and_then(|pmr| pmr.metallic_roughness_texture.as_ref())
                .map(|t| self.texture_to_dynamic_image(t.index))
            {
                pbr_material.metallic_roughness_texture = texture;
            }
            
            if let Some(texture) = material.normal_texture.as_ref()
                .map(|t| self.texture_to_dynamic_image(t.index))
            {
                pbr_material.normal_texture = texture;
            }

            if let Some(texture) = material.occlusion_texture.as_ref()
                .map(|t| self.texture_to_dynamic_image(t.index))
            {
                pbr_material.occlusion_texture = texture;
            }

            if let Some(texture) = material.emissive_texture.as_ref()
                .map(|t| self.texture_to_dynamic_image(t.index))
            {
                pbr_material.emissive_texture = texture;
            }
        }

        pbr_material
    }

    pub fn to_pbr_meshes(&self) -> Vec<super::pbr::Mesh> {
        let mut mesh_instances = scene_to_mesh_instances(&self.scene);
        let mut pbr_meshes = vec![];
        for mesh_idx in 0..self.scene.meshes.len() {
            let mesh = &self.scene.meshes[mesh_idx];
            let mut pbr_primitives = vec![];
            for primitive_idx in 0..mesh.primitives.len() {
                let primitive = &mesh.primitives[primitive_idx];
                let vertices = self.primitive_to_pbr_vertices(primitive);
                let indices = self.accessor_to_pbr_indices(primitive.indices);
                let material = self.material_to_pbr(primitive.material);
                pbr_primitives.push(super::pbr::Primitive {
                    vertices,
                    indices,
                    material,
                });
            }
            pbr_meshes.push(super::pbr::Mesh {
                primitives: pbr_primitives,
                instances: mesh_instances.remove(&mesh_idx).unwrap(),
            });
        }

        pbr_meshes
    }
}

