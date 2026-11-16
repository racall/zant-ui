//! Syntax tree of the TSX subset `zant-tsx` compiles (`docs/tsx.md`).
//! Every node keeps the byte offset it starts at for error messages.
//!
//! Text is `Vec<u8>` (not `String`): decoded string literals, template quasis
//! and JSX text may hold WTF-8 bytes (lone surrogates, `\xNN` escapes), which
//! Rust's UTF-8 `String` cannot represent. Child nodes are `Box<T>`, optional
//! ones `Option<Box<T>>`.

/// Byte offset into the source.
pub type Loc = u32;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Name {
    pub name: Vec<u8>,
    pub loc: Loc,
}

/// A type annotation. Named types (`u32`, `string`, `Todo`, `JSX.Element`)
/// have `name`, optional `args` and no `detail`; structured types (`Todo[]`, `"a" | "b"`,
/// `{ id: number }`, `(id: number) => void`) carry a `detail` and an empty
/// `name`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeRef {
    pub name: Vec<u8>,
    pub loc: Loc,
    pub detail: Option<Box<TypeDetail>>,
    /// Type arguments of a named type.
    pub args: Vec<TypeRef>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TypeDetail {
    /// `T[]` or `Array<T>`
    Array(TypeRef),
    /// `A | B | ...` (two or more members)
    Union(Vec<TypeRef>),
    /// `"done"`
    StringLit(Vec<u8>),
    /// `null` or `undefined`
    Null,
    /// `{ id: number; title?: string }` (also an interface body)
    Object(Vec<TypeField>),
    /// `(id: number) => void`
    Func(FuncType),
    /// `A & B & { extra: string }` (two or more members); `interface B
    /// extends A { .. }` is the intersection of `A` and the body.
    Intersection(Vec<TypeRef>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeField {
    pub name: Name,
    pub ty: TypeRef,
    pub optional: bool,
}

/// A generic type parameter; variance modifiers are parsed but erased.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeParam {
    pub name: Name,
    pub constraint: Option<TypeRef>,
    pub default: Option<TypeRef>,
    pub is_const: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FuncType {
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub ret: TypeRef,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Expr {
    pub loc: Loc,
    pub data: ExprData,
}

/// The kinds of expression.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExprData {
    /// Raw literal text (`12`, `0.5`, `0xff1677ff`).
    Number(Vec<u8>),
    /// Decoded string literal.
    String(Vec<u8>),
    Boolean(bool),
    NullLit,
    Ident(Vec<u8>),
    Template(Template),
    Unary(Unary),
    Binary(Binary),
    Cond(Cond),
    Call(Call),
    Member(Member),
    Arrow(Box<Function>),
    Jsx(Box<Jsx>),
    Object(Vec<Property>),
    /// `[a, ...rest]`; spread items are `.spread` expressions.
    Array(Vec<Box<Expr>>),
    /// `...x` inside an array literal, object literal or call.
    Spread(Box<Expr>),
    /// `a[i]` / `a?.[i]`
    Index(Index),
    /// `x!` (TypeScript non-null assertion)
    NonNull(Box<Expr>),
    /// `x as T`
    As(As),
    Assign(Assign),
    Update(Update),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    /// Cooked text between the expressions (`exprs.len + 1` parts).
    pub quasis: Vec<Vec<u8>>,
    pub exprs: Vec<Box<Expr>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unary {
    pub op: Vec<u8>,
    pub operand: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Binary {
    pub op: Vec<u8>,
    pub lhs: Box<Expr>,
    pub rhs: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cond {
    pub cond: Box<Expr>,
    pub then: Box<Expr>,
    pub else_: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    pub callee: Box<Expr>,
    pub type_args: Vec<TypeRef>,
    pub args: Vec<Box<Expr>>,
    /// `f?.(args)`: nothing happens (the arguments are not evaluated) when
    /// `f` is absent.
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Member {
    pub object: Box<Expr>,
    pub name: Name,
    /// `a?.b`
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Index {
    pub object: Box<Expr>,
    pub index: Box<Expr>,
    /// `a?.[i]`
    pub optional: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct As {
    pub expr: Box<Expr>,
    pub ty: TypeRef,
    /// `expr satisfies T`: checked against `T` without a cast.
    pub satisfies: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Property {
    pub key: Name,
    pub value: Box<Expr>,
    /// `{ ...value }`: `key` is unused.
    pub spread: bool,
}

/// `x = e`, `x += e`, ...
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Assign {
    pub op: Vec<u8>,
    pub target: Box<Expr>,
    pub value: Box<Expr>,
}

/// `x++` / `--x`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    pub op: Vec<u8>,
    pub target: Box<Expr>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Jsx {
    pub type_args: Vec<TypeRef>,
    pub loc: Loc,
    /// Empty for a fragment `<>...</>`.
    pub name: Vec<u8>,
    pub attrs: Vec<Attr>,
    pub children: Vec<Child>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attr {
    pub name: Name,
    /// Null for a bare attribute (`<Input password />` means `true`).
    pub value: Option<Box<Expr>>,
    /// `{...value}`: `name` is empty (`codegen::spread` expands it).
    pub spread: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Child {
    /// JSX text after React's whitespace rules (never empty).
    Text(JsxText),
    /// `{expr}`; null for an empty container or a `{/* comment */}`.
    Expr(Option<Box<Expr>>),
    Element(Box<Jsx>),
}

/// A run of JSX text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JsxText {
    pub value: Vec<u8>,
    pub loc: Loc,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Param {
    pub name: Name,
    pub ty: Option<TypeRef>,
    /// `x?: T`
    pub optional: bool,
    /// `function Row({ todo, title: label }: Props)`: properties and local bindings
    /// (the param's `name` is then the whole object's, `props`).
    pub destructure: Vec<DestructuredProp>,
    /// `{ size = 14 }`: a default per destructured name (null: none).
    pub destructure_defaults: Vec<Option<Box<Expr>>>,
    /// `unit = "item"`: the argument callers may leave out.
    pub default: Option<Box<Expr>>,
    /// `...items: T[]`: the last parameter collects the remaining arguments.
    pub rest: bool,
}

/// A parameter property: `name` is the field, `local` is its binding.
/// Keeping property names here also preserves existing prop visitors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DestructuredProp {
    pub name: Vec<u8>,
    pub loc: Loc,
    pub local: Name,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Function {
    pub type_params: Vec<TypeParam>,
    pub loc: Loc,
    /// Empty for an arrow function.
    pub name: Vec<u8>,
    pub params: Vec<Param>,
    pub ret: Option<TypeRef>,
    pub body: Body,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    Block(Vec<Box<Stmt>>),
    /// Concise arrow body: `() => setCount(0)`.
    Expr(Box<Expr>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stmt {
    pub loc: Loc,
    pub data: StmtData,
}

/// The kinds of statement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StmtData {
    Decl(Decl),
    Func(Box<Function>),
    Ret(Option<Box<Expr>>),
    If(If),
    For(For),
    ForOf(ForOf),
    While(While),
    Switch(Switch),
    Break,
    Continue,
    Expr(Box<Expr>),
    Block(Vec<Box<Stmt>>),
}

/// C-style loop; omitted condition means true.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct For {
    pub init: Option<Box<Stmt>>,
    pub cond: Option<Box<Expr>>,
    pub update: Option<Box<Expr>>,
    pub body: Box<Stmt>,
}

/// Array iteration with a fresh binding for each element.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForOf {
    pub name: Name,
    pub is_const: bool,
    pub iterable: Box<Expr>,
    pub body: Box<Stmt>,
}

/// Pre-test while or post-test do/while.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct While {
    pub cond: Box<Expr>,
    pub body: Box<Stmt>,
    pub do_first: bool,
}

/// Cases retain source order; a null test is the default clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Switch {
    pub value: Box<Expr>,
    pub cases: Vec<Case>,
}

/// Empty case bodies group with the following clause.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Case {
    pub loc: Loc,
    pub value: Option<Box<Expr>>,
    pub body: Vec<Box<Stmt>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decl {
    pub is_const: bool,
    pub pattern: Pattern,
    pub ty: Option<TypeRef>,
    pub init: Option<Box<Expr>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pattern {
    Ident(Name),
    /// `const [value, setValue] = ...`; null for a hole (`[, b]`).
    Array(Vec<Option<Name>>),
    /// `const { ok, label: text } = ...` (custom hooks, `crate::hooks`).
    Object(Vec<ObjectBinding>),
}

/// `key` in `{ key }`, `{ key: local }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectBinding {
    pub key: Name,
    pub local: Name,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct If {
    pub cond: Box<Expr>,
    pub then: Box<Stmt>,
    pub else_: Option<Box<Stmt>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Import {
    pub loc: Loc,
    pub source: Vec<u8>,
    pub source_loc: Loc,
    pub type_only: bool,
    pub names: Vec<ImportName>,
    /// React's default / namespace binding; ignored except for qualified hooks and Fragment.
    pub default_name: Option<Name>,
    pub namespace_name: Option<Name>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportName {
    /// `import { type Name }` (the whole import may also be type-only).
    pub type_only: bool,
    pub imported: Name,
    pub local: Name,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Module {
    pub imports: Vec<Import>,
    pub functions: Vec<TopFunction>,
    /// `interface` / `type` declarations, in source order.
    pub types: Vec<TypeDecl>,
    /// Top-level `const` values that are not components.
    pub consts: Vec<TopConst>,
    /// `export declare function` signatures (native module specs, native/*.ts).
    pub declared: Vec<TopFunction>,
    /// `enum` declarations (their type is also in `types`).
    pub enums: Vec<EnumDecl>,
}

/// `enum Mode { Light = "light" }`: members are inlined where they are read
/// (`Mode.Light`), as tsc does for `const enum`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: Name,
    pub members: Vec<EnumMember>,
    pub exported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EnumMember {
    pub name: Name,
    pub value: EnumValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EnumValue {
    Text(Vec<u8>),
    /// The number as a Rust `f64` literal (`5.0`, `-1.0`).
    Number(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TypeDecl {
    pub type_params: Vec<TypeParam>,
    pub name: Name,
    pub ty: TypeRef,
    pub exported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopConst {
    pub name: Name,
    pub ty: Option<TypeRef>,
    pub value: Box<Expr>,
    pub exported: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TopFunction {
    pub func: Box<Function>,
    pub exported: bool,
    pub default: bool,
}
