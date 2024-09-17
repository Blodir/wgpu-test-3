@group(0) @binding(2) var<uniform> inverse_view_proj_rot: mat4x4<f32>;

@group(1) @binding(0) var environment_texture: texture_cube<f32>;
@group(1) @binding(1) var environment_texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) world_position: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32
) -> VertexOutput {
    var out: VertexOutput;
    var pos = array(
        vec2(1.0, 1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, -1.0),
        vec2(-1.0, 1.0)
    );
    let clip_pos = vec4<f32>(pos[vertex_index], 1, 1);
    let world_pos = inverse_view_proj_rot * clip_pos;
    out.clip_position = clip_pos;
    out.world_position = world_pos;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let rd = normalize(in.world_position.xyz);

    let sample =
        textureSampleLevel(
            environment_texture,
            environment_texture_sampler,
            rd,
            0.0
        );
    var col = sample.rgb;

    // exposure
    let exposure = 0.0;
    let exposure_factor = pow(2.0, exposure);
    col = col * exposure_factor;

    // reinhard tone mapping
    col = col / (col + vec3f(1.0));

    return vec4f(col, sample.a);
}

