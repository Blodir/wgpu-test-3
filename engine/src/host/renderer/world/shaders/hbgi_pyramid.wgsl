struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi: vec4<f32>,
    @location(1) depth: vec4<f32>,
    @location(2) normal: vec4<f32>,
}

@group(0) @binding(0) var source_hbgi_texture: texture_2d<f32>;
@group(0) @binding(1) var source_hbgi_sampler: sampler;
@group(0) @binding(2) var source_depth_texture: texture_depth_2d;
@group(0) @binding(3) var source_normal_texture: texture_2d<f32>;
@group(0) @binding(4) var source_normal_sampler: sampler;

@group(0) @binding(5) var downsample_hbgi_texture: texture_2d<f32>;
@group(0) @binding(6) var downsample_depth_texture: texture_2d<f32>;
@group(0) @binding(7) var downsample_normal_texture: texture_2d<f32>;
@group(0) @binding(8) var downsample_sampler: sampler;

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    var pos = array(
        vec2(1.0, 1.0),
        vec2(1.0, -1.0),
        vec2(-1.0, -1.0),
        vec2(-1.0, 1.0)
    );
    var tex_coords = array(
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
        vec2(0.0, 0.0)
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0);
}

fn sample_depth_hiz_2x2(coord: vec2u) -> f32 {
    let dims = textureDimensions(downsample_depth_texture);
    let max_coord = dims - vec2u(1u, 1u);
    let c00 = min(coord, max_coord);
    let c10 = min(coord + vec2u(1u, 0u), max_coord);
    let c01 = min(coord + vec2u(0u, 1u), max_coord);
    let c11 = min(coord + vec2u(1u, 1u), max_coord);
    let d00 = textureLoad(downsample_depth_texture, vec2i(c00), 0).x;
    let d10 = textureLoad(downsample_depth_texture, vec2i(c10), 0).x;
    let d01 = textureLoad(downsample_depth_texture, vec2i(c01), 0).x;
    let d11 = textureLoad(downsample_depth_texture, vec2i(c11), 0).x;
    return max(max(d00, d10), max(d01, d11));
}

@fragment
fn fs_copy_base(in: VertexOutput) -> FragmentOutput {
    let dims = textureDimensions(source_depth_texture);
    let max_coord = vec2f(dims) - vec2f(1.0);
    let coord = vec2i(clamp(in.tex_coords * vec2f(dims), vec2f(0.0), max_coord));
    let hbgi = textureSampleLevel(source_hbgi_texture, source_hbgi_sampler, in.tex_coords, 0.0);
    let depth = textureLoad(source_depth_texture, coord, 0);
    let normal = textureSampleLevel(
        source_normal_texture,
        source_normal_sampler,
        in.tex_coords,
        0.0
    );
    return FragmentOutput(
        hbgi,
        vec4f(depth, 0.0, 0.0, 1.0),
        normal
    );
}

@fragment
fn fs_downsample(in: VertexOutput) -> FragmentOutput {
    let dst_coord = vec2u(in.clip_position.xy);
    let depth = sample_depth_hiz_2x2(dst_coord * 2u);
    let hbgi = textureSampleLevel(downsample_hbgi_texture, downsample_sampler, in.tex_coords, 0.0);
    let normal = textureSampleLevel(
        downsample_normal_texture,
        downsample_sampler,
        in.tex_coords,
        0.0
    );
    return FragmentOutput(
        hbgi,
        vec4f(depth, 0.0, 0.0, 1.0),
        vec4f(safe_normalize3(normal.xyz), normal.w)
    );
}
