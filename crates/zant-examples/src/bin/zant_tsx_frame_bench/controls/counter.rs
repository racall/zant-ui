//! `counter.tsx` written by hand in Rust: the control group of the
//! `counter-clicks` / `counter-typing` scenarios.
#![forbid(unsafe_code)]

use zant::Str;
use zant::layout::flex;
use zant::renderer::text;
use zant::ui::{
    self, App, BoxOptions, ButtonOptions, InputOptions, TextOptions, ToggleOptions, ViewOptions,
};

/// The counter page state.
#[derive(Default)]
pub struct Counter {
    count: u32,
    step: u32,
    name: Str,
    large: bool,
    notify: bool,
}

impl Counter {
    /// The initial state: count 0, step 1, name "Zant", notify on.
    pub fn new() -> Counter {
        Counter {
            count: 0,
            step: 1,
            name: Str::Static("Zant"),
            large: false,
            notify: true,
        }
    }

    fn on_name(&mut self, value: &str) {
        self.name = Str::copy(value);
    }

    fn increment(&mut self) {
        self.count += self.step;
    }

    fn reset(&mut self) {
        self.count = 0;
    }

    fn toggle_step(&mut self) {
        self.step = if self.step == 1 { 5 } else { 1 };
    }

    fn set_large(&mut self, on: bool) {
        self.large = on;
    }

    fn set_notify(&mut self, on: bool) {
        self.notify = on;
    }
}

impl App for Counter {
    fn render(&mut self) -> ui::Node {
        let mut children = vec![
            ui::text(
                format!("Hello, {}!", self.name.as_str()),
                TextOptions {
                    key: Some(Str::Static("title")),
                    size: if self.large { 28.0 } else { 20.0 },
                    weight: text::Weight::Semibold,
                    ..TextOptions::default()
                },
            ),
            ui::text(
                format!("Count: {} (×2 = {})", self.count, self.count * 2),
                TextOptions {
                    key: Some(Str::Static("count")),
                    ..TextOptions::default()
                },
            ),
            ui::input(InputOptions {
                key: Some(Str::Static("name")),
                value: Some(self.name.clone()),
                placeholder: Str::Static("Your name"),
                width: Some(240.0),
                on_change: Some(ui::text_action(|state: &mut Counter, value: &str| {
                    state.on_name(value)
                })),
                ..InputOptions::default()
            }),
            ui::row(BoxOptions {
                gap: 8.0,
                align_items: flex::Align::Center,
                children: vec![
                    ui::button(ButtonOptions {
                        key: Some(Str::Static("inc")),
                        text: Str::copy(&format!("+{}", self.step)),
                        variant: zant::core::node::ButtonType::Primary,
                        on_click: Some(ui::action(|state: &mut Counter| state.increment())),
                        ..ButtonOptions::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some(Str::Static("reset")),
                        text: Str::Static("Reset"),
                        disabled: self.count == 0,
                        on_click: Some(ui::action(|state: &mut Counter| state.reset())),
                        ..ButtonOptions::default()
                    }),
                    ui::button(ButtonOptions {
                        key: Some(Str::Static("step")),
                        text: Str::copy(&format!("Step {}", self.step)),
                        on_click: Some(ui::action(|state: &mut Counter| state.toggle_step())),
                        ..ButtonOptions::default()
                    }),
                ],
                ..BoxOptions::default()
            }),
            ui::r#switch(ToggleOptions {
                key: Some(Str::Static("large")),
                label: Str::Static("Large title"),
                checked: self.large,
                on_change: Some(ui::bool_action(|state: &mut Counter, on: bool| {
                    state.set_large(on)
                })),
                ..ToggleOptions::default()
            }),
            ui::checkbox(ToggleOptions {
                key: Some(Str::Static("notify")),
                label: Str::Static("Notify me"),
                checked: self.notify,
                on_change: Some(ui::bool_action(|state: &mut Counter, on: bool| {
                    state.set_notify(on)
                })),
                ..ToggleOptions::default()
            }),
        ];
        if self.notify {
            children.push(ui::text(
                "Notifications are on",
                TextOptions {
                    key: Some(Str::Static("note")),
                    ..TextOptions::default()
                },
            ));
        }
        if self.count >= 10 {
            children.push(ui::text(
                "That is a lot of clicks.",
                TextOptions {
                    key: Some(Str::Static("many")),
                    ..TextOptions::default()
                },
            ));
        }
        ui::view(ViewOptions {
            padding: ui::inset(24.0),
            gap: 14.0,
            align_items: flex::Align::Start,
            children,
            ..ViewOptions::default()
        })
    }
}
