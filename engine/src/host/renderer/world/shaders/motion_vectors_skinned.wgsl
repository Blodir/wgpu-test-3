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

struct InstanceData {
    m0: vec4<f32>,
    m1: vec4<f32>,
    m2: vec4<f32>,
    m3: vec4<f32>,
    itr_0: vec4<f32>,
    itr_1: vec4<f32>,
    itr_2: vec4<f32>,
    offsets: vec4<u32>,
}

@group(1) @binding(2) var<storage, read> curr_instances: array<InstanceData>;
@group(1) @binding(3) var<storage, read> prev_instances: array<InstanceData>;

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
fn vs_main(
    @builtin(instance_index) instance_index: u32,
    model: VertexInput,
) -> VertexOutput {
    let instance = curr_instances[instance_index];
    let curr_transform = mat4x4<f32>(
        instance.m0,
        instance.m1,
        instance.m2,
        instance.m3,
    );
    let curr_pos = skin_position_curr(
        instance.offsets.x,
        model.joints,
        model.weights,
        model.position,
    );
    let has_prev_pose = instance.offsets.y != 0xffffffffu;
    var prev_pos = curr_pos;
    var prev_transform = curr_transform;
    if (has_prev_pose) {
        let prev_instance = prev_instances[instance.offsets.y];
        prev_transform = mat4x4<f32>(
            prev_instance.m0,
            prev_instance.m1,
            prev_instance.m2,
            prev_instance.m3,
        );
        prev_pos = skin_position_prev(
            prev_instance.offsets.x,
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
