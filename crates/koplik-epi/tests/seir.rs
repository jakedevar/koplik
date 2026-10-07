use koplik_contracts::v1::{BaselineCoverage, R0, ScenarioInput};
use koplik_epi::{
    Band, Compartments, Step, default_parameters, derive_seed, fingerprint::fingerprint_steps,
    simulate_ensemble, simulate_member,
};
use sha2::{Digest, Sha256};

fn fixture() -> ScenarioInput {
    serde_json::from_str(include_str!(
        "../../../data/fixtures/seir/synthetic-scenario.json"
    ))
    .unwrap()
}

#[test]
fn fixture_is_explicitly_synthetic_and_content_addressed() {
    let raw = include_bytes!("../../../data/fixtures/seir/synthetic-source.json");
    let metadata: serde_json::Value = serde_json::from_slice(raw).unwrap();
    assert_eq!(metadata["synthetic"], true);
    let input = fixture();
    for n in &input.nodes {
        assert_eq!(
            n.provenance.as_slice()[0].sha256.as_str(),
            hex::encode(Sha256::digest(raw))
        );
        assert_eq!(
            n.provenance.as_slice()[0].source_id,
            "synthetic-seir-fixture-not-observations"
        );
    }
    let defaults = default_parameters();
    assert_eq!(defaults.r0, input.parameters.r0);
    assert_eq!(
        defaults.latent_period_days,
        input.parameters.latent_period_days
    );
    assert_eq!(
        defaults.infectious_period_days,
        input.parameters.infectious_period_days
    );
    assert_eq!(
        defaults.mmr_effectiveness_one_dose,
        input.parameters.mmr_effectiveness_one_dose
    );
    assert_eq!(
        defaults.mmr_effectiveness_two_doses,
        input.parameters.mmr_effectiveness_two_doses
    );
}

#[test]
fn conservation_nonnegativity_and_no_within_step_cascade_across_seeded_cases() {
    // Reproducible property grid: vary seed, dt, coverage, population, and coupling.
    for seed in 0..48_u64 {
        let mut input = fixture();
        input.seed = seed;
        input.run_count = 1;
        input.parameters.horizon_days = 12;
        input.parameters.time_step_days = [0.25, 0.3, 1.0, 2.0][(seed % 4) as usize];
        if seed % 2 == 0 {
            input.parameters.gravity = None;
        }
        for (index, n) in input.nodes.iter_mut().enumerate() {
            n.population = 500 + seed * 11 + index as u64 * 31;
        }
        for o in &mut input.coverage_overrides {
            o.coverage_pct = (seed * 2) as f64;
        }
        let result = simulate_member(&input, 0).unwrap();
        for step in &result.steps {
            for (counts, node) in step.nodes.iter().zip(&input.nodes) {
                assert_eq!(
                    counts.susceptible + counts.exposed + counts.infectious + counts.recovered,
                    node.population
                );
                for count in [
                    counts.susceptible,
                    counts.exposed,
                    counts.infectious,
                    counts.recovered,
                ] {
                    assert!(count <= node.population);
                }
            }
        }
        for pair in result.steps.windows(2) {
            assert!(pair[1].day > pair[0].day);
            assert!(pair[1].day - pair[0].day <= input.parameters.time_step_days + 1e-12);
            for (old, new) in pair[0].nodes.iter().zip(&pair[1].nodes) {
                let se = old.susceptible - new.susceptible;
                let ir = new.recovered - old.recovered;
                let ei = old.exposed + se - new.exposed;
                assert!(se <= old.susceptible && ei <= old.exposed && ir <= old.infectious);
            }
        }
        assert_eq!(result.steps.last().unwrap().day, 12.0);
    }
}

#[test]
fn explicit_seeds_reproduce_and_members_are_independent_of_execution_order() {
    let mut input = fixture();
    input.run_count = 3;
    input.parameters.horizon_days = 40;
    let ensemble = simulate_ensemble(&input).unwrap();
    assert_eq!(simulate_ensemble(&input).unwrap(), ensemble);
    for k in (0..input.run_count).rev() {
        assert_eq!(
            simulate_member(&input, k).unwrap(),
            ensemble.members[k as usize]
        );
        assert!((12.0..=18.0).contains(&ensemble.members[k as usize].r0));
    }
    assert_ne!(
        ensemble.members[0].fingerprint,
        ensemble.members[1].fingerprint
    );
    assert_ne!(ensemble.members[0].seed, ensemble.members[1].seed);
    assert_ne!(ensemble.members[0].r0, ensemble.members[1].r0);
    input.seed += 1;
    assert_ne!(
        simulate_member(&input, 0).unwrap().fingerprint,
        ensemble.members[0].fingerprint
    );
    input.parameters.r0 = R0::Fixed { value: 15.0 };
    assert_eq!(simulate_member(&input, 0).unwrap().r0, 15.0);
}

