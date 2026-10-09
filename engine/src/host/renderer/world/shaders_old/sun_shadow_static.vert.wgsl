@group(0) @binding(0) var<uniform> sun_shadow_light_view_proj: mat4x4<f32>;

struct InstanceData {
    m_1: vec4<f32>,
    m_2: vec4<f32>,
    m_3: vec4<f32>,
    m_4: vec4<f32>,
    itr_1: vec4<f32>,
    itr_2: vec4<f32>,
    itr_3: vec4<f32>,
}

@group(1) @binding(0) var<storage, read> instances: array<InstanceData>;

struct VertexInput {
    @location(8) position: vec3<f32>,
}

@vertex
fn vs_main(@builtin(instance_index) instance_index: u32, model: VertexInput) -> @builtin(position) vec4<f32> {
    let instance = instances[instance_index];
    let transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );
    let world_position = transform * vec4<f32>(model.position, 1.0);
    return sun_shadow_light_view_proj * world_position;
}
