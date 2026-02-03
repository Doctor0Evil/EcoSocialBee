use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeeRiskCoords {
    pub r_rf: f64,
    pub r_noise: f64,
    pub r_vib: f64,
    pub r_thermal: f64,
    pub r_light: f64,
    pub r_chem: f64,
    pub r_sigma: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeeRiskWeights {
    pub w_rf: f64,
    pub w_noise: f64,
    pub w_vib: f64,
    pub w_thermal: f64,
    pub w_light: f64,
    pub w_chem: f64,
    pub w_sigma: f64,
    pub v_safe: f64,
    pub v_crit: f64,
    pub r_hard: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeeRiskSummary {
    pub v_bee: f64,
    pub max_r: f64,
    pub bee_neural_safe: bool,
}

pub fn compute_v_bee(r: &BeeRiskCoords, w: &BeeRiskWeights) -> BeeRiskSummary {
    let mut v = 0.0;
    let mut max_r = 0.0;

    let coords = [
        ("rf", r.r_rf, w.w_rf),
        ("noise", r.r_noise, w.w_noise),
        ("vib", r.r_vib, w.w_vib),
        ("thermal", r.r_thermal, w.w_thermal),
        ("light", r.r_light, w.w_light),
        ("chem", r.r_chem, w.w_chem),
        ("sigma", r.r_sigma, w.w_sigma),
    ];

    for (_, rv, wv) in coords {
        let rv_pos = rv.max(0.0);
        v += wv * rv_pos * rv_pos;
        if rv_pos > max_r {
            max_r = rv_pos;
        }
    }

    let bee_neural_safe =
        v <= w.v_safe &&
        max_r <= w.r_hard;

    BeeRiskSummary { v_bee: v, max_r, bee_neural_safe }
}

/// Hard gate: return true if emission is permitted.
pub fn permit_emission(summary: &BeeRiskSummary) -> bool {
    summary.bee_neural_safe
}
