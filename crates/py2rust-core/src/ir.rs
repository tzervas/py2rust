//! The **typed semantic IR** — the representation between source-language ASTs and
//! target-language backends. See `docs/SEMANTIC-IR.md` for the full architecture.
//!
//! # Why this module exists
//!
//! The existing lowering surface is typed `Python AST -> Option<String>`
//! ([`crate::emit`]). Every decision must be reachable from the node in hand, because
//! no value survives between node visits. Ownership, exception effects, trait
//! obligations and type identity are **whole-program** properties, so they cannot be
//! stated in that architecture at all — not for lack of match arms, but for lack of
//! anywhere to put the answer.
//!
//! This module is that place. It is deliberately **inert** at this stage: nothing in
//! the existing pipeline references it, so it cannot regress any current behaviour
//! (migration Stage 0, `docs/SEMANTIC-IR.md` §5).
//!
//! # Guarantee tags (VR-5)
//!
//! - The lattices ([`IrType::join`], [`Ownership::join`]) are **Empirical**: unit-tested
//!   over their interesting pairs, including every conservative-top case.
//! - The IR node vocabulary is **Declared**: a design surface, not yet validated by a
//!   round-trip against [`crate::emit`]'s output (that is migration Stage 1).
//!
//! # The safety property
//!
//! Both lattices are **conservative toward refusal**. Joining disagreeing information
//! yields a top element ([`IrType::Dynamic`], [`Ownership::Contended`]) that means
//! "there is no single answer — gap it", never an arbitrary pick. For ownership this
//! is the difference between an honest gap and Rust that *compiles and misbehaves*,
//! which is the worst failure this transpiler can produce.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// A source position carried through lowering so a gap can always name its origin.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
pub struct IrSpan {
    /// Byte offset of the construct in the originating source file.
    pub start: u32,
    /// Byte offset one past the end of the construct.
    pub end: u32,
}

impl IrSpan {
    /// A span covering `start..end`.
    #[must_use]
    pub fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The type lattice.
///
/// Both ends matter and they are **not** the same thing:
///
/// - [`IrType::Unknown`] — nothing inferred *yet*. Ask again later.
/// - [`IrType::Dynamic`] — inference **proved** there is no single type. This is a
///   positive result and the honest-gap trigger.
///
/// Conflating the two is how a transpiler ends up guessing at a name that Python
/// rebound to a different type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IrType {
    /// Not yet inferred (lattice bottom for inference purposes).
    Unknown,
    /// Proven to have no single static type — must gap, never guess.
    Dynamic,
    /// `None` / `()`.
    Unit,
    /// Integer of a given width and signedness.
    Int {
        /// Bit width (Python `int` is arbitrary-precision; 64 is the default lowering).
        bits: u8,
        /// Whether the integer is signed.
        signed: bool,
    },
    /// Floating point (`f64`).
    Float,
    /// Boolean.
    Bool,
    /// Owned string.
    Str,
    /// Homogeneous list.
    List(Box<IrType>),
    /// Mapping from key type to value type.
    Dict(Box<IrType>, Box<IrType>),
    /// Set of a single element type.
    Set(Box<IrType>),
    /// Fixed-arity heterogeneous tuple.
    Tuple(Vec<IrType>),
    /// `Optional[T]` / `T | None`.
    Optional(Box<IrType>),
    /// A nominal user-defined type (class, struct).
    Named(String),
    /// A generic parameter introduced by trait synthesis or inference.
    TypeVar(String),
    /// A callable.
    Fn {
        /// Parameter types, in order.
        params: Vec<IrType>,
        /// Return type.
        ret: Box<IrType>,
    },
}

impl IrType {
    /// Signed 64-bit integer — the default lowering for Python `int`.
    #[must_use]
    pub fn i64() -> Self {
        IrType::Int {
            bits: 64,
            signed: true,
        }
    }

