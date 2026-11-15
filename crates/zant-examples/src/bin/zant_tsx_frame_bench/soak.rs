//! Memory soak: one child process and one long-lived runtime per scenario/side.
//! A complete untimed cycle warms allocator/layout caches. Checkpoints are at
//! equivalent UI states; timings at middle/end cover the preceding half.
//! Stats buffers are reserved before warm-up and never grow during the soak.
#![forbid(unsafe_code)]

use std::process::Command;

use zant::Str;
use zant::core::events::{Event, Key, KeyEvent, Modifiers, Scroll};
use zant::core::node::NodeData;
use zant::platform::memory::resident_bytes;
use zant::ui::{ActionCx, App};

use super::{Harness, Stats, controls, counter_case, list_case, local_soak_case, todos_case};

const CASES: [(&str, usize); 4] = [
    ("todos-churn", 50),
    ("local-churn", 500),
    ("list-churn", 200),
    ("counter-churn", 100), // 100 clicks and one bounded edit per cycle
];

pub fn requested(args: &[String]) -> bool {
    std::env::var("ZANT_BENCH_SOAK").is_ok_and(|value| value == "1")
        || args.iter().any(|arg| arg == "soak" || arg == "--soak-case")
}

pub fn run(args: &[String]) {
    if args.first().is_some_and(|arg| arg == "--soak-case") {
        assert_eq!(args.len(), 3, "internal soak arguments: scenario side");
        let name = &args[1];
        let cycles = CASES
            .iter()
            .find(|case| case.0 == name)
            .expect("soak case")
            .1;
        let leak = match (name.as_str(), args[2].as_str()) {
            ("todos-churn", "tsx") => {
                measure(todos_case::app::Todos::default(), name, "tsx", cycles)
            }
            ("todos-churn", "manual") => {
                measure(controls::todos::Todos::new(), name, "manual", cycles)
            }
            ("local-churn", "tsx") => measure(
                local_soak_case::app::LocalBench::default(),
                name,
                "tsx",
                cycles,
            ),
            ("local-churn", "manual") => {
                measure(controls::local::LocalBench::soak(), name, "manual", cycles)
            }
            ("list-churn", "tsx") => {
                measure(list_case::app::ListBench::default(), name, "tsx", cycles)
            }
            ("list-churn", "manual") => {
                let rows = (1..=10_000)
                    .map(|id| controls::list::Row {
                        id,
                        title: Str::Static("Row title"),
                    })
                    .collect();
                measure(controls::list::ListBench::new(rows), name, "manual", cycles)
            }
            ("counter-churn", "tsx") => {
                measure(counter_case::app::Counter::default(), name, "tsx", cycles)
            }
            ("counter-churn", "manual") => {
                measure(controls::counter::Counter::new(), name, "manual", cycles)
            }
            _ => panic!("unknown soak case or side"),
        };
        if leak {
            std::process::exit(1);
        }
        return;
    }

    let filters: Vec<_> = args.iter().filter(|arg| arg.as_str() != "soak").collect();
    let selected: Vec<_> = CASES
        .iter()
        .filter(|(name, _)| {
            filters.is_empty()
                || filters
                    .iter()
                    .any(|arg| arg.as_str() == *name || name.starts_with(&format!("{arg}-")))
        })
        .collect();
    if selected.is_empty() {
        eprintln!("no soak scenario matches {args:?}");
        std::process::exit(2);
    }
    println!(
        "zant-tsx-frame-bench soak: RSS bytes; frame times ms; one process/runtime per side; one warm-up cycle"
    );
    let mut leaks = 0;
    let mut errors = 0;
    for (name, _) in selected {
        for side in ["tsx", "manual"] {
            let status = Command::new(std::env::current_exe().expect("benchmark executable"))
                .args(["--soak-case", name, side])
                .status()
                .expect("launch isolated soak");
            match status.code() {
                Some(0) => {}
                Some(1) => leaks += 1,
                _ => errors += 1,
            }
        }
    }
    println!("soak: leaks={leaks} errors={errors}");
    if errors > 0 {
        std::process::exit(2);
    }
    if leaks > 0 {
        std::process::exit(1);
    }
}

#[derive(Clone, Copy)]
struct Snapshot {
    rss: u64,
    nodes: usize,
    slots: usize,
    text_layouts: i64,
}

