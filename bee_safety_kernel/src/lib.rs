use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Corridor kinds enforced by the Bee Safety Kernel.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum CorridorKind {
    EMF,
    Thermal,
    Acoustic,
    Chemical,
}

/// Envelope parameters for one corridor at a given point in space-time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CorridorEnvelope {
    pub kind: CorridorKind,
    /// Upper bound L_max (e.g., V/m, °C, dB, mg/m^3).
    pub l_max: f64,
    /// Lower bound L_min (optional; often 0.0 for safety).
    pub l_min: f64,
}

/// Bee-relevant spatial context for a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeeContext {
    /// Bee-sensitivity scalar H(x_i) in [0, +inf), >= 1.0 near hives.
    pub bee_sensitivity: f64,
    /// True if node is inside a strict no-emission hive bubble.
    pub in_hive_exclusion: bool,
    /// Vertical distance to dominant bee flight band (m).
    pub dz_to_bee_band: f64,
}

/// Local predicted levels for each corridor produced by hardware or a local model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PredictedLevels {
    pub kind: CorridorKind,
    /// Predicted level L_k at this node (already aggregated over frequency band if needed).
    pub level: f64,
}

/// Node state exposed to the Bee Safety Kernel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeState {
    pub node_id: String,
    /// Proposed actuation intensity u in [0,1] (fan duty, EMF duty, etc.).
    pub duty_cycle: f64,
    /// Pollutant mass removed in this interval [kg].
    pub mass_removed_kg: f64,
    /// Hazard-weighted NanoKarmaBytes for this interval.
    pub nano_karma_bytes: f64,
    /// Normalized power cost in [0,1].
    pub power_cost: f64,
    /// Existing Cybo-Air geospatial weight w_i^{cyb}.
    pub cybo_weight: f64,
    /// Bee context at node location.
    pub bee_ctx: BeeContext,
    /// Predicted local levels for all corridors at the proposed duty_cycle.
    pub predicted_levels: Vec<PredictedLevels>,
}

/// Scalar parameters governing corridor enforcement and duty-cycle update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelParams {
    pub eta_mass: f64,
    pub eta_karma: f64,
    pub eta_geo: f64,
    pub eta_power: f64,
    pub eta_bee: f64,
    pub m_ref: f64,
    pub k_ref: f64,
    pub phi_ref: f64,
    pub alpha_z: f64,
    pub beta_s: f64,
}

/// Result of a kernel evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelDecision {
    pub node_id: String,
    /// Updated, bee-safe duty cycle in [0,1].
    pub safe_duty_cycle: f64,
    /// True if emission is permitted under current envelopes.
    pub permitted: bool,
    /// Sum of corridor penalties Φ_i(u).
    pub phi_penalty: f64,
    /// Bee-weighted eco-impact score S_i^{bee} in [0,1].
    pub eco_impact_bee: f64,
}

/// Errors raised by the kernel.
#[derive(Debug, Error)]
pub enum KernelError {
    #[error("No corridor envelopes provided")]
    NoEnvelopes,
    #[error("Duty cycle must be in [0,1], got {0}")]
    InvalidDutyCycle(f64),
}

/// Core Bee Safety Kernel object.
pub struct BeeSafetyKernel {
    /// Corridor envelopes indexed by CorridorKind.
    pub envelopes: Vec<CorridorEnvelope>,
    pub params: KernelParams,
}

impl BeeSafetyKernel {
    pub fn new(envelopes: Vec<CorridorEnvelope>, params: KernelParams) -> Result<Self, KernelError> {
        if envelopes.is_empty() {
            return Err(KernelError::NoEnvelopes);
        }
        Ok(Self { envelopes, params })
    }

    fn envelope_for(&self, kind: CorridorKind) -> Option<&CorridorEnvelope> {
        self.envelopes.iter().find(|e| e.kind == kind)
    }

    /// Compute corridor penalty Φ_i(u) as in Eq. (5) using local predicted levels.
    fn compute_phi(&self, node: &NodeState) -> f64 {
        let mut phi = 0.0;
        for pl in &node.predicted_levels {
            if let Some(env) = self.envelope_for(pl.kind) {
                let over = (pl.level - env.l_max).max(0.0);
                let under = (env.l_min - pl.level).max(0.0);
                phi += over * over + under * under;
            }
        }
        // Weight by bee sensitivity; hive exclusion makes any non-zero penalty very large.
        let bee_factor = if node.bee_ctx.in_hive_exclusion {
            1e6
        } else {
            node.bee_ctx.bee_sensitivity.max(1.0)
        };
        phi * bee_factor
    }

