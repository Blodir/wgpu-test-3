async fn run() {
    // load image::DynamicImage,
    // initialize wgpu
    let instance = wgpu::Instance::default();
    let adapter = instance.request_adapter(&Default::default()).await.unwrap();
    let (device, queue) = adapter.request_device(
        &wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits())
        },
        None,
    ).await.unwrap();

    // read .hdr (equirectangular.rs)
    // render prefiltered_env_map (renderer.rs:148)
    // render diffuse_irradiance (renderer.rs:158)

    // write prefiltered_env_map
    // write diffuse_irradiance
}

fn main() {
}

