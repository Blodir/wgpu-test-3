use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek};
use bytemuck::{cast_slice, try_cast_slice};
use serde_json::Result;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};

fn buffer_to_ascii(buffer: &[u8]) -> String {
    buffer.iter().map(|&x| x as char).collect()
}

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
    pub position: u32,
    #[serde(rename = "NORMAL")]
    pub normal: Option<u32>,
    #[serde(rename = "TANGENT")]
    pub tangent: Option<u32>,

    /* The rest of the fields can't be mapped so they're collected in a hashmap:
    * TEXCOORD_n
    * COLOR_n
    * JOINTS_n
    * WEIGHTS_n
    */
    #[serde(flatten)]
    pub additional_fields: HashMap<String, u32>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub indices: u32,
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
    #[serde(rename = "texCoord")]
    pub tex_coord: Option<usize>,
}

/*
* Green channel = roughness
* Blue channel = metalness
*/
#[derive(Serialize, Deserialize, Debug)]
pub struct MetallicRoughnessTexture {
    pub index: usize,
    #[serde(rename = "texCoord")]
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
pub struct Material {
    pub name: Option<String>,
    #[serde(rename = "pbrMetallicRoughness")]
    pub pbr_metallic_roughness: PbrMetallicRoughness,
    // ..
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
    pub materials: Vec<Material>,
    pub textures: Vec<Texture>,
    pub images: Vec<Image>,
    pub samplers: Vec<Sampler>,
}

pub struct JSONChunk {
    pub chunk_length: u32,
    pub chunk_type: String,
    pub chunk_data: SceneDescription,
    pub raw_json: String,
}

pub struct AccessorDataElement<'a> {
    pub buffer_slice: &'a [u8],
    pub stride: u64,
}

pub struct GLBObject {
    pub magic: String,
    pub version: u32,
    pub length: u32,
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

impl GLBObject {
    pub fn new(file: &mut File) -> io::Result<GLBObject> {
        let mut magic_buffer = [0u8; 4];
        file.read_exact(&mut magic_buffer)?;
        let magic = buffer_to_ascii(&magic_buffer);

        let mut version_buffer = [0u8; 4];
        file.read_exact(&mut version_buffer)?;
        let version = u32::from_le_bytes(version_buffer);

        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let length = u32::from_le_bytes(length_buffer);

        let json_chunk = GLBObject::parse_json_chunk(file)?;
        let binary_buffer = GLBObject::parse_binary_buffer(file)?;

        Ok(
            Self {
                magic, version, length, json_chunk, binary_buffer //accessor_data_buffers: binary_chunk
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
        let chunk_data_string = buffer_to_ascii(&data_buffer);
        println!("{}", chunk_data_string);
        let chunk_data = serde_json::from_str(&chunk_data_string)?;

        Ok(JSONChunk { chunk_length, chunk_type, raw_json: chunk_data_string, chunk_data })
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
}

pub struct GLTFSceneRef<'a> {
    pub desc: &'a SceneDescription,
    pub accessor_data: Vec<AccessorDataElement<'a>>,
}

impl<'a> GLTFSceneRef<'a> {
    pub fn new(glb_object: &'a GLBObject) -> Self {
        let desc = &glb_object.json_chunk.chunk_data;
        let accessor_data = GLTFSceneRef::get_accessor_data(&glb_object.binary_buffer, &desc);
        Self { desc, accessor_data }
    }

    fn get_accessor_data(buffer: &'a Vec<u8>, desc: &SceneDescription) -> Vec<AccessorDataElement<'a>> {
        let mut accessor_data: Vec<AccessorDataElement<'a>> = vec![];
        for accessor in &desc.accessors {
            let buffer_view = &desc.buffer_views[accessor.buffer_view as usize];
            let start_offset = buffer_view.byte_offset.unwrap_or(0u32) as usize + accessor.byte_offset.unwrap_or(0u32) as usize;
            let end_offset = buffer_view.byte_offset.unwrap_or(0u32) as usize + buffer_view.byte_length as usize;
            let slice = &buffer[start_offset..end_offset];

            let stride = {
                let data_element_size =
                    get_accessor_component_count(accessor) as u64
                    * get_accessor_component_size(accessor) as u64;
                let s = buffer_view.byte_stride.unwrap_or(0u32);
                if s > 0 { s as u64 } else { data_element_size }
            };

            accessor_data.push(AccessorDataElement { buffer_slice: slice, stride });
        }
        accessor_data
    }
}

