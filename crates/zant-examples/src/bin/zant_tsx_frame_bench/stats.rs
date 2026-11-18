//! Frame durations in milliseconds and comparisons within paired rounds.

#[derive(Clone, Default)]
pub struct Stats {
    pub samples: Vec<f64>,
}

impl Stats {
    pub fn new() -> Stats {
        Stats::default()
    }

    pub fn add(&mut self, ms: f64) {
        self.samples.push(ms);
    }

    pub fn extend(&mut self, other: &Stats) {
        self.samples.extend_from_slice(&other.samples);
    }

    pub fn frames(&self) -> usize {
        self.samples.len()
    }

    pub fn avg(&self) -> f64 {
        if self.samples.is_empty() {
            return 0.0;
        }
        self.samples.iter().sum::<f64>() / self.samples.len() as f64
    }

    fn sorted(&self) -> Vec<f64> {
        let mut sorted = self.samples.clone();
        sorted.sort_unstable_by(f64::total_cmp);
        sorted
    }

    pub(super) fn median(&self) -> f64 {
        median_sorted(&self.sorted())
    }

    pub fn p95(&self) -> f64 {
        let sorted = self.sorted();
        match sorted.len() {
            0 => 0.0,
            len => sorted[usize::min(len - 1, (len * 95) / 100)],
        }
    }

    pub fn max(&self) -> f64 {
        self.sorted().last().copied().unwrap_or(0.0)
    }
}

// For even counts, use the midpoint of the two central samples.
fn median_sorted(sorted: &[f64]) -> f64 {
    let mid = sorted.len() / 2;
    match sorted.len() {
        0 => 0.0,
        len if len.is_multiple_of(2) => sorted[mid - 1].midpoint(sorted[mid]),
        _ => sorted[mid],
    }
}

#[derive(Default)]
pub(super) struct PairedRounds {
    pub(super) rounds: Vec<(Stats, Stats)>,
}

impl PairedRounds {
    /// Keep the numerator side fixed even when it ran second.
    pub(super) fn ratios(&self) -> (f64, f64) {
        let mut ratios: Vec<_> = self
            .rounds
            .iter()
            .map(|(left, right)| {
                let denominator = right.median();
                if denominator > 0.0 {
                    left.median() / denominator
                } else {
                    f64::INFINITY
                }
            })
            .collect();
        let noise = ratios
            .iter()
            .map(|ratio| (ratio - 1.0).abs())
            .fold(0.0, f64::max);
        ratios.sort_unstable_by(f64::total_cmp);
        (median_sorted(&ratios), noise)
    }

    fn pooled(&self) -> (Stats, Stats) {
        let mut left = Stats::new();
        let mut right = Stats::new();
        for (a, b) in &self.rounds {
            left.extend(a);
            right.extend(b);
        }
        (left, right)
    }
}

pub(super) struct Row {
    pub(super) kind: &'static str,
    pub(super) name: &'static str,
    pub(super) comparison: PairedRounds,
    pub(super) control: PairedRounds,
}

impl Row {
    pub(super) fn new(kind: &'static str, name: &'static str) -> Self {
        Self {
            kind,
            name,
            comparison: PairedRounds::default(),
            control: PairedRounds::default(),
        }
    }

    pub(super) fn report(&self) -> f64 {
        let (ratio, _) = self.comparison.ratios();
        let (aa_ratio, noise) = self.control.ratios();
        let marker = if noise > 0.10 { " noisy" } else { "" };
        let (tsx, manual) = self.comparison.pooled();
        println!(
            "{} {} frames={} tsx_avg={:.4} manual_avg={:.4} ratio={ratio:.4} tsx_p95={:.4} manual_p95={:.4} tsx_max={:.4} manual_max={:.4} tsx_median={:.4} manual_median={:.4} aa_ratio={aa_ratio:.4} noise={noise:.4}{marker}",
            self.kind,
            self.name,
            tsx.frames(),
            tsx.avg(),
            manual.avg(),
            tsx.p95(),
            manual.p95(),
            tsx.max(),
            manual.max(),
            tsx.median(),
            manual.median(),
        );
        ratio
    }
}
