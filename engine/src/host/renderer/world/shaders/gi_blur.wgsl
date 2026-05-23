@group(0) @binding(0) var raw_hbgi_texture: texture_2d<f32>;
@group(0) @binding(1) var raw_hbgi_sampler: sampler;
@group(0) @binding(2) var raw_hbgi_irradiance_texture: texture_2d<f32>;
@group(0) @binding(3) var raw_hbgi_irradiance_sampler: sampler;
@group(0) @binding(4) var gbuffer_normal_roughness: texture_2d<f32>;
@group(0) @binding(5) var gbuffer_normal_roughness_sampler: sampler;
@group(0) @binding(6) var depth_texture: texture_depth_2d;
@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi: vec4<f32>,
    @location(1) hbgi_irradiance: vec4<f32>,
}

const KERNEL_RADIUS: i32 = 2;
const SPATIAL_SIGMA: f32 = 1.5;
const NORMAL_SIGMA: f32 = 24.0;
const POSITION_SIGMA: f32 = 3.0;

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

fn safe_normalize3(v: vec3<f32>) -> vec3<f32> {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn sample_half_res_uv(pixel_coord: vec2i, dims: vec2i) -> vec2f {
    return (vec2f(pixel_coord) + vec2f(0.5)) / vec2f(dims);
}

fn reconstruct_world_position_from_depth(uv: vec2f, depth: f32) -> vec3f {
    let clip = vec4f(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        depth,
        1.0
    );
    let world = inverse_view_proj * clip;
    if (abs(world.w) > 1e-8) {
        return world.xyz / world.w;
    }
    return vec3f(0.0);
}

fn sample_world_position_from_depth(uv: vec2f) -> vec4f {
    let full_dims = vec2i(textureDimensions(depth_texture));
    let pixel = clamp(vec2i(uv * vec2f(full_dims)), vec2i(0), full_dims - vec2i(1));
    let depth = textureLoad(depth_texture, pixel, 0);
    if (depth >= 1.0) {
        return vec4f(0.0);
    }
    let pixel_uv = (vec2f(pixel) + vec2f(0.5)) / vec2f(full_dims);
    return vec4f(reconstruct_world_position_from_depth(pixel_uv, depth), 1.0);
}

fn bilateral_weight(
    center_normal: vec3f,
    center_position: vec3f,
    sample_normal: vec3f,
    sample_position: vec3f,
    offset: vec2f
) -> f32 {
    let spatial = exp(-dot(offset, offset) / (2.0 * SPATIAL_SIGMA * SPATIAL_SIGMA));
    let normal_term = exp(-max(1.0 - dot(center_normal, sample_normal), 0.0) * NORMAL_SIGMA);
    let position_delta = length(sample_position - center_position);
    let position_term = exp(-position_delta * POSITION_SIGMA);
    return spatial * normal_term * position_term;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let half_dims = vec2i(textureDimensions(raw_hbgi_texture, 0));
    let pixel_f = in.tex_coords * vec2f(half_dims);
    let pixel = clamp(vec2i(pixel_f), vec2i(0), half_dims - vec2i(1));
    let uv = sample_half_res_uv(pixel, half_dims);

    let center_world = sample_world_position_from_depth(uv);
    if (center_world.w < 0.5) {
        return FragmentOutput(vec4f(0.0, 0.0, 1.0, 1.0), vec4f(0.0));
    }

    let center_normal = safe_normalize3(textureSampleLevel(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv,
        0.0
    ).xyz);
    let center_position = center_world.xyz;

    var hbgi_acc = vec4f(0.0);
    var hbgi_irradiance_acc = vec4f(0.0);
    var weight_acc = 0.0;

    for (var y: i32 = -KERNEL_RADIUS; y <= KERNEL_RADIUS; y += 1) {
        for (var x: i32 = -KERNEL_RADIUS; x <= KERNEL_RADIUS; x += 1) {
            let sample_pixel = clamp(pixel + vec2i(x, y), vec2i(0), half_dims - vec2i(1));
            let sample_uv = sample_half_res_uv(sample_pixel, half_dims);
            let sample_world = sample_world_position_from_depth(sample_uv);
            if (sample_world.w < 0.5) {
                continue;
            }

            let sample_normal = safe_normalize3(textureSampleLevel(
                gbuffer_normal_roughness,
                gbuffer_normal_roughness_sampler,
                sample_uv,
                0.0
            ).xyz);
            let offset = vec2f(f32(x), f32(y));
            let weight = bilateral_weight(
                center_normal,
                center_position,
                sample_normal,
                sample_world.xyz,
                offset
            );

            hbgi_acc += textureSampleLevel(raw_hbgi_texture, raw_hbgi_sampler, sample_uv, 0.0) * weight;
            hbgi_irradiance_acc += textureSampleLevel(
                raw_hbgi_irradiance_texture,
                raw_hbgi_irradiance_sampler,
                sample_uv,
                0.0
            ) * weight;
            weight_acc += weight;
        }
    }

    if (weight_acc <= 1e-6) {
        let raw_hbgi = textureSampleLevel(raw_hbgi_texture, raw_hbgi_sampler, uv, 0.0);
        let raw_hbgi_irradiance = textureSampleLevel(
            raw_hbgi_irradiance_texture,
            raw_hbgi_irradiance_sampler,
            uv,
            0.0
        );
        return FragmentOutput(
            vec4f(safe_normalize3(raw_hbgi.xyz), raw_hbgi.w),
            raw_hbgi_irradiance
        );
    }

    let blurred_hbgi = hbgi_acc / weight_acc;
    let blurred_hbgi_irradiance = hbgi_irradiance_acc / weight_acc;
    return FragmentOutput(
        vec4f(safe_normalize3(blurred_hbgi.xyz), blurred_hbgi.w),
        blurred_hbgi_irradiance
    );
}
