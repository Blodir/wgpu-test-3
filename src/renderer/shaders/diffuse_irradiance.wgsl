@group(0) @binding(0) var environment_texture: texture_cube<f32>;
@group(0) @binding(1) var environment_texture_sampler: sampler;

@group(1) @binding(0) var<uniform> face_rot: mat4x4<f32>;

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

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let N = normalize(in.position.xyz);
    var irradiance = vec3f(0);

    var up = vec3f(0, 1, 0);
    let right = normalize(cross(up, N));
    up = normalize(cross(N, right));

    let sample_delta = 0.025;
    var n_samples: u32 = 0u;

    for (var phi = 0.0; phi < 2.0 * PI; phi += sample_delta) {
        for (var theta = 0.0; theta < 0.5 * PI; theta += sample_delta) {
            let tangent_sample = vec3f(sin(theta) * cos(phi), sin(theta) * sin(phi), cos(theta));
            let sample_vec = tangent_sample.x * right + tangent_sample.y * up + tangent_sample.z * N;
            let sample_radiance = textureSample(
                environment_texture,
                environment_texture_sampler,
                sample_vec
            ).rgb;
            let total_sample_radiance = sample_radiance.r + sample_radiance.g + sample_radiance.b;
            let clamped_radiance = sample_radiance * (MAX_TOTAL_RADIANCE / max(total_sample_radiance, MAX_TOTAL_RADIANCE));
            irradiance += clamped_radiance * cos(theta) * sin(theta);
            n_samples++;
        }
    }
    irradiance = PI * irradiance * (1.0 / f32(n_samples));

    return vec4f(irradiance, 1.0);
}

