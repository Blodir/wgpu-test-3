use std::{sync::Arc, time::Instant};

use arc_swap::{ArcSwapOption, Guard};

pub struct UiSnapshot<S> {
    pub tick: u64,
    pub timestamp: Instant,
    pub snap: S,
}

pub type UiSnapshotGuard<S> = Guard<Option<Arc<UiSnapshot<S>>>>;

pub struct UiSnapshotHandoff<S> {
    latest: ArcSwapOption<UiSnapshot<S>>,
}

impl<S> UiSnapshotHandoff<S> {
    pub fn new() -> Self {
        Self {
            latest: ArcSwapOption::from(None),
        }
    }

    pub fn publish(&self, tick: u64, snapshot: S) {
        self.latest.store(Some(Arc::new(UiSnapshot {
            tick,
            timestamp: Instant::now(),
            snap: snapshot,
        })));
    }

    pub fn load(&self) -> UiSnapshotGuard<S> {
        self.latest.load()
    }
}
