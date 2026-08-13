@group(0) @binding(0) var skybox_texture: texture_2d<f32>;
@group(0) @binding(1) var skybox_texture_sampler: sampler;
@group(0) @binding(2) var current_final_color_texture: texture_2d<f32>;
@group(0) @binding(3) var final_color_sampler: sampler;
@group(0) @binding(4) var previous_final_color_texture: texture_2d<f32>;
@group(0) @binding(5) var current_depth_pyramid_texture: texture_2d<f32>;
@group(0) @binding(6) var previous_depth_pyramid_texture: texture_2d<f32>;
@group(0) @binding(7) var motion_vectors_texture: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
}

const DEPTH_REJECTION_THRESHOLD: f32 = 1e-2;
const MAX_HISTORY_LENGTH: f32 = 8.0;

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
        vec2(1.0, 0.0),
        vec2(1.0, 1.0),
        vec2(0.0, 1.0),
        vec2(0.0, 0.0),
    );
    out.clip_position = vec4<f32>(pos[vertex_index], 0, 1);
    out.tex_coords = tex_coords[vertex_index];
    return out;
}

fn clamp_coord(coord: vec2i, max_coord: vec2i) -> vec2i {
    return clamp(coord, vec2i(0), max_coord);
}

fn clamp_coordf(coord: vec2f, max_coord: vec2f) -> vec2f {
    return clamp(coord, vec2f(0), max_coord);
}

fn compose_current_hdr(scene_color_sample: vec4f, skybox_sample: vec4f) -> vec3f {
    // scene_color_sample.rgb is premultiplied by scene_color_sample.a after raster blending.
    // Composite with skybox using premultiplied-alpha math.
    return scene_color_sample.xyz + skybox_sample.xyz * (1.0 - scene_color_sample.w);
}

// https://www.elopezr.com/temporal-aa-and-the-quest-for-the-holy-trail/
@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let uv = in.tex_coords;
    let dims_u = textureDimensions(current_final_color_texture);
    let dims = vec2f(dims_u);
    let max_coords = vec2i(dims_u) - vec2i(1);
    let coords = min(vec2i(uv * dims), max_coords);

    let skybox_sample = textureSample(skybox_texture, skybox_texture_sampler, uv);
    let scene_color_sample = textureSample(current_final_color_texture, final_color_sampler, uv);
    let curr_color = compose_current_hdr(scene_color_sample, skybox_sample);

    let curr_depth = textureLoad(current_depth_pyramid_texture, coords, 0).x;
    if (curr_depth <= 0.0) {
        return vec4f(skybox_sample.xyz, 1.0);
    }

    let motion = textureLoad(motion_vectors_texture, coords, 0).xy;
    let prev_uv = uv - motion;
    if (any(prev_uv < vec2f(0.0)) || any(prev_uv > vec2f(1.0))) {
        return vec4f(curr_color, 1.0);
    }

    let prev_coord = clamp_coordf(prev_uv * dims - vec2f(0.5), vec2f(max_coords));
    let prev_coord_floor = floor(prev_coord);
    let prev_coord_frac = fract(prev_coord);
    let taps = array(
        vec2i(prev_coord_floor),
        vec2i(prev_coord_floor) + vec2i(1, 0),
        vec2i(prev_coord_floor) + vec2i(0, 1),
        vec2i(prev_coord_floor) + vec2i(1, 1),
    );
    let tap_weights = array(
        (1.0 - prev_coord_frac.x) * (1.0 - prev_coord_frac.y),
        prev_coord_frac.x * (1.0 - prev_coord_frac.y),
        (1.0 - prev_coord_frac.x) * prev_coord_frac.y,
        prev_coord_frac.x * prev_coord_frac.y,
    );

    var color_acc = vec3f(0.0);
    var history_length_acc = 0.0;
    var weight_acc = 0.0;
    for (var i = 0; i < 4; i++) {
        let tap_coords = clamp_coord(taps[i], max_coords);
        let tap_uv = (vec2f(tap_coords) + vec2f(0.5)) / dims;
        let tap_weight = tap_weights[i];
        let prev_depth = textureLoad(previous_depth_pyramid_texture, tap_coords, 0).x;
        if (prev_depth <= 0.0) {
            continue;
        }

        let depth_delta = abs(curr_depth - prev_depth);
        if (depth_delta >= DEPTH_REJECTION_THRESHOLD) {
            continue;
        }

        let prev_color_sample = textureSample(previous_final_color_texture, final_color_sampler, tap_uv);
        color_acc += prev_color_sample.rgb * tap_weight;
        history_length_acc += prev_color_sample.a * tap_weight;
        weight_acc += tap_weight;
    }

    if (weight_acc <= 0.0) {
        return vec4f(curr_color, 1.0);
    }

    color_acc /= weight_acc;
    history_length_acc /= weight_acc;

    var max_scene_color = scene_color_sample.rgb;
    var min_scene_color = scene_color_sample.rgb;
    for (var x: i32 = -1; x <= 1; x++) {
        for (var y: i32 = -1; y <= 1; y++) {
            if (x == 0 && y == 0) {
                continue;
            }
            let neighbor_coords = clamp(coords + vec2i(x, y), vec2i(0), max_coords);
            let neighbor_uv = (vec2f(neighbor_coords) + vec2f(0.5)) / dims;
            let neighbor_scene_color = textureSample(current_final_color_texture, final_color_sampler, neighbor_uv).rgb;
            max_scene_color = max(max_scene_color, neighbor_scene_color.rgb);
            min_scene_color = min(min_scene_color, neighbor_scene_color.rgb);
        }
    }

    let skybox_contrib = skybox_sample.xyz * (1.0 - scene_color_sample.w);
    let max_w_skybox = max_scene_color.xyz + skybox_contrib;
    let min_w_skybox = min_scene_color.xyz + skybox_contrib;
    let clamped_prev_color = clamp(color_acc, min_w_skybox, max_w_skybox);

    var blend_factor = 1.0 / (1.0 + min(history_length_acc, MAX_HISTORY_LENGTH));
    //blend_factor = pow(blend_factor, 1.0 - min(0.99, length(motion)));
    blend_factor = mix(blend_factor, 1.0, min(1.0, pow(length(motion), 0.2)));

    let blend_corrected_clamped_prev_color = mix(color_acc, clamped_prev_color, blend_factor);
    let resolved_color = mix(blend_corrected_clamped_prev_color, curr_color, blend_factor);
    return vec4f(resolved_color, min(history_length_acc + 1.0, MAX_HISTORY_LENGTH));
    //return vec4f(select(vec3f(0.0), vec3f(1.0), blend_factor < 0.5), history_length_acc + 1.0);
    //return vec4f(select(vec3f(0.0), vec3f(1.0), history_length_acc < 0.0), history_length_acc + 1.0);
    //return vec4f(vec3f(blend_factor), history_length_acc + 1.0);
    //return vec4f(vec3f(history_length_acc / 8.0), history_length_acc + 1.0);
    //return vec4f(curr_color, 1.0);
}
