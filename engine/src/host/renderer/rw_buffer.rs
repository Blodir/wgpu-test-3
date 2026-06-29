use crate::host::{
    renderer::adaptive_buffer::{AdaptiveBuffer, AdaptiveBufferOptions},
    wgpu_context::WgpuContext,
};

pub struct RWBufferOptions {
    pub label: Option<String>,
    pub usage: wgpu::BufferUsages,
}

pub struct RWBuffer {
    buf_0: AdaptiveBuffer,
    buf_1: AdaptiveBuffer,
    head: u8,
}
impl RWBuffer {
    pub fn new(options: RWBufferOptions, wgpu_context: &WgpuContext) -> Self {
        let label_0 = options.label.clone().map(|s| s.to_string() + " buf_0");
        let opt_0 = AdaptiveBufferOptions {
            label: label_0,
            usage: options.usage,
        };
        let label_1 = options.label.clone().map(|s| s.to_string() + " buf_1");
        let opt_1 = AdaptiveBufferOptions {
            label: label_1,
            usage: options.usage,
        };
        let buf_0 = AdaptiveBuffer::new(opt_0, wgpu_context);
        let buf_1 = AdaptiveBuffer::new(opt_1, wgpu_context);

        Self {
            buf_0,
            buf_1,
            head: 0,
        }
    }

    /// Allocates a new buffer if out of space
    pub fn write(&mut self, bytes: &[u8], wgpu_context: &WgpuContext) {
        let write_buf = if self.head == 0 {
            &mut self.buf_0
        } else {
            &mut self.buf_1
        };
        write_buf.write(bytes, wgpu_context);
    }

    pub fn swap(&mut self) {
        self.head = (self.head + 1) % 2;
    }

    pub fn get_write_buf(&self) -> &wgpu::Buffer {
        if self.head == 0 {
            &self.buf_0.buf
        } else {
            &self.buf_1.buf
        }
    }

    pub fn get_read_buf(&self) -> &wgpu::Buffer {
        if self.head == 1 {
            &self.buf_0.buf
        } else {
            &self.buf_1.buf
        }
    }
}
