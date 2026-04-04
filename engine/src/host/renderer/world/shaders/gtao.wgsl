@group(0) @binding(0) var gbuffer_normal_roughness: texture_2d<f32>;
@group(0) @binding(1) var gbuffer_normal_roughness_sampler: sampler;
@group(0) @binding(2) var gbuffer_world_position: texture_2d<f32>;
@group(0) @binding(3) var gbuffer_world_position_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const PI: f32 = 3.1415927;
const DIRECTIONS: u32 = 8u;
const STEPS_PER_DIRECTION: u32 = 4u;
const RADIUS_PIXELS: f32 = 8.0;
const RADIUS_WORLD: f32 = 30.0;
const NORMAL_BIAS: f32 = 0.05;

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
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    // G-buffer sampling requires Y flip in fullscreen passes.
    let uv = vec2f(in.tex_coords.x, 1.0 - in.tex_coords.y);
    let world_pos = textureSample(
        gbuffer_world_position,
        gbuffer_world_position_sampler,
        uv
    );
    if (world_pos.w < 0.5) {
        return vec4f(1.0, 1.0, 1.0, 1.0);
    }

    let normal_sample = textureSample(
        gbuffer_normal_roughness,
        gbuffer_normal_roughness_sampler,
        uv
    );
    let normal = normalize(normal_sample.xyz);

    let inv_resolution = 1.0 / vec2f(textureDimensions(gbuffer_world_position, 0));
    var occlusion = 0.0;
    var sample_count: u32 = 0u;
    for (var dir_idx: u32 = 0u; dir_idx < DIRECTIONS; dir_idx += 1u) {
        let theta = 2.0 * PI * (f32(dir_idx) / f32(DIRECTIONS));
        let dir = vec2f(cos(theta), sin(theta));
        for (var step_idx: u32 = 1u; step_idx <= STEPS_PER_DIRECTION; step_idx += 1u) {
            let step_t = f32(step_idx) / f32(STEPS_PER_DIRECTION);
            let offset = dir * (RADIUS_PIXELS * step_t) * inv_resolution;
            let sample_uv = clamp(uv + offset, vec2f(0.0), vec2f(1.0));
            let sample_world = textureSample(
                gbuffer_world_position,
                gbuffer_world_position_sampler,
                sample_uv
            );
            if (sample_world.w < 0.5) {
                continue;
            }

            let to_sample = sample_world.xyz - world_pos.xyz;
            let dist = length(to_sample);
            if (dist < 0.0001) {
                continue;
            }
            let to_sample_dir = to_sample / dist;
            let horizon = max(dot(normal, to_sample_dir) - NORMAL_BIAS, 0.0);
            let distance_weight = 1.0 - clamp(dist / RADIUS_WORLD, 0.0, 1.0);
            occlusion += horizon * distance_weight;
            sample_count += 1u;
        }
    }

    if (sample_count == 0u) {
        return vec4f(1.0, 1.0, 1.0, 1.0);
    }
    let avg_occlusion = occlusion / f32(sample_count);
    let ao = clamp(1.0 - avg_occlusion, 0.0, 1.0);
    return vec4f(ao, ao, ao, 1.0);
}
