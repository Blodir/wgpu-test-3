@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;
@group(0) @binding(3) var<uniform> camera_forward: vec3<f32>;

struct SunShadowUniform {
    light_view_proj: array<mat4x4<f32>, 4>,
    split_depths: vec4<f32>,
    cascade_params: vec4<u32>,
}

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
@group(1) @binding(12) var<uniform> sun_shadow: SunShadowUniform;
@group(1) @binding(13) var sun_shadow_texture: texture_depth_2d_array;
@group(1) @binding(14) var sun_shadow_sampler: sampler_comparison;

@group(2) @binding(0) var gbuffer_albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_albedo_ao_sampler: sampler;
@group(2) @binding(2) var gbuffer_normal_roughness: texture_2d<f32>;
@group(2) @binding(3) var gbuffer_normal_roughness_sampler: sampler;
@group(2) @binding(4) var gbuffer_emissive_metallic: texture_2d<f32>;
@group(2) @binding(5) var gbuffer_emissive_metallic_sampler: sampler;
@group(2) @binding(6) var gbuffer_world_position: texture_2d<f32>;
@group(2) @binding(7) var gbuffer_world_position_sampler: sampler;

@group(3) @binding(0) var hbgi_texture: texture_2d<f32>;
@group(3) @binding(1) var hbgi_texture_sampler: sampler;
@group(3) @binding(2) var hbgi_irradiance_texture: texture_2d<f32>;
@group(3) @binding(3) var hbgi_irradiance_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) final_color: vec4<f32>,
    @location(1) gi_source: vec4<f32>,
}

const PI: f32 = 3.1415927;
const MAX_REFLECTION_LOD: f32 = 4.0;
const MAX_POINT_LIGHTS: u32 = 64u;

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let a2 = a * a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH * NdotH;

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
    let cascade_depth = max(dot(world_position - camera_position, camera_forward), 0.0);
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
    let cascade_depth = max(dot(world_position - camera_position, camera_forward), 0.0);
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
        vec2(1.0, 1.0),
        vec2(1.0, 0.0),
        vec2(0.0, 0.0),
        vec2(0.0, 1.0)
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0.0, 1.0);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let uv = vec2f(in.tex_coords.x, 1.0 - in.tex_coords.y);
    let surface_color_ao = textureSample(
        gbuffer_albedo_ao,
        gbuffer_albedo_ao_sampler,
        uv
    );
    let normal_roughness = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    let emissive_metallic = textureSample(
        gbuffer_emissive_metallic,
        gbuffer_emissive_metallic_sampler,
        uv
    );
    let world_position = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    let hbgi = textureSample(
        hbgi_texture,
        hbgi_texture_sampler,
        uv
    );
    let hbgi_irradiance = textureSample(
        hbgi_irradiance_texture,
        hbgi_irradiance_sampler,
        uv
    ).rgba;
    if (world_position.w < 0.5) {
        return FragmentOutput(vec4f(0.0, 0.0, 0.0, 0.0), vec4f(0.0, 0.0, 0.0, 0.0));
    }

    let N = normalize(normal_roughness.xyz);
    let surface_roughness = normal_roughness.w;
    let surface_color = surface_color_ao.rgb;
    let ao = surface_color_ao.a;
    let surface_emissive = emissive_metallic.rgb;
    let surface_metallic = emissive_metallic.a;

    let V = normalize(camera_position - world_position.xyz);
    let R = reflect(-V, N);
    let prefiltered_color = textureSampleLevel(environment_texture, environment_texture_sampler, R, surface_roughness * MAX_REFLECTION_LOD).rgb;
    let F0 = mix(vec3f(0.04), surface_color, surface_metallic);

    var direct_diffuse = vec3f(0.0);
    var direct_specular = vec3f(0.0);
    {
        let L = normalize(-light_dir);
        let sun_shadow = sample_sun_shadow(world_position.xyz, N, L);
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
        let to_light = point_light_position_range.xyz - world_position.xyz;
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

    let bent_n_w = hbgi.xyz;
    let F_env = fresnel_schlick_roughness(max(dot(bent_n_w, V), 0.0), F0, surface_roughness);
    let k_s2 = F_env;
    var k_d2 = 1.0 - k_s2;
    k_d2 *= 1.0 - surface_metallic;

    let far_field_sample = textureSample(
        diffuse_irradiance_texture,
        diffuse_irradiance_texture_sampler,
        bent_n_w // Use bent normal from HBGI
    );

    // TODO: this is a temp function to scale back the (brutal) effect of large radius HBGI AO
    let diffuse_ao = pow(
        clamp(0.0, 1.0, hbgi_F0_compensation(hbgi.w) * hbgi.w - 0.1) + 0.1,
        0.4
    ) * ao;

    let brdf_specular_lut = textureSample(
        brdf_lut,
        brdf_lut_sampler,
        vec2(max(dot(bent_n_w, V), 0.0), 1.0 - surface_roughness)
    ).rg;
    let specular_ao = diffuse_ao; // TODO figure out a realistic ao function for specular?
    let far_specular = prefiltered_color * (F_env * brdf_specular_lut.x + brdf_specular_lut.y) * specular_ao * environment_map_intensity;

    // Horizon-Based Indirect Lighting (HBIL) - Benoit Mayaux - Section 2.2.3, 2.3.
    let E_far = far_field_sample.rgb * diffuse_ao * environment_map_intensity;
    let E_near = hbgi_irradiance.rgb * (1 - F0);
    let far_diffuse  = E_far * k_d2 * surface_color / PI;
    let near_diffuse = E_near * surface_color / PI;

    let final_diffuse = far_diffuse + near_diffuse + direct_diffuse;
    let final_specular = direct_specular + far_specular;

    let final_color = vec4f(
        final_diffuse +
        final_specular +
        surface_emissive,
        1.0
    );
    let gi_source = vec4f(
        final_diffuse + surface_emissive,
        hbgi.w
    );
    //return FragmentOutput(vec4f(E_near, 1.0), gi_source);
    //return FragmentOutput(vec4f(vec3f(hbgi_irradiance.w), 1.0), gi_source);
    return FragmentOutput(final_color, gi_source);
    //return FragmentOutput(vec4f(vec3f(hbgi.w), 1.0), gi_source);
    //return FragmentOutput(vec4f(hbgi.xyz, 1.0), gi_source);
    //return FragmentOutput(vec4f((hbgi.xyz + 1.0) / 2.0, 1.0), gi_source);
}
