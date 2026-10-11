@group(1) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(1) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;
@group(1) @binding(4) var<uniform> camera_view_rotation: mat3x3<f32>;

struct SunShadowUniform {
    light_view_proj: array<mat4x4<f32>, 4>,
    split_depths: vec4<f32>,
    cascade_params: vec4<u32>,
}

@group(2) @binding(0) var<uniform> light_dir: vec3<f32>;
@group(2) @binding(1) var<uniform> light_col: vec3<f32>;
@group(2) @binding(2) var environment_texture: texture_cube<f32>;
@group(2) @binding(3) var environment_texture_sampler: sampler;
@group(2) @binding(4) var env_di_texture: texture_cube<f32>;
@group(2) @binding(5) var env_di_texture_sampler: sampler;
@group(2) @binding(6) var brdf_lut: texture_2d<f32>;
@group(2) @binding(7) var brdf_lut_sampler: sampler;
@group(2) @binding(8) var<uniform> environment_map_intensity: f32;
@group(2) @binding(9) var<uniform> point_light_count: vec4<u32>;
@group(2) @binding(10) var<uniform> point_light_positions_ranges: array<vec4<f32>, 64>;
@group(2) @binding(11) var<uniform> point_light_colors_intensities: array<vec4<f32>, 64>;
@group(2) @binding(12) var<uniform> sun_shadow: SunShadowUniform;
@group(2) @binding(13) var sun_shadow_texture: texture_depth_2d_array;
@group(2) @binding(14) var sun_shadow_sampler: sampler_comparison;

@group(0) @binding(0) var gbuffer_albedo_ao: texture_2d<f32>;
@group(0) @binding(1) var gbuffer_albedo_ao_sampler: sampler;
@group(0) @binding(2) var gbuffer_normal_roughness: texture_2d<f32>;
@group(0) @binding(3) var gbuffer_normal_roughness_sampler: sampler;
@group(0) @binding(4) var gbuffer_emissive_metallic: texture_2d<f32>;
@group(0) @binding(5) var gbuffer_emissive_metallic_sampler: sampler;
@group(0) @binding(6) var depth_texture: texture_depth_2d;

@group(0) @binding(7) var ssgi_denoise: texture_2d<f32>;
@group(0) @binding(8) var ssgi_denoise_sampler: sampler;
@group(0) @binding(9) var ssr_denoise: texture_2d<f32>;
@group(0) @binding(10) var ssr_denoise_sampler: sampler;
@group(0) @binding(11) var reprojected_diffuse: texture_2d<f32>;
@group(0) @binding(12) var bent_normal_ao_denoise: texture_2d<f32>;
@group(0) @binding(13) var normal_roughness_history: texture_2d<f32>;
@group(0) @binding(14) var depth_pyramid: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) world_direction: vec3<f32>,
}

struct FragmentOutput {
    @location(0) final_hdr: vec4<f32>,
    @location(1) diffuse_history: vec4<f32>,
}

const PI: f32 = 3.1415927;
const MAX_REFLECTION_LOD: f32 = 4.0;
const MAX_POINT_LIGHTS: u32 = 64u;
const MIN_GGX_ROUGHNESS: f32 = 0.01;
const UPSAMPLE_NORMAL_EXPONENT: f32 = 16.0; // 16.0
//const UPSAMPLE_RELATIVE_DEPTH_THRESHOLD: f32 = 0.01; // 0.01

fn safe_normalize3(v: vec3f, fallback: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8 && len < 1e20) {
        return v / len;
    }
    return fallback;
}

struct IndirectLightingSamples {
    ssgi: vec3f,
    ssr: vec3f,
}

