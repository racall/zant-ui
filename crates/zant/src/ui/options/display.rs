//! Options of display components: dividers, tags, cards, alerts, avatars, progress.

use super::*;

/// `ui::divider`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct DividerOptions {
    pub key: Option<Str>,
    pub margin: Edges,
    pub color: Option<u32>,
}

/// `ui::tag`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct TagOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub color: Option<u32>,
    pub background: Option<u32>,
}

/// `ui::card`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct CardOptions {
    pub key: Option<Str>,
    pub title: Option<Str>,
    pub extra: Option<Str>,
    pub children: Vec<Node>,
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub gap: f32,
    pub padding: Edges,
    pub bordered: bool,
}

impl Default for CardOptions {
    fn default() -> Self {
        CardOptions {
            key: None,
            title: None,
            extra: None,
            children: Vec::new(),
            width: None,
            height: None,
            gap: 12.0,
            padding: Edges::all(20.0),
            bordered: true,
        }
    }
}

/// `ui::alert` status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AlertType {
    #[default]
    Info,
    Success,
    Warning,
    ErrorState,
}

/// `ui::alert`.
#[derive(Clone, Default, zant_macros::TsxProps)]
pub struct AlertOptions {
    pub key: Option<Str>,
    pub r#type: AlertType,
    pub title: Str,
    pub description: Str,
}

/// `ui::avatar`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct AvatarOptions {
    pub key: Option<Str>,
    pub text: Str,
    pub size: f32,
    pub background: Option<u32>,
    pub color: Option<u32>,
}

impl Default for AvatarOptions {
    fn default() -> Self {
        AvatarOptions {
            key: None,
            text: Str::Static("?"),
            size: 32.0,
            background: None,
            color: None,
        }
    }
}

/// `ui::progress`.
#[derive(Clone, zant_macros::TsxProps)]
pub struct ProgressOptions {
    pub key: Option<Str>,
    pub percent: f32,
    pub width: f32,
    pub height: f32,
    pub color: Option<u32>,
}

impl Default for ProgressOptions {
    fn default() -> Self {
        ProgressOptions {
            key: None,
            percent: 0.0,
            width: 200.0,
            height: 8.0,
            color: None,
        }
    }
}

/// Compile-time reflection metadata for the TSX compiler.
///
/// `#[derive(TsxProps)]` emits an `impl TsxProps` for each option struct; the
/// `zant-tsx-tags` generator reads [`TSX_REGISTRY`] and writes
/// `crates/zant-tsx/src/ui_tags/`. A `proc-macro` crate cannot export these
/// types itself, so they live here, next to the structs they describe.
///
/// Field *classification* (integer / enum / color / nested options …) stays
/// in the generator because Rust cannot enumerate an enum's variants at run
/// time.
pub mod tsx_props {
    /// One field of an option struct.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TsxField {
        /// Rust field name with the `r#` prefix stripped (`type` for `r#type`).
        pub name: &'static str,
        /// Rust type as token text (`Option<TextAction>`); the generator
        /// re-parses and classifies it.
        pub ty: &'static str,
        /// The TSX prop must be given (the field has no default value).
        pub required: bool,
        /// `#[tsx(default = "..")]` as written; `None` means `Default::default()`.
        pub default: Option<&'static str>,
    }

    /// One reflected option struct.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct TsxStruct {
        /// Rust type name (`ButtonOptions`).
        pub name: &'static str,
        /// The struct's doc comment, lines joined with `\n`.
        pub doc: &'static str,
        /// `module_path!()` of the struct (`zant::ui::options`).
        pub module_path: &'static str,
        /// Fields in declaration order.
        pub fields: &'static [TsxField],
    }

    /// Implemented by `#[derive(TsxProps)]`.
    pub trait TsxProps {
        /// The struct's reflection metadata.
        const TSX: TsxStruct;
    }
}
