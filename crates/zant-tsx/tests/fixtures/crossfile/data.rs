//! Native data for tests/fixtures/crossfile/ (Rust mirror of the Zig fixture
//! `tests/tsx/crossfile/data.zig`).

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Text,
    Image,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Text => "text",
            Kind::Image => "image",
        }
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct Data {
    pub row: i64,
    pub kind: Kind,
    pub label: zant::Str,
}

pub fn sample() -> Vec<Data> {
    vec![
        Data {
            row: 1,
            kind: Kind::Text,
            label: "one".into(),
        },
        Data {
            row: 2,
            kind: Kind::Image,
            label: "two".into(),
        },
    ]
}