    /// Lattice join.
    ///
    /// `Unknown` is the identity (it carries no information). Equal types join to
    /// themselves. Structural types join element-wise. **Two disagreeing concrete
    /// types join to [`IrType::Dynamic`]** — never to an arbitrary pick. That single
    /// rule is what stops the transpiler inventing a type for a rebound name.
    #[must_use]
    pub fn join(&self, other: &IrType) -> IrType {
        match (self, other) {
            (IrType::Unknown, t) | (t, IrType::Unknown) => t.clone(),
            (IrType::Dynamic, _) | (_, IrType::Dynamic) => IrType::Dynamic,
            (a, b) if a == b => a.clone(),

            // `T` joined with `Optional[T]` stays optional — widening, not a conflict.
            (IrType::Optional(a), b) | (b, IrType::Optional(a)) if a.as_ref() == b => {
                IrType::Optional(a.clone())
            }
            (IrType::Unit, t) | (t, IrType::Unit) => IrType::Optional(Box::new(t.clone())),

            (IrType::List(a), IrType::List(b)) => IrType::List(Box::new(a.join(b))),
            (IrType::Set(a), IrType::Set(b)) => IrType::Set(Box::new(a.join(b))),
            (IrType::Dict(ak, av), IrType::Dict(bk, bv)) => {
                IrType::Dict(Box::new(ak.join(bk)), Box::new(av.join(bv)))
            }
            (IrType::Tuple(a), IrType::Tuple(b)) if a.len() == b.len() => {
                IrType::Tuple(a.iter().zip(b).map(|(x, y)| x.join(y)).collect())
            }

            // Integers of differing width/signedness: widening is a semantic change
            // Python does not make, so this is a genuine conflict.
            _ => IrType::Dynamic,
        }
    }

    /// Whether this type can be lowered to a single concrete Rust type.
    ///
    /// [`IrType::Unknown`] is *not* lowerable: emitting for a type inference never
    /// resolved would be a guess.
    #[must_use]
    pub fn is_lowerable(&self) -> bool {
        match self {
            IrType::Unknown | IrType::Dynamic => false,
            IrType::List(t) | IrType::Set(t) | IrType::Optional(t) => t.is_lowerable(),
            IrType::Dict(k, v) => k.is_lowerable() && v.is_lowerable(),
            IrType::Tuple(ts) => ts.iter().all(IrType::is_lowerable),
            IrType::Fn { params, ret } => {
                params.iter().all(IrType::is_lowerable) && ret.is_lowerable()
            }
            _ => true,
        }
    }
}

// ---------------------------------------------------------------------------
// Ownership
// ---------------------------------------------------------------------------

/// The ownership lattice — the output of escape/alias analysis.
///
/// This is the dangerous axis: a wrong choice here produces Rust that **compiles and
/// silently misbehaves**. `Owned` and `Rc<RefCell<T>>` are both valid Rust; only one
/// preserves the Python semantics. So the lattice has an explicit top,
/// [`Ownership::Contended`], meaning "aliasing not provable" — which **must** produce a
/// gap and no code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Ownership {
    /// Exactly one live binding — lowers to `T`.
    Owned,
    /// A non-escaping reference — lowers to `&T` or `&mut T`.
    Borrowed {
        /// Whether the borrow is unique/mutable.
        mutable: bool,
    },
    /// Multiple readers, no writer — lowers to `Rc<T>`.
    Shared,
    /// Multiple holders with at least one writer — lowers to `Rc<RefCell<T>>`.
    SharedMutable,
    /// Aliasing could not be proven. **Emit nothing; record a gap.**
    Contended,
}

impl Ownership {
    /// Lattice join, conservative toward [`Ownership::Contended`].
    ///
    /// Any join involving `Contended` is `Contended` — unprovability is absorbing.
    /// A shared value that is also mutated anywhere becomes `SharedMutable`: the
    /// analysis never downgrades sharing to a move.
    #[must_use]
    pub fn join(self, other: Ownership) -> Ownership {
        use Ownership::{Borrowed, Contended, Owned, Shared, SharedMutable};
        match (self, other) {
            (Contended, _) | (_, Contended) => Contended,
            (a, b) if a == b => a,

            (SharedMutable, _) | (_, SharedMutable) => SharedMutable,
            // Sharing plus any mutable access is interior mutability, not a move.
            (Shared, Borrowed { mutable: true }) | (Borrowed { mutable: true }, Shared) => {
                SharedMutable
            }
            (Shared, _) | (_, Shared) => Shared,

            (Borrowed { .. }, Borrowed { .. }) => Borrowed { mutable: true },
            // An owner that is also borrowed elsewhere in a way the analysis merged
            // is not provably single-owner.
            (Owned, Borrowed { .. }) | (Borrowed { .. }, Owned) => Contended,

            _ => Contended,
        }
    }

