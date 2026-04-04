use std::{ops::Range, sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};

use crate::{
    game::scene_tree::{SceneNodeId, Sun},
    host::assets::{
        io::asset_formats::rigfile::SRT,
        store::{MaterialRenderId, ModelRenderId, TextureRenderId},
    },
};

#[derive(Clone, Copy)]
pub struct PointLightSnapshot {
    pub position: glam::Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

#[derive(Debug)]
pub struct EnvironmentMapSnapshot {
    pub prefiltered: TextureRenderId,
    pub di: TextureRenderId,
}

pub struct LightsSnapshot {
    pub sun: Sun,
    pub environment_map_intensity: f32,
    pub point_lights: Vec<PointLightSnapshot>,
    pub environment_map: Option<EnvironmentMapSnapshot>,
}

#[derive(Clone, Copy)]
pub struct AnimationSnapshot(pub u64);

pub struct SkinnedInstanceSnapshot {
    pub node_id: SceneNodeId,
    pub model_transform: SRT,
    pub animation: Option<AnimationSnapshot>,
    pub dirty: bool,
    pub prev_index: Option<u32>,
}

pub struct StaticInstanceSnapshot {
    pub node_id: SceneNodeId,
    pub model_transform: SRT,
    pub animation: Option<AnimationSnapshot>,
    pub dirty: bool,
    pub prev_index: Option<u32>,
}

pub struct SubmeshBatch {
    pub instances: Vec<u32>, // indexes into skinned_instances/static_instances
    pub submesh_idx: usize,
}

pub struct MeshBatch {
    pub model_id: ModelRenderId,
    pub submesh_range: Range<usize>, // indexes into both submesh and instance ranges arrays
}

pub struct MaterialBatch {
    pub material_id: MaterialRenderId,
    pub mesh_range: Range<usize>,
}

#[derive(Default)]
pub struct PassBatches {
    pub submesh_batches: Vec<SubmeshBatch>,
    pub material_batches: Vec<MaterialBatch>, // indexes into mesh batches
    pub mesh_batches: Vec<MeshBatch>,         // indexes into submesh batches
    pub skinned_batch: Range<usize>,          // indexes into material batches
    pub static_batch: Range<usize>,           // indexes into material batches
}

#[derive(Default)]
pub struct MeshDrawSnapshot {
    pub opaque_batch: PassBatches,
    pub transparent_batch: PassBatches,
    pub skinned_instances: Vec<SkinnedInstanceSnapshot>,
    pub static_instances: Vec<StaticInstanceSnapshot>,
}

pub struct FixedSnapshot {
    pub mesh_draw_snapshot: MeshDrawSnapshot,
    pub lights: LightsSnapshot,
}
impl Default for FixedSnapshot {
    fn default() -> Self {
        Self {
            lights: LightsSnapshot {
                sun: Sun::default(),
                environment_map_intensity: 1.0,
                point_lights: vec![],
                environment_map: None,
            },
            mesh_draw_snapshot: MeshDrawSnapshot::default(),
        }
    }
}

pub type FixedSnapshotGuard = Guard<Arc<FixedSnapshotPair>>;

#[derive(Clone)]
pub struct FixedSnapshotPair {
    pub prev: Arc<FixedSnapshot>,
    pub prev_timestamp: Instant,
    pub curr: Arc<FixedSnapshot>,
    pub curr_timestamp: Instant,
}

pub struct FixedSnapshotHandoff {
    pair: ArcSwap<FixedSnapshotPair>,
}

impl FixedSnapshotHandoff {
    pub fn new(init: FixedSnapshot) -> Self {
        let init = Arc::new(init);
        let pair = FixedSnapshotPair {
            prev: init.clone(),
            prev_timestamp: Instant::now(),
            curr: init,
            curr_timestamp: Instant::now(),
        };
        Self {
            pair: ArcSwap::from(Arc::new(pair)),
        }
    }

    pub fn publish(&self, snap: FixedSnapshot) {
        let old = self.pair.load();
        let next = FixedSnapshotPair {
            prev: old.curr.clone(),
            prev_timestamp: old.curr_timestamp,
            curr: Arc::new(snap),
            curr_timestamp: Instant::now(),
        };
        self.pair.store(Arc::new(next));
    }

    pub fn load(&self) -> FixedSnapshotGuard {
        self.pair.load()
    }
}
