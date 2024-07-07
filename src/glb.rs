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
    pub accessor_data_buffers: Vec<DataBuffer>,
}

fn get_accessor_component_count(accessor: &Accessor) -> u8 {
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

fn get_accessor_component_size(accessor: &Accessor) -> u8 {
    match accessor.component_type {
        ComponentType::SignedByte => 8,
        ComponentType::UnsignedByte => 8,
        ComponentType::SignedShort => 16,
        ComponentType::UnsignedShort => 16,
        ComponentType::UnsignedInt => 32,
        ComponentType::Float => 32,
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
        let binary_chunk = GLBObject::parse_binary_chunk(file, &json_chunk.chunk_data)?;

        Ok(
            Self {
                magic, version, length, json_chunk, accessor_data_buffers: binary_chunk
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

    fn parse_binary_chunk(file: &mut File, json_data: &JSONData) -> io::Result<Vec<DataBuffer>> {
        let mut length_buffer = [0u8; 4];
        file.read_exact(&mut length_buffer)?;
        let chunk_length = u32::from_le_bytes(length_buffer);

        let mut type_buffer = [0u8; 4];
        file.read_exact(&mut type_buffer)?;
        let chunk_type = buffer_to_ascii(&type_buffer);

        // start reading the buffer (there's only one in glb so we don't have to loop!)
        let mut accessor_data_buffers: Vec<DataBuffer> = Vec::new();
        let start_of_buffer_offset = file.seek(io::SeekFrom::Current(0))?;

        let mut current_buffer_view = 0u8;
        let mut current_buffer_view_offset = start_of_buffer_offset;

        for buffer_view in &json_data.buffer_views {
            // go to the start of the buffer view
            let mut current_offset = current_buffer_view_offset + buffer_view.byte_offset as u64;
            file.seek(io::SeekFrom::Start(current_offset))?;

            let stride = buffer_view.byte_stride.unwrap_or(0u32);
            let accessors = json_data.accessors.iter().filter(|x| x.buffer_view == current_buffer_view);
            for accessor in accessors {
                // go to the start of the accessor
                let accessor_byte_offset = accessor.byte_offset.unwrap_or(0u32) as u64;
                current_offset += accessor_byte_offset;
                file.seek(io::SeekFrom::Start(current_offset))?;
                 
                let element_byte_length = get_accessor_component_count(accessor) * get_accessor_component_size(accessor);
                let mut accessor_byte_buffer: Vec<u8> = Vec::with_capacity((accessor.count * element_byte_length as u32).try_into().unwrap());
                for _i in 0..accessor.count {
                    // read values to byte buffer
                    let mut temp = vec![0u8, element_byte_length];
                    file.read_exact(&mut temp)?;
                    accessor_byte_buffer.extend(temp);

                    // stride forward
                    current_offset += stride as u64;
                    file.seek(io::SeekFrom::Start(current_offset))?;
                }
                // write the data
                let data_buffer = convert_accessor_element_buffer(accessor, accessor_byte_buffer);
                accessor_data_buffers.push(data_buffer);
            }
            // prep next buffer view
            current_buffer_view_offset += (buffer_view.byte_offset + buffer_view.byte_length) as u64;
            current_buffer_view += 1;
        }
        Ok(
            accessor_data_buffers
        )
    }
}

