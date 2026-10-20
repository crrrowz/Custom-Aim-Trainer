use aim_scenarios::battery::derive_scenario_seed;
use aim_scenarios::generator::*;
use aim_scenarios::schema::*;

#[test]
fn test_golden_trials_forms_0_to_4() {
    let sc: Scenario = ron::from_str(include_str!(
        "../../../content/scenarios/assess.v1.flick_static.ron"
    ))
    .unwrap();

    for form in 0..5 {
        let seed = derive_scenario_seed(form, &sc.id);
        let trials = generate_trials(&sc, seed);
        assert_eq!(trials.len(), 96, "Form {form} must have 96 trials");

        // Verify all 48 design cells appear exactly twice
        if let TargetLayout::Fitts { grid, .. } = &sc.layout {
            for &dist in &grid.distances_deg {
                for &radius in &grid.radius_sizes_deg {
                    for octant in 0..8 {
                        let matching = trials
                            .iter()
                            .filter(|t| {
                                (t.distance_deg - dist).abs() < 1e-9
                                    && (t.radius_deg - radius).abs() < 1e-9
                                    && t.octant == octant
                            })
                            .count();
                        assert_eq!(matching, 2, "Form {form} cell imbalance");
                    }
                }
            }
        }
    }
}
