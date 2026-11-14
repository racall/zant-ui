//! `local_bench.tsx` written by hand in Rust (`examples/tsx/local_manual.zig`
//! ported): the control group of the `local-mount` / `local-click` scenarios.
//!
//! Each `<Counter>` child owns a `ui::local_state`, exactly like the TSX page's
//! per-instance `useState`. Zig passed the child's arena struct as the action
//! context; Rust actions run against the app root, so the closure captures the
//! local-state `Rc` and ignores the root state.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use zant::Str;
use zant::layout::flex;
use zant::ui::{self, App, BoxOptions, ButtonOptions, TextOptions, ViewOptions};

fn counter_row(id: u32, odd: bool) -> ui::Node {
    let theme = ui::current_theme();
    let state: Rc<RefCell<u32>> = ui::local_state(ui::child_id(0, u64::from(id)), || 0u32);
    let count = *state.borrow();
    ui::row(BoxOptions {
        key: Some(Str::copy(&format!("{id}"))),
        gap: 8.0,
        align_items: flex::Align::Center,
        height: Some(36.0),
        background: Some(if odd {
            theme.color_fill_quaternary
        } else {
            theme.color_bg_container
        }),
        children: vec![
            ui::text(format!("Row {id}: {count}"), TextOptions::default()),
            ui::button(ButtonOptions {
                key: Some(Str::copy(&format!("inc-{id}"))),
                text: Str::Static("+1"),
                on_click: Some(ui::action(move |_: &mut LocalBench| {
                    *state.borrow_mut() += 1;
                })),
                ..ButtonOptions::default()
            }),
        ],
        ..BoxOptions::default()
    })
}

/// Zig `local_manual.LocalBench`.
pub struct LocalBench {
    ids: Vec<u32>,
    next: u32,
}

impl LocalBench {
    pub fn new() -> LocalBench {
        LocalBench {
            ids: Vec::new(),
            next: 1,
        }
    }

    fn add(&mut self) {
        self.ids.push(self.next);
        self.next += 1;
    }
}

impl App for LocalBench {
    /// Zig `LocalBench.render`.
    fn render(&mut self) -> ui::Node {
        let mut children = Vec::with_capacity(self.ids.len() + 1);
        children.push(ui::button(ButtonOptions {
            key: Some(Str::Static("add")),
            text: Str::Static("Add row"),
            on_click: Some(ui::action(|state: &mut LocalBench| state.add())),
            ..ButtonOptions::default()
        }));
        for &id in &self.ids {
            children.push(counter_row(id, id % 2 == 1));
        }
        ui::view(ViewOptions {
            padding: ui::inset(12.0),
            gap: 4.0,
            children,
            ..ViewOptions::default()
        })
    }
}
