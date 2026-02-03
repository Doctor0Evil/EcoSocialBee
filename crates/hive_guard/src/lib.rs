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

// Hex-stamp: 0xa1b2c3d4e5f67890
// Knowledge-Factor: 0.93, Eco-impact: 0.90, Risk-of-harm: 0.13

pub mod bands {
    /// Corridor bands for a single bee-relevant metric (dimensionless risk 0–1).
    #[derive(Clone, Debug)]
    pub struct CorridorBands {
        pub var_id: &'static str,
        pub units: &'static str,      // e.g., "dimensionless", "C", "ug/m3"
        pub safe: f64,                // safe band upper bound (<= gold)
        pub gold: f64,                // preferred band upper bound (<= hard)
        pub hard: f64,                // hard limit (must not be exceeded)
        pub weight: f64,              // contribution to residual V
        pub lyap_channel: u32,        // for diagnostics
        pub mandatory: bool,          // true => no corridor, no build
    }

    impl CorridorBands {
        pub fn new(
            var_id: &'static str,
            units: &'static str,
            safe: f64,
            gold: f64,
            hard: f64,
            weight: f64,
            lyap_channel: u32,
            mandatory: bool,
        ) -> Self {
            Self {
                var_id,
                units,
                safe,
                gold,
                hard,
                weight,
                lyap_channel,
                mandatory,
            }
        }
    }
}

pub mod risk {
    use super::bands::CorridorBands;

    /// Single normalized risk coordinate r_x in [0, 1] with uncertainty.
    #[derive(Clone, Debug)]
    pub struct RiskCoord {
        pub var_id: &'static str,
        pub value: f64,   // normalized risk coordinate r_x
        pub sigma: f64,   // uncertainty
        pub bands: CorridorBands,
    }

    /// Aggregate residual V_t and decision flags for a hive step.
    #[derive(Clone, Debug)]
    pub struct Residual {
        pub vt: f64,
        pub coords: Vec<RiskCoord>,
        pub derate: bool,
        pub stop: bool,
    }

    /// Piecewise-linear normalization into r_x using safegoldhard bands.
    pub fn to_risk(measured: f64, bands: &CorridorBands) -> f64 {
        if measured <= bands.safe {
            0.0
        } else if measured >= bands.hard {
            1.0
        } else {
            // Map [safe, hard] -> [0, 1]
            (measured - bands.safe) / (bands.hard - bands.safe)
        }
    }

    /// Compute V_t = sum_j w_j * r_j.
    pub fn compute_residual(coords: &[RiskCoord]) -> f64 {
        coords
            .iter()
            .map(|c| c.bands.weight * c.value)
            .sum()
    }
}

pub mod hive {
    use super::bands::CorridorBands;
    use super::risk::{compute_residual, to_risk, Residual, RiskCoord};

    /// Bee-centered envelope: no human fields; only hive and landscape metrics.
    #[derive(Clone, Debug)]
    pub struct HiveEnvelope {
        pub hive_id: String,
        pub region: String,
        // Core hive metrics (raw physical values).
        pub brood_temp_c: f64,
        pub hive_temp_c: f64,
        pub hive_humidity_pct: f64,
        pub nectar_kg: f64,
        pub pollen_kg: f64,
        pub forager_load_pct: f64,       // fraction of foragers vs. sustainable load
        pub toxin_index_air: f64,        // e.g., normalized pesticide index
        pub toxin_index_wax: f64,
        pub forage_radius_km: f64,
        pub eco_band: EcoBand,
    }

    #[derive(Clone, Debug, Copy, PartialEq, Eq)]
    pub enum EcoBand {
        Safe,
        Warning,
        Critical,
    }

    /// Environmental, landscape-level adjustment; never direct bee actuation.
    #[derive(Clone, Debug)]
    pub struct HiveSystemAdjustment {
        pub hive_id: String,
        pub delta_wildflower_area_m2: f64,
        pub delta_pesticide_use_pct: f64,
        pub delta_irrigation_m3_per_day: f64,
        pub delta_light_pollution_lm: f64,
        pub delta_foraging_corridor_km: f64,
        pub rationale: &'static str,
    }

    /// Corridors required for bee safety (temperature, toxins, forage, etc.).
    #[derive(Clone, Debug)]
    pub struct HiveCorridors {
        pub temp_bands: CorridorBands,
        pub brood_temp_bands: CorridorBands,
        pub humidity_bands: CorridorBands,
        pub toxin_air_bands: CorridorBands,
        pub toxin_wax_bands: CorridorBands,
        pub forage_radius_bands: CorridorBands,
        pub forager_load_bands: CorridorBands,
    }

    /// Policy thresholds summarized as KER for the hive corridor state.
    #[derive(Clone, Debug)]
    pub struct HiveKER {
        pub knowledge_factor: f64,   // 0–1 coverage of critical bee variables
        pub eco_impact: f64,         // 0–1 eco benefit kernel
        pub risk_of_harm: f64,       // 0–1 residual corridor penetration
    }