    /// Whether this ownership state permits emitting code at all.
    ///
    /// [`Ownership::Contended`] does not: an honest gap always beats a lowering that
    /// compiles and changes behaviour.
    #[must_use]
    pub fn is_lowerable(self) -> bool {
        self != Ownership::Contended
    }

    /// How this ownership wraps a lowered Rust type, if it is lowerable.
    #[must_use]
    pub fn wrap_rust(self, inner: &str) -> Option<String> {
        match self {
            Ownership::Owned => Some(inner.to_string()),
            Ownership::Borrowed { mutable: false } => Some(format!("&{inner}")),
            Ownership::Borrowed { mutable: true } => Some(format!("&mut {inner}")),
            Ownership::Shared => Some(format!("Rc<{inner}>")),
            Ownership::SharedMutable => Some(format!("Rc<RefCell<{inner}>>")),
            Ownership::Contended => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Effects
// ---------------------------------------------------------------------------

/// A single effect an IR node may perform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Effect {
    /// May raise the named exception type (`None` = unknown/bare `raise`).
    Raises(Option<String>),
    /// Mutates the named place.
    Mutates(String),
    /// Does not return normally (`sys.exit`, infinite loop, unconditional raise).
    Diverges,
    /// Performs I/O.
    Io,
    /// Suspends (`await`).
    Await,
    /// Effects could not be determined — propagates like `Raises(None)` but is
    /// distinguishable in a gap report.
    Unknown,
}

/// The set of effects attached to an IR node, propagated bottom-up.
///
/// A function whose row contains [`Effect::Raises`] lowers to `-> Result<T, E>`, and
/// its call sites become `?`. That is what makes exceptions an **inter-procedural**
/// problem rather than a per-`try`-node emitter template — which is what they actually
/// are, and why the per-construct architecture could never close them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRow(pub BTreeSet<Effect>);

impl EffectRow {
    /// An empty (pure) effect row.
    #[must_use]
    pub fn pure() -> Self {
        Self(BTreeSet::new())
    }

    /// Add one effect.
    pub fn add(&mut self, e: Effect) {
        self.0.insert(e);
    }

    /// Union with another row — the bottom-up propagation step.
    #[must_use]
    pub fn union(mut self, other: &EffectRow) -> Self {
        self.0.extend(other.0.iter().cloned());
        self
    }

    /// Whether anything in this row forces a `Result`-typed return.
    #[must_use]
    pub fn needs_result(&self) -> bool {
        self.0
            .iter()
            .any(|e| matches!(e, Effect::Raises(_) | Effect::Unknown))
    }

    /// Whether the row is empty.
    #[must_use]
    pub fn is_pure(&self) -> bool {
        self.0.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Trait obligations (duck typing -> nominal traits)
// ---------------------------------------------------------------------------

/// A structural requirement discovered from *usage*, to be synthesized into a trait.
///
/// A parameter used as `x.read()` accretes `{ method: "read", arity: 0 }`. Grouping
/// every obligation on a parameter and emitting one synthetic trait turns
/// structural-at-runtime into nominal-at-compile-time — which requires collecting usage
/// across a whole body, and is therefore impossible node-locally.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct TraitObligation {
    /// Method name invoked on the value.
    pub method: String,
    /// Number of arguments passed (excluding `self`).
    pub arity: usize,
    /// Where the obligation was witnessed.
    pub span: IrSpan,
}

// ---------------------------------------------------------------------------
// IR nodes
// ---------------------------------------------------------------------------

/// A binding (a local, parameter, or module-level name) with its inferred facts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    /// Source-level name.
    pub name: String,
    /// Inferred type.
    pub ty: IrType,
    /// Inferred ownership.
    pub own: Ownership,
    /// Structural requirements witnessed on this binding.
    pub obligations: Vec<TraitObligation>,
}

impl Binding {
    /// A binding with nothing inferred yet.
    #[must_use]
    pub fn unresolved(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ty: IrType::Unknown,
            own: Ownership::Owned,
            obligations: Vec::new(),
        }
    }

    /// Whether this binding can be emitted at all.
    #[must_use]
    pub fn is_lowerable(&self) -> bool {
        self.ty.is_lowerable() && self.own.is_lowerable()
    }
}

/// Binary operators, target-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// Python `/` — true division, always float.
    TrueDiv,
    /// Python `//` — floor division (**not** Rust `/`, which truncates).
    FloorDiv,
    /// Python `%` — modulo (**not** Rust `%`, which is remainder).
    Mod,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// Logical and.
    And,
    /// Logical or.
    Or,
}