// attenuates indirect radiance based when the half res pixel that ssgi/ssr uses
// doesn't match the full res pixel that we're looking at
fn upsample_indirect_lighting(uv: vec2f, full_normal: vec3f, full_depth: f32, half_coords: vec2i) -> IndirectLightingSamples {
    //let half_depth = textureLoad(depth_pyramid, half_coords, 1).r;
    let half_normal = textureLoad(normal_roughness_history, half_coords, 0).xyz;

    let normal_weight = pow(clamp(dot(full_normal, half_normal), 0.0, 1.0), UPSAMPLE_NORMAL_EXPONENT);
    //let relative_depth_difference = abs(full_depth - half_depth) / max(max(full_depth, half_depth), 1e-8);
    //let depth_weight = clamp(1.0 - relative_depth_difference / UPSAMPLE_RELATIVE_DEPTH_THRESHOLD, 0.0, 1.0);
    //let attenuation = normal_weight * depth_weight;
    let attenuation = normal_weight;
    //if (!(half_depth > 0.0 && attenuation > 0.0)) {
    if (!(attenuation > 0.0)) {
        return IndirectLightingSamples(vec3f(0.0), vec3f(0.0));
    }
    let ssgi_sample = textureSampleLevel(ssgi_denoise, ssgi_denoise_sampler, uv, 0.0).rgb;
    let ssr_sample = textureSampleLevel(ssr_denoise, ssr_denoise_sampler, uv, 0.0).rgb;
    return IndirectLightingSamples(
        // intentionally bypassed, GI is diffuse enough for the attenuation to be unnecessary
        ssgi_sample,
        mix(vec3f(0.0), ssr_sample, attenuation)
    );
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

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let roughness = max(a, MIN_GGX_ROUGHNESS);
    let a2 = roughness * roughness;
    let NdotH = clamp(dot(N, H), 0.0, 1.0);
    let NdotH2 = NdotH * NdotH;

    let nom = a2;
    var denom = (1.0 - NdotH2) + NdotH2 * a2;
    denom = PI * denom * denom;

    return nom / denom;
}

fn geometry_schlick_ggx(NdotV: f32, roughness: f32) -> f32 {
    let r = (roughness + 1.0);
    let k = (r * r) / 8.0;
    let denom = NdotV * (1.0 - k) + k;
    return NdotV / denom;
}

fn geometry_smith(N: vec3f, V: vec3f, L: vec3f, roughness: f32) -> f32 {
    let NdotV = max(dot(N, V), 0.0);
    let NdotL = max(dot(N, L), 0.0);
    let ggx1 = geometry_schlick_ggx(NdotV, roughness);
    let ggx2 = geometry_schlick_ggx(NdotL, roughness);
    return ggx1 * ggx2;
}

fn fresnel_schlick(cos_theta: f32, F0: vec3f) -> vec3f {
    return F0 + (1.0 - F0) * pow(1.0 - cos_theta, 5.0);
}

