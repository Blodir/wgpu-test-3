@group(0) @binding(0) var skybox_texture: texture_2d<f32>;
@group(0) @binding(1) var skybox_texture_sampler: sampler;
@group(0) @binding(2) var resolve_texture: texture_2d<f32>;
@group(0) @binding(3) var resolve_texture_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
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
    var tex_coords = array(
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
        vec2(0.0, 1.0),
    );
    let p = pos[vertex_index];
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = vec2f(in.tex_coords.x, 1.0 - in.tex_coords.y);
    let skybox_sample =
        textureSample(
            skybox_texture,
            skybox_texture_sampler,
            uv
        );

    let resolve_sample =
        textureSample(
            resolve_texture,
            resolve_texture_sampler,
            uv
        );

    var col = mix(skybox_sample.xyz, resolve_sample.xyz, resolve_sample.w);

    // exposure
    let exposure = 1.0;
    let exposure_factor = pow(2.0, exposure);
    col = col * exposure_factor;

    // reinhard tone mapping
    col = col / (col + vec3f(1.0));

    return vec4f(col, 1.0);
}