/// A literal value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrLit {
    /// Integer literal.
    Int(i64),
    /// Float literal.
    Float(f64),
    /// Boolean literal.
    Bool(bool),
    /// String literal.
    Str(String),
    /// `None`.
    None,
}

/// An IR expression: a node plus the facts inference attached to it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrExpr {
    /// The expression shape.
    pub kind: IrExprKind,
    /// Inferred type.
    pub ty: IrType,
    /// Effects performed while evaluating.
    pub effects: EffectRow,
    /// Source position.
    pub span: IrSpan,
}

impl IrExpr {
    /// An expression with a known type and no effects.
    #[must_use]
    pub fn pure(kind: IrExprKind, ty: IrType) -> Self {
        Self {
            kind,
            ty,
            effects: EffectRow::pure(),
            span: IrSpan::default(),
        }
    }
}

/// Expression shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrExprKind {
    /// A literal.
    Lit(IrLit),
    /// A name reference.
    Name(String),
    /// Binary operation.
    Bin(BinOp, Box<IrExpr>, Box<IrExpr>),
    /// Unary negation.
    Neg(Box<IrExpr>),
    /// Logical not.
    Not(Box<IrExpr>),
    /// A call: callee name and arguments.
    Call(String, Vec<IrExpr>),
    /// A method call — the site that produces a [`TraitObligation`].
    MethodCall(Box<IrExpr>, String, Vec<IrExpr>),
    /// Indexing.
    Index(Box<IrExpr>, Box<IrExpr>),
    /// Attribute access.
    Attr(Box<IrExpr>, String),
    /// A list literal.
    ListLit(Vec<IrExpr>),
    /// A tuple literal.
    TupleLit(Vec<IrExpr>),
    /// An iterator pipeline (the lowered form of a comprehension).
    IterChain {
        /// The source being iterated.
        source: Box<IrExpr>,
        /// The loop variable.
        var: String,
        /// Optional filter predicate (`if` clause).
        filter: Option<Box<IrExpr>>,
        /// The element-producing expression; `None` = identity.
        map: Option<Box<IrExpr>>,
    },
}

/// Statement shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IrStmtKind {
    /// Introduce a binding.
    Let {
        /// The binding being introduced.
        binding: Binding,
        /// Its initializer.
        value: IrExpr,
    },
    /// Assign to an existing place.
    Assign {
        /// Target place.
        target: IrExpr,
        /// Assigned value.
        value: IrExpr,
    },
    /// Expression evaluated for effect.
    Expr(IrExpr),
    /// Return from the enclosing function.
    Return(Option<IrExpr>),
    /// Conditional.
    If {
        /// Condition.
        cond: IrExpr,
        /// Taken branch.
        then: Vec<IrStmt>,
        /// Fallthrough branch.
        orelse: Vec<IrStmt>,
    },
    /// `while` loop.
    While {
        /// Loop condition.
        cond: IrExpr,
        /// Loop body.
        body: Vec<IrStmt>,
    },
    /// `for` loop over an iterable.
    For {
        /// Loop variable.
        var: String,
        /// Iterable.
        iter: IrExpr,
        /// Loop body.
        body: Vec<IrStmt>,
    },
    /// Raise an exception.
    Raise(Option<IrExpr>),
    /// A guarded scope. `finally` is represented as a **scope-exit obligation**, not a
    /// syntactic template, so a backend may lower it to a drop guard.
    Try {
        /// Protected body.
        body: Vec<IrStmt>,
        /// Handlers, keyed by caught exception type (`None` = bare `except`).
        handlers: Vec<(Option<String>, Vec<IrStmt>)>,
        /// Statements that must run on every exit path.
        finally: Vec<IrStmt>,
    },
    /// `break`.
    Break,
    /// `continue`.
    Continue,
}

