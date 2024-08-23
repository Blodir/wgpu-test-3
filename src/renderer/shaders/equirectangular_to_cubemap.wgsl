// https://learnopengl.com/PBR/IBL/Diffuse-irradiance

@group(0) @binding(0) var equirectangular_texture: texture_2d<f32>;
@group(0) @binding(1) var equirectangular_texture_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
}

const invAtan = vec2(0.1593, 0.3183);
fn sample_equirectangular_map(v: vec3f) -> vec2f {
    let uv = vec2(atan(v.z, v.x), asin(v.y));
    uv *= invAtan;
    uv += 0.5;
    return uv;
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = model.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
}

