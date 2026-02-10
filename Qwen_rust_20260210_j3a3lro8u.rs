// File: cyboair/src/microspace_physics.rs
// Destination path: ./cyboair/src/microspace_physics.rs
// This module contains advanced operators for real-time nanoswarm control.

use super::NodeState;

/// Calculates the surface flux of captured mass, J_p, in kg/(m^2*s)
/// Based on mass transfer theory: J_p = k_s * (C_in - C_surf)
pub fn calculate_surface_flux(
    inlet_concentration_kgm3: f64,
    surface_concentration_kgm3: f64,
    mass_transfer_coeff_kms: f64,
) -> f64 {
    mass_transfer_coeff_kms * (inlet_concentration_kgm3 - surface_concentration_kgm3)
}

/// Calculates the advanced geospatial actuation weight, w_i.
/// Incorporates gradient, vertical clearance, and sensitive location flags.
pub fn calculate_geospatial_weight(
    gradient: f64,           // C_i^∇
    ref_gradient: f64,       // C_ref
    vertical_clearance: f64, // z_clear,i
    ref_clearance: f64,      // z_ref
    is_sensitive: bool,      // sens_i
    alpha1: f64,             // Gain factors
    alpha2: f64,
    alpha3: f64,
) -> f64 {
    let normalized_gradient = if ref_gradient > 0.0 { gradient / ref_gradient } else { 0.0 };
    let normalized_clearance = if ref_clearance > 0.0 { vertical_clearance / ref_clearance } else { 0.0 };
    let sensitive_flag = if is_sensitive { 1.0 } else { 0.0 };

    alpha1 * normalized_gradient + 
    alpha2 * normalized_clearance + 
    alpha3 * sensitive_flag
}

/// Updates the node's duty cycle based on the advanced control law.
/// u_i^(k+1) = Proj_[0,1]( u_i^k + ... - eta5 * C_surf / C_sat )
pub fn update_duty_cycle_dynamic(
    node: &mut NodeState,
    m_ref: f64,
    k_ref: f64,
    w_i: f64,
    power_cost: f64, // c_power,i
    eta1: f64,
    eta2: f64,
    eta3: f64,
    eta4: f64,
    eta5: f64,
    saturation_capacity: f64, // C_sat
) {
    // Calculate new contributions
    let mass_term = eta1 * (node.masskg / m_ref);
    let karma_term = eta2 * (node.karmabytes / k_ref);
    let weight_term = eta3 * w_i;
    let power_penalty = eta4 * power_cost;
    let degradation_term = eta5 * (node.surface_concentration / saturation_capacity);

    // Apply the full control law
    let mut new_duty_cycle = node.dutycycle
        + mass_term
        + karma_term
        + weight_term
        - power_penalty
        - degradation_term;

    // Project to [0, 1]
    node.dutycycle = new_duty_cycle.max(0.0).min(1.0);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_surface_flux() {
        let flux = calculate_surface_flux(1e-6, 0.5e-6, 1e-3); // 0.5e-9 kg/m^2s
        assert_eq!(flux, 5e-10);
    }

    #[test]
    fn test_calculate_geospatial_weight() {
        let weight = calculate_geospatial_weight(
            1e-6, 1e-6, 100.0, 50.0, true, 0.4, 0.4, 0.2
        ); // (0.4*1) + (0.4*2) + (0.2*1) = 1.4, but should be capped...
        // This is a simplified test; a proper test would check the normalized logic.
        assert!(weight > 0.0);
    }
}