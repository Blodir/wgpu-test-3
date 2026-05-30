use wgpu::Origin3d;

use crate::host::wgpu_context::{WgpuContext};

pub struct RWTextureWriteParams {
    data_layout: wgpu::TexelCopyBufferLayout,
    size: wgpu::Extent3d,
    origin: Origin3d,
    mip_level: u32,
    aspect: wgpu::TextureAspect,
}

pub struct RWTexture {
    tex_0: wgpu::Texture,
    tex_1: wgpu::Texture,
    head: u8,
}
impl RWTexture {
    pub fn new(descriptor: wgpu::TextureDescriptor, wgpu_context: &WgpuContext) -> Self {
        let label_0 = descriptor.label.clone().map(|s| { s.to_string() + " tex_0" });
        let label_1 = descriptor.label.clone().map(|s| { s.to_string() + " tex_1" });

        let desc_0 = wgpu::TextureDescriptor {
            label: label_0.as_deref(),
            ..descriptor
        };
        let desc_1 = wgpu::TextureDescriptor {
            label: label_1.as_deref(),
            ..descriptor
        };
        let tex_0 = wgpu_context.device.create_texture(&desc_0);
        let tex_1 = wgpu_context.device.create_texture(&desc_1);

        Self {
            head: 0,
            tex_0,
            tex_1,
        }
    }

    pub fn write(
        &mut self,
        bytes: &[u8],
        params: RWTextureWriteParams,
        wgpu_context: &WgpuContext
    ) {
        let write_tex = if self.head == 0 {
            &self.tex_0
        } else {
            &self.tex_1
        };
        let copy_tex = wgpu::TexelCopyTextureInfo {
            texture: write_tex,
            mip_level: params.mip_level,
            origin: params.origin,
            aspect: params.aspect,
        };
        wgpu_context.queue.write_texture(copy_tex, bytes, params.data_layout, params.size);
    }

    pub fn swap(&mut self) {
        self.head = (self.head + 1) % 2;
    }

    pub fn get_write_idx(&self) -> u8 {
        self.head
    }

    pub fn get_read_idx(&self) -> u8 {
        (self.head + 1) % 2
    }

    pub fn get_write_tex(&self) -> &wgpu::Texture {
        if self.head == 0 {
            &self.tex_0
        } else {
            &self.tex_1
        }
    }

    pub fn get_read_tex(&self) -> &wgpu::Texture {
        if self.head == 1 {
            &self.tex_0
        } else {
            &self.tex_1
        }
    }
}

pub struct RWTextureView {
    view_0: wgpu::TextureView,
    view_1: wgpu::TextureView,
}
impl RWTextureView {
    pub fn new(rw_tex: &RWTexture, desc: &wgpu::TextureViewDescriptor<'_>) -> Self {
        let view_0 = rw_tex.tex_0.create_view(desc);
        let view_1 = rw_tex.tex_1.create_view(desc);
        Self { view_0, view_1 }
    }

    pub fn get_write_view(&self, rw_tex: &RWTexture) -> &wgpu::TextureView {
        if rw_tex.get_write_idx() == 0 {
            &self.view_0
        } else {
            &self.view_1
        }
    }

    pub fn get_read_view(&self, rw_tex: &RWTexture) -> &wgpu::TextureView {
        if rw_tex.get_read_idx() == 0 {
            &self.view_0
        } else {
            &self.view_1
        }
    }
}
