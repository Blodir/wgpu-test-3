@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

@group(1) @binding(0)
var<uniform> material: vec4<f32>;

struct InstanceInput {
    @location(0) m_1: vec4<f32>,
    @location(1) m_2: vec4<f32>,
    @location(2) m_3: vec4<f32>,
    @location(3) m_4: vec4<f32>,
}

struct VertexInput {
    @location(4) position: vec3<f32>,
    @location(5) normal: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) position: vec3<f32>
}

@vertex
fn vs_main(
    instance: InstanceInput,
    model: VertexInput,
) -> VertexOutput {
    let transform = mat4x4<f32>(
        instance.m_1,
        instance.m_2,
        instance.m_3,
        instance.m_4,
    );
    var out: VertexOutput;
    out.clip_position = view_proj * transform * vec4<f32>(model.position, 1.0);
    out.normal = model.normal;
    out.position = model.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var lightDir = normalize(vec3<f32>(-1., 1., 1.));
    var color = (dot(in.normal, lightDir) + 1.) / 2.;
    return vec4<f32>(vec3(color), 1.) * material;
}