    /// No-corridor, no-build invariant: all mandatory corridors must be present
    /// and well-formed before any hive can be admitted to the governed stack.
    pub fn corridor_present(c: &HiveCorridors) -> bool {
        let bands = [
            &c.temp_bands,
            &c.brood_temp_bands,
            &c.humidity_bands,
            &c.toxin_air_bands,
            &c.toxin_wax_bands,
            &c.forage_radius_bands,
            &c.forager_load_bands,
        ];

        bands.iter().all(|b| {
            (!b.mandatory) || (b.hard > 0.0 && b.gold <= b.hard && b.safe <= b.gold)
        })
    }

    /// Compute hive residual and band (Safe / Warning / Critical).
    pub fn evaluate_hive(env: &HiveEnvelope, corridors: &HiveCorridors) -> Residual {
        let coords = vec![
            RiskCoord {
                var_id: corridors.temp_bands.var_id,
                sigma: 0.05,
                value: to_risk(env.hive_temp_c, &corridors.temp_bands),
                bands: corridors.temp_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.brood_temp_bands.var_id,
                sigma: 0.05,
                value: to_risk(env.brood_temp_c, &corridors.brood_temp_bands),
                bands: corridors.brood_temp_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.humidity_bands.var_id,
                sigma: 0.05,
                value: to_risk(env.hive_humidity_pct, &corridors.humidity_bands),
                bands: corridors.humidity_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.toxin_air_bands.var_id,
                sigma: 0.10,
                value: to_risk(env.toxin_index_air, &corridors.toxin_air_bands),
                bands: corridors.toxin_air_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.toxin_wax_bands.var_id,
                sigma: 0.10,
                value: to_risk(env.toxin_index_wax, &corridors.toxin_wax_bands),
                bands: corridors.toxin_wax_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.forage_radius_bands.var_id,
                sigma: 0.05,
                value: to_risk(env.forage_radius_km, &corridors.forage_radius_bands),
                bands: corridors.forage_radius_bands.clone(),
            },
            RiskCoord {
                var_id: corridors.forager_load_bands.var_id,
                sigma: 0.05,
                value: to_risk(env.forager_load_pct, &corridors.forager_load_bands),
                bands: corridors.forager_load_bands.clone(),
            },
        ];

        let vt = compute_residual(&coords);

        let mut derate = false;
        let mut stop = false;

        for c in &coords {
            if c.value >= 1.0 {
                // Hard violation: hive in critical corridor → stop.
                stop = true;
            } else if c.value > c.bands.gold {
                // Between gold and hard: derate.
                derate = true;
            }
        }

        Residual { vt, coords, derate, stop }
    }

    /// Runtime invariant: no adjustment may increase bee risk or violate hard limits.
    /// This is the "safestep" analogue for hives.
    pub fn safe_step(prev: &Residual, next: &Residual) -> Residual {
        let mut decision = next.clone();

        // Lyapunov monotonicity outside the safe interior.
        if next.vt > prev.vt && prev.coords.iter().any(|c| c.value > 0.0) {
            decision.derate = true;
            decision.stop = true;
        }

        // Any hard-limit violation in next state forces stop.
        for c in &next.coords {
            if c.value >= 1.0 {
                decision.stop = true;
            }
        }

        decision
    }

    /// Example policy: no action may increase pesticide exposure, raise hive
    /// temperature above safe band, or reduce forage radius below corridor.
    pub fn policy_allows_adjustment(
        envelope_before: &HiveEnvelope,
        envelope_after: &HiveEnvelope,
        corridors: &HiveCorridors,
    ) -> bool {
        // Pesticide / toxin invariants (monotone non-increasing).
        if envelope_after.toxin_index_air > envelope_before.toxin_index_air {
            return false;
        }
        if envelope_after.toxin_index_wax > envelope_before.toxin_index_wax {
            return false;
        }

        // Hive temperature must not move from <= safe band to > safe band.
        let r_before_temp =
            super::risk::to_risk(envelope_before.hive_temp_c, &corridors.temp_bands);
        let r_after_temp =
            super::risk::to_risk(envelope_after.hive_temp_c, &corridors.temp_bands);
        if r_before_temp <= corridors.temp_bands.safe && r_after_temp > corridors.temp_bands.safe {
            return false;
        }

        // Forage radius must not shrink below safe band.
        let r_before_forage = super::risk::to_risk(
            envelope_before.forage_radius_km,
            &corridors.forage_radius_bands,
        );
        let r_after_forage = super::risk::to_risk(
            envelope_after.forage_radius_km,
            &corridors.forage_radius_bands,
        );
        if r_after_forage > r_before_forage {
            return false;
        }

        true
    }
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
