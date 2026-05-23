struct MotionCameraUniform {
    curr_view_proj: mat4x4<f32>,
    curr_inverse_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> camera: MotionCameraUniform;

struct InstanceData {
    curr_m0: vec4<f32>,
    curr_m1: vec4<f32>,
    curr_m2: vec4<f32>,
    curr_m3: vec4<f32>,
    itr_0: vec4<f32>,
    itr_1: vec4<f32>,
    itr_2: vec4<f32>,
}

@group(1) @binding(0) var<storage, read> instances: array<InstanceData>;

struct VertexInput {
    @location(8) position: vec3<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) curr_clip: vec4<f32>,
    @location(1) prev_clip: vec4<f32>,
}

@vertex
fn vs_main(
    @builtin(instance_index) instance_index: u32,
    model: VertexInput,
) -> VertexOutput {
    let instance = instances[instance_index];
    let curr_transform = mat4x4<f32>(
        instance.curr_m0,
        instance.curr_m1,
        instance.curr_m2,
        instance.curr_m3,
    );
    let curr_clip = camera.curr_view_proj * curr_transform * vec4<f32>(model.position, 1.0);
    let world_position = camera.curr_inverse_view_proj * curr_clip;
    let prev_clip = camera.prev_view_proj * (world_position / max(world_position.w, 1e-8));

    var out: VertexOutput;
    out.clip_position = curr_clip;
    out.curr_clip = curr_clip;
    out.prev_clip = prev_clip;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec2<f32> {
    if (abs(in.curr_clip.w) < 1e-6 || abs(in.prev_clip.w) < 1e-6) {
        return vec2<f32>(0.0);
    }
    let curr_ndc = in.curr_clip.xy / in.curr_clip.w;
    let prev_ndc = in.prev_clip.xy / in.prev_clip.w;
    return (curr_ndc - prev_ndc) * vec2<f32>(0.5, -0.5);
}
