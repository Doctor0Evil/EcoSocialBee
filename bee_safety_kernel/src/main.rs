use bee_safety_kernel::*;

fn main() {
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
    let kernel = BeeSafetyKernel::new(envs, params).expect("kernel init");

    let node = NodeState {
        node_id: "CYB-AIR-CANOPY-01".to_string(),
        duty_cycle: 0.6,
        mass_removed_kg: 1.5e-6,
        nano_karma_bytes: 4.0e9,
        power_cost: 0.4,
        cybo_weight: 0.8,
        bee_ctx: BeeContext {
            bee_sensitivity: 2.5,
            in_hive_exclusion: false,
            dz_to_bee_band: 10.0,
        },
        predicted_levels: vec![
            PredictedLevels { kind: CorridorKind::EMF, level: 0.4 },
            PredictedLevels { kind: CorridorKind::Thermal, level: 1.2 },
            PredictedLevels { kind: CorridorKind::Acoustic, level: 45.0 },
            PredictedLevels { kind: CorridorKind::Chemical, level: 0.03 },
        ],
    };

    let decision = kernel.evaluate_node(node).expect("decision");
    println!("node_id,safe_duty_cycle,permitted,phi_penalty,eco_impact_bee");
    println!("{},{:.3},{},{:.3},{:.3}",
        decision.node_id,
        decision.safe_duty_cycle,
        decision.permitted,
        decision.phi_penalty,
        decision.eco_impact_bee
    );
}
