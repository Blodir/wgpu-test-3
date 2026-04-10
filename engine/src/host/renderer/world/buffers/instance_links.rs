#[derive(Clone, Default)]
pub struct SnapshotInstanceLinks {
    heads: Vec<Option<u32>>,
    next: Vec<Option<u32>>,
}

impl SnapshotInstanceLinks {
    pub fn cursor(&self) -> SnapshotInstanceCursor {
        SnapshotInstanceCursor {
            next_by_snapshot: self.heads.clone(),
        }
    }

    pub fn next_instance(&self, instance_idx: u32) -> Option<u32> {
        self.next.get(instance_idx as usize).copied().flatten()
    }
}

pub struct SnapshotInstanceCursor {
    next_by_snapshot: Vec<Option<u32>>,
}

impl SnapshotInstanceCursor {
    pub fn take_next(
        &mut self,
        snapshot_idx: u32,
        links: &SnapshotInstanceLinks,
    ) -> Option<u32> {
        let slot = self.next_by_snapshot.get_mut(snapshot_idx as usize)?;
        let current = *slot;
        if let Some(instance_idx) = current {
            *slot = links.next_instance(instance_idx);
        }
        current
    }
}

pub struct SnapshotInstanceLinksBuilder {
    heads: Vec<Option<u32>>,
    tails: Vec<Option<u32>>,
    next: Vec<Option<u32>>,
}

impl SnapshotInstanceLinksBuilder {
    pub fn new(snapshot_count: usize) -> Self {
        Self {
            heads: vec![None; snapshot_count],
            tails: vec![None; snapshot_count],
            next: vec![],
        }
    }

    pub fn push(&mut self, snapshot_idx: usize) -> u32 {
        let instance_idx = self.next.len() as u32;
        self.next.push(None);
        if let Some(prev_tail) = self.tails[snapshot_idx] {
            self.next[prev_tail as usize] = Some(instance_idx);
        } else {
            self.heads[snapshot_idx] = Some(instance_idx);
        }
        self.tails[snapshot_idx] = Some(instance_idx);
        instance_idx
    }

    pub fn finish(self) -> SnapshotInstanceLinks {
        SnapshotInstanceLinks {
            heads: self.heads,
            next: self.next,
        }
    }
}
