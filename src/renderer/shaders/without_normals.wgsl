@group(0) @binding(0)
var<uniform> view_proj: mat4x4<f32>;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(1) position: vec3<f32>
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = view_proj * vec4<f32>(model.position, 1.0);
    out.position = model.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(vec3(.5, .5, .5), 1.);
}