fn fresnel_schlick_roughness(cos_theta: f32, F0: vec3f, roughness: f32) -> vec3f {
    return F0 + (max(vec3f(1.0 - roughness), F0) - F0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

fn active_sun_shadow_far_depth() -> f32 {
    let cascade_count = max(sun_shadow.cascade_params.x, 1u);
    if (cascade_count == 1u) {
        return sun_shadow.split_depths.x;
    }
    if (cascade_count == 2u) {
        return sun_shadow.split_depths.y;
    }
    if (cascade_count == 3u) {
        return sun_shadow.split_depths.z;
    }
    return sun_shadow.split_depths.w;
}

fn select_sun_shadow_cascade(world_position: vec3f) -> u32 {
    let cascade_depth = max(dot(world_position - camera_position, camera_view_rotation[2]), 0.0);
    let cascade_count = max(sun_shadow.cascade_params.x, 1u);
    if (cascade_count == 1u || cascade_depth <= sun_shadow.split_depths.x) {
        return 0u;
    }
    if (cascade_count == 2u || cascade_depth <= sun_shadow.split_depths.y) {
        return 1u;
    }
    if (cascade_count == 3u || cascade_depth <= sun_shadow.split_depths.z) {
        return 2u;
    }
    return min(3u, cascade_count - 1u);
}

fn sample_sun_shadow(world_position: vec3f, N: vec3f, L: vec3f) -> f32 {
    let cascade_depth = max(dot(world_position - camera_position, camera_view_rotation[2]), 0.0);
    if (cascade_depth > active_sun_shadow_far_depth()) {
        return 1.0;
    }
    let cascade_index = select_sun_shadow_cascade(world_position);
    let shadow_clip = sun_shadow.light_view_proj[cascade_index] * vec4f(world_position, 1.0);
    if (shadow_clip.w <= 0.0) {
        return 1.0;
    }
    let shadow_ndc = shadow_clip.xyz / shadow_clip.w;
    let shadow_uv = vec2f(
        shadow_ndc.x * 0.5 + 0.5,
        0.5 - shadow_ndc.y * 0.5
    );
    let shadow_depth = shadow_ndc.z;
    if (
        shadow_uv.x < 0.0 || shadow_uv.x > 1.0 ||
        shadow_uv.y < 0.0 || shadow_uv.y > 1.0 ||
        shadow_depth <= 0.0 || shadow_depth >= 1.0
    ) {
        return 1.0;
    }

    let n_dot_l = max(dot(N, L), 0.0);
    let bias = max(0.0015 * (1.0 - n_dot_l), 0.0003);
    let texel_size = 1.0 / vec2f(textureDimensions(sun_shadow_texture));

    var visibility = 0.0;
    for (var y: i32 = -1; y <= 1; y += 1) {
        for (var x: i32 = -1; x <= 1; x += 1) {
            let offset = vec2f(f32(x), f32(y)) * texel_size;
            let uv = clamp(shadow_uv + offset, vec2f(0.0), vec2f(1.0));
            visibility += textureSampleCompare(
                sun_shadow_texture,
                sun_shadow_sampler,
                uv,
                i32(cascade_index),
                shadow_depth - bias,
            );
        }
    }
    return visibility / 9.0;
}

fn hbgi_F0_compensation(ao: f32) -> f32 {
    let alpha = acos(clamp(1.0 - ao, -1.0, 1.0));

    // Normalize aperture: 0 = closed cone, 1 = full hemisphere aperture PI/2
    let a = clamp(alpha / (0.5 * PI), 0.0, 1.0);

    return a * (
        1.0 + pow(1.0 - a, 0.75) * 0.5
    );
}

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
    let clip_pos = vec4<f32>(pos[vertex_index], 1, 1);
    let world_pos = inverse_view_proj * clip_pos;
    out.world_direction = world_pos.xyz / world_pos.w - camera_position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = in.tex_coords;
    let surface_color_ao = textureSample(
        gbuffer_albedo_ao,
        gbuffer_albedo_ao_sampler,
        uv
    );
    var normal_roughness = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    //normal_roughness.w = 0.0;
    let emissive_metallic = textureSample(
        gbuffer_emissive_metallic,
        gbuffer_emissive_metallic_sampler,
        uv
    );
    let depth_dims = vec2i(textureDimensions(depth_texture));
    let depth_pixel = clamp(vec2i(uv * vec2f(depth_dims)), vec2i(0), depth_dims - vec2i(1));
    let depth = textureLoad(depth_texture, depth_pixel, 0);
    let half_max_coords = vec2i(textureDimensions(reprojected_diffuse)) - vec2i(1);
    let half_coords = min(vec2i(in.clip_position.xy) / 2, half_max_coords);
    let history_length = textureLoad(reprojected_diffuse, half_coords, 0).a;

    if (depth <= 0.0) {
        let rd = normalize(in.world_direction);

        let sample =
            textureSampleLevel(
                environment_texture,
                environment_texture_sampler,
                rd,
                0.0
            );

        let gi_source = vec4f(
            sample.rgb,
            history_length
        );
        return FragmentOutput(sample.rgba, gi_source);
    }
    let world_position = reconstruct_world_position_from_depth(uv, depth);

    let N = normalize(normal_roughness.xyz);
    let indirect_samples = upsample_indirect_lighting(uv, N, depth, half_coords);
    let surface_roughness = normal_roughness.w;
    let surface_color = surface_color_ao.rgb;
    let ao = surface_color_ao.a;
    let surface_emissive = emissive_metallic.rgb;
    let surface_metallic = emissive_metallic.a;

    let V = normalize(camera_position - world_position);
    let R = reflect(-V, N);
    let F0 = mix(vec3f(0.04), surface_color, surface_metallic);

    var direct_diffuse = vec3f(0.0);
    var direct_specular = vec3f(0.0);
    {
        let L = normalize(-light_dir);
        let sun_shadow = sample_sun_shadow(world_position, N, L);
        let H = normalize(V + L);
        let radiance = light_col;

        let NDF = distribution_ggx(N, H, surface_roughness);
        let G = geometry_smith(N, V, L, surface_roughness);
        let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        let omega_0 = max(dot(N, V), 0.0);
        let omega_i = max(dot(N, L), 0.0);
        let num = NDF * G * F;
        let denom = 4.0 * max(omega_0 * omega_i, 0.0001);
        let specular = num / denom;

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic);
        let NdotL = max(dot(N, L), 0.0);
        direct_diffuse += k_d * surface_color / PI * radiance * NdotL * sun_shadow;
        direct_specular += specular * radiance * NdotL * sun_shadow;
    }

    let clamped_point_light_count = min(point_light_count.x, MAX_POINT_LIGHTS);
    for (var light_idx: u32 = 0u; light_idx < clamped_point_light_count; light_idx += 1u) {
        let point_light_position_range = point_light_positions_ranges[light_idx];
        let point_light_color_intensity = point_light_colors_intensities[light_idx];
        let to_light = point_light_position_range.xyz - world_position;
        let light_distance = length(to_light);
        let light_range = max(point_light_position_range.w, 0.001);
        if (light_distance >= light_range) {
            continue;
        }
        let L = to_light / max(light_distance, 0.0001);
        let H = normalize(V + L);
        let range_falloff = 1.0 - clamp(light_distance / light_range, 0.0, 1.0);
        let attenuation = (range_falloff * range_falloff) / max(light_distance * light_distance, 0.01);
        let radiance = point_light_color_intensity.rgb * point_light_color_intensity.w * attenuation;

        let NDF = distribution_ggx(N, H, surface_roughness);
        let G = geometry_smith(N, V, L, surface_roughness);
        let F = fresnel_schlick(max(dot(H, V), 0.0), F0);

        let omega_0 = max(dot(N, V), 0.0);
        let omega_i = max(dot(N, L), 0.0);
        let num = NDF * G * F;
        let denom = 4.0 * max(omega_0 * omega_i, 0.0001);
        let specular = num / denom;

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic);
        let NdotL = max(dot(N, L), 0.0);
        direct_diffuse += k_d * surface_color / PI * radiance * NdotL;
        direct_specular += specular * radiance * NdotL;
    }

    let F_env = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, surface_roughness);
    let k_s2 = F_env;
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let brdf_specular_lut = textureSample(
        brdf_lut,
        brdf_lut_sampler,
        vec2(max(dot(N, V), 0.0), 1.0 - surface_roughness)
    ).rg;

    let bent_ao = textureSampleLevel(
        bent_normal_ao_denoise,
        ssgi_denoise_sampler,
        uv,
        0.0
    );
    let bent_n_w = safe_normalize3(bent_ao.xyz, N);
    let far_field_sample = textureSampleLevel(
        env_di_texture,
        env_di_texture_sampler,
        bent_n_w,
        0.0
    );
    let E_far = far_field_sample.rgb * bent_ao.w * environment_map_intensity;
    let E_near = indirect_samples.ssgi;
    let indirect_diffuse = (E_near + E_far) * k_d2 * surface_color / PI;

    let accumulated_ssr_radiance = indirect_samples.ssr;
    let ssr_specular = accumulated_ssr_radiance * (F_env * brdf_specular_lut.x + brdf_specular_lut.y) * ao;

    let final_diffuse = indirect_diffuse + direct_diffuse;
    let final_specular = direct_specular + ssr_specular;

    let final_color = vec4f(
        final_diffuse +
        final_specular +
        surface_emissive,
        1.0
    );
    let gi_source = vec4f(
        final_diffuse + surface_emissive,
        history_length
    );
    //return FragmentOutput(vec4f(E_near, 1.0), gi_source);
    //return FragmentOutput(vec4f(vec3f(hbgi_irradiance.w), 1.0), gi_source);
    return FragmentOutput(final_color, gi_source);
    //return FragmentOutput(vec4f(hbgi_irradiance.xyz, 1.0), gi_source);
    //return FragmentOutput(vec4f(vec3f(hbgi.w), 1.0), gi_source);
    //return FragmentOutput(vec4f(hbgi.xyz, 1.0), gi_source);
    //return FragmentOutput(vec4f((hbgi.xyz + 1.0) / 2.0, 1.0), gi_source);
}