/// An IR statement with its propagated effect row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrStmt {
    /// The statement shape.
    pub kind: IrStmtKind,
    /// Effects performed by this statement (including its sub-tree).
    pub effects: EffectRow,
    /// Source position.
    pub span: IrSpan,
}

/// A function in IR form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IrFunction {
    /// Function name.
    pub name: String,
    /// Parameters, each carrying its own inferred facts and obligations.
    pub params: Vec<Binding>,
    /// Declared/inferred return type.
    pub ret: IrType,
    /// Body.
    pub body: Vec<IrStmt>,
    /// Effects propagated from the body — decides `-> Result<..>`.
    pub effects: EffectRow,
    /// Source position.
    pub span: IrSpan,
}

impl IrFunction {
    /// Whether every parameter and the return type can be emitted.
    ///
    /// A `false` here is the honest-gap trigger: something in the signature is
    /// [`IrType::Dynamic`] or [`Ownership::Contended`].
    #[must_use]
    pub fn is_lowerable(&self) -> bool {
        self.ret.is_lowerable() && self.params.iter().all(Binding::is_lowerable)
    }
}

/// A whole module in IR form — the unit the whole-program analyses run over.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct IrModule {
    /// Module label (usually the source path).
    pub name: String,
    /// Functions defined in the module.
    pub functions: Vec<IrFunction>,
    /// Module-level bindings.
    pub globals: Vec<Binding>,
}

// ---------------------------------------------------------------------------
// Refusal
// ---------------------------------------------------------------------------

/// Why a backend refused to lower an IR node.
///
/// The `Mechanically*` / `Unimplemented` split is load-bearing and mirrors
/// `docs/SEMANTIC-IR.md` §4: a mechanically-impossible refusal is permanent and
/// principled and should never be counted against coverage; an unimplemented one is a
/// backlog item. Collapsing them hides the asymptote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum LowerRefusal {
    /// Inference proved the value has no single static type.
    DynamicType {
        /// The name whose type could not be resolved.
        name: String,
    },
    /// Alias analysis could not prove an ownership discipline. Emitting anything here
    /// risks Rust that compiles and misbehaves.
    ContendedOwnership {
        /// The name whose aliasing was unprovable.
        name: String,
    },
    /// The construct cannot be mechanically translated by any transpiler — the
    /// information does not exist in the source program.
    MechanicallyImpossible {
        /// What was refused.
        construct: String,
        /// Why it is permanently impossible.
        why: String,
    },
    /// Real work, no barrier in principle.
    Unimplemented {
        /// What is not yet handled.
        construct: String,
    },
}

impl LowerRefusal {
    /// Whether this refusal is permanent (as opposed to a backlog item).
    #[must_use]
    pub fn is_permanent(&self) -> bool {
        matches!(
            self,
            LowerRefusal::MechanicallyImpossible { .. }
                | LowerRefusal::DynamicType { .. }
                | LowerRefusal::ContendedOwnership { .. }
        )
    }
}

