//! Syntax tree of the TSX subset `zant-tsx` compiles (`docs/tsx.md`).
//! Every node keeps the byte offset it starts at for error messages.

/// Byte offset into the source.
pub const Loc = u32;

pub const Name = struct {
    name: []const u8,
    loc: Loc,
};

/// A type annotation. Named types (`u32`, `string`, `Todo`, `JSX.Element`)
/// have `name` and no `detail`; structured types (`Todo[]`, `"a" | "b"`,
/// `{ id: number }`, `(id: number) => void`) carry a `detail` and an empty
/// `name`.
pub const TypeRef = struct {
    name: []const u8,
    loc: Loc,
    detail: ?*const TypeDetail = null,
};

pub const TypeDetail = union(enum) {
    /// `T[]` or `Array<T>`
    array: TypeRef,
    /// `A | B | ...` (two or more members)
    union_: []const TypeRef,
    /// `"done"`
    string_lit: []const u8,
    /// `null` or `undefined`
    null_,
    /// `{ id: number; title?: string }` (also an interface body)
    object: []const TypeField,
    /// `(id: number) => void`
    func: FuncType,
};

pub const TypeField = struct {
    name: Name,
    ty: TypeRef,
    optional: bool = false,
};

pub const FuncType = struct {
    params: []const Param,
    ret: TypeRef,
};

pub const Expr = struct {
    loc: Loc,
    data: Data,

    pub const Data = union(enum) {
        /// Raw literal text (`12`, `0.5`, `0xff1677ff`).
        number: []const u8,
        /// Decoded string literal.
        string: []const u8,
        boolean: bool,
        null_lit,
        ident: []const u8,
        template: Template,
        unary: Unary,
        binary: Binary,
        cond: Cond,
        call: Call,
        member: Member,
        arrow: *Function,
        jsx: *Jsx,
        object: []const Property,
        /// `[a, ...rest]`; spread items are `.spread` expressions.
        array: []const *Expr,
        /// `...x` inside an array literal, object literal or call.
        spread: *Expr,
        /// `a[i]` / `a?.[i]`
        index: Index,
        /// `x!` (TypeScript non-null assertion)
        non_null: *Expr,
        /// `x as T`
        as: As,
        assign: Assign,
        update: Update,
    };
};

pub const Template = struct {
    /// Cooked text between the expressions (`exprs.len + 1` parts).
    quasis: []const []const u8,
    exprs: []const *Expr,
};

pub const Unary = struct {
    op: []const u8,
    operand: *Expr,
};

pub const Binary = struct {
    op: []const u8,
    lhs: *Expr,
    rhs: *Expr,
};

pub const Cond = struct {
    cond: *Expr,
    then: *Expr,
    else_: *Expr,
};

pub const Call = struct {
    callee: *Expr,
    type_args: []const TypeRef,
    args: []const *Expr,
};

pub const Member = struct {
    object: *Expr,
    name: Name,
    /// `a?.b`
    optional: bool = false,
};

pub const Index = struct {
    object: *Expr,
    index: *Expr,
    /// `a?.[i]`
    optional: bool = false,
};

pub const As = struct {
    expr: *Expr,
    ty: TypeRef,
};

pub const Property = struct {
    key: Name,
    value: *Expr,
    /// `{ ...value }`: `key` is unused.
    spread: bool = false,
};

/// `x = e`, `x += e`, ...
pub const Assign = struct {
    op: []const u8,
    target: *Expr,
    value: *Expr,
};

/// `x++` / `--x`
pub const Update = struct {
    op: []const u8,
    target: *Expr,
};

pub const Jsx = struct {
    loc: Loc,
    /// Empty for a fragment `<>...</>`.
    name: []const u8,
    attrs: []const Attr,
    children: []const Child,
};

pub const Attr = struct {
    name: Name,
    /// Null for a bare attribute (`<Input password />` means `true`).
    value: ?*Expr,
    /// `{...value}`: `name` is empty (`spread.zig` expands it).
    spread: bool = false,
};

