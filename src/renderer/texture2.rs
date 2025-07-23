fn align_to_256(n: usize) -> usize {
    (n + 255) & !255
}

pub fn upload_texture(
    data: &[u8],
    base_width: u32,
    base_height: u32,
    mips: u32,
    layers: u32,
    format: wgpu::TextureFormat,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> wgpu::Texture {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: base_width,
            height: base_height,
            depth_or_array_layers: layers,
        },
        mip_level_count: mips,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });

    let mut current_offset = 0usize;

    for layer in 0..layers {
        for mip in 0..mips {
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

            let width = (base_width >> mip).max(1);
            let height = (base_height >> mip).max(1);
            let extent = wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            };

            match format {
                wgpu::TextureFormat::Bc6hRgbUfloat => {
                    let block_width = ((width + 3) / 4) as usize;
                    let block_height = ((height + 3) / 4) as usize;

                    let row_size = block_width * 16; // unaligned row in bytes
                    let padded_row_size = align_to_256(row_size as usize);
                    let mip_size = row_size * block_height;

                    let mip_data = &data[current_offset..current_offset + mip_size];

                    // Pad the data row-by-row
                    let mut padded_data = vec![0u8; padded_row_size * block_height];
                    for row in 0..block_height {
                        let src_start = row * row_size;
                        let dst_start = row * padded_row_size;
                        padded_data[dst_start..dst_start + row_size]
                            .copy_from_slice(&mip_data[src_start..src_start + row_size]);
                    }

                    queue.write_texture(
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
                _ => todo!(),
            }
        }
    }

    texture
}
