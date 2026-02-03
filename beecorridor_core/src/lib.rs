use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RFEnvelope {
    pub f_ghz_min: f64,
    pub f_ghz_max: f64,
    pub e_base_vpm: f64,
    pub e_no_effect_vpm: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RFMeasurement {
    pub f_ghz: f64,
    pub e_vpm: f64,
}

fn in_band(b: &RFEnvelope, f: f64) -> bool {
    f >= b.f_ghz_min && f <= b.f_ghz_max
}

pub fn r_rf(
    envs: &[RFEnvelope],
    meas: &[RFMeasurement],
) -> f64 {
    let mut r_max = 0.0;
    for m in meas {
        if let Some(b) = envs.iter().find(|b| in_band(b, m.f_ghz)) {
            let denom = (b.e_no_effect_vpm - b.e_base_vpm).max(1e-9);
            let num = (m.e_vpm - b.e_base_vpm).max(0.0);
            let r = num / denom;
            if r > r_max {
                r_max = r;
            }
        }
    }
    r_max
}

/// Enforce "no corridor, no emission" for RF:
/// returns true if emission is permitted.
pub fn rf_permit(
    envs: &[RFEnvelope],
    meas: &[RFMeasurement],
    r_hard: f64,
) -> bool {
    let r = r_rf(envs, meas);
    r < r_hard
}
