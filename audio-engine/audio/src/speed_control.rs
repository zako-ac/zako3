use std::time::Duration;

// i love pid, but linear for now

#[derive(Clone, Debug)]
pub struct SpeedControlConfig {
    pub min_delay: Duration,
    pub max_delay: Duration,
    pub target_fill_ratio: f32,
}

pub fn calculate_delay(
    config: &SpeedControlConfig,
    current_fill: usize,
    capacity: usize,
) -> Duration {
    let fill_ratio = current_fill as f32 / capacity as f32;

    if fill_ratio > config.target_fill_ratio {
        let scale = (fill_ratio - config.target_fill_ratio) / config.target_fill_ratio;
        config.min_delay + (config.max_delay - config.min_delay).mul_f32(scale)
    } else {
        config.min_delay
    }
}
