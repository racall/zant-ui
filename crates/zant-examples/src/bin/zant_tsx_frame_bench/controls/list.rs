//! The 10 000-row list of `list_bench.tsx` written by hand in Rust: the
//! control group of the `list-scroll` / `list-click` scenarios.
#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use zant::Str;
use zant::flex;
use zant::ui::{
    self, App, BoxOptions, ButtonOptions, ListOptions, ListState, TextOptions, ViewOptions,
};

pub struct Row {
    pub id: u32,
    pub title: Str,
}

/// The list page state. The rows are a shared `Rc` so the virtual list's
/// item closure does not borrow the app state.
pub struct ListBench {
    rows: Rc<Vec<Row>>,
    picked: u32,
    list: Rc<RefCell<ListState>>,
}

impl ListBench {
    pub fn new(rows: Vec<Row>) -> ListBench {
        ListBench {
            rows: Rc::new(rows),
            picked: 0,
            list: Rc::new(RefCell::new(ListState::new())),
        }
    }

    fn item(rows: &Rc<Vec<Row>>, index: usize) -> ui::Node {
        let theme = ui::current_theme();
        let row = &rows[index];
        let id = row.id;
        ui::row(BoxOptions {
            key: Some(Str::copy(&format!("row-{id}"))),
            gap: 8.0,
            align_items: flex::Align::Center,
            height: Some(40.0),
            background: Some(if index.is_multiple_of(2) {
                theme.color_fill_quaternary
            } else {
                theme.color_bg_container
            }),
            children: vec![
                ui::text(row.title.clone(), TextOptions::default()),
                ui::button(ButtonOptions {
                    key: Some(Str::copy(&format!("pick-{id}"))),
                    text: Str::Static("Pick"),
                    on_click: Some(ui::action(move |state: &mut ListBench| {
                        state.picked = id;
                    })),
                    ..ButtonOptions::default()
                }),
            ],
            ..BoxOptions::default()
        })
    }
}

impl App for ListBench {
    fn render(&mut self) -> ui::Node {
        let rows = Rc::clone(&self.rows);
        let list_rows = Rc::clone(&rows);
        let ids = Rc::clone(&rows);
        let count = rows.len();
        ui::view(ViewOptions {
            grow: 1.0,
            padding: ui::inset(12.0),
            gap: 8.0,
            children: vec![
                ui::text(
                    format!("Picked {}", self.picked),
                    TextOptions {
                        key: Some(Str::Static("picked")),
                        ..TextOptions::default()
                    },
                ),
                ui::list(
                    &self.list,
                    ListOptions {
                        key: Str::Static("rows"),
                        count,
                        item: Rc::new(move |index| ListBench::item(&list_rows, index)),
                        item_id: Some(Rc::new(move |index| u64::from(ids[index].id))),
                        item_height: Some(40.0),
                        ..ListOptions::default()
                    },
                ),
            ],
            ..ViewOptions::default()
        })
    }
}
