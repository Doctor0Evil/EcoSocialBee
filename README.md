# EcoSocialBee

EcoSocialBee implements Cybernetical-Honeybees patterns in Rust: bee-centered, rights-first environmental intelligence with hard-invariant guards, ALN-style policy shards, and EcoImpact metrics.

Core ideas:

- Bees as primary rights-holders: no direct neural or body-level actuation, only environment and policy changes that improve or maintain hive safety.
- Human biophysical and device-use signals are used only as **eco-proxy** metrics (e.g., device-hour displacement, energy reductions), never as templates for bee pain or tolerance.
- ALN-like shards and Rust types encode invariants so harmful actions become unrepresentable: schema firewalls, guarded adjustments, and monotone EcoImpactScore constraints.

Main components:

- `aln/`: declarative ALN-style shards (EcoSocialBeeImpact2026v1, HiveEcoEnvelope2026v1, NeurorightsForBeesTemplate2026v1).
- `crates/hive_guard`: `HiveEnvelope`, `HiveSystemAdjustment`, and safety guards enforcing “no negative externalization to bees”.
- `crates/ecosocialbee_core`: EcoImpactScore, HeatRiskIndex, and EcoSocialBee per-human impact accounting.
- `crates/beecorridor_router`: a Reality.os–style scheduler that routes human tasks through hive-safe corridors under hard safety constraints.

Build:

```bash
cargo build
Run router example:

```bash
cargo run -p beecorridor_router
Simulate a farm scenario:

```bash
cargo run --example simulate_farm_scenario

***

## ALN shards

### aln/EcoSocialBeeImpact2026v1.aln

```json
{
  "$schema": "https://aln.ecosocialbee.org/schema/v1",
  "shard_id": "qpudatashards/particles/EcoSocialBeeImpact2026v1.aln",
  "title": "EcoSocialBeeImpact2026v1",
  "description": "Per-human eco-social impact on pollinator safety, tracking habitat, sprays, light/noise, and device-hour displacement.",
  "version": "1.0.0",
  "kind": "EcoSocialHumanImpact",
  "invariants": {
    "no_analogy_to_non_humans": true,
    "human_only_flags": ["pain_index", "tolerance_index"],
    "monotone_eco_safety": "EcoImpactScore_new >= EcoImpactScore_old"
  },
  "fields": {
    "actor_human_id": { "type": "string", "role": "pseudonymous_id" },
    "timestamp": { "type": "string", "format": "date-time" },
    "pollinator_habitat_area_m2": { "type": "number", "min": 0 },
    "pollinator_habitat_quality_index": { "type": "number", "min": 0, "max": 1 },
    "reduced_spray_events_count": { "type": "integer", "min": 0 },
    "reduced_spray_volume_l": { "type": "number", "min": 0 },
    "reduced_light_pollution_hours": { "type": "number", "min": 0 },
    "reduced_noise_pollution_hours": { "type": "number", "min": 0 },
    "device_hour_displacement": {
      "type": "number",
      "description": "Hours of high-intensity device use displaced by eco-positive behavior."
    },
    "estimated_kwh_saved": { "type": "number", "min": 0 },
    "estimated_emissions_kg_co2e_avoided": { "type": "number", "min": 0 },
    "eco_impact_score": {
      "type": "number",
      "description": "Composite EcoImpactScoreForHiveCorridor derived from human-side actions."
    },
    "flags": {
      "type": "object",
      "properties": {
        "human_only_pain_index": { "type": "number", "tag": "nontransferable.human_only" },
        "human_only_tolerance_index": { "type": "number", "tag": "nontransferable.human_only" }
      },
      "additionalProperties": false
    }
  }
}