#[test]
fn higher_coverage_reduces_final_ensemble_median_on_synthetic_scenarios() {
    // Statistical regression property, not a claim of pathwise ordering for
    // every finite Monte Carlo sample. Cumulative infections, not I(t): a
    // delayed smaller epidemic may have higher late-day prevalence.
    for base in [0, 1, 1353] {
        let mut input = fixture();
        input.seed = base;
        input.run_count = 101;
        for n in &mut input.nodes {
            n.population = 1000;
        }
        let mut previous = vec![f64::INFINITY; input.nodes.len()];
        for coverage in [20.0, 50.0, 80.0, 95.0] {
            for o in &mut input.coverage_overrides {
                o.coverage_pct = coverage;
            }
            let output = simulate_ensemble(&input).unwrap();
            for (index, row) in output
                .daily
                .iter()
                .filter(|d| d.day == input.parameters.horizon_days)
                .enumerate()
            {
                assert!(
                    row.cumulative_infections.median <= previous[index],
                    "seed={base} coverage={coverage} county={} median={} previous={}",
                    row.geography,
                    row.cumulative_infections.median,
                    previous[index]
                );
                previous[index] = row.cumulative_infections.median;
            }
        }
    }
}

#[test]
fn coupling_transmits_pressure_without_moving_people_and_zero_scale_is_isolation() {
    let mut input = fixture();
    input.run_count = 1;
    input.parameters.gravity = None;
    let isolated = simulate_member(&input, 0).unwrap();
    for step in &isolated.steps {
        assert_eq!(step.nodes[0].exposed, 0);
        assert_eq!(step.nodes[0].infectious, 0);
    }
    input.parameters.gravity = fixture().parameters.gravity;
    input.parameters.gravity.as_mut().unwrap().scale = 0.0;
    assert_eq!(simulate_member(&input, 0).unwrap(), isolated);
    input.parameters.gravity.as_mut().unwrap().scale = 0.1;
    let coupled = simulate_member(&input, 0).unwrap();
    assert!(
        coupled.steps.last().unwrap().nodes[0].susceptible < coupled.steps[0].nodes[0].susceptible
    );
    assert_ne!(coupled.fingerprint, isolated.fingerprint);
}

#[test]
fn daily_bands_cover_integer_days_and_incidence_is_not_vaccine_immunity() {
    let mut input = fixture();
    input.run_count = 1;
    input.parameters.horizon_days = 4;
    input.parameters.time_step_days = 0.3;
    let output = simulate_ensemble(&input).unwrap();
    assert_eq!(output.daily.len(), 5 * input.nodes.len());
    let b = |value: f64| Band {
        median: value,
        lower_50: value,
        upper_50: value,
        lower_90: value,
        upper_90: value,
    };
    for (index, row) in output.daily.iter().enumerate() {
        let node_index = index % input.nodes.len();
        assert_eq!(row.day as usize, index / input.nodes.len());
        assert_eq!(row.geography, input.nodes[node_index].id);
        let step = output.members[0]
            .steps
            .iter()
            .find(|s| s.day == f64::from(row.day))
            .unwrap();
        assert_eq!(
            row.susceptible,
            b(step.nodes[node_index].susceptible as f64)
        );
        let n = &input.nodes[node_index];
        if row.day == 0 {
            assert_eq!(row.new_exposures, b(0.0));
            assert_eq!(
                row.cumulative_infections,
                b(f64::from(n.initial_exposed + n.initial_infectious))
            );
        }
    }
    for node in &input.nodes {
        let rows: Vec<_> = output
            .daily
            .iter()
            .filter(|d| d.geography == node.id)
            .collect();
        let total: f64 = rows.iter().map(|d| d.new_exposures.median).sum();
        assert_eq!(
            rows.last().unwrap().cumulative_infections.median,
            total + f64::from(node.initial_exposed + node.initial_infectious)
        );
    }
}

