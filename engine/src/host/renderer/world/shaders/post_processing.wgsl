@group(0) @binding(0) var skybox_texture: texture_2d<f32>;
@group(0) @binding(1) var skybox_texture_sampler: sampler;
@group(0) @binding(2) var scene_color_texture: texture_2d<f32>;
@group(0) @binding(3) var scene_color_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const FXAA_SPAN_MAX: f32 = 8.0;
const FXAA_REDUCE_MUL: f32 = 1.0 / 8.0;
const FXAA_REDUCE_MIN: f32 = 1.0 / 128.0;

fn luma(rgb: vec3f) -> f32 {
    return dot(rgb, vec3f(0.299, 0.587, 0.114));
}

fn sample_post_color(uv: vec2f) -> vec3f {
    let uv_flipped = vec2f(uv.x, 1.0 - uv.y);
    let skybox_sample = textureSample(skybox_texture, skybox_texture_sampler, uv_flipped);
    let scene_color_sample = textureSample(scene_color_texture, scene_color_sampler, uv_flipped);

    // scene_color_sample.rgb is premultiplied by scene_color_sample.a after raster blending.
    // Composite with skybox using premultiplied-alpha math.
    var col = scene_color_sample.xyz + skybox_sample.xyz * (1.0 - scene_color_sample.w);

    // exposure
    let exposure = -2.0;
    let exposure_factor = pow(2.0, exposure);
    col = col * exposure_factor;

    // reinhard tone mapping
    col = col / (col + vec3f(1.0));
    return col;
}

fn fxaa(uv: vec2f, inv_resolution: vec2f) -> vec3f {
    let rgb_nw = sample_post_color(clamp(uv + vec2f(-1.0, -1.0) * inv_resolution, vec2f(0.0), vec2f(1.0)));
    let rgb_ne = sample_post_color(clamp(uv + vec2f(1.0, -1.0) * inv_resolution, vec2f(0.0), vec2f(1.0)));
    let rgb_sw = sample_post_color(clamp(uv + vec2f(-1.0, 1.0) * inv_resolution, vec2f(0.0), vec2f(1.0)));
    let rgb_se = sample_post_color(clamp(uv + vec2f(1.0, 1.0) * inv_resolution, vec2f(0.0), vec2f(1.0)));
    let rgb_m = sample_post_color(uv);

    let luma_nw = luma(rgb_nw);
    let luma_ne = luma(rgb_ne);
    let luma_sw = luma(rgb_sw);
    let luma_se = luma(rgb_se);
    let luma_m = luma(rgb_m);

    let luma_min = min(luma_m, min(min(luma_nw, luma_ne), min(luma_sw, luma_se)));
    let luma_max = max(luma_m, max(max(luma_nw, luma_ne), max(luma_sw, luma_se)));

    var dir = vec2f(
        -((luma_nw + luma_ne) - (luma_sw + luma_se)),
        (luma_nw + luma_sw) - (luma_ne + luma_se)
    );

    let dir_reduce = max(
        (luma_nw + luma_ne + luma_sw + luma_se) * (0.25 * FXAA_REDUCE_MUL),
        FXAA_REDUCE_MIN
    );
    let rcp_dir_min = 1.0 / (min(abs(dir.x), abs(dir.y)) + dir_reduce);
    dir = clamp(dir * rcp_dir_min, vec2f(-FXAA_SPAN_MAX), vec2f(FXAA_SPAN_MAX)) * inv_resolution;

    let rgb_a = 0.5 * (
        sample_post_color(clamp(uv + dir * (1.0 / 3.0 - 0.5), vec2f(0.0), vec2f(1.0))) +
        sample_post_color(clamp(uv + dir * (2.0 / 3.0 - 0.5), vec2f(0.0), vec2f(1.0)))
    );
    let rgb_b = rgb_a * 0.5 + 0.25 * (
        sample_post_color(clamp(uv + dir * -0.5, vec2f(0.0), vec2f(1.0))) +
        sample_post_color(clamp(uv + dir * 0.5, vec2f(0.0), vec2f(1.0)))
    );
    let luma_b = luma(rgb_b);

    if luma_b < luma_min || luma_b > luma_max {
        return rgb_a;
    }
    return rgb_b;
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
    let uv = in.tex_coords;
    let tex_dims = vec2f(textureDimensions(scene_color_texture, 0));
    let inv_resolution = 1.0 / tex_dims;
    let col = fxaa(uv, inv_resolution);
    return vec4f(col, 1.0);
}
