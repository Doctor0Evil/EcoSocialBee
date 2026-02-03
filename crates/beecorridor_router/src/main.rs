use anyhow::Result;
use chrono::Utc;
use rand::seq::SliceRandom;
use rand::thread_rng;

use ecosocialbee_core::{EcoImpactScore, HabitatStabilityIndex, HeatRiskIndex, HumanEcoProxy, ToxinLoadIndex};
use hive_guard::{classify_risk, EcoBand, HiveEnvelope, HiveInnerLedger, HiveSystemAdjustment, RiskEnvelope};

#[derive(Debug, Clone)]
pub enum HumanTaskKind {
    FarmingSprayReduction,
    PlantWildflowers,
    AdjustIrrigation,
    DimLights,
    ReduceNoise,
}

#[derive(Debug, Clone)]
pub struct HumanTask {
    pub id: String,
    pub kind: HumanTaskKind,
    pub eco_reward_hint: f32,
}

#[derive(Debug, Clone)]
pub struct RoutedTask {
    pub task: HumanTask,
    pub hive_id: String,
    pub accepted: bool,
    pub reason: String,
}

fn sample_hives() -> Vec<HiveEnvelope> {
    vec![
        HiveEnvelope {
            hive_id: "hive-alpha".into(),
            brood_frames: 8,
            nectar_kg: 12.0,
            pollen_kg: 4.5,
            hive_temperature_c: 34.0,
            forager_load: 0.7,
            ambient_toxin_ppb: 20.0,
            forage_diversity_index: 0.8,
            forage_radius_m: 1500.0,
            eco_band: EcoBand::Safe,
            eco_impact_score_corridor: 75.0,
            safe_temperature_c_min: 32.0,
            safe_temperature_c_max: 36.0,
            safe_toxin_ppb_max: 50.0,
            safe_forage_diversity_index_min: 0.5,
            safe_forage_radius_m_min: 1000.0,
        },
        HiveEnvelope {
            hive_id: "hive-beta".into(),
            brood_frames: 6,
            nectar_kg: 8.0,
            pollen_kg: 3.0,
            hive_temperature_c: 37.5,
            forager_load: 0.9,
            ambient_toxin_ppb: 80.0,
            forage_diversity_index: 0.4,
            forage_radius_m: 800.0,
            eco_band: EcoBand::Warning,
            eco_impact_score_corridor: 45.0,
            safe_temperature_c_min: 32.0,
            safe_temperature_c_max: 36.0,
            safe_toxin_ppb_max: 50.0,
            safe_forage_diversity_index_min: 0.5,
            safe_forage_radius_m_min: 1000.0,
        },
    ]
}

fn task_to_adjustment(task: &HumanTask, hive: &HiveEnvelope) -> HiveSystemAdjustment {
    let now = Utc::now();
    match task.kind {
        HumanTaskKind::FarmingSprayReduction => HiveSystemAdjustment {
            id: format!("adj-{}-{}", hive.hive_id, task.id),
            timestamp: now,
            hive_id: hive.hive_id.clone(),
            delta_pesticide_exposure_ppb: -10.0,
            delta_shade_fraction: 0.0,
            delta_water_availability_index: 0.0,
            delta_forage_radius_m: 0.0,
            delta_forage_diversity_index: 0.05,
            delta_artificial_light_nits: 0.0,
            delta_noise_db: 0.0,
            delta_eco_impact_score_corridor: 5.0,
        },
        HumanTaskKind::PlantWildflowers => HiveSystemAdjustment {
            id: format!("adj-{}-{}", hive.hive_id, task.id),
            timestamp: now,
            hive_id: hive.hive_id.clone(),
            delta_pesticide_exposure_ppb: 0.0,
            delta_shade_fraction: 0.0,
            delta_water_availability_index: 0.1,
            delta_forage_radius_m: 200.0,
            delta_forage_diversity_index: 0.15,
            delta_artificial_light_nits: 0.0,
            delta_noise_db: 0.0,
            delta_eco_impact_score_corridor: 10.0,
        },
        HumanTaskKind::AdjustIrrigation => HiveSystemAdjustment {
            id: format!("adj-{}-{}", hive.hive_id, task.id),
            timestamp: now,
            hive_id: hive.hive_id.clone(),
            delta_pesticide_exposure_ppb: 0.0,
            delta_shade_fraction: 0.0,
            delta_water_availability_index: 0.1,
            delta_forage_radius_m: 0.0,
            delta_forage_diversity_index: 0.02,
            delta_artificial_light_nits: 0.0,
            delta_noise_db: 0.0,
            delta_eco_impact_score_corridor: 2.0,
        },
        HumanTaskKind::DimLights => HiveSystemAdjustment {
            id: format!("adj-{}-{}", hive.hive_id, task.id),
            timestamp: now,
            hive_id: hive.hive_id.clone(),
            delta_pesticide_exposure_ppb: 0.0,
            delta_shade_fraction: 0.0,
            delta_water_availability_index: 0.0,
            delta_forage_radius_m: 0.0,
            delta_forage_diversity_index: 0.0,
            delta_artificial_light_nits: -50.0,
            delta_noise_db: 0.0,
            delta_eco_impact_score_corridor: 1.0,
        },
        HumanTaskKind::ReduceNoise => HiveSystemAdjustment {
            id: format!("adj-{}-{}", hive.hive_id, task.id),
            timestamp: now,
            hive_id: hive.hive_id.clone(),
            delta_pesticide_exposure_ppb: 0.0,
            delta_shade_fraction: 0.0,
            delta_water_availability_index: 0.0,
            delta_forage_radius_m: 0.0,
            delta_forage_diversity_index: 0.0,
            delta_artificial_light_nits: 0.0,
            delta_noise_db: -10.0,
            delta_eco_impact_score_corridor: 1.0,
        },
    }
}

