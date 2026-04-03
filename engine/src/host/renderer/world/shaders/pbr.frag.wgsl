@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;
@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;

@group(1) @binding(0) var<uniform> light_dir: vec3<f32>;
@group(1) @binding(1) var<uniform> light_col: vec3<f32>;
@group(1) @binding(2) var environment_texture: texture_cube<f32>;
@group(1) @binding(3) var environment_texture_sampler: sampler;
@group(1) @binding(4) var diffuse_irradiance_texture: texture_cube<f32>;
@group(1) @binding(5) var diffuse_irradiance_texture_sampler: sampler;
@group(1) @binding(6) var brdf_lut: texture_2d<f32>;
@group(1) @binding(7) var brdf_lut_sampler: sampler;
@group(1) @binding(8) var<uniform> environment_map_intensity: f32;
@group(1) @binding(9) var<uniform> point_light_count: vec4<u32>;
@group(1) @binding(10) var<uniform> point_light_positions_ranges: array<vec4<f32>, 64>;
@group(1) @binding(11) var<uniform> point_light_colors_intensities: array<vec4<f32>, 64>;

@group(2) @binding(0) var<uniform> base_color_factor: vec4<f32>;
@group(2) @binding(1) var<uniform> metallic_factor: f32;
@group(2) @binding(2) var<uniform> roughness_factor: f32;
@group(2) @binding(3) var<uniform> emissive_factor: vec3<f32>;
@group(2) @binding(4) var normal_texture: texture_2d<f32>;
@group(2) @binding(5) var normal_texture_sampler: sampler;
@group(2) @binding(6) var occlusion_texture: texture_2d<f32>;
@group(2) @binding(7) var occlusion_texture_sampler: sampler;
@group(2) @binding(8) var emissive_texture: texture_2d<f32>;
@group(2) @binding(9) var emissive_texture_sampler: sampler;
@group(2) @binding(10) var base_color_texture: texture_2d<f32>;
@group(2) @binding(11) var base_color_texture_sampler: sampler;
@group(2) @binding(12) var metallic_roughness_texture: texture_2d<f32>;
@group(2) @binding(13) var metallic_roughness_texture_sampler: sampler;
@group(2) @binding(14) var<uniform> normal_texture_scale: f32;
@group(2) @binding(15) var<uniform> alpha_mask_enabled: u32;
@group(2) @binding(16) var<uniform> alpha_cutoff: f32;
@group(2) @binding(17) var<uniform> alpha_blend_enabled: u32;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,

    @location(0) tangent: vec3<f32>,
    @location(1) bitangent: vec3<f32>,
    @location(2) world_position: vec4<f32>,
    @location(3) normal: vec3<f32>,
    @location(4) normal_tex_coords: vec2<f32>,
    @location(5) occlusion_tex_coords: vec2<f32>,
    @location(6) emissive_tex_coords: vec2<f32>,
    @location(7) base_color_tex_coords: vec2<f32>,
    @location(8) metallic_roughness_tex_coords: vec2<f32>,
}

const PI: f32 = 3.1415927;
const MAX_REFLECTION_LOD: f32 = 4.0;
const MAX_POINT_LIGHTS: u32 = 64u;

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let a2 = a*a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH*NdotH;

    let nom = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let normal_sample =
        textureSample(
            normal_texture,
            normal_texture_sampler,
            in.normal_tex_coords
        );
    var N = in.normal;
    if (normal_sample.w > 0.5) { // w encodes whether normal mapping should be used
        let n: vec3f = normalize(normal_sample.rgb * 2.0 - 1.0) * vec3f(normal_texture_scale, normal_texture_scale, 1.0);
        let TBN = mat3x3(in.tangent, in.bitangent, in.normal);
        N = normalize(TBN * n);
    }

    let V = normalize(camera_position - in.world_position.xyz);
    let R = reflect(-V, N);

    // base_color_texture is sampled from an sRGB texture, so it is already linear here.
    let surface_color =
        textureSample(
            base_color_texture,
            base_color_texture_sampler,
            in.base_color_tex_coords
        ) * base_color_factor;
    if (alpha_mask_enabled == 1u && surface_color.a < alpha_cutoff) {
        discard;
    }
    let metallic_roughness =
        textureSample(
            metallic_roughness_texture,
            metallic_roughness_texture_sampler,
            in.metallic_roughness_tex_coords
        );
    let surface_metallic = metallic_roughness.b * metallic_factor;
    let surface_roughness = metallic_roughness.g * roughness_factor;
    let surface_emissive_sample =
        textureSample(
            emissive_texture,
            emissive_texture_sampler,
            in.emissive_tex_coords
        );
    let surface_emissive = surface_emissive_sample.rgb * emissive_factor;
    let ao = textureSample(occlusion_texture, occlusion_texture_sampler, in.occlusion_tex_coords);
    let prefiltered_color = textureSampleLevel(environment_texture, environment_texture_sampler, R, surface_roughness * MAX_REFLECTION_LOD).rgb;

    let F0 = mix(vec3f(0.04), surface_color.xyz, surface_metallic);

    // ---------------- //
    // Lighting
    // ---------------- //
    var Lo = vec3f(0.0);
    {
        let L = normalize(-light_dir); // reverse light direction (pointing from surface toward light source)
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

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic); // diffuse/refracted

        let NdotL = max(dot(N, L), 0.0);
        Lo += (k_d * surface_color.xyz / PI + specular) * radiance * NdotL;
    }

    let clamped_point_light_count = min(point_light_count.x, MAX_POINT_LIGHTS);
    for (var light_idx: u32 = 0u; light_idx < clamped_point_light_count; light_idx += 1u) {
        let point_light_position_range = point_light_positions_ranges[light_idx];
        let point_light_color_intensity = point_light_colors_intensities[light_idx];
        let to_light = point_light_position_range.xyz - in.world_position.xyz;
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

        let k_d = (vec3f(1.0) - F) * (1.0 - surface_metallic); // diffuse/refracted
        let NdotL = max(dot(N, L), 0.0);
        Lo += (k_d * surface_color.xyz / PI + specular) * radiance * NdotL;
    }

    // ---------------- //
    // IBL
    // ---------------- //

    let F_env = fresnel_schlick_roughness(max(dot(N, V), 0.0), F0, surface_roughness);

    let k_s2 = F_env;
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let irradiance =
        textureSample(
            diffuse_irradiance_texture,
            diffuse_irradiance_texture_sampler,
            N
        ).rgb;
    let diffuse = irradiance * surface_color.rgb;

    // in opengl texture origin is at bottom-left so the y coordinate has to be flipped here (as opposed to learnopengl tutorial)
    let brdf = textureSample(brdf_lut, brdf_lut_sampler, vec2(max(dot(N, V), 0.0), 1 - surface_roughness)).rg;
    let specular_env = prefiltered_color * (F_env * brdf.x + brdf.y);
    let ambient = (k_d2 * diffuse + specular_env) * ao.r * environment_map_intensity;

    // ---------------- //

    var col = ambient + Lo + (surface_emissive * surface_emissive_sample.a);

    let out_alpha = select(1.0, surface_color.a, alpha_blend_enabled == 1u);
    return vec4f(col, out_alpha);
}
