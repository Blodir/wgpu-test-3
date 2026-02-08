use std::collections::{HashMap, HashSet};
use std::path::Path;

use gltf::{Document, Node};
use engine::resource_system::file_formats::skeletonfile;

use super::gltf_utils::read_mat4;
use super::utils::ensure_parent_dir_exists;

/// Returns a joint reindexing map (old -> new)
pub fn bake_skeletonfile(
    gltf: &Document,
    buffers: &Vec<gltf::buffer::Data>,
    output_path: &str,
) -> Result<HashMap<u32, u32>, Box<dyn std::error::Error>> {
    ensure_parent_dir_exists(Path::new(output_path))?;
    let nodes: Vec<Node> = gltf.nodes().collect();
    let mut joint_idxs = HashSet::<usize>::new();
    let mut reindex = HashMap::<u32, u32>::new();
    let mut inverse_bind_matrices = HashMap::<u32, [[f32; 4]; 4]>::new();

    for ref skin in gltf.skins() {
        let joints: Vec<_> = skin.joints().collect();
        for joint in &joints {
            joint_idxs.insert(joint.index());
        }
        if let Some(accessor) = skin.inverse_bind_matrices() {
            let ibms = read_mat4(&accessor, buffers);
            if ibms.len() != joints.len() {
                return Err(format!(
                    "inverseBindMatrices count ({}) does not match joint count ({}) for skin {}",
                    ibms.len(),
                    joints.len(),
                    skin.index()
                )
                .into());
            }
            for (joint_node, ibm) in joints.iter().zip(ibms.iter()) {
                inverse_bind_matrices.insert(joint_node.index() as u32, *ibm);
            }
        } else {
            return Err(format!(
                "Skin {} is missing inverseBindMatrices; cannot bake skeleton",
                skin.index()
            )
            .into());
        }
    }
    let mut joints = Vec::<&Node>::new();
    for old_idx in joint_idxs {
        let new_idx = joints.len();
        joints.push(&nodes[old_idx]);
        reindex.insert(old_idx as u32, new_idx as u32);
    }

    let mut output_joints: Vec<skeletonfile::Joint> = vec![];
    for gltf_joint in joints {
        let mapped_joint = skeletonfile::Joint {
            name: gltf_joint.name().map(|slice| slice.to_string()),
            children: gltf_joint
                .children()
                .map(|child| *reindex.get(&(child.index() as u32)).unwrap())
                .collect(),
            trs: gltf_joint.transform().matrix(),
            inverse_bind_matrix: *inverse_bind_matrices
                .get(&(gltf_joint.index() as u32))
                .ok_or_else(|| {
                    format!(
                        "Missing inverse bind matrix for joint {}",
                        gltf_joint.index()
                    )
                })?,
        };
        output_joints.push(mapped_joint);
    }

    let skeleton = skeletonfile::Skeleton {
        joints: output_joints,
    };
    let json = serde_json::to_string_pretty(&skeleton)?;
    std::fs::write(output_path, json)?;

    Ok(reindex)
}
