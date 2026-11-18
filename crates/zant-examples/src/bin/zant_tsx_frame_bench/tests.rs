//! Unit tests.

use super::*;

#[test]
fn p95_uses_the_clamped_95_percent_index() {
    let mut stats = Stats::new();
    for i in 1..=100 {
        stats.add(f64::from(i));
    }
    assert_eq!(stats.frames(), 100);
    assert_eq!(stats.avg(), 50.5);
    // 0-based index `len * 95 / 100` of the sorted samples: 96 of 100.
    assert_eq!(stats.p95(), 96.0);
    assert_eq!(stats.max(), 100.0);
}

fn samples(values: &[f64]) -> Stats {
    Stats {
        samples: values.to_vec(),
    }
}

#[test]
fn median_handles_unsorted_odd_even_and_single_samples() {
    assert_eq!(samples(&[30.0, 1.0, 3.0]).median(), 3.0);
    assert_eq!(samples(&[50.0, 1.0, 4.0, 2.0]).median(), 3.0);
    assert_eq!(samples(&[7.0]).median(), 7.0);
}

#[test]
fn ratio_takes_frame_medians_before_comparing_paired_rounds() {
    // Ratios 2, 1, 3. Dividing independent medians across rounds gives 30/10.
    let pairs = stats::PairedRounds {
        rounds: vec![
            (samples(&[1.0, 2.0, 100.0]), samples(&[1.0])),
            (samples(&[100.0]), samples(&[100.0])),
            (samples(&[30.0]), samples(&[10.0])),
        ],
    };
    assert_eq!(pairs.ratios().0, 2.0);
}

#[test]
fn noise_includes_the_farthest_round_on_either_side_of_one() {
    for (outlier, expected) in [(0.5, 0.5), (1.75, 0.75)] {
        let pairs = stats::PairedRounds {
            rounds: [0.875, 1.0, 1.0, 1.125, outlier]
                .map(|value| (samples(&[value]), samples(&[1.0])))
                .into(),
        };
        assert_eq!(pairs.ratios(), (1.0, expected));
    }
}

#[test]
fn app_pools_frames_within_each_round() {
    let suite = Suite {
        app: "example",
        scenarios: &["long", "short"],
        tsx: || vec![samples(&[2.0; 3]), samples(&[20.0])],
        manual: || vec![samples(&[2.0; 3]), samples(&[4.0])],
    };
    let rows = measure(&suite, 2, 1.0);
    assert_eq!(rows[0].comparison.ratios().0, 1.0);
    assert_eq!(rows[2].comparison.ratios().0, 5.0);
    for row in rows {
        assert_eq!(row.control.rounds.len(), 2);
        assert_eq!(row.control.ratios(), (1.0, 0.0));
    }
}

#[test]
fn scenario_arguments_accept_apps_scenarios_and_short_names() {
    let counter = &SUITES[0];
    assert!(want(&[], counter.app, counter.scenarios));
    assert!(want(
        &[String::from("counter")],
        counter.app,
        counter.scenarios
    ));
    assert!(want(
        &[String::from("counter-typing")],
        counter.app,
        counter.scenarios
    ));
    assert!(want(
        &[String::from("typing")],
        counter.app,
        counter.scenarios
    ));
    let settings = &SUITES[5];
    assert!(want(
        &[String::from("settings")],
        settings.app,
        settings.scenarios
    ));
    assert!(!want(
        &[String::from("counter")],
        settings.app,
        settings.scenarios
    ));
}

#[test]
fn the_generated_counter_app_is_driven_like_the_control() {
    // The TSX side is the generated root component (not a stub): its keys
    // are laid out and a click runs the generated action.
    let mut tsx = Harness::new(counter_case::app::Counter::default());
    assert!(
        tsx.bounds("inc").is_some(),
        "the generated page is laid out"
    );
    assert!(tsx.click("inc").is_ok(), "the generated action runs");
    // The hand-written control is laid out the same.
    let mut manual = Harness::new(controls::counter::Counter::new());
    assert!(manual.bounds("inc").is_some());
    assert!(manual.click("inc").is_ok());
}

#[test]
fn the_generated_list_page_builds_the_ten_thousand_rows_lazily() {
    // The root `<List>` is the real generated one (`ListBench.tsx`: rows
    // built once by `useMemo`, then the virtual list): early rows exist,
    // the last row was never built (10 000 rows, one screen of nodes).
    let mut tsx = Harness::new(list_case::app::ListBench::default());
    assert!(tsx.bounds("picked").is_some());
    assert!(tsx.bounds("pick-1").is_some(), "the first row is on screen");
    assert!(
        tsx.bounds("pick-10000").is_none(),
        "the virtual list must not build all 10 000 rows"
    );
    assert!(tsx.click("pick-1").is_ok());
}

/// The keyed nodes of a laid-out window (sorted), the structural fingerprint
/// of the UI tree.
fn keyed_nodes(harness: &Harness) -> Vec<String> {
    let mut keys: Vec<String> = harness
        .rt
        .tree
        .nodes
        .iter()
        .filter(|node| node.alive)
        .filter_map(|node| node.key.as_ref().map(|key| key.as_str().to_owned()))
        .collect();
    keys.sort();
    keys
}

#[test]
fn the_generated_tree_has_the_control_keys() {
    // The controls mirror the TSX page's tree; the keyed nodes of the first
    // frame must be the same set (settings: five sections, 40 rows +
    // switches; counter: its nine controls).
    let tsx = Harness::new(settings_case::app::Settings::default());
    let manual = Harness::new(controls::settings::Settings::new());
    let tsx_keys = keyed_nodes(&tsx);
    // changes + 5 sections + 40 rows + 40 switches
    assert!(tsx_keys.len() >= 86, "{tsx_keys:?}");
    assert_eq!(tsx_keys, keyed_nodes(&manual));

    let tsx = Harness::new(counter_case::app::Counter::default());
    let manual = Harness::new(controls::counter::Counter::new());
    let counter_keys = keyed_nodes(&tsx);
    for key in [
        "title", "count", "name", "inc", "reset", "step", "large", "notify", "note",
    ] {
        assert!(counter_keys.iter().any(|name| name == key), "missing {key}");
    }
    assert_eq!(counter_keys, keyed_nodes(&manual));
}

#[test]
fn suites_cover_the_six_acceptance_apps() {
    let apps: Vec<&str> = SUITES.iter().map(|suite| suite.app).collect();
    for app in ["counter", "todos", "list", "local", "markdown", "settings"] {
        assert!(apps.contains(&app), "no suite for {app}");
    }
    // counter 2 + todos 3 + list 2 + local 2 + markdown 1 + settings 1
    assert_eq!(
        SUITES
            .iter()
            .map(|suite| suite.scenarios.len())
            .sum::<usize>(),
        11
    );
}
