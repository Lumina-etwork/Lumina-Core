use alloc::vec::Vec;

/// A single observed system-wide usage sample.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UsageSample {
    /// Unix timestamp in seconds.
    pub timestamp_secs: u64,
    /// Provisioned capacity units available at the sample time.
    pub capacity_units: u64,
    /// Consumed capacity units at the sample time.
    pub used_units: u64,
}

impl UsageSample {
    pub const fn new(timestamp_secs: u64, capacity_units: u64, used_units: u64) -> Self {
        Self {
            timestamp_secs,
            capacity_units,
            used_units,
        }
    }

    /// Utilization in basis points. Values above 10_000 indicate overcommit.
    pub fn utilization_bps(&self) -> u32 {
        if self.capacity_units == 0 {
            return 0;
        }

        ((self.used_units.saturating_mul(10_000)) / self.capacity_units) as u32
    }
}

/// Capacity recommendation derived from historical usage trend analysis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapacityPlan {
    pub current_capacity_units: u64,
    pub current_used_units: u64,
    pub projected_used_units: u64,
    pub recommended_capacity_units: u64,
    pub headroom_bps: u32,
    pub trend_per_day_units: i64,
    pub scale_out_required: bool,
    pub days_until_threshold: Option<u32>,
}

/// Deterministic planner for hot-path capacity checks.
pub struct CapacityPlanner {
    target_utilization_bps: u32,
    scale_out_threshold_bps: u32,
    forecast_horizon_days: u32,
    min_samples: usize,
}

impl CapacityPlanner {
    pub const fn new(
        target_utilization_bps: u32,
        scale_out_threshold_bps: u32,
        forecast_horizon_days: u32,
        min_samples: usize,
    ) -> Self {
        Self {
            target_utilization_bps,
            scale_out_threshold_bps,
            forecast_horizon_days,
            min_samples,
        }
    }

    pub const fn conservative_default() -> Self {
        Self::new(7_000, 8_500, 30, 3)
    }

    pub fn plan(&self, samples: &[UsageSample]) -> Option<CapacityPlan> {
        if samples.len() < self.min_samples || self.target_utilization_bps == 0 {
            return None;
        }

        let latest = *samples.iter().max_by_key(|sample| sample.timestamp_secs)?;
        let trend_per_day_units = self.trend_per_day(samples);
        let projected_delta = trend_per_day_units.saturating_mul(self.forecast_horizon_days as i64);
        let projected_used_units = if projected_delta.is_negative() {
            latest
                .used_units
                .saturating_sub(projected_delta.unsigned_abs())
        } else {
            latest.used_units.saturating_add(projected_delta as u64)
        };

        let recommended_capacity_units = ceil_div(
            projected_used_units.saturating_mul(10_000),
            self.target_utilization_bps as u64,
        )
        .max(latest.capacity_units);

        let projected_utilization_bps =
            utilization_bps(projected_used_units, latest.capacity_units);
        let scale_out_required = projected_utilization_bps >= self.scale_out_threshold_bps;

        Some(CapacityPlan {
            current_capacity_units: latest.capacity_units,
            current_used_units: latest.used_units,
            projected_used_units,
            recommended_capacity_units,
            headroom_bps: 10_000u32
                .saturating_sub(utilization_bps(latest.used_units, latest.capacity_units)),
            trend_per_day_units,
            scale_out_required,
            days_until_threshold: days_until_threshold(
                latest.used_units,
                latest.capacity_units,
                trend_per_day_units,
                self.scale_out_threshold_bps,
            ),
        })
    }

    fn trend_per_day(&self, samples: &[UsageSample]) -> i64 {
        let mut ordered: Vec<UsageSample> = samples.to_vec();
        ordered.sort_by_key(|sample| sample.timestamp_secs);

        let first = match ordered.first() {
            Some(sample) => sample,
            None => return 0,
        };

        let base = first.timestamp_secs;
        let n = ordered.len() as i128;
        let mut sum_x = 0i128;
        let mut sum_y = 0i128;
        let mut sum_xy = 0i128;
        let mut sum_x2 = 0i128;

        for sample in &ordered {
            let x = ((sample.timestamp_secs.saturating_sub(base)) / 86_400) as i128;
            let y = sample.used_units as i128;
            sum_x += x;
            sum_y += y;
            sum_xy += x * y;
            sum_x2 += x * x;
        }

        let denominator = n * sum_x2 - sum_x * sum_x;
        if denominator == 0 {
            return 0;
        }

        ((n * sum_xy - sum_x * sum_y) / denominator) as i64
    }
}

fn utilization_bps(used_units: u64, capacity_units: u64) -> u32 {
    if capacity_units == 0 {
        return 0;
    }
    ((used_units.saturating_mul(10_000)) / capacity_units) as u32
}

fn ceil_div(numerator: u64, denominator: u64) -> u64 {
    numerator / denominator + u64::from(!numerator.is_multiple_of(denominator))
}

fn days_until_threshold(
    used_units: u64,
    capacity_units: u64,
    trend_per_day_units: i64,
    threshold_bps: u32,
) -> Option<u32> {
    if trend_per_day_units <= 0 || capacity_units == 0 {
        return None;
    }

    let threshold_units = capacity_units.saturating_mul(threshold_bps as u64) / 10_000;
    if used_units >= threshold_units {
        return Some(0);
    }

    Some(ceil_div(threshold_units - used_units, trend_per_day_units as u64) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(day: u64, used: u64) -> UsageSample {
        UsageSample::new(day * 86_400, 1_000, used)
    }

    #[test]
    fn forecasts_linear_growth_and_recommends_headroom() {
        let planner = CapacityPlanner::new(7_000, 8_500, 10, 3);
        let plan = planner
            .plan(&[sample(0, 500), sample(1, 520), sample(2, 540)])
            .unwrap();

        assert_eq!(plan.trend_per_day_units, 20);
        assert_eq!(plan.projected_used_units, 740);
        assert_eq!(plan.recommended_capacity_units, 1_058);
        assert!(!plan.scale_out_required);
        assert_eq!(plan.days_until_threshold, Some(16));
    }

    #[test]
    fn flags_scale_out_when_projection_crosses_threshold() {
        let planner = CapacityPlanner::new(7_000, 8_500, 5, 3);
        let plan = planner
            .plan(&[sample(0, 700), sample(1, 760), sample(2, 820)])
            .unwrap();

        assert!(plan.scale_out_required);
        assert_eq!(plan.days_until_threshold, Some(1));
        assert!(plan.recommended_capacity_units > plan.current_capacity_units);
    }

    #[test]
    fn ignores_negative_trend_for_threshold_eta() {
        let planner = CapacityPlanner::conservative_default();
        let plan = planner
            .plan(&[sample(0, 800), sample(1, 760), sample(2, 720)])
            .unwrap();

        assert_eq!(plan.trend_per_day_units, -40);
        assert_eq!(plan.days_until_threshold, None);
        assert_eq!(plan.recommended_capacity_units, 1_000);
    }
}