pub const Child = union(enum) {
    /// JSX text after React's whitespace rules (never empty).
    text: Text,
    /// `{expr}`; null for an empty container or a `{/* comment */}`.
    expr: ?*Expr,
    element: *Jsx,

    pub const Text = struct {
        value: []const u8,
        loc: Loc,
    };
};

pub const Param = struct {
    name: Name,
    ty: ?TypeRef,
    /// `x?: T`
    optional: bool = false,
    /// `function Row({ todo, onToggle }: Props)`: the destructured names
    /// (the param's `name` is then the whole object's, `props`).
    destructure: []const Name = &.{},
    /// `{ size = 14 }`: a default per destructured name (null: none).
    destructure_defaults: []const ?*Expr = &.{},
    /// `unit = "item"`: the argument callers may leave out.
    default: ?*Expr = null,
};

pub const Function = struct {
    loc: Loc,
    /// Empty for an arrow function.
    name: []const u8,
    params: []const Param,
    ret: ?TypeRef,
    body: Body,
};

pub const Body = union(enum) {
    block: []const *Stmt,
    /// Concise arrow body: `() => setCount(0)`.
    expr: *Expr,
};

pub const Stmt = struct {
    loc: Loc,
    data: Data,

    pub const Data = union(enum) {
        decl: Decl,
        func: *Function,
        ret: ?*Expr,
        if_: If,
        for_: For,
        for_of: ForOf,
        while_: While,
        switch_: Switch,
        break_,
        continue_,
        expr: *Expr,
        block: []const *Stmt,
    };
};

/// C-style loop; omitted condition means true.
pub const For = struct {
    init: ?*Stmt,
    cond: ?*Expr,
    update: ?*Expr,
    body: *Stmt,
};

/// Array iteration with a fresh binding for each element.
pub const ForOf = struct {
    name: Name,
    is_const: bool,
    iterable: *Expr,
    body: *Stmt,
};

/// Pre-test while or post-test do/while.
pub const While = struct {
    cond: *Expr,
    body: *Stmt,
    do_first: bool = false,
};

/// Cases retain source order; a null test is the default clause.
pub const Switch = struct {
    value: *Expr,
    cases: []const Case,
};

/// Empty case bodies group with the following clause.
pub const Case = struct {
    loc: Loc,
    value: ?*Expr,
    body: []const *Stmt,
};

pub const Decl = struct {
    is_const: bool,
    pattern: Pattern,
    ty: ?TypeRef,
    init: ?*Expr,
};

pub const Pattern = union(enum) {
    ident: Name,
    /// `const [value, setValue] = ...`; null for a hole (`[, b]`).
    array: []const ?Name,
};

pub const If = struct {
    cond: *Expr,
    then: *Stmt,
    else_: ?*Stmt,
};

pub const Import = struct {
    loc: Loc,
    source: []const u8,
    source_loc: Loc,
    type_only: bool,
    names: []const ImportName,
    /// React's default / namespace binding; ignored except for qualified hooks and Fragment.
    default_name: ?Name = null,
    namespace_name: ?Name = null,
};

pub const ImportName = struct {
    /// `import { type Name }` (the whole import may also be type-only).
    type_only: bool = false,
    imported: Name,
    local: Name,
};

pub const Module = struct {
    imports: []const Import,
    functions: []const TopFunction,
    /// `interface` / `type` declarations, in source order.
    types: []const TypeDecl = &.{},
    /// Top-level `const` values that are not components.
    consts: []const TopConst = &.{},
};

pub const TypeDecl = struct {
    name: Name,
    ty: TypeRef,
    exported: bool,
};

pub const TopConst = struct {
    name: Name,
    ty: ?TypeRef,
    value: *Expr,
    exported: bool,
};

pub const TopFunction = struct {
    func: *Function,
    exported: bool,
    default: bool,
};
