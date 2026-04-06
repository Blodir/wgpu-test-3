@group(0) @binding(0) var<uniform> sun_shadow_light_view_proj: mat4x4<f32>;

struct InstanceInput {
    @location(0) m_1: vec4<f32>,
    @location(1) m_2: vec4<f32>,
    @location(2) m_3: vec4<f32>,
    @location(3) m_4: vec4<f32>,
}

struct VertexInput {
    @location(8) position: vec3<f32>,
}

@vertex
fn vs_main(instance: InstanceInput, model: VertexInput) -> @builtin(position) vec4<f32> {
    let transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );
    let world_position = transform * vec4<f32>(model.position, 1.0);
    return sun_shadow_light_view_proj * world_position;
}
