use wgpu::util::DeviceExt as _;

use crate::host::wgpu_context::WgpuContext;

pub struct AdaptiveBufferOptions {
    pub label: Option<String>,
    pub usage: wgpu::BufferUsages,
}

pub struct AdaptiveBuffer {
    pub buf: wgpu::Buffer,
    options: AdaptiveBufferOptions,
}
impl AdaptiveBuffer {
    pub fn new(options: AdaptiveBufferOptions, wgpu_context: &WgpuContext) -> Self {
        let descriptor = wgpu::BufferDescriptor {
            label: options.label.as_deref(),
            size: 0,
            usage: options.usage,
            mapped_at_creation: false,
        };
        let buf = wgpu_context.device.create_buffer(&descriptor);

        Self { buf, options }
    }

    /// Allocates a new buffer if out of space
    pub fn write(&mut self, bytes: &[u8], wgpu_context: &WgpuContext) {
        let bytes_len = bytes.len() as u64;
        if bytes_len <= self.buf.size() {
            wgpu_context.queue.write_buffer(&self.buf, 0, bytes);
        } else {
            let new_buf =
                wgpu_context
                    .device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: self.options.label.as_deref(),
                        contents: bytes,
                        usage: self.options.usage,
                    });
            self.buf = new_buf;
        }
    }
}
