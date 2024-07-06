use std::fs::File;
use std::io::{self, Read, Seek};
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
    pub byte_offset: Option<u8>,
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

#[derive(Serialize, Deserialize, Debug)]
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
    pub byte_offset : u32,
    #[serde(rename = "byteStride")]
    pub byte_stride : Option<u32>,
    pub target: BufferViewTarget,
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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Primitive {
    pub indices: u32,
    pub attributes: PrimitiveAttributes,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Mesh {
    pub name: String,
    pub primitives: Vec<Primitive>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Node {
    pub name: String,
    pub mesh: u8,
    pub rotation: [f64; 4],
    pub translation: [f64; 3],
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Scene {
    pub name: String,
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

pub struct BinaryChunk {
}

pub struct GLBObject {
    pub magic: String,
    pub version: u32,
    pub length: u32, // entire file in bytes
    pub json_chunk: JSONChunk,
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

        Ok(
            Self {
                magic, version, length, json_chunk
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
        let chunk_data = serde_json::from_str(&chunk_data_string)?;

        Ok(JSONChunk { chunk_length, chunk_type, raw_json: chunk_data_string, chunk_data })
    }

    fn parse_binary_chunk(file: &mut File, json_data: &JSONData) -> io::Result<BinaryChunk> {
        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let chunk_length = u32::from_le_bytes(length_buffer);

        let mut type_buffer = [0u8; 4];
        file.read_exact(&mut type_buffer)?;
        let chunk_type = buffer_to_ascii(&type_buffer);

        // start reading the buffer (there's only one in gltf for some reason!)

        let start_of_buffer_offset = file.seek(io::SeekFrom::Current(0))?;
        let mut current_buffer_view = 0u8;
        for buffer_view in &json_data.buffer_views {
            if buffer_view.byte_offset > 0 {
                file.seek(io::SeekFrom::Start(start_of_buffer_offset + buffer_view.byte_offset as u64))?;
            }
            let start_of_buffer_view_offset = file.seek(io::SeekFrom::Current(0))?;
            let accessor = json_data.accessors.iter().find(|x| x.buffer_view == current_buffer_view).unwrap_or_else(|| panic!("No accessor found for bufferview: {}", current_buffer_view));
            if let Some(accessor_byte_offset) = accessor.byte_offset {
                let current_offset = file.seek(io::SeekFrom::Current(0))?;
                file.seek(io::SeekFrom::Start(current_offset + accessor_byte_offset as u64))?;
            }

            let stride = buffer_view.byte_stride.unwrap_or(0u32);

            // read actual data elements
            loop {
                let mut current_offset = file.seek(io::SeekFrom::Current(0))?;
                if current_offset > start_of_buffer_view_offset + buffer_view.byte_length as u64 {
                    break;
                }

                // read differently depending on accessor_type and componentType ...
                // TODO

                if stride > 0 {
                    current_offset = file.seek(io::SeekFrom::Current(0))?;
                    file.seek(io::SeekFrom::Start(current_offset + stride as u64))?;
                }
            }

            current_buffer_view += 1;
        }
        Ok(BinaryChunk {})
    }
}

