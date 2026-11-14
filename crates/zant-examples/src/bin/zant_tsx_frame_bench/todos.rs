//! `todos.tsx` vs the hand-written todo app: 1000 items typed + Enter, 500
//! checkbox toggles, 60 filter switches — one harness per side.
#![forbid(unsafe_code)]

use zant::core::events::{Event, Key, KeyEvent, Modifiers};
use zant::ui::App;

use super::controls::todos::Todos as ManualTodos;
use super::{Harness, Stats, todos_case};

/// The TSX root component (`src/App.tsx` of `tsx/corpus/todos.tsx`).
type TsxTodos = todos_case::app::Todos;

const ITEMS: usize = 1000;
const TOGGLES: usize = 500;
const FILTERS: usize = 60;

/// Add / toggle / filter through one runtime.
fn run(app: impl App) -> Vec<Stats> {
    let mut harness = Harness::new(app);

    // add: type a title into the input and press Enter, `items` times
    harness
        .click("new")
        .expect("the new-todo input is on screen");
    let mut add = Stats::new();
    for index in 0..ITEMS {
        let _ = harness
            .rt
            .dispatch(Event::TextInput(format!("Task {index}")));
        let _ = harness.rt.dispatch(Event::KeyDown(KeyEvent {
            key: Key::Enter,
            modifiers: Modifiers::default(),
            repeat: false,
        }));
        add.add(harness.frame());
    }

    // toggle: click the first 20 checkboxes in turn (they are on screen)
    let mut toggle = Stats::new();
    for index in 0..TOGGLES {
        let key = format!("check-{}", index % 20 + 1);
        toggle.add(harness.click(&key).expect("todos on screen"));
    }

    // filter: All / Active / Done
    let mut filter = Stats::new();
    let segments = ["filter-1-hit", "filter-2-hit", "filter-0-hit"];
    for index in 0..FILTERS {
        filter.add(
            harness
                .click(segments[index % segments.len()])
                .expect("the filter is on screen"),
        );
    }
    vec![add, toggle, filter]
}

/// The TSX side: the generated root component of `todos.tsx`.
pub fn tsx() -> Vec<Stats> {
    run(TsxTodos::default())
}

/// The hand-written control.
pub fn manual() -> Vec<Stats> {
    run(ManualTodos::new())
}
