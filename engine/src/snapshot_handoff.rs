use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};

use crate::game::build_snapshot::RenderSnapshot;

pub type SnapshotGuard = Guard<Arc<SnapshotPair>>;

#[derive(Clone)]
pub struct SnapshotPair {
    pub prev: Arc<RenderSnapshot>,
    pub prev_timestamp: Instant,
    pub curr: Arc<RenderSnapshot>,
    pub curr_timestamp: Instant,
}

pub struct SnapshotHandoff {
    pair: ArcSwap<SnapshotPair>,
}

impl SnapshotHandoff {
    pub fn new(init: RenderSnapshot) -> Self {
        let init = Arc::new(init);
        let pair = SnapshotPair {
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
        let next = SnapshotPair {
            prev: old.curr.clone(),
            prev_timestamp: old.curr_timestamp,
            curr: Arc::new(snap),
            curr_timestamp: Instant::now(),
        };
        self.pair.store(Arc::new(next));
    }

    pub fn load(&self) -> SnapshotGuard {
        self.pair.load()
    }
}