impl std::fmt::Display for LowerRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LowerRefusal::DynamicType { name } => {
                write!(
                    f,
                    "`{name}` has no single static type (rebound across types)"
                )
            }
            LowerRefusal::ContendedOwnership { name } => write!(
                f,
                "aliasing of `{name}` is not provable; refusing to guess an ownership lowering"
            ),
            LowerRefusal::MechanicallyImpossible { construct, why } => {
                write!(f, "{construct}: mechanically impossible — {why}")
            }
            LowerRefusal::Unimplemented { construct } => {
                write!(f, "{construct}: not yet implemented")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_join_identity() {
        assert_eq!(IrType::Unknown.join(&IrType::Str), IrType::Str);
        assert_eq!(IrType::Str.join(&IrType::Unknown), IrType::Str);
    }

    #[test]
    fn equal_types_join_to_themselves() {
        assert_eq!(IrType::i64().join(&IrType::i64()), IrType::i64());
    }

    /// The central honesty rule: a name rebound to a different type has no single
    /// Rust type, and the lattice must say so rather than pick one.
    #[test]
    fn disagreeing_concrete_types_join_to_dynamic() {
        assert_eq!(IrType::i64().join(&IrType::Str), IrType::Dynamic);
        assert_eq!(IrType::Float.join(&IrType::Bool), IrType::Dynamic);
        // Differing integer widths are a real conflict, not a silent widening.
        let i32t = IrType::Int {
            bits: 32,
            signed: true,
        };
        assert_eq!(IrType::i64().join(&i32t), IrType::Dynamic);
    }

    #[test]
    fn dynamic_is_absorbing() {
        assert_eq!(IrType::Dynamic.join(&IrType::Str), IrType::Dynamic);
        assert_eq!(IrType::Str.join(&IrType::Dynamic), IrType::Dynamic);
    }

    #[test]
    fn containers_join_elementwise_and_propagate_dynamic() {
        let a = IrType::List(Box::new(IrType::i64()));
        let b = IrType::List(Box::new(IrType::Str));
        assert_eq!(a.join(&b), IrType::List(Box::new(IrType::Dynamic)));
        assert!(!a.join(&b).is_lowerable());
    }

    #[test]
    fn none_widens_to_optional() {
        assert_eq!(
            IrType::Unit.join(&IrType::Str),
            IrType::Optional(Box::new(IrType::Str))
        );
        let opt = IrType::Optional(Box::new(IrType::Str));
        assert_eq!(opt.join(&IrType::Str), opt);
    }

    #[test]
    fn unknown_is_not_lowerable() {
        assert!(!IrType::Unknown.is_lowerable());
        assert!(!IrType::Dynamic.is_lowerable());
        assert!(IrType::i64().is_lowerable());
    }

    /// The safety property. Unprovable aliasing must absorb every join, so no path
    /// through the lattice can launder a `Contended` into an emittable ownership.
    #[test]
    fn contended_ownership_is_absorbing() {
        for o in [
            Ownership::Owned,
            Ownership::Borrowed { mutable: false },
            Ownership::Borrowed { mutable: true },
            Ownership::Shared,
            Ownership::SharedMutable,
            Ownership::Contended,
        ] {
            assert_eq!(o.join(Ownership::Contended), Ownership::Contended);
            assert_eq!(Ownership::Contended.join(o), Ownership::Contended);
        }
    }

    #[test]
    fn shared_plus_mutation_is_interior_mutability_not_a_move() {
        assert_eq!(
            Ownership::Shared.join(Ownership::Borrowed { mutable: true }),
            Ownership::SharedMutable
        );
        assert_eq!(
            Ownership::Shared.join(Ownership::SharedMutable),
            Ownership::SharedMutable
        );
    }

    /// `a = b` on a list, then mutation: must never come back `Owned`.
    #[test]
    fn aliased_then_mutated_never_lowers_to_a_move() {
        let alias = Ownership::Owned.join(Ownership::Shared);
        let after_mutation = alias.join(Ownership::Borrowed { mutable: true });
        assert_ne!(after_mutation, Ownership::Owned);
        assert_eq!(after_mutation, Ownership::SharedMutable);
        assert_eq!(
            after_mutation.wrap_rust("Vec<i64>").as_deref(),
            Some("Rc<RefCell<Vec<i64>>>")
        );
    }

    #[test]
    fn owned_joined_with_borrow_is_not_provably_single_owner() {
        assert_eq!(
            Ownership::Owned.join(Ownership::Borrowed { mutable: false }),
            Ownership::Contended
        );
    }

    #[test]
    fn contended_refuses_to_emit_any_rust() {
        assert!(!Ownership::Contended.is_lowerable());
        assert_eq!(Ownership::Contended.wrap_rust("Vec<i64>"), None);
        assert_eq!(Ownership::Owned.wrap_rust("i64").as_deref(), Some("i64"));
        assert_eq!(
            Ownership::Borrowed { mutable: true }
                .wrap_rust("i64")
                .as_deref(),
            Some("&mut i64")
        );
    }

    #[test]
    fn effects_union_and_force_result() {
        let mut a = EffectRow::pure();
        a.add(Effect::Io);
        assert!(!a.needs_result());
        let mut b = EffectRow::pure();
        b.add(Effect::Raises(Some("ValueError".into())));
        let joined = a.union(&b);
        assert!(joined.needs_result());
        assert!(!joined.is_pure());
        assert!(EffectRow::pure().is_pure());
    }

    #[test]
    fn unknown_effects_conservatively_force_result() {
        let mut r = EffectRow::pure();
        r.add(Effect::Unknown);
        assert!(r.needs_result());
    }

    #[test]
    fn a_function_with_a_dynamic_param_is_not_lowerable() {
        let mut p = Binding::unresolved("x");
        p.ty = IrType::Dynamic;
        let f = IrFunction {
            name: "f".into(),
            params: vec![p],
            ret: IrType::i64(),
            body: vec![],
            effects: EffectRow::pure(),
            span: IrSpan::new(0, 1),
        };
        assert!(!f.is_lowerable());
    }

    #[test]
    fn a_function_with_a_contended_param_is_not_lowerable() {
        let mut p = Binding::unresolved("xs");
        p.ty = IrType::List(Box::new(IrType::i64()));
        p.own = Ownership::Contended;
        let f = IrFunction {
            name: "f".into(),
            params: vec![p],
            ret: IrType::Unit,
            body: vec![],
            effects: EffectRow::pure(),
            span: IrSpan::default(),
        };
        assert!(!f.is_lowerable());
    }

    /// The impossible/unimplemented distinction must survive into the refusal type —
    /// collapsing it is how an asymptote gets hidden.
    #[test]
    fn refusal_distinguishes_permanent_from_backlog() {
        let perm = LowerRefusal::MechanicallyImpossible {
            construct: "eval".into(),
            why: "program text is not known until runtime".into(),
        };
        assert!(perm.is_permanent());
        assert!(perm.to_string().contains("mechanically impossible"));

        let todo = LowerRefusal::Unimplemented {
            construct: "async def".into(),
        };
        assert!(!todo.is_permanent());

        assert!(LowerRefusal::ContendedOwnership { name: "xs".into() }.is_permanent());
        assert!(LowerRefusal::DynamicType { name: "v".into() }.is_permanent());
    }

    #[test]
    fn ir_round_trips_through_serde() {
        let f = IrFunction {
            name: "add".into(),
            params: vec![Binding {
                name: "a".into(),
                ty: IrType::i64(),
                own: Ownership::Owned,
                obligations: vec![],
            }],
            ret: IrType::i64(),
            body: vec![IrStmt {
                kind: IrStmtKind::Return(Some(IrExpr::pure(
                    IrExprKind::Name("a".into()),
                    IrType::i64(),
                ))),
                effects: EffectRow::pure(),
                span: IrSpan::new(0, 3),
            }],
            effects: EffectRow::pure(),
            span: IrSpan::new(0, 10),
        };
        let json = serde_json::to_string(&f).expect("serialize");
        let back: IrFunction = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(f, back);
    }

    #[test]
    fn method_call_sites_are_where_obligations_come_from() {
        let recv = IrExpr::pure(IrExprKind::Name("x".into()), IrType::Unknown);
        let call = IrExpr::pure(
            IrExprKind::MethodCall(Box::new(recv), "read".into(), vec![]),
            IrType::Unknown,
        );
        match &call.kind {
            IrExprKind::MethodCall(_, m, args) => {
                assert_eq!(m, "read");
                assert!(args.is_empty());
            }
            _ => panic!("expected a method call"),
        }
    }
}
