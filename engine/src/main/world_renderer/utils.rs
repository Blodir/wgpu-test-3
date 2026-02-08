use glam::Quat;

fn read_shaders(path: &str) -> std::io::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut contents = String::new();
    std::io::Read::read_to_string(&mut file, &mut contents)?;
    Ok(contents)
}

fn read_fallback_shaders() -> std::io::Result<String> {
    let mut file = std::fs::File::open("engine/src/main/world/shaders/fallback.wgsl")?;
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

pub fn lerpf32(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

pub fn lerpu64(a: u64, b: u64, t: f32) -> u64 {
    a + ((b - a) as f32 * t).round() as u64
}

pub trait QuatExt {
    fn nlerp(self, other: Quat, t: f32) -> Quat;
}

impl QuatExt for Quat {
    #[inline]
    fn nlerp(self, other: Quat, t: f32) -> Quat {
        let mut b = other;
        if self.dot(other) < 0.0 {
            b = -b;
        }
        (self * (1.0 - t) + b * t).normalize()
    }
}
