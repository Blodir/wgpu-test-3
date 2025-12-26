use std::{fs::File, path::Path};

use wgpu_test_3::resource_manager::file_formats::{dds::{create_dds, gltf_img_to_dxgi_format}, materialfile};

use crate::utils::ensure_parent_dir_exists;

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
) -> materialfile::SampledTexture {
    let tex_output_path = format!("assets/shared/{}.placeholder.dds", tex_name);

    export_image_as_dds(
        &data,
        &Path::new(&tex_output_path),
        srgb,
        gltf::material::AlphaMode::Opaque,
    )
    .expect(&format!("Failed to write texture {}", tex_output_path));

    let sampler = materialfile::Sampler {
        mag_filter: materialfile::FilterMode::Linear,
        min_filter: materialfile::FilterMode::Linear,
        mipmap_filter: materialfile::MipmapFilterMode::None,
        wrap_u: materialfile::WrapMode::Repeat,
        wrap_v: materialfile::WrapMode::Repeat,
        wrap_w: materialfile::WrapMode::Repeat,
    };

    materialfile::SampledTexture {
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
) -> materialfile::SampledTexture {
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
        gltf::texture::MagFilter::Linear => materialfile::FilterMode::Linear,
        gltf::texture::MagFilter::Nearest => materialfile::FilterMode::Nearest,
    };

    let min_filter = match gltf_sampler
        .min_filter()
        .unwrap_or(gltf::texture::MinFilter::Linear)
    {
        gltf::texture::MinFilter::Nearest
        | gltf::texture::MinFilter::NearestMipmapNearest
        | gltf::texture::MinFilter::NearestMipmapLinear => materialfile::FilterMode::Nearest,

        gltf::texture::MinFilter::Linear
        | gltf::texture::MinFilter::LinearMipmapNearest
        | gltf::texture::MinFilter::LinearMipmapLinear => materialfile::FilterMode::Linear,
    };

    let mipmap_filter = match gltf_sampler.min_filter() {
        Some(
            gltf::texture::MinFilter::NearestMipmapNearest
            | gltf::texture::MinFilter::LinearMipmapNearest,
        ) => materialfile::MipmapFilterMode::Nearest,
        Some(
            gltf::texture::MinFilter::NearestMipmapLinear
            | gltf::texture::MinFilter::LinearMipmapLinear,
        ) => materialfile::MipmapFilterMode::Linear,
        Some(gltf::texture::MinFilter::Nearest | gltf::texture::MinFilter::Linear) | None => {
            materialfile::MipmapFilterMode::None
        }
    };

    let wrap_u = match gltf_sampler.wrap_s() {
        gltf::texture::WrappingMode::Repeat => materialfile::WrapMode::Repeat,
        gltf::texture::WrappingMode::ClampToEdge => materialfile::WrapMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => materialfile::WrapMode::MirroredRepeat,
    };

    let wrap_v = match gltf_sampler.wrap_t() {
        gltf::texture::WrappingMode::Repeat => materialfile::WrapMode::Repeat,
        gltf::texture::WrappingMode::ClampToEdge => materialfile::WrapMode::ClampToEdge,
        gltf::texture::WrappingMode::MirroredRepeat => materialfile::WrapMode::MirroredRepeat,
    };

    // gltf has no wrap_w
    let wrap_w = materialfile::WrapMode::Repeat;

    materialfile::SampledTexture {
        source: tex_output_path,
        sampler: materialfile::Sampler {
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
) -> Option<materialfile::SampledTexture> {
    if let Some(tex_info) = material.pbr_metallic_roughness().base_color_texture() {
        Some(
            bake_texture(
                &tex_info.texture(),
                images,
                true,
                material.alpha_mode(),
                model_name,
                "base_color",
            )
        )
    } else {
        None
        /*
        println!("WARNING: material doesn't have a base color texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[1u16, 1u16, 1u16, 1u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, true, "base_color")
        */
    }
}

fn bake_metallic_roughness_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> Option<materialfile::SampledTexture> {
    if let Some(tex_info) = material
        .pbr_metallic_roughness()
        .metallic_roughness_texture()
    {
        Some(
            bake_texture(
                &tex_info.texture(),
                images,
                false,
                material.alpha_mode(),
                model_name,
                "metallic_roughness",
            )
        )
    } else {
        None
        /*
        println!("WARNING: material doesn't have a metallic roughness texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[0x0000u16, 0x3800u16, 0x0000u16, 0x3C00u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "metallic_roughness")
        */
    }
}

fn bake_normals_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> Option<materialfile::SampledTexture> {
    if let Some(tex_info) = material.normal_texture() {
        Some(
            bake_texture(
                &tex_info.texture(),
                images,
                false,
                material.alpha_mode(),
                model_name,
                "normals",
            )
        )
    } else {
        None
        /*
        println!("WARNING: material doesn't have a normals texture, using placeholder");
        let data = gltf::image::Data {
            pixels: bytemuck::cast_slice(&[0x0000u16, 0x0000u16, 0x3C00u16, 0x3C00u16]).to_vec(),
            format: gltf::image::Format::R16G16B16A16,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "normals")
        */
    }
}

fn bake_occlusion_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> Option<materialfile::SampledTexture> {
    if let Some(tex_info) = material.occlusion_texture() {
        Some(
            bake_texture(
                &tex_info.texture(),
                images,
                false,
                material.alpha_mode(),
                model_name,
                "occlusion",
            )
        )
    } else {
        None
        /*
        println!("WARNING: material doesn't have an occlusion texture, using placeholder");
        let data = gltf::image::Data {
            pixels: vec![u8::MAX],
            format: gltf::image::Format::R8,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "occlusion")
        */
    }
}

fn bake_emissive_tex(
    material: &gltf::Material,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
) -> Option<materialfile::SampledTexture> {
    if let Some(tex_info) = material.emissive_texture() {
        Some(
            bake_texture(
                &tex_info.texture(),
                images,
                false,
                material.alpha_mode(),
                model_name,
                "emissive",
            )
        )
    } else {
        None
        /*
        println!("WARNING: material doesn't have an emissive texture, using placeholder");
        let data = gltf::image::Data {
            pixels: vec![0u8],
            format: gltf::image::Format::R8,
            width: 1,
            height: 1,
        };
        bake_placeholder_texture(data, false, "emissive")
        */
    }
}

pub fn bake_material(
    material: &gltf::Material,
    buffers: &Vec<gltf::buffer::Data>,
    images: &Vec<gltf::image::Data>,
    model_name: &str,
    json_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let base_color_texture = bake_base_color_tex(&material, images, model_name);
    let normal_texture = bake_normals_tex(&material, images, model_name);
    let metallic_roughness_texture = bake_metallic_roughness_tex(&material, images, model_name);
    let occlusion_texture = bake_occlusion_tex(&material, images, model_name);
    let emissive_texture = bake_emissive_tex(&material, images, model_name);
    let material = materialfile::Material {
        base_color_factor: material.pbr_metallic_roughness().base_color_factor(),
        metallic_factor: material.pbr_metallic_roughness().metallic_factor(),
        roughness_factor: material.pbr_metallic_roughness().roughness_factor(),
        emissive_factor: material.emissive_factor(),
        normal_texture_scale: material.normal_texture().map(|n| n.scale()).unwrap_or(1f32),
        occlusion_strength: material.occlusion_texture().map(|o| o.strength()).unwrap_or(1f32),
        alpha_mode: match material.alpha_mode() {
            gltf::material::AlphaMode::Blend => materialfile::AlphaMode::Blend,
            gltf::material::AlphaMode::Mask => materialfile::AlphaMode::Mask,
            gltf::material::AlphaMode::Opaque => materialfile::AlphaMode::Opaque,
        },
        alpha_cutoff: material.alpha_cutoff().unwrap_or(0.5f32),
        double_sided: material.double_sided(),
        base_color_texture,
        normal_texture,
        metallic_roughness_texture,
        occlusion_texture,
        emissive_texture,
    };

    let json = serde_json::to_string_pretty(&material)?;
    std::fs::write(json_path, json)?;
    Ok(())
}