    /// Compute bee-refined geospatial weight w_i^{bee} from Eq. (7).
    fn compute_bee_weight(&self, node: &NodeState) -> f64 {
        let p = &self.params;
        let base = node.cybo_weight.max(0.0);
        let dz = node.bee_ctx.dz_to_bee_band.abs();
        let band_factor = (-p.alpha_z * dz).exp();
        let exclusion_factor = if node.bee_ctx.in_hive_exclusion { 0.0 } else { 1.0 };
        base * band_factor * exclusion_factor
    }

    /// Compute bee-normalized eco-impact S_i^{bee} from Eq. (8).
    fn compute_eco_impact_bee(&self, node: &NodeState, phi: f64) -> f64 {
        let p = &self.params;
        let s_mass = (node.nano_karma_bytes / (self.params.k_ref + 1e-9)).min(2.0);
        let s_pollutant = 1.0 - (s_mass - 1.0).abs(); // crude compression to [0,1]
        let s_bee = 1.0 - (phi / (p.phi_ref + 1e-9)).min(1.0);
        let s = p.beta_s * s_pollutant + (1.0 - p.beta_s) * s_bee;
        s.clamp(0.0, 1.0)
    }

    /// Evaluate one node and return a bee-safe duty cycle and decision.
    pub fn evaluate_node(&self, mut node: NodeState) -> Result<KernelDecision, KernelError> {
        if !(0.0..=1.0).contains(&node.duty_cycle) {
            return Err(KernelError::InvalidDutyCycle(node.duty_cycle));
        }

        let p = &self.params;

        // Corridor penalty Φ_i(u)
        let phi = self.compute_phi(&node);

        // Bee-refined geospatial weight
        let w_bee = self.compute_bee_weight(&node);

        // Mass and Karma normalization
        let m_norm = node.mass_removed_kg / (p.m_ref + 1e-12);
        let k_norm = node.nano_karma_bytes / (p.k_ref + 1e-12);

        // Duty-cycle update (Eq. 6)
        let mut u = node.duty_cycle
            + p.eta_mass * m_norm
            + p.eta_karma * k_norm
            + p.eta_geo * w_bee
            - p.eta_power * node.power_cost
            - p.eta_bee * (phi / (p.phi_ref + 1e-12));

        // Projection Π_[0,1]
        if u < 0.0 {
            u = 0.0;
        } else if u > 1.0 {
            u = 1.0;
        }

        let eco_impact_bee = self.compute_eco_impact_bee(&node, phi);
        let permitted = phi == 0.0 && !node.bee_ctx.in_hive_exclusion;

        Ok(KernelDecision {
            node_id: node.node_id,
            safe_duty_cycle: u,
            permitted,
            phi_penalty: phi,
            eco_impact_bee,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_kernel_allows_safe_node() {
        let envs = vec![
            CorridorEnvelope { kind: CorridorKind::EMF, l_min: 0.0, l_max: 1.0 },
            CorridorEnvelope { kind: CorridorKind::Thermal, l_min: 0.0, l_max: 2.0 },
            CorridorEnvelope { kind: CorridorKind::Acoustic, l_min: 0.0, l_max: 60.0 },
            CorridorEnvelope { kind: CorridorKind::Chemical, l_min: 0.0, l_max: 0.1 },
        ];
        let params = KernelParams {
            eta_mass: 0.05,
            eta_karma: 0.02,
            eta_geo: 0.1,
            eta_power: 0.05,
            eta_bee: 0.2,
            m_ref: 1e-6,
            k_ref: 1e9,
            phi_ref: 1.0,
            alpha_z: 0.05,
            beta_s: 0.7,
        };
        let kernel = BeeSafetyKernel::new(envs, params).unwrap();

        let node = NodeState {
            node_id: "CYB-AIR-CANOPY-01".to_string(),
            duty_cycle: 0.5,
            mass_removed_kg: 2e-6,
            nano_karma_bytes: 5e9,
            power_cost: 0.3,
            cybo_weight: 0.8,
            bee_ctx: BeeContext {
                bee_sensitivity: 2.0,
                in_hive_exclusion: false,
                dz_to_bee_band: 15.0,
            },
            predicted_levels: vec![
                PredictedLevels { kind: CorridorKind::EMF, level: 0.3 },
                PredictedLevels { kind: CorridorKind::Thermal, level: 1.0 },
                PredictedLevels { kind: CorridorKind::Acoustic, level: 40.0 },
                PredictedLevels { kind: CorridorKind::Chemical, level: 0.02 },
            ],
        };

        let decision = kernel.evaluate_node(node).unwrap();
        assert!(decision.permitted);
        assert!(decision.safe_duty_cycle >= 0.0 && decision.safe_duty_cycle <= 1.0);
    }
}
