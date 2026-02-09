use std::{collections::HashMap, sync::Arc};

use super::wgpu_context::WgpuContext;
use super::assets::io::asset_formats::materialfile;

pub struct SamplerCache {
    pub cache: HashMap<materialfile::Sampler, Arc<wgpu::Sampler>>,
}
impl SamplerCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new()
        }
    }
    pub fn get(&mut self, sampler: &materialfile::Sampler, wgpu_context: &WgpuContext) -> Arc<wgpu::Sampler> {
        self.cache
            .entry(sampler.clone())
            .or_insert_with(|| {
                Arc::new(
                    wgpu_context
                        .device
                        .create_sampler(&sampler.to_wgpu_descriptor(None))
                )
            }).clone()
    }
}
