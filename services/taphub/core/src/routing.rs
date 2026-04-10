use zako3_types::OnlineTapStates;

pub struct DynamicSampler {
    cursor: f64,
}

impl DynamicSampler {
    pub fn new() -> Self {
        Self { cursor: 0.5 }
    }

    fn next_pick(&mut self, ids: &[u64], weights: &[f64]) -> u64 {
        const PHI: f64 = 0.618_033_988_749_895;
        self.cursor = (self.cursor + PHI) % 1.0;

        let total_weight: f64 = weights.iter().sum();
        let target = self.cursor * total_weight;

        let mut running_sum = 0.0_f64;
        for (i, &w) in weights.iter().enumerate() {
            running_sum += w;
            if running_sum >= target {
                return ids[i];
            }
        }
        ids[ids.len() - 1]
    }

    pub fn next_connection_id(&mut self, states: &OnlineTapStates) -> Option<u64> {
        if states.is_empty() {
            return None;
        }

        let ids: Vec<u64> = states.iter().map(|s| s.connection_id).collect();
        let weights: Vec<f64> = states.iter().map(|s| s.selection_weight as f64).collect();

        Some(self.next_pick(&ids, &weights))
    }
}
