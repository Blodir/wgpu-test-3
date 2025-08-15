fn read_shaders(path: &str) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents)?;
    Ok(contents)
}

fn read_fallback_shaders() -> std::io::Result<String> {
    let mut file = std::fs::File::open("src/renderer/shaders/fallback.wgsl")?;
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents)?;
    Ok(contents)
}

pub fn create_shader_module(device: &wgpu::Device, path: &str) -> wgpu::ShaderModule {
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    {
        let source = wgpu::ShaderSource::Wgsl(read_shaders(path).unwrap_or_else(|e| {
            println!("Error reading shader: {}", e);
            read_fallback_shaders().unwrap()
        }).into());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source
        });

        device.poll(wgpu::Maintain::Wait);
        let error = pollster::FutureExt::block_on(device.pop_error_scope());
        match error {
            Some(e) => Err(e),
            None => Ok(shader),
        }
    }.unwrap_or_else(|e| {
        println!("Shader compilation failed: {}", e);
        let source = wgpu::ShaderSource::Wgsl(read_fallback_shaders().unwrap().into());
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source
        })
    })
}

