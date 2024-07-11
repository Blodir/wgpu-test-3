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
    pub normal: u32,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub indices: u32,
    pub attributes: PrimitiveAttributes,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mesh {
    pub name: Option<String>,
    pub primitives: Vec<Primitive>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Node {
    pub name: Option<String>,
    pub mesh: Option<u8>,
    pub rotation: Option<[f64; 4]>,
    pub translation: Option<[f64; 3]>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Scene {
    pub name: Option<String>,
    pub nodes: Vec<u8>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct JSONData {
    pub accessors: Vec<Accessor>,
    pub asset: Asset,
    #[serde(rename = "bufferViews")]
    pub buffer_views: Vec<BufferView>,
    pub buffers: Vec<Buffer>,
    pub meshes: Vec<Mesh>,
    pub nodes: Vec<Node>,
    pub scene: u8,
    pub scenes: Vec<Scene>,
}

pub struct JSONChunk {
    pub chunk_length: u32,
    pub chunk_type: String,
    pub chunk_data: JSONData,
    pub raw_json: String,
}

pub enum DataBuffer {
    I8(Vec<i8>),
    U8(Vec<u8>),
    I16(Vec<i16>),
    U16(Vec<u16>),
    U32(Vec<u32>),
    F32(Vec<f32>),
}

pub struct GLBObject {
    pub magic: String,
    pub version: u32,
    pub length: u32, // entire file in bytes
    pub json_chunk: JSONChunk,
    pub binary_buffer: Vec<u8>,
    //pub accessor_data_buffers: Vec<DataBuffer>,
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

fn convert_accessor_element_buffer(accessor: &Accessor, buffer: Vec<u8>) -> DataBuffer {
    match accessor.component_type {
        ComponentType::SignedByte => DataBuffer::I8(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to i8 failed: {}", e)).to_vec()),
        ComponentType::UnsignedByte => DataBuffer::U8(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to i8 failed: {}", e)).to_vec()),
        ComponentType::SignedShort => DataBuffer::I16(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to i16 failed: {}", e)).to_vec()),
        ComponentType::UnsignedShort => DataBuffer::U16(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to u16 failed: {}", e)).to_vec()),
        ComponentType::UnsignedInt => DataBuffer::U32(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to u32 failed: {}", e)).to_vec()),
        ComponentType::Float => DataBuffer::F32(try_cast_slice(&buffer).unwrap_or_else(|e| panic!("Casting accessor data buffer to f32 failed: {}", e)).to_vec()),
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
        let binary_buffer = GLBObject::parse_binary_buffer(file, &json_chunk.chunk_data)?;

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

    fn parse_binary_buffer(file: &mut File, json_data: &JSONData) -> io::Result<Vec<u8>> {
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

