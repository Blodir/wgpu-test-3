use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwapOption, Guard};

use crate::game::build_snapshot::CameraSnapshot;
use crate::game::sim::SimDebugInfo;

#[derive(Clone, Copy)]
pub struct CameraSnapshotPair {
    pub prev: CameraSnapshot,
    pub prev_timestamp: Instant,
    pub curr: CameraSnapshot,
    pub curr_timestamp: Instant,
}

pub struct VarSnapshot<S> {
    pub tick: u64,
    pub timestamp: Instant,
    pub camera_pair: CameraSnapshotPair,
    pub sim_debug: SimDebugInfo,
    pub snap: S,
}

pub type VarSnapshotGuard<S> = Guard<Option<Arc<VarSnapshot<S>>>>;

pub struct VarSnapshotHandoff<S> {
    latest: ArcSwapOption<VarSnapshot<S>>,
}

impl<S> VarSnapshotHandoff<S> {
    pub fn new() -> Self {
        Self {
            latest: ArcSwapOption::from(None),
        }
    }

    pub fn publish(
        &self,
        tick: u64,
        camera_pair: CameraSnapshotPair,
        sim_debug: SimDebugInfo,
        snapshot: S,
    ) {
        self.latest.store(Some(Arc::new(VarSnapshot {
            tick,
            timestamp: Instant::now(),
            camera_pair,
            sim_debug,
            snap: snapshot,
        })));
    }

    pub fn load(&self) -> VarSnapshotGuard<S> {
        self.latest.load()
    }
}
