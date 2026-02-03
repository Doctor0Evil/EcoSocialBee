use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// HeatRiskIndex for air around hives (0-1, higher is riskier).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HeatRiskIndex(pub f32);

impl HeatRiskIndex {
    pub fn new(temp_c: f32, baseline_c: f32) -> Self {
        let delta = (temp_c - baseline_c).max(0.0);
        let idx = (delta / 15.0).clamp(0.0, 1.0);
        HeatRiskIndex(idx)
    }
}

/// ToxinLoadIndex (0-1) from ppb concentration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ToxinLoadIndex(pub f32);

impl ToxinLoadIndex {
    pub fn from_ppb(ppb: f32, safe_max_ppb: f32) -> Self {
        let ratio = (ppb / safe_max_ppb).clamp(0.0, 2.0);
        let idx = (ratio / 2.0).clamp(0.0, 1.0);
        ToxinLoadIndex(idx)
    }
}

/// HabitatStabilityIndex (0-1) using forage diversity and radius.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HabitatStabilityIndex(pub f32);

impl HabitatStabilityIndex {
    pub fn new(diversity_index: f32, radius_m: f32, min_radius_m: f32) -> Self {
        let diversity = diversity_index.clamp(0.0, 1.0);
        let radius_factor = (radius_m / min_radius_m).clamp(0.0, 2.0) / 2.0;
        let idx = 0.6 * diversity + 0.4 * radius_factor;
        HabitatStabilityIndex(idx.clamp(0.0, 1.0))
    }
}

/// EcoImpactScoreForHiveCorridor (0-100, higher is better).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct EcoImpactScore(pub f32);

impl EcoImpactScore {
    pub fn from_indices(
        heat: HeatRiskIndex,
        toxin: ToxinLoadIndex,
        habitat: HabitatStabilityIndex,
    ) -> Self {
        let risk_component = (heat.0 + toxin.0) / 2.0; // 0-1, higher worse
        let habitat_component = habitat.0; // 0-1, higher better
        let score = (0.7 * habitat_component + 0.3 * (1.0 - risk_component)) * 100.0;
        EcoImpactScore(score.clamp(0.0, 100.0))
    }
}

/// Human eco-proxy metrics, explicitly human-only, not projected to bees.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HumanEcoProxy {
    pub actor_human_id: String,
    pub timestamp: DateTime<Utc>,
    pub device_hour_displacement: f32,
    pub estimated_kwh_saved: f32,
    pub estimated_emissions_kg_co2e_avoided: f32,
    pub pollinator_habitat_area_m2: f32,
    pub pollinator_habitat_quality_index: f32, // 0-1
    pub reduced_spray_events_count: u32,
    pub reduced_spray_volume_l: f32,
    pub reduced_light_pollution_hours: f32,
    pub reduced_noise_pollution_hours: f32,
    pub eco_impact_score: EcoImpactScore,
    // Markers for non-transferable human-only signals.
    pub human_only_pain_index: Option<f32>,
    pub human_only_tolerance_index: Option<f32>,
}

impl HumanEcoProxy {
    /// Construct a proxy with monotone eco impact: caller must ensure new_score >= old_score if chaining.
    pub fn new(
        actor_human_id: impl Into<String>,
        timestamp: DateTime<Utc>,
        device_hour_displacement: f32,
        estimated_kwh_saved: f32,
        estimated_emissions_kg_co2e_avoided: f32,
        pollinator_habitat_area_m2: f32,
        pollinator_habitat_quality_index: f32,
        reduced_spray_events_count: u32,
        reduced_spray_volume_l: f32,
        reduced_light_pollution_hours: f32,
        reduced_noise_pollution_hours: f32,
    ) -> Self {
        let habitat_idx =
            HabitatStabilityIndex((pollinator_habitat_quality_index).clamp(0.0, 1.0));
        // For human-side records, use neutral heat/toxin (0.5) placeholders; actual hive metrics are bee-centered.
        let heat = HeatRiskIndex(0.5);
        let toxin = ToxinLoadIndex(0.5);
        let eco_impact_score = EcoImpactScore::from_indices(heat, toxin, habitat_idx);

        Self {
            actor_human_id: actor_human_id.into(),
            timestamp,
            device_hour_displacement,
            estimated_kwh_saved,
            estimated_emissions_kg_co2e_avoided,
            pollinator_habitat_area_m2,
            pollinator_habitat_quality_index,
            reduced_spray_events_count,
            reduced_spray_volume_l,
            reduced_light_pollution_hours,
            reduced_noise_pollution_hours,
            eco_impact_score,
            human_only_pain_index: None,
            human_only_tolerance_index: None,
        }
    }
}
