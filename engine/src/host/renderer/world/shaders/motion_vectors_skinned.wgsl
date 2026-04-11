struct MotionCameraUniform {
    curr_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
}

struct BoneMat34 {
    r0: vec4<f32>,
    r1: vec4<f32>,
    r2: vec4<f32>,
}

@group(0) @binding(0) var<uniform> camera: MotionCameraUniform;
@group(1) @binding(0) var<storage, read> curr_bones: array<BoneMat34>;
@group(1) @binding(1) var<storage, read> prev_bones: array<BoneMat34>;

struct InstanceInput {
    @location(0) curr_m0: vec4<f32>,
    @location(1) curr_m1: vec4<f32>,
    @location(2) curr_m2: vec4<f32>,
    @location(3) curr_m3: vec4<f32>,
    @location(4) curr_palette_offset: u32,
    @location(5) prev_m0: vec4<f32>,
    @location(6) prev_m1: vec4<f32>,
    @location(7) prev_m2: vec4<f32>,
    @location(8) prev_m3: vec4<f32>,
    @location(9) prev_palette_offset: u32,
}

struct VertexInput {
    @location(10) weights: vec4<f32>,
    @location(11) position: vec3<f32>,
    @location(12) joints: vec4<u32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) curr_clip: vec4<f32>,
    @location(1) prev_clip: vec4<f32>,
}

fn apply_bone_to_position(mat: BoneMat34, position: vec4<f32>) -> vec3<f32> {
    return vec3<f32>(
        dot(mat.r0, position),
        dot(mat.r1, position),
        dot(mat.r2, position),
    );
}

fn skin_position_curr(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
) -> vec3<f32> {
    let pos = vec4<f32>(position, 1.0);
    var skinned = vec3<f32>(0.0);
    skinned += weights.x * apply_bone_to_position(curr_bones[palette_offset + joints.x], pos);
    skinned += weights.y * apply_bone_to_position(curr_bones[palette_offset + joints.y], pos);
    skinned += weights.z * apply_bone_to_position(curr_bones[palette_offset + joints.z], pos);
    skinned += weights.w * apply_bone_to_position(curr_bones[palette_offset + joints.w], pos);
    return skinned;
}

fn skin_position_prev(
    palette_offset: u32,
    joints: vec4<u32>,
    weights: vec4<f32>,
    position: vec3<f32>,
) -> vec3<f32> {
    let pos = vec4<f32>(position, 1.0);
    var skinned = vec3<f32>(0.0);
    skinned += weights.x * apply_bone_to_position(prev_bones[palette_offset + joints.x], pos);
    skinned += weights.y * apply_bone_to_position(prev_bones[palette_offset + joints.y], pos);
    skinned += weights.z * apply_bone_to_position(prev_bones[palette_offset + joints.z], pos);
    skinned += weights.w * apply_bone_to_position(prev_bones[palette_offset + joints.w], pos);
    return skinned;
}

@vertex
fn vs_main(instance: InstanceInput, model: VertexInput) -> VertexOutput {
    let curr_transform = mat4x4<f32>(
        instance.curr_m0,
        instance.curr_m1,
        instance.curr_m2,
        instance.curr_m3,
    );
    let prev_transform = mat4x4<f32>(
        instance.prev_m0,
        instance.prev_m1,
        instance.prev_m2,
        instance.prev_m3,
    );
    let curr_pos = skin_position_curr(
        instance.curr_palette_offset,
        model.joints,
        model.weights,
        model.position,
    );
    let has_prev_pose = instance.prev_palette_offset != 0xffffffffu;
    var prev_pos = curr_pos;
    if (has_prev_pose) {
        prev_pos = skin_position_prev(
            instance.prev_palette_offset,
            model.joints,
            model.weights,
            model.position,
        );
    }
    let curr_clip = camera.curr_view_proj * curr_transform * vec4<f32>(curr_pos, 1.0);
    var prev_clip = curr_clip;
    if (has_prev_pose) {
        prev_clip = camera.prev_view_proj * prev_transform * vec4<f32>(prev_pos, 1.0);
    }

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
