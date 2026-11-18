//! The tag table's types: tags, props, value spellings, prop kinds, content kinds and skipped builders.

/// How a prop's value is checked and which kind of field it sets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    /// `f32` or `Option<f32>` field: any number expression (ints convert).
    #[default]
    Number,
    /// Integer field of type `int_type` (`numberOfLines` u16, `order` i32):
    /// an integer expression.
    Integer,
    Boolean,
    /// String or optional string field: a string expression.
    String,
    /// Rust enum field: a string literal (or a `?:` of string literals) that
    /// must be one of `values` (its `.ts` spelling); emits `.rust`.
    Enumeration,
    /// `Option<u32>` ARGB field: "#rgb" / "#rrggbb" / "#rrggbbaa" string literals or a
    /// number expression (`0xff1677ff`).
    Color,
    /// One or more sides of an `Edges` field (`padding`, `margin`): a number
    /// expression applied to `sides`. Several props on the same field combine;
    /// later props win per side.
    Edges,
    /// `f32` / `Option<f32>` field that also takes a "50%" string literal, which sets
    /// `percent_field` instead.
    Dimension,
    /// A static string literal that sets a bool field: `values` maps each
    /// accepted `.ts` spelling to the bool field (`.rust`) it sets to true; an
    /// empty `.rust` accepts the spelling and sets nothing.
    Flag,
    /// RN `flex: n`: `grow = n, shrink = 1, basis = 0`.
    Flex,
    /// `actions::Action` (`() => void`).
    Action,
    /// `actions::BoolAction` (`(checked: boolean) => void`).
    BoolAction,
    /// `actions::TextAction` (`(value: string) => void`).
    TextAction,
    /// `actions::IndexAction` (`(index: usize) => void`).
    IndexAction,
    /// `ui::Node` field (required unless optional): an element expression.
    Node,
    /// `?ui::Node` field: an element expression, or `null`.
    OptionalNode,
    /// `Vec<ui::Node>` field: the element's children (also settable as a prop
    /// holding an array of elements).
    Children,
    /// `Vec<String>`-equivalent field: an array of strings.
    StringList,
    /// `&[T]` field with T a struct: an array of object literals whose keys
    /// are `item_props` (emits the item type `item_type`).
    StructList,
    /// Struct field whose fields all have defaults (`labels`): one object
    /// literal whose keys are `item_props` (emits `item_type { …,
    /// ..Default::default() }`; omitted keys keep the struct's defaults).
    Struct,
    /// `?zant::image::Image`: an in-memory image (TSX type `Image`, from a
    /// Rust module such as a thumbnail cache).
    Image,
    /// `?desktop::Shortcut`: a shortcut string ("ctrl+cmd+v"; "" / null for
    /// none).
    Shortcut,
    /// `?ui::HotkeyAction`: `(shortcut: string) => void`.
    ShortcutAction,
}

/// One or more sides of an `Edges` field.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Sides {
    pub top: bool,
    pub right: bool,
    pub bottom: bool,
    pub left: bool,
}

impl Sides {
    pub const NONE: Sides = Sides {
        top: false,
        right: false,
        bottom: false,
        left: false,
    };
    pub const ALL: Sides = Sides {
        top: true,
        right: true,
        bottom: true,
        left: true,
    };
    pub const HORIZONTAL: Sides = Sides {
        top: false,
        right: true,
        bottom: false,
        left: true,
    };
    pub const VERTICAL: Sides = Sides {
        top: true,
        right: false,
        bottom: true,
        left: false,
    };

    /// Any side selected.
    pub fn any(self) -> bool {
        self.top || self.right || self.bottom || self.left
    }
}

/// One accepted spelling of an enum value and what it emits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Value {
    /// The spelling TSX accepts (`"space-between"`).
    pub ts: &'static str,
    /// The Rust-side (snake_case) spelling of the enum value
    /// (`space_between`).
    pub rust: &'static str,
}

/// One TSX prop of a tag. Fields without a value use their [`Default`]; the
/// native table only spells out the ones it sets.
#[derive(Clone, Debug)]
pub struct Prop {
    /// TSX prop name (`backgroundColor`).
    pub name: &'static str,
    /// Rust options field (`background`).
    pub field: &'static str,
    pub kind: Kind,
    /// `.enumeration` / `.flag` spellings.
    pub values: Vec<Value>,
    /// `.edges` sides.
    pub sides: Sides,
    /// `.dimension` percent field.
    pub percent_field: &'static str,
    /// `.integer`: the Rust integer type (`u16`, `i32`).
    pub int_type: &'static str,
    /// The field has no default value: the prop must be given.
    pub required: bool,
    /// `.struct_list` / `.struct`: the element (or struct) type (`ui::TabItem`) and its fields as
    /// props (camelCase names, same kinds; nesting allowed).
    pub item_type: &'static str,
    pub item_props: Vec<Prop>,
    /// The field lives in a nested options struct that TSX flattens
    /// (`ClickableOptions.box`): emits `.parent = .{ .field = … }`. Empty for
    /// top-level fields.
    pub parent: &'static str,
}

impl Default for Prop {
    fn default() -> Self {
        Prop {
            name: "",
            field: "",
            kind: Kind::Number,
            values: Vec::new(),
            sides: Sides::NONE,
            percent_field: "",
            int_type: "",
            required: false,
            item_type: "",
            item_props: Vec::new(),
            parent: "",
        }
    }
}

/// Where the children of a tag go.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Content {
    /// Child elements go to `.children` (text children are an error).
    Children,
    /// Text children form the first argument: `ui::text(value, .{...})`.
    TextArg,
    /// Text children set the string field `content_field` (Button `text`,
    /// Checkbox `label`), same as the prop.
    TextField,
    /// The only child element sets the `Node` field `content_field` (Tooltip
    /// `child`), same as the prop.
    Child,
    /// No children allowed.
    #[default]
    None,
}

/// One supported `ui.*` builder.
#[derive(Clone, Debug, Default)]
pub struct Tag {
    /// JSX name (`View`).
    pub name: &'static str,
    /// Rust builder (`ui::view`).
    pub builder: &'static str,
    /// Builder takes a stable instance id before its options.
    pub instance_id: bool,
    pub content: Content,
    pub content_field: &'static str,
    pub props: Vec<Prop>,
}

/// A builder intentionally excluded from the generated table.
#[derive(Clone, Copy, Debug)]
pub struct Skipped {
    pub builder: &'static str,
    pub reason: &'static str,
}
