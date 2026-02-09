use crate::main::wgpu_context::WgpuContext;

pub struct TextureLoadData {
    pub data: Vec<u8>,
    pub base_width: u32,
    pub base_height: u32,
    pub mips: u32,
    pub layers: u32,
    pub format: wgpu::TextureFormat,
}

fn align_to_256(n: usize) -> usize {
    (n + 255) & !255
}

pub fn upload_texture(
    data: &TextureLoadData,
    wgpu_context: &WgpuContext,
) -> wgpu::Texture {

    let texture = wgpu_context.device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: data.base_width,
            height: data.base_height,
            depth_or_array_layers: data.layers,
        },
        mip_level_count: data.mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: data.format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut current_offset = 0usize;

    for layer in 0..data.layers {
        for mip in 0..data.mips {
            let image_copy_texture = wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: mip,
                origin: wgpu::Origin3d {
                    x: 0,
                    y: 0,
                    z: layer,
                },
                aspect: wgpu::TextureAspect::All,
            };

            let width = (data.base_width >> mip).max(1);
            let height = (data.base_height >> mip).max(1);
            let extent = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            match data.format {
                wgpu::TextureFormat::Bc6hRgbUfloat => {
                    let block_width = ((width + 3) / 4) as usize;
                    let block_height = ((height + 3) / 4) as usize;

                    let row_size = block_width * 16; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size as usize);
                    let mip_size = row_size * block_height;

                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad the data row-by-row
                    let mut padded_data = vec![0u8; padded_row_size * block_height];
                    for row in 0..block_height {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(block_height as u32),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                wgpu::TextureFormat::Bc2RgbaUnormSrgb | wgpu::TextureFormat::Bc2RgbaUnorm => {
                    let block_width = ((width + 3) / 4) as usize;
                    let block_height = ((height + 3) / 4) as usize;

                    const BYTES_PER_BLOCK: usize = 16; // BC2/DXT3 block size
                    let row_size = block_width * BYTES_PER_BLOCK; // unaligned row in bytes (in blocks)
                    let padded_row_size = align_to_256(row_size);
                    let mip_size = row_size * block_height;

                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad the data row-by-row to meet the bytes_per_row alignment
                    let mut padded_data = vec![0u8; padded_row_size * block_height];
                    for row in 0..block_height {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(block_height as u32),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                wgpu::TextureFormat::R8Unorm => {
                    let bytes_per_pixel = 1;
                    let row_size = width as usize * bytes_per_pixel; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size);
                    let mip_size = row_size * height as usize;

                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad each row to 256-byte alignment
                    let mut padded_data = vec![0u8; padded_row_size * height as usize];
                    for row in 0..height as usize {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(height),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm => {
                    let bytes_per_pixel = 4;
                    let row_size = width as usize * bytes_per_pixel; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size);
                    let mip_size = row_size * height as usize;

                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad each row to 256-byte alignment
                    let mut padded_data = vec![0u8; padded_row_size * height as usize];
                    for row in 0..height as usize {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(height),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                wgpu::TextureFormat::Rgba32Float => {
                    // 4 channels * 32-bit float
                    let bytes_per_pixel = std::mem::size_of::<f32>() * 4; // 16
                    let row_size = width as usize * bytes_per_pixel; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size); // must be multiple of 256
                    let mip_size = row_size * height as usize; // bytes in this mip (tightly packed)

                    // If your source is &[u8] containing RGBA32F bytes:
                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad each row to 256-byte alignment
                    let mut padded_data = vec![0u8; padded_row_size * height as usize];
                    for row in 0..height as usize {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(height),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                wgpu::TextureFormat::Rgba16Float => {
                    let bytes_per_pixel = std::mem::size_of::<u16>() * 4;
                    let row_size = width as usize * bytes_per_pixel; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size); // must be multiple of 256
                    let mip_size = row_size * height as usize; // bytes in this mip (tightly packed)

                    // If your source is &[u8] containing RGBA32F bytes:
                    let mip_data = &data.data[current_offset..current_offset + mip_size];

                    // Pad each row to 256-byte alignment
                    let mut padded_data = vec![0u8; padded_row_size * height as usize];
                    for row in 0..height as usize {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    wgpu_context.queue.write_texture(
                        image_copy_texture,
                        &padded_data,
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_row_size as u32),
                            rows_per_image: Some(height),
                        },
                        extent,
                    );

                    current_offset += mip_size;
                }
                f => {
                    dbg!(f);
                    todo!();
                }
            }
        }
    }

    texture
}
