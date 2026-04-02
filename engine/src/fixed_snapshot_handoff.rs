use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwap, Guard};

use crate::game::build_snapshot::FixedSnapshot;

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

    pub fn publish(&self, mut snap: FixedSnapshot) {
        let old = self.pair.load();
        snap.link_previous(&old.curr);
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
