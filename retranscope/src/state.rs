use std::{collections::VecDeque, time::Duration};

const MAX_SAMPLES: usize = 512;

pub struct Series {
    pub samples: VecDeque<f64>,
    pub prev_total: Option<u64>,
}

impl Default for Series {
    fn default() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_SAMPLES),
            prev_total: None,
        }
    }
}

impl Series {
    pub fn record_sample(&mut self, total_bytes: u64, elapsed: Duration) {
        let delta = match self.prev_total {
            Some(prev) => total_bytes.saturating_sub(prev) as f64,
            None => 0.0,
        };
        self.prev_total = Some(total_bytes);
        let secs = elapsed.as_secs_f64();
        let rate = if secs > 0.0 { delta / secs } else { 0.0 };
        if self.samples.len() >= MAX_SAMPLES {
            self.samples.pop_front();
        }
        self.samples.push_back(rate);
    }

    pub fn current_rate(&self) -> f64 {
        self.samples.back().copied().unwrap_or(0.0)
    }
}

#[derive(Default)]
pub struct State {
    pub trans: Series,
    pub retrans: Series,
}
