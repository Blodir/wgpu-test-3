@group(0) @binding(0) var reprojected_diffuse: texture_2d<f32>;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
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
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let source_dims = vec2i(textureDimensions(reprojected_diffuse));
    let target_dims = max(source_dims / vec2i(2), vec2i(1));
    let target_coords = vec2i(in.clip_position.xy);
    let scale = vec2f(source_dims) / vec2f(target_dims);
    let start = vec2f(target_coords) * scale;
    let end = vec2f(target_coords + vec2i(1)) * scale;
    let first_coords = vec2i(floor(start));
    let end_coords = min(vec2i(ceil(end)), source_dims);

    var color_acc = vec4f(0.0);
    for (var sample_y = first_coords.y; sample_y < end_coords.y; sample_y += 1) {
        for (var sample_x = first_coords.x; sample_x < end_coords.x; sample_x += 1) {
            let sample_coords = vec2i(sample_x, sample_y);
            let overlap = max(
                min(vec2f(sample_coords + vec2i(1)), end) - max(vec2f(sample_coords), start),
                vec2f(0.0)
            );
            let weight = overlap.x * overlap.y;
            color_acc += textureLoad(reprojected_diffuse, sample_coords, 0) * weight;
        }
    }
    return color_acc / (scale.x * scale.y);
}
