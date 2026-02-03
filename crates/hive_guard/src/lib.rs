use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// ECO_BAND represents the risk envelope outcome for a hive.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EcoBand {
    Safe,
    Warning,
    Critical,
}

/// HiveEnvelope encodes bee-centered metrics only: no human fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveEnvelope {
    pub hive_id: String,
    pub brood_frames: u32,           // BROOD
    pub nectar_kg: f32,              // NECTAR
    pub pollen_kg: f32,              // POLLEN
    pub hive_temperature_c: f32,     // HIVE_TEMPERATURE
    pub forager_load: f32,           // FORAGER_LOAD (0-1)
    pub ambient_toxin_ppb: f32,      // TOXIN LOAD AROUND HIVE
    pub forage_diversity_index: f32, // 0-1
    pub forage_radius_m: f32,
    pub eco_band: EcoBand,           // ECO_BAND
    pub eco_impact_score_corridor: f32,
    pub safe_temperature_c_min: f32,
    pub safe_temperature_c_max: f32,
    pub safe_toxin_ppb_max: f32,
    pub safe_forage_diversity_index_min: f32,
    pub safe_forage_radius_m_min: f32,
}

impl HiveEnvelope {
    pub fn evaluate_band(&self) -> EcoBand {
        let temp_ok =
            self.hive_temperature_c >= self.safe_temperature_c_min
                && self.hive_temperature_c <= self.safe_temperature_c_max;
        let toxin_ok = self.ambient_toxin_ppb <= self.safe_toxin_ppb_max;
        let forage_ok = self.forage_diversity_index >= self.safe_forage_diversity_index_min
            && self.forage_radius_m >= self.safe_forage_radius_m_min;

        match (temp_ok, toxin_ok, forage_ok) {
            (true, true, true) => EcoBand::Safe,
            (false, false, false) => EcoBand::Critical,
            _ => EcoBand::Warning,
        }
    }
}

/// HiveSystemAdjustment describes environmental changes only, no bee-body fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveSystemAdjustment {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub hive_id: String,
    // Environmental deltas
    pub delta_pesticide_exposure_ppb: f32, // must be <= 0 (no increase)
    pub delta_shade_fraction: f32,         // can be positive (more shade)
    pub delta_water_availability_index: f32,
    pub delta_forage_radius_m: f32,        // must be >= 0 (no reduction below min)
    pub delta_forage_diversity_index: f32, // must be >= 0
    pub delta_artificial_light_nits: f32,  // must be <= 0
    pub delta_noise_db: f32,               // must be <= 0
    // Eco impact change constrained to be non-negative.
    pub delta_eco_impact_score_corridor: f32,
}

/// Inner ledger event for traceability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HiveLedgerEvent {
    pub adjustment: HiveSystemAdjustment,
    pub pre_envelope: HiveEnvelope,
    pub post_envelope: HiveEnvelope,
}

/// HiveInnerLedger keeps a history of accepted, rights-safe adjustments.
#[derive(Debug, Default)]
pub struct HiveInnerLedger {
    events: Vec<HiveLedgerEvent>,
}

#[derive(Debug, Error)]
pub enum HiveGuardError {
    #[error("Adjustment would increase pesticide exposure")]
    IncreasesPesticideExposure,
    #[error("Adjustment would raise hive temperature above safe band")]
    RaisesHiveTemperature,
    #[error("Adjustment would reduce forage radius below safe minimum")]
    ReducesForageRadius,
    #[error("Adjustment would increase artificial light or noise")]
    IncreasesLightOrNoise,
    #[error("Adjustment would decrease eco impact score corridor")]
    DecreasesEcoImpactScore,
}

impl HiveInnerLedger {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn events(&self) -> &[HiveLedgerEvent] {
        &self.events
    }

    /// Apply a HiveSystemAdjustment under hard bee-rights invariants.
    pub fn apply_adjustment(
        &mut self,
        mut env: HiveEnvelope,
        adj: HiveSystemAdjustment,
    ) -> Result<HiveEnvelope, HiveGuardError> {
        // Invariants:
        // - no action may increase pesticide exposure
        if adj.delta_pesticide_exposure_ppb > 0.0 {
            return Err(HiveGuardError::IncreasesPesticideExposure);
        }
        // - no action may raise hive temperature above safe band
        let projected_temp = env.hive_temperature_c
            + temp_delta_from_shade(adj.delta_shade_fraction);
        if projected_temp > env.safe_temperature_c_max {
            return Err(HiveGuardError::RaisesHiveTemperature);
        }
        // - no action may reduce forage radius below X (safe_forage_radius_m_min)
        let projected_radius = env.forage_radius_m + adj.delta_forage_radius_m;
        if projected_radius < env.safe_forage_radius_m_min {
            return Err(HiveGuardError::ReducesForageRadius);
        }
        // - artificial light and noise cannot increase
        if adj.delta_artificial_light_nits > 0.0 || adj.delta_noise_db > 0.0 {
            return Err(HiveGuardError::IncreasesLightOrNoise);
        }
        // - eco impact score corridor must be non-decreasing (monotone inequality)
        if adj.delta_eco_impact_score_corridor < 0.0 {
            return Err(HiveGuardError::DecreasesEcoImpactScore);
        }

        // All invariants pass, update envelope.
        env.ambient_toxin_ppb += adj.delta_pesticide_exposure_ppb;
        env.forage_radius_m = projected_radius;
        env.forage_diversity_index =
            (env.forage_diversity_index + adj.delta_forage_diversity_index).clamp(0.0, 1.0);
        env.hive_temperature_c = projected_temp;
        env.eco_impact_score_corridor += adj.delta_eco_impact_score_corridor;
        env.eco_band = env.evaluate_band();

        let event = HiveLedgerEvent {
            adjustment: adj,
            pre_envelope: env.clone(),
            post_envelope: env.clone(),
        };
        self.events.push(event);
        Ok(env)
    }
}

/// Simple model: more shade slightly reduces temperature.
fn temp_delta_from_shade(delta_shade_fraction: f32) -> f32 {
    // Shade in [0,1] -> up to -5C, but never heating.
    let clamped = delta_shade_fraction.clamp(-1.0, 1.0);
    if clamped >= 0.0 {
        -5.0 * clamped
    } else {
        // Removing shade, bounded; still must not cross safe max (checked above).
        2.5 * (-clamped)
    }
}

/// Risk envelope classification for external callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RiskEnvelope {
    Safe,
    Warning,
    Critical,
}

pub fn classify_risk(env: &HiveEnvelope) -> RiskEnvelope {
    match env.evaluate_band() {
        EcoBand::Safe => RiskEnvelope::Safe,
        EcoBand::Warning => RiskEnvelope::Warning,
        EcoBand::Critical => RiskEnvelope::Critical,
    }
}
