use std::{collections::HashMap, sync::Arc};

use super::{utils, wgpu_context::WgpuContext};

type ShaderId = String;

pub struct ShaderCache {
    pub cache: HashMap<ShaderId, Arc<wgpu::ShaderModule>>,
}
impl ShaderCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new()
        }
    }
    pub fn get(&mut self, shader: ShaderId, wgpu_context: &WgpuContext) -> Arc<wgpu::ShaderModule> {
        self.cache
            .entry(shader.clone())
            .or_insert_with(|| {
                Arc::new(
                    utils::create_shader_module(&wgpu_context.device, &shader)
                )
            }).clone()
    }
}
