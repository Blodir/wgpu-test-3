use std::{fs::File, io::Read as _};

use block_compression::{BC6HSettings, BC7Settings, CompressionVariant};
use ddsfile::{Caps2, Dds, DxgiFormat};

pub fn dxgi_format_to_compression_variant(
    format: &ddsfile::DxgiFormat,
) -> Option<block_compression::CompressionVariant> {
    match format {
        DxgiFormat::BC1_Typeless => Some(CompressionVariant::BC1),
        DxgiFormat::BC1_UNorm => Some(CompressionVariant::BC1),
        DxgiFormat::BC1_UNorm_sRGB => Some(CompressionVariant::BC1),

        DxgiFormat::BC2_Typeless => Some(CompressionVariant::BC2),
        DxgiFormat::BC2_UNorm => Some(CompressionVariant::BC2),
        DxgiFormat::BC2_UNorm_sRGB => Some(CompressionVariant::BC2),

        DxgiFormat::BC3_Typeless => Some(CompressionVariant::BC3),
        DxgiFormat::BC3_UNorm => Some(CompressionVariant::BC3),
        DxgiFormat::BC3_UNorm_sRGB => Some(CompressionVariant::BC3),

        DxgiFormat::BC4_SNorm => Some(CompressionVariant::BC4),
        DxgiFormat::BC4_Typeless => Some(CompressionVariant::BC4),
        DxgiFormat::BC4_UNorm => Some(CompressionVariant::BC4),

        DxgiFormat::BC5_UNorm => Some(CompressionVariant::BC5),
        DxgiFormat::BC5_SNorm => Some(CompressionVariant::BC5),
        DxgiFormat::BC5_Typeless => Some(CompressionVariant::BC5),

        DxgiFormat::BC6H_SF16 => Some(CompressionVariant::BC6H(BC6HSettings::fast())),
        DxgiFormat::BC6H_Typeless => Some(CompressionVariant::BC6H(BC6HSettings::fast())),
        DxgiFormat::BC6H_UF16 => Some(CompressionVariant::BC6H(BC6HSettings::fast())),

        DxgiFormat::BC7_Typeless => Some(CompressionVariant::BC7(BC7Settings::opaque_fast())),
        DxgiFormat::BC7_UNorm => Some(CompressionVariant::BC7(BC7Settings::alpha_fast())),
        DxgiFormat::BC7_UNorm_sRGB => Some(CompressionVariant::BC7(BC7Settings::alpha_fast())),

        _ => None,
    }
}

pub fn dds_format_to_wgpu(format: ddsfile::DxgiFormat) -> wgpu::TextureFormat {
    match format {
        DxgiFormat::BC6H_UF16 => wgpu::TextureFormat::Bc6hRgbUfloat,
        DxgiFormat::BC2_UNorm_sRGB => wgpu::TextureFormat::Bc2RgbaUnormSrgb,
        DxgiFormat::BC2_UNorm => wgpu::TextureFormat::Bc2RgbaUnorm,
        DxgiFormat::R8_UNorm => wgpu::TextureFormat::R8Unorm,
        DxgiFormat::R16G16B16A16_Float => wgpu::TextureFormat::Rgba16Float,
        DxgiFormat::R32G32B32A32_Float => wgpu::TextureFormat::Rgba32Float,
        f => {
            dbg!(f);
            todo!();
        }
    }
}

/// Expects data layout: face0mip0, face0mip1, face1mip0, ..
/// Note: all input data should be in RGBA for compat with the compression library
pub fn create_dds(
    data: Vec<Vec<u8>>,
    format: &DxgiFormat,
    width: u32,
    height: u32,
    layers: u32,
    mips: u32,
    is_cubemap: bool,
    alpha_mode: ddsfile::AlphaMode,
) -> ddsfile::Dds {
    let compressed_data = dxgi_format_to_compression_variant(format).map(|variant| {
        let mut all_blocks = vec![];
        for face in 0..layers {
            for mip in 0..mips {
                let d = &data[(face * mips + mip) as usize];
                let width = (width >> mip).max(1);
                let height = (height >> mip).max(1);

                let required_size = variant.blocks_byte_size(width, height);
                let mut blocks_buffer = vec![0u8; required_size];
                match variant {
                    CompressionVariant::BC6H(settings) => {
                        let u16_slice: &[u16] = bytemuck::cast_slice(&d);
                        let f16_slice: Vec<block_compression::half::f16> = u16_slice
                            .iter()
                            .map(|&bits| block_compression::half::f16::from_bits(bits))
                            .collect();
                        block_compression::encode::compress_rgba16(
                            variant,
                            &f16_slice,
                            &mut blocks_buffer,
                            width,
                            height,
                            width * 4,
                        )
                    }
                    _ => block_compression::encode::compress_rgba8(
                        variant,
                        &d,
                        &mut blocks_buffer,
                        width,
                        height,
                        width * 4,
                    ),
                }

                all_blocks.extend(blocks_buffer);
            }
        }
        all_blocks
    });

    let params = ddsfile::NewDxgiParams {
        height,
        width,
        depth: None,
        format: *format,
        mipmap_levels: if mips > 0 { Some(mips) } else { None },
        array_layers: if layers > 0 { Some(layers) } else { None },
        is_cubemap,
        caps2: if is_cubemap {
            Some(Caps2::CUBEMAP | Caps2::CUBEMAP_ALLFACES)
        } else {
            None
        },
        resource_dimension: ddsfile::D3D10ResourceDimension::Texture2D,
        alpha_mode,
    };

    let mut dds = ddsfile::Dds::new_dxgi(params).expect("Failed to create DDS");
    dds.data = compressed_data.unwrap_or(data.into_iter().flatten().collect());
    dds
}
