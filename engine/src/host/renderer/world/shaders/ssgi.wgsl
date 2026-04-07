@group(0) @binding(1) var<uniform> camera_position: vec3<f32>;

@group(1) @binding(0) var gtao_texture: texture_2d<f32>;
@group(1) @binding(1) var gtao_texture_sampler: sampler;

@group(2) @binding(0) var gbuffer_albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var gbuffer_albedo_ao_sampler: sampler;
@group(2) @binding(2) var gbuffer_normal_roughness: texture_2d<f32>;
@group(2) @binding(3) var gbuffer_normal_roughness_sampler: sampler;
@group(2) @binding(4) var gbuffer_emissive_metallic: texture_2d<f32>;
@group(2) @binding(5) var gbuffer_emissive_metallic_sampler: sampler;
@group(2) @binding(6) var gbuffer_world_position: texture_2d<f32>;
@group(2) @binding(7) var gbuffer_world_position_sampler: sampler;

@group(3) @binding(0) var final_color_texture: texture_2d<f32>;
@group(3) @binding(1) var final_color_texture_sampler: sampler;
@group(3) @binding(2) var gi_source_texture: texture_2d<f32>;
@group(3) @binding(3) var gi_source_texture_sampler: sampler;
struct SsgiSettings {
    params0: vec4<f32>,
    params1: vec4<f32>,
}

@group(3) @binding(4) var<uniform> ssgi_settings: SsgiSettings;

const PI: f32 = 3.14159265358979323846;
const SSGI_DIRECTIONS: u32 = 4u;
const SSGI_STEPS: u32 = 4u;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

fn sample_final_color(uv: vec2<f32>) -> vec4<f32> {
    return textureSample(
        final_color_texture,
        final_color_texture_sampler,
        uv
    );
}

fn saturate(x: f32) -> f32 {
    return clamp(x, 0.0, 1.0);
}

fn safe_normalize(v: vec3<f32>) -> vec3<f32> {
    let len2 = dot(v, v);
    if (len2 < 1e-8) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    return v * inverseSqrt(len2);
}

fn safe_normalize2(v: vec2<f32>) -> vec2<f32> {
    let len2 = dot(v, v);
    if (len2 < 1e-8) {
        return vec2<f32>(1.0, 0.0);
    }
    return v * inverseSqrt(len2);
}

fn hash12(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let q = p3 + dot(p3, p3.yzx + 33.33);
    return fract((q.x + q.y) * q.z);
}

fn rotate2(v: vec2<f32>, angle: f32) -> vec2<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec2<f32>(c * v.x - s * v.y, s * v.x + c * v.y);
}

fn tangent_basis_from_normal(n: vec3<f32>) -> mat3x3<f32> {
    let up = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(0.0, 1.0, 0.0), abs(n.z) > 0.999);
    let t = safe_normalize(cross(up, n));
    let b = cross(n, t);
    return mat3x3<f32>(t, b, n);
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
    return out;
}