#[test]
fn reject_invalid_native_inputs_and_never_fill_missing_coverage() {
    let check = |change: fn(&mut ScenarioInput)| {
        let mut input = fixture();
        change(&mut input);
        assert!(simulate_member(&input, 0).is_err());
    };
    check(|s| s.coverage_overrides.clear());
    check(|s| s.nodes.swap(0, 1));
    check(|s| s.nodes[0].id = s.nodes[1].id);
    check(|s| s.nodes.clear());
    check(|s| s.run_count = 0);
    check(|s| s.nodes[0].population = 0);
    check(|s| s.nodes[0].population = (1 << 53) + 1);
    check(|s| s.nodes[0].initial_exposed = u32::MAX);
    check(|s| s.coverage_overrides.push(s.coverage_overrides[0]));
    check(|s| s.coverage_overrides[0].geography = "49001".parse().unwrap());
    check(|s| s.coverage_overrides[0].coverage_pct = f64::NAN);
    check(|s| s.parameters.latent_period_days = 0.0);
    check(|s| s.parameters.time_step_days = f64::MIN_POSITIVE);
    check(|s| {
        s.parameters.r0 = R0::Fixed {
            value: f64::INFINITY,
        }
    });
    check(|s| s.nodes[0].centroid.latitude = 91.0);
    check(|s| s.nodes[0].centroid = s.nodes[1].centroid);
    check(|s| s.parameters.gravity.as_mut().unwrap().origin_exponent = f64::MAX);
    assert!(simulate_member(&fixture(), 1000).is_err());
}

#[test]
fn zero_seed_cases_stay_disease_free_even_at_extreme_rates() {
    let mut input = fixture();
    input.run_count = 1;
    input.parameters.horizon_days = 3;
    input.parameters.r0 = R0::Fixed { value: f64::MAX };
    input.parameters.infectious_period_days = f64::MIN_POSITIVE;
    for n in &mut input.nodes {
        n.initial_exposed = 0;
        n.initial_infectious = 0;
    }
    let result = simulate_member(&input, 0).unwrap();
    for s in &result.steps {
        assert_eq!(s.nodes, result.steps[0].nodes);
    }
}

#[test]
fn fingerprint_has_specified_byte_order_and_includes_initial_state() {
    let first = Compartments {
        susceptible: 1,
        exposed: 2,
        infectious: 3,
        recovered: 4,
    };
    let second = Compartments {
        susceptible: 0,
        exposed: 3,
        infectious: 2,
        recovered: 5,
    };
    let steps = vec![
        Step {
            day: 0.0,
            nodes: vec![first, second],
        },
        Step {
            day: 1.0,
            nodes: vec![second, first],
        },
    ];
    let mut reference = Sha256::new();
    for v in [
        1.0_f64, 2.0, 3.0, 4.0, 0.0, 3.0, 2.0, 5.0, 0.0, 3.0, 2.0, 5.0, 1.0, 2.0, 3.0, 4.0,
    ] {
        reference.update(v.to_le_bytes());
    }
    assert_eq!(fingerprint_steps(&steps), hex::encode(reference.finalize()));
    assert_ne!(fingerprint_steps(&steps), fingerprint_steps(&steps[1..]));
    assert_ne!(derive_seed(0, 1), derive_seed(1, 0));
}

/// Golden fingerprint of the synthetic fixture's member 0. Any change to the
/// seeded path (RNG mapping, sampler, step order, seed derivation) changes this
/// value and must be deliberate; the same value must appear on wasm32.
#[test]
fn fixture_member_zero_fingerprint_is_stable() {
    let member = simulate_member(&fixture(), 0).unwrap();
    assert_eq!(
        member.fingerprint,
        "7a7471b1ed6d648d9a376d591ed21be513b90128d5f5e7c759c184689d5c25fb"
    );
    assert_eq!(member.steps.len(), 181);
}

#[test]
fn reported_baseline_coverage_and_override_give_identical_trajectories() {
    let overridden = fixture();
    let mut reported = fixture();
    for node in &mut reported.nodes {
        let coverage_pct = overridden
            .coverage_overrides
            .iter()
            .find(|o| o.geography == node.id)
            .unwrap()
            .coverage_pct;
        node.baseline_coverage = BaselineCoverage::Reported {
            coverage_pct,
            imputed: false,
            imputation_method: None,
            provenance: node.provenance.clone(),
        };
    }
    reported.coverage_overrides.clear();
    assert_eq!(
        simulate_member(&reported, 0).unwrap(),
        simulate_member(&overridden, 0).unwrap()
    );
    // An imputed value must declare its method; the engine refuses otherwise.
    if let BaselineCoverage::Reported { imputed, .. } = &mut reported.nodes[0].baseline_coverage {
        *imputed = true;
    }
    assert!(simulate_member(&reported, 0).is_err());
}
