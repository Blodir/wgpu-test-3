@group(0) @binding(0) var environment_texture: texture_cube<f32>;
@group(0) @binding(1) var environment_texture_sampler: sampler;

@group(1) @binding(0) var<uniform> face_rot: mat4x4<f32>;

@group(2) @binding(0) var<uniform> roughness: f32;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) position: vec3<f32>,
}

const PI: f32 = 3.1415927;
const MAX_TOTAL_RADIANCE: f32 = 50.0;

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
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.position = (vec4<f32>(pos[vertex_index], 1, 1) * face_rot).xyz;
    return out;
}

fn distribution_ggx(N: vec3f, H: vec3f, a: f32) -> f32 {
    let a2 = a*a;
    let NdotH = max(dot(N, H), 0.0);
    let NdotH2 = NdotH*NdotH;

    let nom = a2;
    var denom = (NdotH2 * (a2 - 1.0) + 1.0);
    denom = PI * denom * denom;

    return nom / denom;
}

fn radical_inverse_vdc(b: u32) -> f32 {
    var bits = (b << 16u) | (b >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10;
}

fn hammersley(i: u32, N: u32) -> vec2f {
    return vec2f(f32(i)/f32(N), radical_inverse_vdc(i));
}

fn importance_sample_ggx(x_i: vec2f, N: vec3f, r: f32) -> vec3f {
    let a = r * r;
    let phi = 2.0 * PI * x_i.x;
    let cos_theta = sqrt((1.0 - x_i.y) / (1.0 + (a * a - 1.0) * x_i.y));
    let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
    var H: vec3f;
    H.x = cos(phi) * sin_theta;
    H.y = sin(phi) * sin_theta;
    H.z = cos_theta;

    let up = select(vec3f(1, 0, 0), vec3f(0, 0, 1) , abs(N.z) < 0.999);
    let tangent = normalize(cross(up, N));
    let bitangent = cross(N, tangent);

    let sample_vec = tangent * H.x + bitangent * H.y + N * H.z;
    return normalize(sample_vec);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.position);
    //return textureSampleLevel(environment_texture, environment_texture_sampler, N, 0.0);
    let R = N;
    let V = R;

    let sample_count = 2048u;

    var total_radiance = vec3f(0);
    var total_weight = 0.0;

    for(var i = 0u; i < sample_count; i = i + 1u) {
        let x_i = hammersley(i, sample_count);
        let H = importance_sample_ggx(x_i, N, roughness);
        let L = normalize(2.0 * dot(V, H) * H - V);

        let NdotL = max(dot(N, L), 0.0);
        if (NdotL > 0.0) {
            let D = distribution_ggx(N, H, roughness);
            let NdotH = max(dot(N, H), 0.0);
            let HdotV = max(dot(H, V), 0.0);
            let pdf = D * NdotH / (4.0 * HdotV) + 0.0001;

            let resolution = f32(textureDimensions(environment_texture).x);

            // solid angle of a texel in the cubemap (the area that texel covers on unit sphere)
            // 4*PI is the total solid angle of a sphere. There's 6 faces each with surface area resolution^2
            let sa_texel = 4.0 * PI / (6.0 * resolution * resolution);
            // the area of the sample is weighted by the probability distribution (for importance sampling)
            let sa_sample = 1.0 / (f32(sample_count) * pdf + 0.0001);

            let mip_level = select(0.5 * log2(sa_sample / sa_texel), 0.0, roughness == 0.0);

            var sample_radiance = textureSampleLevel(environment_texture, environment_texture_sampler, L, mip_level).rgb;
            // clamp total radiance to prevent bright spots in the prefilter map (though some energy is lost!)
            let total_sample_radiance = sample_radiance.r + sample_radiance.g + sample_radiance.b;
            let clamped_radiance = sample_radiance * (MAX_TOTAL_RADIANCE / max(total_sample_radiance, MAX_TOTAL_RADIANCE));

            total_radiance += clamped_radiance * NdotL;
            total_weight += NdotL;
        }
    }

    total_radiance = total_radiance / total_weight;

    return vec4f(total_radiance, 1.0);
}

