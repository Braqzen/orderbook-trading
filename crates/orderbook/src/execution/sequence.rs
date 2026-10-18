use std::collections::HashMap;
use uuid::Uuid;

pub struct ExecutionSequence {
    next: HashMap<Uuid, u64>,
}

impl ExecutionSequence {
    pub fn new() -> Self {
        Self {
            next: HashMap::new(),
        }
    }

    pub fn next(&mut self, client_id: Uuid) -> u64 {
        let seq = self.next.entry(client_id).or_insert(0);
        *seq += 1;
        *seq
    }
}
