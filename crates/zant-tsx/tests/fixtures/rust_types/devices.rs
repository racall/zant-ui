//! A native module for tests/fixtures/rust_types/ (Rust mirror of the Zig
//! fixture `tests/tsx/zig_types/devices.zig`): Rust types and functions used
//! from TSX.

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Desktop,
    Laptop,
    Phone,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Desktop => "Desktop",
            Kind::Laptop => "Laptop",
            Kind::Phone => "Phone",
        }
    }
}

#[derive(Clone, Default, PartialEq)]
pub struct Address {
    pub host: zant::Str,
    pub port: u16,
}

#[derive(Clone, Default, PartialEq)]
pub struct Peer {
    pub name: zant::Str,
    pub kind: Kind,
    pub address: Address,
    pub online: bool,
    pub tags: Vec<zant::Str>,
    pub last_seen: Option<i64>,
}

pub fn peers() -> Vec<Peer> {
    vec![
        Peer {
            name: "Mac".into(),
            kind: Kind::Laptop,
            address: Address {
                host: "10.0.0.2".into(),
                port: 7070,
            },
            online: true,
            tags: vec!["home".into()],
            last_seen: Some(1),
        },
        Peer {
            name: "PC".into(),
            kind: Kind::Desktop,
            address: Address {
                host: "10.0.0.3".into(),
                port: 7070,
            },
            online: false,
            tags: Vec::new(),
            last_seen: None,
        },
    ]
}

pub fn label(peer: Peer) -> zant::Str {
    zant::Str::copy(&format!(
        "{}:{}",
        peer.address.host, peer.address.port
    ))
}

#[allow(non_snake_case)]
pub fn firstOnline(list: Vec<Peer>) -> Option<Peer> {
    list.into_iter().find(|peer| peer.online)
}
