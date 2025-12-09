use std::{collections::HashMap, fs::File, io::Write as _, path::Path};

use gltf::{Accessor, Document};
use wgpu_test_3::renderer::render_resources::animationfile::{self, BinRef, Sampler3, SamplerQuat, Target, Track};

use crate::{gltf_utils::{read3f32, read4f32}, utils::ensure_parent_dir_exists};

use super::gltf_utils::readf32;

pub struct TempTargetSamplers<'a> {
    pub translation: Option<gltf::animation::Sampler<'a>>,
    pub rotation: Option<gltf::animation::Sampler<'a>>,
    pub scale: Option<gltf::animation::Sampler<'a>>,
}

pub fn bake_animation(
    animation: &gltf::Animation,
    buffers: &Vec<gltf::buffer::Data>,
    joint_reindex: &HashMap<u32, u32>,
    json_path: &str,
    binary_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let duration = animation
        .samplers()
        .into_iter()
        .map(|s| readf32(&s.input(), buffers).last().unwrap().clone())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0f32);
    let mut tracks: Vec<Track> = vec![];
    let mut binary_data: Vec<u8> = vec![];
    let mut current_binary_offset = 0u32;

    let mut targets = HashMap::<Target, TempTargetSamplers>::new();
    for channel in animation.channels() {
        let target_node = channel.target().node();
        let target = if let Some(joint_idx) = joint_reindex.get(&(target_node.index() as u32)) {
            Target::SkeletonJoint(*joint_idx)
        } else {
            // TODO primitive groups... does this make sense?
            // need to also implement re evaluation of all animation values if a non-bone node is moved
            println!("animation is targetting a non-joint node... todo");
            todo!();
            Target::PrimitiveGroup(target_node.mesh().map(|m| m.index()).unwrap_or(target_node.index()) as u32)
        };

        let entry = targets.entry(target).or_insert(TempTargetSamplers {
            translation: None,
            rotation: None,
            scale: None,
        });

        let sampler = channel.sampler();
        match channel.target().property() {
            gltf::animation::Property::Translation => entry.translation = Some(sampler),
            gltf::animation::Property::Rotation => entry.rotation = Some(sampler),
            gltf::animation::Property::Scale => entry.scale = Some(sampler),
            // Morph targets not supported yet
            _ => (),
        }
    }

    // for each target
    for (target, samplers) in targets.into_iter() {
        // check if shared time array can be used
        let t_input = samplers.translation.as_ref().map(|s| s.input());
        let r_input = samplers.rotation.as_ref().map(|s| s.input());
        let s_input = samplers.scale.as_ref().map(|s| s.input());

        let t_idx = t_input.as_ref().map(|i| i.index());
        let r_idx = r_input.as_ref().map(|i| i.index());
        let s_idx = s_input.as_ref().map(|i| i.index());

        let (mut shared_inputs, mut translation_inputs, mut rotation_inputs, mut scale_inputs) =
            (None, None, None, None);

        match (t_idx, r_idx, s_idx) {
            (Some(t), Some(r), Some(s)) if t == r && r == s => {
                shared_inputs = t_input;
            }
            (Some(t), Some(r), _) if t == r => {
                shared_inputs = t_input;
                scale_inputs = s_input;
            }
            (Some(t), _, Some(s)) if t == s => {
                shared_inputs = t_input;
                rotation_inputs = r_input;
            }
            (_, Some(r), Some(s)) if r == s => {
                shared_inputs = r_input;
                translation_inputs = t_input;
            }
            _ => {
                translation_inputs = t_input;
                rotation_inputs = r_input;
                scale_inputs = s_input;
            }
        }

        let shared_times_data = shared_inputs
            .map(|i| readf32(&i, buffers));
        let translation_times_data = translation_inputs
            .map(|i| readf32(&i, buffers));
        let rotation_times_data = rotation_inputs
            .map(|i| readf32(&i, buffers));
        let scale_times_data = scale_inputs
            .map(|i| readf32(&i, buffers));

        let translation_values_data = samplers.translation.as_ref()
            .map(|s| s.output()).map(|i| read3f32(&i, buffers));
        let rotation_values_data = samplers.rotation.as_ref()
            .map(|s| s.output()).map(|i| read4f32(&i, buffers));
        let scale_values_data = samplers.scale.as_ref()
            .map(|s| s.output()).map(|i| read3f32(&i, buffers));

        // construct binary refs
        // times first, then values

        let shared_times = shared_times_data.map(|data| {
            let offset = current_binary_offset;
            let count = data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&data));
            current_binary_offset = binary_data.len() as u32;
            BinRef { offset, count }
        });

        let translation_times = translation_times_data.map(|data| {
            let offset = current_binary_offset;
            let count = data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&data));
            current_binary_offset = binary_data.len() as u32;
            BinRef { offset, count }
        });

        let rotation_times = rotation_times_data.map(|data| {
            let offset = current_binary_offset;
            let count = data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&data));
            current_binary_offset = binary_data.len() as u32;
            BinRef { offset, count }
        });

        let scale_times = scale_times_data.map(|data| {
            let offset = current_binary_offset;
            let count = data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&data));
            current_binary_offset = binary_data.len() as u32;
            BinRef { offset, count }
        });

        // BIG TODO need to map the data because non-joint hierarchy gets flattened... so all data takes parents into account

        let translation = translation_values_data.map(|values_data| {
            let times = translation_times;
            let interpolation = samplers.translation.unwrap().interpolation().into();

            let offset = current_binary_offset;
            let count = values_data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&values_data));
            current_binary_offset = binary_data.len() as u32;
            let values = BinRef { offset, count };

            Sampler3 { times, values, interpolation }
        });

        let rotation = rotation_values_data.map(|values_data| {
            let times = rotation_times;
            let interpolation = samplers.rotation.unwrap().interpolation().into();

            let offset = current_binary_offset;
            let count = values_data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&values_data));
            current_binary_offset = binary_data.len() as u32;
            let values = BinRef { offset, count };

            SamplerQuat { times, values, interpolation }
        });

        let scale = scale_values_data.map(|values_data| {
            let times = scale_times;
            let interpolation = samplers.scale.unwrap().interpolation().into();

            let offset = current_binary_offset;
            let count = values_data.len() as u32;
            binary_data.extend_from_slice(bytemuck::cast_slice(&values_data));
            current_binary_offset = binary_data.len() as u32;
            let values = BinRef { offset, count };

            Sampler3 { times, values, interpolation }
        });

        let track = animationfile::Track {
            target,
            shared_times,
            translation,
            rotation,
            scale,
        };
        tracks.push(track);
    }

    let animation_clip = animationfile::AnimationClip {
        duration,
        tracks,
        primitive_groups: vec![], // TODO
        binary_path: binary_path.to_string(),
    };

    // write files
    ensure_parent_dir_exists(Path::new(json_path))?;
    let mut binary_file = File::create(binary_path)?;
    binary_file.write_all(binary_data.as_ref())?;

    let json = serde_json::to_string_pretty(&animation_clip)?;
    std::fs::write(json_path, json)?;

    Ok(())
}
