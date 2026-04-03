use std::collections::{HashMap, HashSet};
use std::path::Path;

use engine::host::assets::io::asset_formats::rigfile;
use glam::{Quat, Vec3};
use gltf::{Document, Node};

use super::gltf_utils::read_mat4;
use super::utils::ensure_parent_dir_exists;

pub struct RigReindex {
    /// old glTF node index -> joint index in `Rig::joint_nodes`
    pub joint_reindex: HashMap<u32, u32>,
    /// old glTF node index -> node index in `Rig::nodes`
    pub node_reindex: HashMap<u32, u32>,
}

pub fn bake_rigfile(
    gltf: &Document,
    buffers: &Vec<gltf::buffer::Data>,
    output_path: &str,
) -> Result<RigReindex, Box<dyn std::error::Error>> {
    ensure_parent_dir_exists(Path::new(output_path))?;

    let nodes: Vec<Node> = gltf.nodes().collect();
    let mut node_has_mesh = vec![false; nodes.len()];
    let mut children = vec![Vec::<usize>::new(); nodes.len()];
    let mut parent = vec![None::<usize>; nodes.len()];

    for node in &nodes {
        let node_idx = node.index();
        node_has_mesh[node_idx] = node.mesh().is_some();

        for child in node.children() {
            let child_idx = child.index();
            if let Some(existing_parent_idx) = parent[child_idx] {
                if existing_parent_idx != node_idx {
                    return Err(format!(
                        "Node {} has multiple parents ({} and {}), which is unsupported by rig format",
                        child_idx, existing_parent_idx, node_idx
                    )
                    .into());
                }
            } else {
                parent[child_idx] = Some(node_idx);
            }
            children[node_idx].push(child_idx);
        }
    }

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
                let key = joint_node.index() as u32;
                if let Some(existing) = old_idx_to_ibm.get(&key) {
                    if existing != ibm {
                        return Err(format!(
                            "Joint node {} has conflicting inverse bind matrices across skins",
                            key
                        )
                        .into());
                    }
                } else {
                    old_idx_to_ibm.insert(key, *ibm);
                }
            }
        } else {
            return Err(format!(
                "Skin {} is missing inverseBindMatrices; cannot bake skeleton",
                skin.index()
            )
            .into());
        }
    }

    // Keep nodes that contribute to renderable hierarchy:
    // - mesh nodes
    // - joint nodes
    // - ancestors of any of the above
    let mut keep_node = vec![false; nodes.len()];
    for (idx, has_mesh) in node_has_mesh.iter().enumerate() {
        if !has_mesh && !joint_idxs.contains(&idx) {
            continue;
        }
        let mut current = Some(idx);
        while let Some(node_idx) = current {
            if keep_node[node_idx] {
                break;
            }
            keep_node[node_idx] = true;
            current = parent[node_idx];
        }
    }

    // Topological order where parents always come before children.
    let mut output_nodes = Vec::<rigfile::Node>::new();
    let mut old_node_idx_to_new_node_idx = HashMap::<u32, u32>::new();
    let mut topo_old_node_order = Vec::<usize>::new();

    let mut kept_roots = Vec::<usize>::new();
    for old_idx in 0..nodes.len() {
        if !keep_node[old_idx] {
            continue;
        }
        if match parent[old_idx] {
            None => true,
            Some(parent_idx) => !keep_node[parent_idx],
        } {
            kept_roots.push(old_idx);
        }
    }

    let mut stack = Vec::<(usize, Option<u32>)>::new();
    for root in kept_roots.iter().rev() {
        stack.push((*root, None));
    }

    while let Some((old_idx, parent_new_idx)) = stack.pop() {
        if old_node_idx_to_new_node_idx.contains_key(&(old_idx as u32)) {
            continue;
        }

        let new_idx = output_nodes.len() as u32;
        old_node_idx_to_new_node_idx.insert(old_idx as u32, new_idx);
        topo_old_node_order.push(old_idx);

        let (translation, rotation, scale) = nodes[old_idx].transform().decomposed();
        output_nodes.push(rigfile::Node {
            name: nodes[old_idx].name().map(str::to_string),
            parent: parent_new_idx,
            transform: rigfile::SRT::new(
                Vec3::from(scale),
                Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]),
                Vec3::from(translation),
            ),
        });

        for child in children[old_idx].iter().rev() {
            if keep_node[*child] {
                stack.push((*child, Some(new_idx)));
            }
        }
    }

    let kept_count = keep_node.iter().filter(|&&k| k).count();
    if kept_count != old_node_idx_to_new_node_idx.len() {
        return Err(format!(
            "Failed to topologically sort retained nodes (expected {}, visited {})",
            kept_count,
            old_node_idx_to_new_node_idx.len()
        )
        .into());
    }

    let mut bind_matrices = vec![glam::Mat4::IDENTITY; output_nodes.len()];
    for (node_idx, node) in output_nodes.iter().enumerate() {
        let local = node.transform.to_mat4();
        bind_matrices[node_idx] = if let Some(parent_idx) = node.parent {
            bind_matrices
                .get(parent_idx as usize)
                .copied()
                .map(|parent_world| parent_world * local)
                .unwrap_or(local)
        } else {
            local
        };
    }

    let mut joint_nodes = Vec::<u32>::new();
    let mut inverse_bind_matrices = Vec::<glam::Mat4>::new();
    for old_idx in topo_old_node_order {
        if !joint_idxs.contains(&old_idx) {
            continue;
        }
        let joint_idx = joint_nodes.len() as u32;
        let new_node_idx = *old_node_idx_to_new_node_idx
            .get(&(old_idx as u32))
            .ok_or_else(|| format!("Missing node remap for joint node {}", old_idx))?;
        let ibm = old_idx_to_ibm
            .get(&(old_idx as u32))
            .ok_or_else(|| format!("Missing inverse bind matrix for joint node {}", old_idx))?;

        joint_nodes.push(new_node_idx);
        inverse_bind_matrices.push(glam::Mat4::from_cols_array_2d(ibm));
        joint_reindex.insert(old_idx as u32, joint_idx);
    }

    let rig = rigfile::Rig {
        nodes: output_nodes,
        bind_matrices,
        joint_nodes,
        inverse_bind_matrices,
    };
    let json = serde_json::to_string_pretty(&rig)?;
    std::fs::write(output_path, json)?;

    Ok(RigReindex {
        joint_reindex,
        node_reindex: old_node_idx_to_new_node_idx,
    })
}
