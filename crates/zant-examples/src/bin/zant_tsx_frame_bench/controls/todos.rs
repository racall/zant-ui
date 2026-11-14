//! `todos.tsx` written by hand in Rust: the control group of the
//! `todos-add` / `todos-toggle` / `todos-filter` scenarios.
#![forbid(unsafe_code)]

use zant::Str;
use zant::layout::flex;
use zant::ui::{
    self, App, BoxOptions, ButtonOptions, HeadingOptions, Icon, IconButtonOptions, InputOptions,
    SegmentedOption, SegmentedOptions, TextOptions, ToggleOptions, ViewOptions,
};

struct Todo {
    id: u32,
    title: String,
    done: bool,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Filter {
    #[default]
    All,
    Active,
    Done,
}

/// The todo page state.
pub struct Todos {
    todos: Vec<Todo>,
    next_id: u32,
    text: String,
    filter: Filter,
}

impl Default for Todos {
    fn default() -> Self {
        Todos {
            todos: Vec::new(),
            next_id: 1,
            text: String::new(),
            filter: Filter::All,
        }
    }
}

impl Todos {
    pub fn new() -> Todos {
        Todos::default()
    }

    fn on_text(&mut self, value: &str) {
        self.text.clear();
        self.text.push_str(value);
    }

    fn add(&mut self) {
        let title = self.text.trim();
        if title.is_empty() {
            return;
        }
        let id = self.next_id;
        self.todos.push(Todo {
            id,
            title: title.to_owned(),
            done: false,
        });
        self.next_id += 1;
        self.text.clear();
    }

    fn index_of(&self, id: u32) -> Option<usize> {
        self.todos.iter().position(|todo| todo.id == id)
    }

    fn toggle(&mut self, id: u32) {
        if let Some(index) = self.index_of(id) {
            self.todos[index].done = !self.todos[index].done;
        }
    }

    fn remove(&mut self, id: u32) {
        if let Some(index) = self.index_of(id) {
            self.todos.remove(index);
        }
    }

    fn clear_done(&mut self) {
        self.todos.retain(|todo| !todo.done);
    }

    fn set_filter(&mut self, index: usize) {
        self.filter = match index {
            0 => Filter::All,
            1 => Filter::Active,
            _ => Filter::Done,
        };
    }

    fn row(todo: &Todo) -> ui::Node {
        let id = todo.id;
        ui::row(BoxOptions {
            key: Some(Str::copy(&format!("todo-{id}"))),
            gap: 8.0,
            align_items: flex::Align::Center,
            children: vec![
                ui::checkbox(ToggleOptions {
                    key: Some(Str::copy(&format!("check-{id}"))),
                    label: Str::copy(&todo.title),
                    checked: todo.done,
                    on_change: Some(ui::bool_action(move |state: &mut Todos, _on: bool| {
                        state.toggle(id)
                    })),
                    ..ToggleOptions::default()
                }),
                ui::row(BoxOptions {
                    grow: 1.0,
                    ..BoxOptions::default()
                }),
                ui::icon_button(IconButtonOptions {
                    key: Some(Str::copy(&format!("remove-{id}"))),
                    icon: Icon::Trash,
                    on_click: Some(ui::action(move |state: &mut Todos| state.remove(id))),
                    ..IconButtonOptions::default()
                }),
            ],
            ..BoxOptions::default()
        })
    }
}

impl App for Todos {
    fn render(&mut self) -> ui::Node {
        let left = self.todos.iter().filter(|todo| !todo.done).count();
        let rows: Vec<ui::Node> = self
            .todos
            .iter()
            .filter(|todo| match self.filter {
                Filter::All => true,
                Filter::Active => !todo.done,
                Filter::Done => todo.done,
            })
            .map(Todos::row)
            .collect();
        let rows_node = if rows.is_empty() {
            ui::text(
                if self.todos.is_empty() {
                    "Nothing to do yet."
                } else {
                    "Nothing here."
                },
                TextOptions {
                    key: Some(Str::Static("empty")),
                    ..TextOptions::default()
                },
            )
        } else {
            ui::view(ViewOptions {
                key: Some(Str::Static("rows")),
                gap: 4.0,
                children: rows,
                ..ViewOptions::default()
            })
        };
        ui::view(ViewOptions {
            padding: ui::inset(24.0),
            gap: 12.0,
            children: vec![
                ui::heading(
                    "Todos",
                    HeadingOptions {
                        level: 3,
                        ..HeadingOptions::default()
                    },
                ),
                ui::row(BoxOptions {
                    gap: 8.0,
                    children: vec![
                        ui::view(ViewOptions {
                            grow: 1.0,
                            shrink: 1.0,
                            basis: Some(0.0),
                            children: vec![ui::input(InputOptions {
                                key: Some(Str::Static("new")),
                                value: Some(Str::copy(&self.text)),
                                placeholder: Str::Static("What needs to be done?"),
                                on_change: Some(ui::text_action(
                                    |state: &mut Todos, value: &str| state.on_text(value),
                                )),
                                on_submit: Some(ui::action(|state: &mut Todos| state.add())),
                                ..InputOptions::default()
                            })],
                            ..ViewOptions::default()
                        }),
                        ui::button(ButtonOptions {
                            key: Some(Str::Static("add")),
                            text: Str::Static("Add"),
                            variant: zant::core::node::ButtonType::Primary,
                            on_click: Some(ui::action(|state: &mut Todos| state.add())),
                            ..ButtonOptions::default()
                        }),
                    ],
                    ..BoxOptions::default()
                }),
                ui::segmented(SegmentedOptions {
                    key: Some(Str::Static("filter")),
                    options: vec![
                        SegmentedOption {
                            label: Str::Static("All"),
                            ..SegmentedOption::default()
                        },
                        SegmentedOption {
                            label: Str::Static("Active"),
                            ..SegmentedOption::default()
                        },
                        SegmentedOption {
                            label: Str::Static("Done"),
                            ..SegmentedOption::default()
                        },
                    ],
                    selected: self.filter as usize,
                    on_change: Some(ui::index_action(|state: &mut Todos, index: usize| {
                        state.set_filter(index)
                    })),
                    ..SegmentedOptions::default()
                }),
                rows_node,
                ui::row(BoxOptions {
                    gap: 12.0,
                    align_items: flex::Align::Center,
                    children: vec![
                        ui::text(
                            format!("{left} {} left", if left == 1 { "item" } else { "items" }),
                            TextOptions {
                                key: Some(Str::Static("left")),
                                ..TextOptions::default()
                            },
                        ),
                        ui::button(ButtonOptions {
                            key: Some(Str::Static("clear")),
                            text: Str::Static("Clear completed"),
                            on_click: Some(ui::action(|state: &mut Todos| state.clear_done())),
                            ..ButtonOptions::default()
                        }),
                    ],
                    ..BoxOptions::default()
                }),
            ],
            ..ViewOptions::default()
        })
    }
}