@fragment
fn fs_passthrough(in: VertexOutput) -> @location(0) vec4<f32> {
    return sample_final_color(in.tex_coords);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;

    let gtao = textureSample(
        gtao_texture,
        gtao_texture_sampler,
        uv
    );
    let albedo_ao = textureSample(
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
    let final_color = sample_final_color(uv);
    if (world_position.w < 0.5) {
        return final_color;
    }

    let P = world_position.xyz;
    let N = safe_normalize(normal_roughness.xyz);
    let V = safe_normalize(camera_position - P);

    let bent_normal = safe_normalize(gtao.xyz * 2.0 - 1.0);
    let ao = saturate(gtao.w);

    let base_color = albedo_ao.rgb;
    let metallic = saturate(emissive_metallic.a);

    // Diffuse color of the receiving surface in metallic workflow.
    let diffuse_color = base_color * (1.0 - metallic);

    let dims = vec2<f32>(textureDimensions(gbuffer_world_position, 0));
    let inv_resolution = 1.0 / dims;

    // Build a basis around the bent normal and use it to bias the screen-space sample directions.
    let bent_basis = tangent_basis_from_normal(bent_normal);

    var indirect_radiance = vec3<f32>(0.0);
    var weight_sum: f32 = 0.0;

    // Per-pixel rotation to reduce banding.
    let jitter = hash12(uv * dims) * 2.0 * PI;

    for (var dir_idx: u32 = 0u; dir_idx < SSGI_DIRECTIONS; dir_idx += 1u) {
        let base_angle = 2.0 * PI * (f32(dir_idx) / f32(SSGI_DIRECTIONS));
        let angle = base_angle + jitter;
        let dir2 = vec2<f32>(cos(angle), sin(angle));

        for (var step_idx: u32 = 1u; step_idx <= SSGI_STEPS; step_idx += 1u) {
            let t = f32(step_idx) / f32(SSGI_STEPS);

            // Bias density toward nearby samples.
            let step_scale = t * t;
            let pixel_radius = ssgi_settings.params0.x * step_scale;

            // Use the bent normal projected into tangent space to bias 2D sampling direction.
            // We treat dir2 as a hemisphere sample around +Z in tangent space and use its XY
            // for a screen-space offset bias, not as a true ray direction.
            let hemi_local = safe_normalize(vec3<f32>(dir2, 1.0));
            let hemi_world = bent_basis * hemi_local;

            // Screen-space direction is still 2D, but we bias it toward the bent-normal lobe.
            let screen_bias = safe_normalize(vec3<f32>(dir2, 0.0) + hemi_world * 0.5);
            let sample_dir_uv = safe_normalize2(screen_bias.xy);

            let offset = sample_dir_uv * pixel_radius * inv_resolution;
            let sample_uv = clamp(uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));

            let sample_world_position = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                sample_uv
            );
            if (sample_world_position.w < 0.5) {
                continue;
            }

            let S = sample_world_position.xyz;
            let D = S - P;
            let dist2 = dot(D, D);
            if (dist2 < 1e-6) {
                continue;
            }

            let dist = sqrt(dist2);
            if (dist > ssgi_settings.params0.y) {
                continue;
            }

            let wi = D / dist;

            // Reject samples that are behind the receiver.
            let receiver_cos = dot(N, wi);
            if (receiver_cos <= 0.0) {
                continue;
            }

            let sample_normal_roughness = textureSample(
                gbuffer_normal_roughness,
                gbuffer_normal_roughness_sampler,
                sample_uv
            );
            let Ns = safe_normalize(sample_normal_roughness.xyz);

            // Sender should roughly face the receiver.
            let sender_cos = dot(Ns, -wi);
            if (sender_cos <= 0.0) {
                continue;
            }

            // Simple thickness / discontinuity rejection.
            let expected_step_world = ssgi_settings.params0.y * t;
            let thickness_weight = 1.0 - saturate((dist - expected_step_world) / max(ssgi_settings.params0.z, 1e-4));

            // Bent-normal confidence: prefer samples in the bent-normal hemisphere.
            let bent_weight = saturate(dot(bent_normal, wi));

            // Distance falloff.
            let falloff = 1.0 - saturate(dist / ssgi_settings.params0.y);
            let falloff2 = falloff * falloff;

            // Extra edge rejection using view direction similarity.
            let sample_view = safe_normalize(camera_position - S);
            let view_consistency = pow(saturate(dot(V, sample_view)), ssgi_settings.params0.w);

            let sample_gi = textureSample(
                gi_source_texture,
                gi_source_texture_sampler,
                sample_uv
            ).rgb;

            let weight =
                receiver_cos *
                sender_cos *
                falloff2 *
                thickness_weight *
                bent_weight *
                view_consistency;

            indirect_radiance += sample_gi * weight;
            weight_sum += weight;
        }
    }

    if (weight_sum > 1e-5) {
        indirect_radiance /= weight_sum;
    } else {
        indirect_radiance = vec3<f32>(0.0);
    }

    // AO dampens the indirect contribution in highly occluded regions.
    indirect_radiance *= ao;

    // Apply diffuse BRDF of the receiving surface.
    let indirect_diffuse = indirect_radiance * diffuse_color * (ssgi_settings.params1.x / PI);

    return vec4<f32>(final_color.rgb + indirect_diffuse, final_color.a);
}
