use std::collections::{HashMap, HashSet};
use std::path::Path;

use gltf::{Document, Node};
use engine::main::assets::io::asset_formats::rigfile;

use super::gltf_utils::read_mat4;
use super::utils::ensure_parent_dir_exists;

/// Returns a joint reindexing map (old -> new)
pub fn bake_rigfile(
    gltf: &Document,
    buffers: &Vec<gltf::buffer::Data>,
    output_path: &str,
) -> Result<HashMap<u32, u32>, Box<dyn std::error::Error>> {
    ensure_parent_dir_exists(Path::new(output_path))?;

    let nodes: Vec<Node> = gltf.nodes().collect();
    let mut joint_idxs = HashSet::<usize>::new();
    let mut joint_reindex = HashMap::<u32, u32>::new();
    let mut old_idx_to_ibm = HashMap::<u32, [[f32; 4]; 4]>::new();

    // collect old indexes of all the joints + inverse bind matrices
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
                old_idx_to_ibm.insert(joint_node.index() as u32, *ibm);
            }
        } else {
            return Err(format!(
                "Skin {} is missing inverseBindMatrices; cannot bake skeleton",
                skin.index()
            )
            .into());
        }
    }

    let mut old_node_idx_to_new_node_idx = HashMap::<u32, u32>::new();

    // TODO
    // filter all nodes which don't have a mesh or joint as descendant
    // topologically sort the nodes
    // keep track of old_node_idx -> new_node_idx mapping

    let mut joint_nodes = Vec::<u32>::new();
    for old_idx in joint_idxs {
        let new_idx = joint_nodes.len();
        joint_nodes.push(old_node_idx_to_new_node_idx.get(old_idx).unwrap());
        joint_reindex.insert(old_idx as u32, new_idx as u32);
    }

    /*
    let mut joints = Vec::<&Node>::new();
    for old_idx in joint_idxs {
        let new_idx = joints.len();
        joints.push(&nodes[old_idx]);
        joint_reindex.insert(old_idx as u32, new_idx as u32);
    }
    */

    /*
    let mut output_joints: Vec<rigfile::Joint> = vec![];
    for gltf_joint in joints {
        let mapped_joint = rigfile::Joint {
            name: gltf_joint.name().map(|slice| slice.to_string()),
            children: gltf_joint
                .children()
                .map(|child| *joint_reindex.get(&(child.index() as u32)).unwrap())
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
    */

    let rig = rigfile::Rig {
        nodes: todo!(),
        joint_nodes,
        inverse_bind_matrices: todo!(),
    };
    let json = serde_json::to_string_pretty(&rig)?;
    std::fs::write(output_path, json)?;

    Ok(joint_reindex)
}