impl Snapshot {
    fn take(h: &Harness) -> Self {
        Self {
            rss: resident_bytes().expect("sample process RSS"),
            nodes: h.rt.tree.live_count(),
            slots: h.rt.local_states.borrow().count(),
            text_layouts: zant::renderer::text::live_layouts(),
        }
    }

    fn report(self, name: &str, side: &str, point: &str, cycles: usize, stats: &Stats) {
        println!(
            "soak-sample {name} side={side} point={point} cycles={cycles} rss_bytes={} nodes={} slots={} text_layouts={} frames={} avg={:.4} p95={:.4} max={:.4}",
            self.rss,
            self.nodes,
            self.slots,
            self.text_layouts,
            stats.frames(),
            stats.avg(),
            stats.p95(),
            stats.max()
        );
    }
}

fn leaks(start: Snapshot, middle: Snapshot, end: Snapshot) -> bool {
    end.nodes > start.nodes
        || end.slots > start.slots
        || end.text_layouts > start.text_layouts
        || u128::from(end.rss) * 100 > u128::from(middle.rss) * 110
}

fn rss_growth(middle: Snapshot, end: Snapshot) -> f64 {
    (end.rss as f64 / middle.rss as f64 - 1.0) * 100.0
}

fn measure(app: impl App, name: &str, side: &str, cycles: usize) -> bool {
    let mut h = Harness::new(app);
    let mut stats = Stats {
        samples: Vec::with_capacity(64_000),
    };
    let mut next_todo = 1;
    cycle(&mut h, name, &mut next_todo, &mut stats);
    let start = Snapshot::take(&h);
    start.report(name, side, "start", 0, &stats);
    stats.samples.clear();
    let mut middle = start;
    // Aggregate without a second growing buffer in the measured process.
    let mut total_ms = 0.0;
    let mut total_frames = 0;
    for index in 1..=cycles {
        cycle(&mut h, name, &mut next_todo, &mut stats);
        if index == cycles / 2 {
            middle = Snapshot::take(&h);
            middle.report(name, side, "middle", index, &stats);
            total_ms += stats.samples.iter().sum::<f64>();
            total_frames += stats.frames();
            stats.samples.clear();
        }
    }
    let end = Snapshot::take(&h);
    end.report(name, side, "end", cycles, &stats);
    total_ms += stats.samples.iter().sum::<f64>();
    total_frames += stats.frames();
    let leak = leaks(start, middle, end);
    println!(
        "soak-verdict {name} side={side} verdict={} node_delta={} slot_delta={} text_layout_delta={} rss_growth_pct={:.2} frames={total_frames} avg={:.4}",
        if leak { "leak" } else { "stable" },
        end.nodes as i64 - start.nodes as i64,
        end.slots as i64 - start.slots as i64,
        end.text_layouts - start.text_layouts,
        rss_growth(middle, end),
        total_ms / total_frames as f64
    );
    leak
}

fn cycle(h: &mut Harness, name: &str, next_todo: &mut u32, stats: &mut Stats) {
    match name {
        "todos-churn" => todos(h, next_todo, stats),
        "local-churn" => {
            stats.add(h.click("add").expect("mount child"));
            assert!(
                h.rt.local_states.borrow().count() > 0,
                "child has local state"
            );
            // The changing id ensures that every mount creates a fresh slot.
            let key =
                h.rt.tree
                    .nodes
                    .iter()
                    .filter(|n| n.alive)
                    .filter_map(|n| n.key.as_deref())
                    .find(|key| key.starts_with("inc-"))
                    .expect("mounted counter")
                    .to_owned();
            stats.add(h.click(&key).expect("update child state"));
            stats.add(h.click("clear").expect("unmount child"));
            assert!(h.bounds(&key).is_none(), "child was actually unmounted");
        }
        "list-churn" => {
            // Visit distinct rows across the entire 10k-row list, then return
            // to the same top viewport (including its overdraw) each cycle.
            for dy in [-16_000.0, 16_000.0] {
                for _ in 0..25 {
                    h.rt.dispatch(Event::Scroll(Scroll {
                        x: 200.0,
                        y: 400.0,
                        dx: 0.0,
                        dy,
                        modifiers: Modifiers::default(),
                    }));
                    stats.add(h.frame());
                }
                let key = if dy < 0.0 { "pick-10000" } else { "pick-1" };
                assert!(h.bounds(key).is_some(), "scroll reached {key}");
            }
        }
        "counter-churn" => {
            for _ in 0..100 {
                stats.add(h.click("inc").expect("increment counter"));
            }
            stats.add(h.click("name").expect("focus name input"));
            // Clear with the real input path so text length stays bounded.
            // Use the platform shortcut without extra modifier keys.
            h.rt.dispatch(Event::KeyDown(KeyEvent {
                key: Key::Character('a'),
                modifiers: Modifiers {
                    control: zant::core::keys::pick(false, true),
                    meta: zant::core::keys::pick(true, false),
                    ..Modifiers::default()
                },
                repeat: false,
            }));
            let value = if (*next_todo).is_multiple_of(2) {
                "Soak name A"
            } else {
                "Soak name B"
            };
            *next_todo += 1;
            stats.add(h.type_text(value));
            let node = h.rt.tree.id_of_key("name").expect("name input");
            assert!(
                matches!(&h.rt.tree.get(node).data, NodeData::Input(data) if data.text == value),
                "typing replaces the selected text"
            );
        }
        _ => unreachable!(),
    }
}

