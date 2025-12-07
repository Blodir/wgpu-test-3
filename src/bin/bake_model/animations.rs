use std::collections::HashMap;

use gltf::Document;
use wgpu_test_3::renderer::render_resources::animationfile::{self, Track, Target};

use super::gltf_utils::readf32;

pub struct TempTargetSamplers<'a> {
    pub translation: Option<&'a gltf::animation::Sampler<'a>>,
    pub rotation: Option<&'a gltf::animation::Sampler<'a>>,
    pub scale: Option<&'a gltf::animation::Sampler<'a>>,
}

pub fn bake_animation(
    gltf: &Document,
    animation: &gltf::Animation,
    buffers: &Vec<gltf::buffer::Data>,
    joint_reindex: HashMap<u32, u32>,
    output_path: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let duration = animation
        .samplers()
        .into_iter()
        .map(|s| readf32(&s.input(), buffers).last().unwrap().clone())
        .max_by(|a, b| a.partial_cmp(b).unwrap())
        .unwrap_or(0f32);
    let mut tracks: Vec<Track> = vec![];
    let mut binary_data: Vec<u8> = vec![];

    let mut targets = HashMap::<Target, TempTargetSamplers>::new();
    for channel in animation.channels() {
        // check if the target node is part of a skeleton
        //  use joint_reindex to get the joint index in the skeletonfile
        // if not, then need to find the primitive instance indices
        // collect the samplers in targets hashmap
    }

    // for each target
    for (target, samplers) in targets {
        // check if all target channels share the same time array

        // read binary times arrays (always scalar f32)
        // let shared_times: Option<Vec<u8>> = ...
        // let translation_times: Option<Vec<u8>> = ...

        // read binary data arrays
        // let translation_data: Option<Vec<u8>> = ... (vec3 f32)
        // let rotation_data: Option<Vec<u8>> = ... (vec4 f32)
        // let scale_data: Option<Vec<u8>> = ... (vec3 f32)

        // BIG TODO need to map the data because hierarchy gets flattened... so all data takes parents into account

        // construct binary refs

        let track = animationfile::Track {
            target,
            shared_times: todo!(),
            translation: todo!(),
            rotation: todo!(),
            scale: todo!(),
        };
        // append binary_data
        tracks.push(track);
    }

    // write binary file

    let animation_clip = animationfile::AnimationClip {
        duration,
        tracks,
        primitive_groups: todo!(),
        binary_path: todo!(),
    };

    let json = serde_json::to_string_pretty(&animation_clip)?;
    std::fs::write(output_path, json)?;

    Ok(())
}
