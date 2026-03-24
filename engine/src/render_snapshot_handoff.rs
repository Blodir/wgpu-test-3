use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};

use crate::game::build_snapshot::RenderSnapshot;

pub type RenderSnapshotGuard = Guard<Arc<RenderSnapshotPair>>;

#[derive(Clone)]
pub struct RenderSnapshotPair {
    pub prev: Arc<RenderSnapshot>,
    pub prev_timestamp: Instant,
    pub curr: Arc<RenderSnapshot>,
    pub curr_timestamp: Instant,
}

pub struct RenderSnapshotHandoff {
    pair: ArcSwap<RenderSnapshotPair>,
}

impl RenderSnapshotHandoff {
    pub fn new(init: RenderSnapshot) -> Self {
        let init = Arc::new(init);
        let pair = RenderSnapshotPair {
            prev: init.clone(),
            prev_timestamp: Instant::now(),
            curr: init,
            curr_timestamp: Instant::now(),
        };
        Self {
            pair: ArcSwap::from(Arc::new(pair)),
        }
    }

    pub fn publish(&self, snap: RenderSnapshot) {
        let old = self.pair.load();
        let next = RenderSnapshotPair {
            prev: old.curr.clone(),
            prev_timestamp: old.curr_timestamp,
            curr: Arc::new(snap),
            curr_timestamp: Instant::now(),
        };
        self.pair.store(Arc::new(next));
    }

    pub fn load(&self) -> RenderSnapshotGuard {
        self.pair.load()
    }
}