fn todos(h: &mut Harness, next_id: &mut u32, stats: &mut Stats) {
    stats.add(h.click("new").expect("focus todo input"));
    for _ in 0..200 {
        h.rt.dispatch(Event::TextInput(format!("Task {next_id}")));
        h.rt.dispatch(Event::KeyDown(KeyEvent {
            key: Key::Enter,
            modifiers: Modifiers::default(),
            repeat: false,
        }));
        stats.add(h.frame());
        *next_id += 1;
    }
    // Ordinary column rows can be offscreen. Invoke their real retained
    // callbacks, one at a time with a render in between (no stale snapshots).
    for id in *next_id - 200..*next_id {
        let key = format!("check-{id}");
        let node = h.rt.tree.id_of_key(&key).expect("new todo exists");
        let NodeData::Toggle(data) = &h.rt.tree.get(node).data else {
            panic!("todo checkbox")
        };
        assert!(!data.checked);
        let action = data.action.clone().expect("toggle callback");
        {
            let mut app = h.rt.app.borrow_mut();
            action.call(&mut ActionCx::new(&mut **app), true);
        }
        drop(action);
        h.invalidate();
        stats.add(h.frame());
        let node = h.rt.tree.id_of_key(&key).expect("toggled todo exists");
        assert!(matches!(&h.rt.tree.get(node).data, NodeData::Toggle(data) if data.checked));
    }
    for key in ["filter-1-hit", "filter-2-hit", "filter-0-hit"] {
        stats.add(h.click(key).expect("filter todos"));
    }
    for id in *next_id - 200..*next_id {
        let key = format!("remove-{id}");
        let node = h.rt.tree.id_of_key(&key).expect("todo to remove exists");
        let NodeData::Button(data) = &h.rt.tree.get(node).data else {
            panic!("remove button")
        };
        let action = data.action.clone().expect("remove callback");
        {
            let mut app = h.rt.app.borrow_mut();
            action.call(&mut ActionCx::new(&mut **app));
        }
        drop(action);
        h.invalidate();
        stats.add(h.frame());
        assert!(h.bounds(&key).is_none(), "todo removed");
    }
    assert!(
        h.bounds("rows").is_none() && h.bounds("empty").is_some(),
        "all todos deleted"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_uses_steady_state_counts_and_strict_ten_percent_rss() {
        let start = Snapshot {
            rss: 50,
            nodes: 10,
            slots: 2,
            text_layouts: 5,
        };
        let middle = Snapshot { rss: 100, ..start };
        assert!(!leaks(start, middle, Snapshot { rss: 110, ..start }));
        assert!(leaks(start, middle, Snapshot { rss: 111, ..start }));
        assert!(leaks(
            start,
            middle,
            Snapshot {
                nodes: 11,
                ..middle
            }
        ));
        assert!(leaks(start, middle, Snapshot { slots: 3, ..middle }));
        assert!(leaks(
            start,
            middle,
            Snapshot {
                text_layouts: 6,
                ..middle
            }
        ));
        assert!(!leaks(
            start,
            middle,
            Snapshot {
                rss: 90,
                nodes: 9,
                slots: 1,
                text_layouts: 4,
            }
        ));
    }
}