fn route_tasks_through_corridors(
    tasks: &[HumanTask],
    hives: &mut [HiveEnvelope],
) -> Vec<RoutedTask> {
    let mut rng = thread_rng();
    let mut ledger = HiveInnerLedger::new();
    let mut results = Vec::new();

    for task in tasks {
        // Prefer hives in worse risk bands to receive protective actions first.
        let mut candidates = hives.to_vec();
        candidates.sort_by_key(|h| match classify_risk(h) {
            RiskEnvelope::Critical => 0,
            RiskEnvelope::Warning => 1,
            RiskEnvelope::Safe => 2,
        });

        let mut routed = None;

        for hive in candidates.iter_mut() {
            let adj = task_to_adjustment(task, hive);
            match ledger.apply_adjustment(hive.clone(), adj) {
                Ok(new_env) => {
                    // Update original hive entry.
                    if let Some(orig) = hives.iter_mut().find(|h| h.hive_id == new_env.hive_id) {
                        *orig = new_env;
                    }

                    routed = Some(RoutedTask {
                        task: task.clone(),
                        hive_id: hive.hive_id.clone(),
                        accepted: true,
                        reason: "Adjustment satisfies all hive safety invariants".into(),
                    });
                    break;
                }
                Err(err) => {
                    // Try next hive; keep explanation for debug.
                    routed.get_or_insert(RoutedTask {
                        task: task.clone(),
                        hive_id: hive.hive_id.clone(),
                        accepted: false,
                        reason: format!("Rejected by hive ledger: {}", err),
                    });
                }
            }
        }

        if let Some(r) = routed {
            results.push(r);
        } else {
            let hive_id = hives
                .choose(&mut rng)
                .map(|h| h.hive_id.clone())
                .unwrap_or_else(|| "none".into());
            results.push(RoutedTask {
                task: task.clone(),
                hive_id,
                accepted: false,
                reason: "No hive could accept adjustment under safety invariants".into(),
            });
        }
    }

    results
}

fn sample_tasks() -> Vec<HumanTask> {
    vec![
        HumanTask {
            id: "task-1".into(),
            kind: HumanTaskKind::PlantWildflowers,
            eco_reward_hint: 0.9,
        },
        HumanTask {
            id: "task-2".into(),
            kind: HumanTaskKind::FarmingSprayReduction,
            eco_reward_hint: 0.8,
        },
        HumanTask {
            id: "task-3".into(),
            kind: HumanTaskKind::DimLights,
            eco_reward_hint: 0.5,
        },
    ]
}

fn main() -> Result<()> {
    let mut hives = sample_hives();
    let tasks = sample_tasks();

    let routed = route_tasks_through_corridors(&tasks, &mut hives);

    println!("BeeCorridorRouter run:");
    for r in routed {
        println!(
            "- Task {:?} -> hive {} | accepted: {} | reason: {}",
            r.task.kind, r.hive_id, r.accepted, r.reason
        );
    }

    // Example human eco-proxy construction (device-hour displacement, etc.).
    let now = Utc::now();
    let proxy = HumanEcoProxy::new(
        "human-actor-123",
        now,
        2.0,
        1.2,
        0.8,
        50.0,
        0.9,
        3,
        10.0,
        4.0,
        2.5,
    );
    let heat = HeatRiskIndex::new(34.0, 30.0);
    let toxin = ToxinLoadIndex::from_ppb(25.0, 50.0);
    let habitat = HabitatStabilityIndex::new(0.8, 1500.0, 1000.0);
    let eco_score = EcoImpactScore::from_indices(heat, toxin, habitat);

    println!(
        "Human eco proxy EcoImpactScore (corridor): {:.2}, hive corridor score example: {:.2}",
        proxy.eco_impact_score.0, eco_score.0
    );

    Ok(())
}
