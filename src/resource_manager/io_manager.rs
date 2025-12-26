use std::{fs::File, io::Read as _};

use ddsfile::{Caps2, Dds};
use generational_arena::Index;
use glam::{Quat, Vec3};

use super::{animation, file_formats::{animationfile, dds, materialfile, modelfile, skeletonfile}, texture::TextureLoadData};

pub enum IoRequest {
    LoadModel { id: Index, path: String },
    LoadMesh { id: Index, path: String },
    LoadMaterial { id: Index, path: String },
    LoadSkeleton { id: Index, path: String },
    LoadAnimationClip { id: Index, path: String },
    LoadAnimation { id: Index, path: String, header: animationfile::AnimationClip },
    LoadTexture { id: Index, path: String, srgb: bool },
}

pub enum IoResponse {
    ModelLoaded { id: Index, model: modelfile::Model },
    MeshLoaded { id: Index, data: Vec<u8> },
    MaterialLoaded { id: Index, material: materialfile::Material },
    SkeletonLoaded { id: Index, skeleton: skeletonfile::Skeleton },
    AnimationClipLoaded { id: Index, clip: animationfile::AnimationClip },
    AnimationLoaded { id: Index, parsed_clip: animation::AnimationClip },
    TextureLoaded { id: Index, data: TextureLoadData },
    Error { path: String, message: String },
}

fn load_json<T>(path: &str) -> Result<T, Box<dyn std::error::Error>>
where
    T: serde::de::DeserializeOwned,
{
    let json_file = std::fs::File::open(path)?;
    let json_reader = std::io::BufReader::new(json_file);
    let model: T = serde_json::from_reader(json_reader)?;
    Ok(model)
}

fn load_bin(path: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    Ok(bytes)
}

fn load_dds(bytes: &mut [u8]) -> TextureLoadData {
    let dds = Dds::read(&mut &bytes[..]).unwrap();

    let format = dds::dds_format_to_wgpu(
        dds.get_dxgi_format()
            .expect("Dds doesn't have a DXGI format."),
    );
    let is_cubemap = dds
        .header
        .caps2
        .contains(Caps2::CUBEMAP);
    let base_width = dds.get_width();
    let base_height = dds.get_height();
    let mips = dds.get_num_mipmap_levels();
    let layers = if is_cubemap { 6 } else { dds.get_num_array_layers() };

    TextureLoadData {
        data: dds.data,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_png(bytes: &mut [u8], srgb: bool) -> TextureLoadData {
    let img: image::DynamicImage = image::load_from_memory(&bytes).unwrap();
    let dimensions = image::GenericImageView::dimensions(&img);
    let (remapped, format): (Vec<u8>, wgpu::TextureFormat) = match (&img, srgb) {
        (image::DynamicImage::ImageRgb32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (image::DynamicImage::ImageRgba32F(_), false) => (
            bytemuck::cast_slice(&img.to_rgba32f().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba32Float,
        ),
        (_, true) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8UnormSrgb,
        ),
        (_, false) => (
            bytemuck::cast_slice(&img.to_rgba8().into_raw()).to_vec(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
    };
    let base_width = dimensions.0;
    let base_height = dimensions.1;
    let mips = 1;
    let layers = 1;

    TextureLoadData {
        data: remapped,
        base_width,
        base_height,
        mips,
        layers,
        format,
    }
}

fn load_texture(path: &str, srgb: bool) -> Result<TextureLoadData, Box<dyn std::error::Error>> {
    let mut file = File::open(path).unwrap();
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).unwrap();

    if bytes.starts_with(b"DDS ") {
        Ok(load_dds(&mut bytes))
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Ok(load_png(&mut bytes, srgb))
    } else {
        Err("invalid texture format".into())
    }
}

pub fn load_animation(
    path: &str,
    header: animationfile::AnimationClip,
) -> Result<animation::AnimationClip, Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;

    let read_f32_ref = |r: &animationfile::BinRef| -> Box<[f32]> {
        let count = r.count as usize;
        let mut output = vec![0f32; count];
        let stride = 4;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output[i] = bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap());
        }

        output.into_boxed_slice()
    };
    let read_vec3_ref = |r: &animationfile::BinRef| -> Box<[Vec3]> {
        let count = r.count as usize;
        let mut output = vec![];
        let stride = 12;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output.push(
                Vec3::from_array([
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 4..idx + 8].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 8..idx + 12].try_into().unwrap()),
                ])
            );
        }

        output.into_boxed_slice()
    };
    let read_quat_ref = |r: &animationfile::BinRef| -> Box<[Quat]> {
        let count = r.count as usize;
        let mut output = vec![];
        let stride = 16;

        for i in 0..count {
            let idx = r.offset as usize + i * stride;
            output.push(
                Quat::from_array([
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx..idx + 4].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 4..idx + 8].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 8..idx + 12].try_into().unwrap()),
                    bytemuck::cast::<[u8; 4], f32>(bytes[idx + 12..idx + 16].try_into().unwrap()),
                ])
            );
        }

        output.into_boxed_slice()
    };

    let duration = header.duration;
    let primitive_groups = header.primitive_groups;
    let tracks: Vec<animation::Track> = header.tracks.iter().map(|track| {
        let target = track.target;
        let shared_times = track.shared_times.as_ref().map(read_f32_ref);
        let translation = track.translation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        let rotation = track.rotation.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_quat_ref(&s.values);
            animation::Channel::<Quat> {
                interpolation, times, values
            }
        });
        let scale = track.scale.as_ref().map(|s| {
            let interpolation = s.interpolation;
            let times = s.times.as_ref().map(read_f32_ref);
            let values = read_vec3_ref(&s.values);
            animation::Channel::<Vec3> {
                interpolation, times, values
            }
        });
        animation::Track {
            target, shared_times, translation, rotation, scale
        }
    }).collect();

    Ok(animation::AnimationClip {
        duration,
        tracks,
        primitive_groups,
    })
}

