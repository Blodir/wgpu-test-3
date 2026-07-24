// https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/

@group(0) @binding(2) var<uniform> inverse_view_proj: mat4x4<f32>;

@group(1) @binding(0) var motion_vectors_texture: texture_2d<f32>;
@group(1) @binding(1) var current_bent_ao_texture: texture_2d<f32>;
@group(1) @binding(2) var prev_bent_ao_reproject_texture: texture_2d<f32>;
@group(1) @binding(3) var current_depth_texture: texture_depth_2d;
@group(1) @binding(4) var current_normal_texture: texture_2d<f32>;
@group(1) @binding(5) var prev_depth_history_texture: texture_2d<f32>;
@group(1) @binding(6) var prev_normal_history_texture: texture_2d<f32>;
@group(1) @binding(7) var current_hbgi_irradiance_texture: texture_2d<f32>;
@group(1) @binding(8) var prev_hbgi_irradiance_reproject_texture: texture_2d<f32>;

struct HbgiReprojectUniform {
    prev_inverse_view_proj: mat4x4<f32>,
}

@group(2) @binding(0) var<uniform> hbgi_reproject_settings: HbgiReprojectUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

struct FragmentOutput {
    @location(0) hbgi_reproject: vec4<f32>,
    @location(1) depth_history: vec4<f32>,
    @location(2) normal_history: vec4<f32>,
    @location(3) hbgi_irradiance_reproject: vec4<f32>,
}

const TEMPORAL_RESPONSE: f32 = 0.2;
const HISTORY_CLAMP_WEIGHT: f32 = 0.25;

fn safe_normalize3(v: vec3f) -> vec3f {
    let len = length(v);
    if (len > 1e-8) {
        return v / len;
    }
    return vec3f(0.0);
}

fn current_frame_output(
    hbgi: vec4f,
    hbgi_irradiance: vec4f,
    depth: f32,
    normal: vec4f,
) -> FragmentOutput {
    return FragmentOutput(
        hbgi,
        vec4f(depth, 0.0, 0.0, 1.0),
        normal,
        hbgi_irradiance
    );
}

fn saturate(value: f32) -> f32 {
    return clamp(value, 0.0, 1.0);
}

fn reconstruct_world_position_from_depth(
    uv: vec2f,
    depth: f32,
    inverse_view_proj_matrix: mat4x4<f32>,
) -> vec3f {
    let clip = vec4f(
        uv.x * 2.0 - 1.0,
        1.0 - uv.y * 2.0,
        depth,
        1.0
    );
    let world = inverse_view_proj_matrix * clip;
    if (abs(world.w) > 1e-8) {
        return world.xyz / world.w;
    }
    return vec3f(0.0);
}

