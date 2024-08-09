use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;
use std::fs;
use std::io::{self, Read};
use pollster::FutureExt as _;

#[derive(Hash, Eq, PartialEq, Debug, Clone, Ord, PartialOrd)]
pub enum ShaderCapability {
    VertexNormals,
    Material,
}

#[derive(Default)]
pub struct ShaderCache {
    pipelines: HashMap<BTreeSet<ShaderCapability>, Shader>,
}

impl ShaderCache {
    pub fn get_shader(&mut self, caps: &BTreeSet<ShaderCapability>, device: &wgpu::Device) -> &Shader {
        self.pipelines.entry(caps.clone()).or_insert_with(|| {
            Shader::new(caps, device)
        })
    }
}

pub struct Shader {
    shader_module: wgpu::ShaderModule
}
impl Shader {
    pub fn new(caps: &BTreeSet<ShaderCapability>, device: &wgpu::Device) -> Self {
        Self {
            shader_module: Self::create_shader_module(device, caps)
        }
    }

    pub fn get_shader_module_ref(&self) -> &wgpu::ShaderModule {
        &self.shader_module
    }

    fn read_shaders(caps: &BTreeSet<ShaderCapability>) -> io::Result<String> {
        let mut file = fs::File::open(Self::choose_shader(caps))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn read_fallback_shaders() -> io::Result<String> {
        let mut file = fs::File::open("src/renderer/shaders/fallback.wgsl")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        Ok(contents)
    }

    fn create_shader_module(device: &wgpu::Device, caps: &BTreeSet<ShaderCapability>) -> wgpu::ShaderModule {
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        {
            let source = wgpu::ShaderSource::Wgsl(Self::read_shaders(caps).unwrap_or_else(|e| {
                println!("Error reading shader: {}", e);
                Self::read_fallback_shaders().unwrap()
            }).into());
            let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source
            });

            // Poll the device to process any pending errors
            device.poll(wgpu::Maintain::Wait);

            // Check for errors
            let error = device.pop_error_scope().block_on();

            match error {
                Some(e) => Err(e),
                None => Ok(shader),
            }
        }.unwrap_or_else(|e| {
            println!("Shader compilation failed: {}", e);
            let source = wgpu::ShaderSource::Wgsl(Self::read_fallback_shaders().unwrap().into());
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader"),
                source
            })
        })
    }

    fn choose_shader(caps: &BTreeSet<ShaderCapability>) -> String {
        if caps.contains(&ShaderCapability::VertexNormals) && caps.contains(&ShaderCapability::Material) {
            String::from("src/renderer/shaders/shader.wgsl")
        } else {
            String::from("src/renderer/shaders/without_normals.wgsl")
        }
    }
}