fn io_worker_loop(
    rx: crossbeam::channel::Receiver<IoRequest>,
    tx: crossbeam::channel::Sender<IoResponse>,
) {
    while let Ok(req) = rx.recv() {
        let result = match req {
            IoRequest::LoadModel { id, path } => load_json::<modelfile::Model>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |model| IoResponse::ModelLoaded { id, model },
                ),
            IoRequest::LoadMesh { id, path } => load_bin(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::MeshLoaded { id, data },
                ),
            IoRequest::LoadMaterial { id, path } => load_json::<materialfile::Material>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |material| IoResponse::MaterialLoaded { id, material },
                ),
            IoRequest::LoadSkeleton { id, path } => load_json::<skeletonfile::Skeleton>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |skeleton| IoResponse::SkeletonLoaded { id, skeleton },
                ),
            IoRequest::LoadAnimationClip { id, path } => load_json::<animationfile::AnimationClip>(&path)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |clip| IoResponse::AnimationClipLoaded { id, clip },
                ),
            IoRequest::LoadAnimation { id, path, header } => load_animation(&path, header)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::AnimationLoaded { id, parsed_clip: data },
                ),
            IoRequest::LoadTexture { id, path, srgb } => load_texture(&path, srgb)
                .map_or_else(
                    |e| IoResponse::Error { path: path.clone(), message: e.to_string() },
                    |data| IoResponse::TextureLoaded { id, data },
                ),
        };

        // ignore send errors on shutdown
        let _ = tx.send(result);
    }
}

pub struct IoManager {
    pub req_tx: crossbeam::channel::Sender<IoRequest>,
    pub res_rx: crossbeam::channel::Receiver<IoResponse>,
    workers: Vec<std::thread::JoinHandle<()>>,
}
impl IoManager {
    pub fn new() -> Self {
        let (req_tx, req_rx) = crossbeam::channel::unbounded();
        let (res_tx, res_rx) = crossbeam::channel::unbounded();

        let workers = (0..2)
            .map(|_| {
                let rx = req_rx.clone();
                let tx = res_tx.clone();
                std::thread::spawn(move || {
                    io_worker_loop(rx, tx);
                })
            })
            .collect();

        Self {
            req_tx, res_rx, workers
        }
    }
}