fn clamp_coord(coord: vec2i, max_coord: vec2i) -> vec2i {
    return clamp(coord, vec2i(0), max_coord);
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
fn fs_main(in: VertexOutput) -> FragmentOutput {
    let full_dims_u = textureDimensions(current_depth_texture);
    let full_dims = vec2f(full_dims_u);
    let full_max_coord = vec2i(full_dims_u) - vec2i(1);
    let full_coord = clamp_coord(vec2i(in.tex_coords * full_dims), full_max_coord);
    let history_dims_u = textureDimensions(current_bent_ao_texture);
    let history_dims = vec2f(history_dims_u);
    let history_max_coord = vec2i(history_dims_u) - vec2i(1);
    let history_coord = clamp_coord(vec2i(in.tex_coords * history_dims), history_max_coord);
    let curr_depth = textureLoad(current_depth_texture, full_coord, 0);
    let current_hbgi = textureLoad(current_bent_ao_texture, history_coord, 0);
    let current_hbgi_irradiance = textureLoad(current_hbgi_irradiance_texture, history_coord, 0);
    let curr_normal_sample = textureLoad(current_normal_texture, full_coord, 0);

    if (curr_depth <= 0.0) {
        return current_frame_output(
            current_hbgi,
            current_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }

    let curr_world_position = reconstruct_world_position_from_depth(
        in.tex_coords,
        curr_depth,
        inverse_view_proj
    );
    let motion = textureLoad(motion_vectors_texture, full_coord, 0).xy;
    let prev_uv = in.tex_coords - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return current_frame_output(
            current_hbgi,
            current_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }

    let prev_coord = clamp_coord(vec2i(prev_uv * history_dims), history_max_coord);
    let prev_depth = textureLoad(prev_depth_history_texture, prev_coord, 0).x;
    if (prev_depth <= 0.0) {
        return current_frame_output(
            current_hbgi,
            current_hbgi_irradiance,
            curr_depth,
            curr_normal_sample
        );
    }

    let prev_world_position = reconstruct_world_position_from_depth(
        prev_uv,
        prev_depth,
        hbgi_reproject_settings.prev_inverse_view_proj
    );
    let pos_similarity = saturate(1.0 - distance(prev_world_position, curr_world_position));
    let blend_weight = mix(1.0, TEMPORAL_RESPONSE, pos_similarity);

    var min_hbgi = textureLoad(
        current_bent_ao_texture,
        clamp_coord(history_coord + vec2i(1, 1), history_max_coord),
        0
    );
    var max_hbgi = min_hbgi;
    var min_hbgi_irradiance = textureLoad(
        current_hbgi_irradiance_texture,
        clamp_coord(history_coord + vec2i(1, 1), history_max_coord),
        0
    );
    var max_hbgi_irradiance = min_hbgi_irradiance;
    for (var sample_idx: i32 = 0; sample_idx < 3; sample_idx += 1) {
        let offset = array<vec2i, 3>(vec2i(1, -1), vec2i(-1, 1), vec2i(-1, -1))[sample_idx];
        let current_hbgi_sample = textureLoad(
            current_bent_ao_texture,
            clamp_coord(history_coord + offset, history_max_coord),
            0
        );
        let current_hbgi_irradiance_sample = textureLoad(
            current_hbgi_irradiance_texture,
            clamp_coord(history_coord + offset, history_max_coord),
            0
        );
        min_hbgi = min(min_hbgi, current_hbgi_sample);
        max_hbgi = max(max_hbgi, current_hbgi_sample);
        min_hbgi_irradiance = min(min_hbgi_irradiance, current_hbgi_irradiance_sample);
        max_hbgi_irradiance = max(max_hbgi_irradiance, current_hbgi_irradiance_sample);
    }

    let prev_hbgi_reproject = textureLoad(prev_bent_ao_reproject_texture, prev_coord, 0);
    let prev_hbgi_irradiance_reproject = textureLoad(
        prev_hbgi_irradiance_reproject_texture,
        prev_coord,
        0
    );
    let clamped_prev_hbgi = clamp(prev_hbgi_reproject, min_hbgi, max_hbgi);
    let filtered_prev_hbgi = mix(prev_hbgi_reproject, clamped_prev_hbgi, HISTORY_CLAMP_WEIGHT);
    let clamped_prev_hbgi_irradiance = clamp(
        prev_hbgi_irradiance_reproject,
        min_hbgi_irradiance,
        max_hbgi_irradiance
    );
    let filtered_prev_hbgi_irradiance = mix(
        prev_hbgi_irradiance_reproject,
        clamped_prev_hbgi_irradiance,
        HISTORY_CLAMP_WEIGHT
    );
    let accumulated_hbgi = mix(filtered_prev_hbgi, current_hbgi, blend_weight);
    let accumulated_hbgi_irradiance = mix(
        filtered_prev_hbgi_irradiance,
        current_hbgi_irradiance,
        blend_weight
    );
    return FragmentOutput(
        vec4f(safe_normalize3(accumulated_hbgi.xyz), accumulated_hbgi.w),
        vec4f(curr_depth, 0.0, 0.0, 1.0),
        curr_normal_sample,
        accumulated_hbgi_irradiance
    );
}
