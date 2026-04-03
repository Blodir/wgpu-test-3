use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};

use crate::game::build_snapshot::CameraSnapshot;
use crate::game::sim::SimDebugInfo;

#[derive(Clone, Copy)]
pub struct CameraSnapshotPair {
    pub prev: CameraSnapshot,
    pub prev_timestamp: Instant,
    pub curr: CameraSnapshot,
    pub curr_timestamp: Instant,
}
impl Default for CameraSnapshotPair {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            prev: CameraSnapshot::default(),
            prev_timestamp: now,
            curr: CameraSnapshot::default(),
            curr_timestamp: now,
        }
    }
}

pub struct VarSnapshot<S> {
    pub tick: u64,
    pub timestamp: Instant,
    pub camera_pair: CameraSnapshotPair,
    pub sim_debug: SimDebugInfo,
    pub snap: S,
}
impl<S: Default> Default for VarSnapshot<S> {
    fn default() -> Self {
        Self {
            tick: 0,
            timestamp: Instant::now(),
            camera_pair: CameraSnapshotPair::default(),
            sim_debug: SimDebugInfo::default(),
            snap: S::default(),
        }
    }
}

pub type VarSnapshotGuard<S> = Guard<Arc<VarSnapshot<S>>>;

pub struct VarSnapshotHandoff<S> {
    latest: ArcSwap<VarSnapshot<S>>,
}

impl<S: Default> VarSnapshotHandoff<S> {
    pub fn new() -> Self {
        Self {
            latest: ArcSwap::from(Arc::new(VarSnapshot::default())),
        }
    }

    pub fn publish(
        &self,
        tick: u64,
        camera_pair: CameraSnapshotPair,
        sim_debug: SimDebugInfo,
        snapshot: S,
    ) {
        self.latest.store(Arc::new(VarSnapshot {
            tick,
            timestamp: Instant::now(),
            camera_pair,
            sim_debug,
            snap: snapshot,
        }));
    }

    pub fn load(&self) -> VarSnapshotGuard<S> {
        self.latest.load()
    }
}
