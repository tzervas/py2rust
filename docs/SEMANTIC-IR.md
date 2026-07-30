# The Semantic IR — architecture, analyses, and the named asymptote

> **Status:** Design `Declared`; `ir.rs` types + ownership lattice `Empirical` (unit-tested).
> This document is the architecture. `crates/py2rust-core/src/ir.rs` is the first slice of it.

## 0. The claim, tested against the code

The brief asked whether py2rust's residual gaps are a **syntax-coverage** problem or a
**semantic-impedance** problem. The code answers it. Two exhibits:

**Exhibit A — there is no representation between AST and text.** The entire lowering
surface is typed `Python AST -> Option<String>`:

```
fn lower_simple_expr_in(expr: &ast::Expr, env: &TypeEnv, expected: Option<&str>) -> Option<String>
```

Every fact a lowering decision needs must be reachable from the node in hand plus a
flat `TypeEnv`. There is nowhere to *put* a whole-program fact, because there is no
value in the pipeline that outlives a single node visit. `Option<String>` cannot carry
"this list is aliased at three call sites."

**Exhibit B — ownership is already being decided, syntactically, and it is already wrong.**
`emit.rs::names_needing_mut` / `scan_mut_names` decide `let mut` by walking for
*reassignment*. That is a **binding-mutation** analysis. Rust's `mut` is a
**value-mutation** requirement. The two coincide only for scalars. Python's
`xs.append(1)` mutates the value and never rebinds the name, so it is invisible to
`scan_mut_names`; `a = b` on a list is a rebind that the scan *does* see but which
carries no information about the aliasing it just created. The analysis is looking at
the wrong edge of the graph. It happens to be right on the numeric corpus, which is why
it has survived.

That is the diagnosis. The gaps are not missing `match` arms. **The per-construct
emitter cannot express these because ownership, exception effects, trait obligations,
and type identity are properties of a whole program, and the architecture's widest unit
of reasoning is one AST node.** Adding emitters makes the corpus wider and the
architecture no more capable. `git show 09e47e2` (list comprehensions, +71 lines in
`emit.rs`) is exactly the shape of work that cannot compound.

**Verdict: thesis upheld, with one correction.** The four gaps named in the brief are
not four problems. They are **one** problem — the absence of a value that survives
between nodes — presenting four ways. That is what makes an IR the right cut rather
than an aesthetic preference: it is the minimum structure in which any of the four can
even be *stated*.

## 1. Prior art: read the snapshot before designing anything

`research/mycelium-transpile-snapshot/` is not a tangent. It is **the second half of
the pipeline the owner is asking for**, already written: a `Rust -> Mycelium`
transpiler (`syn::parse_file` -> `.myc` + `.gap.json`).

What it got right, and what this design keeps:

- **Never-silent dispatch (G2).** Exhaustive dispatch whose fallback arm returns
  `Err(GapReason)`, never a placeholder. py2rust already ported this into `gap.rs`.
- **Guarantee tags (VR-5).** `Declared` / `Empirical` / `Proven`, never upgraded past
  the checked basis. This is the discipline that makes an honest gap a first-class
  output rather than an apology.
- **The two-metric split.** `expressible_fraction` (text was emitted) vs
  `checked_fraction` (a real oracle accepted it). The gap between those two numbers is
  precisely the "clever wrong lowering" surface. Keep both.
- **`symtab.rs` is the tell.** Its module doc is a small essay on cross-module name
  resolution — `self::`/`super::` peeling, root-file-only lexical shadowing,
  cross-phylum keys. That is a **whole-program analysis**, and it exists because the
  `Import` gap class could not be closed any other way. The snapshot's author hit the
  same wall from the Rust side and built exactly one whole-program table to get past
  it. This design generalizes that move instead of rediscovering it per gap class.

What it got wrong, and what this design avoids:

- **It is still `syn -> surface text`.** `emit/patterns`, `emit/derives`, `emit/calls`,
  `type_map.rs`, `prim_map.rs` are a beautifully factored *emitter*, not an IR. The
  factoring is table-driven ("a new mapping is an additive row, never a shared-body
  edit") — genuinely good, and it is the ceiling of that architecture. `symtab.rs` is
  bolted to the side because there was no IR to hang it on.
- **So the snapshot's structure is the mistake to avoid, and its `symtab.rs` is the IR
  you want, in embryo.** Take the honesty model and the whole-program table; leave the
  text-emitter spine.

The decisive point for forward compatibility: the snapshot consumes **Rust**. If
py2rust grows an IR that Rust can also be lowered *into*, then the snapshot's emitter
becomes an **IR -> Mycelium backend** and its `syn` front half is replaced by
`Rust -> IR`. Python and Rust then share one set of analyses. If py2rust stays a direct
emitter, that work starts from zero — which is the brief's stated fear, and it is
correct.

## 2. What the IR is

A **typed, effect-annotated, ownership-annotated** representation. Source-language
agnostic on the way in; target-language agnostic on the way out.

```
Python AST ─┐                                    ┌─> Rust     (backend)
            ├─> [ IR ] ─> analyses ─> [ IR' ] ───┤
Rust (syn) ─┘                                    └─> Mycelium (backend)
```

Four axes of annotation. The IR is a normal expression/statement tree; the *annotations*
are the content.

### 2.1 Types — `IrType`

A lattice, not an enum of Rust spellings. Crucially it has both ends:

- `Unknown` — top; nothing inferred yet.
- `Dynamic` — **inference proved there is no single type.** This is not failure; it is a
  *positive result* and it is the honest lowering trigger. Distinguishing `Unknown` from
  `Dynamic` is the entire reason the lattice exists: one means "ask later", the other
  means "there is no answer, gap it."
- Concrete: `Int{bits,signed}`, `Float`, `Bool`, `Str`, `List(T)`, `Dict(K,V)`,
  `Tuple(..)`, `Set(T)`, `Optional(T)`, `Named(..)`, `TypeVar(..)`, `Fn{..}`.

`join` is the lattice join. Two different concrete types join to `Dynamic` — not to an
arbitrary pick. That single rule is what stops the transpiler guessing when a name is
rebound to a different type.

### 2.2 Ownership — `Ownership`, and why it is the dangerous one

The brief is right that ownership is the worst failure mode, because a wrong choice
**compiles**. `Owned` vs `Rc<RefCell<T>>` are both valid Rust; only one preserves
Python's semantics, and the wrong one is silent.

So ownership is a lattice with an explicit **`Contended`** top:

| Variant | Meaning | Rust |
|---|---|---|
| `Owned` | exactly one live binding | `T` |
| `Borrowed{mutable}` | non-escaping reference | `&T` / `&mut T` |
| `Shared` | multiple readers, no writer | `Rc<T>` |
| `SharedMutable` | multiple holders, ≥1 writer | `Rc<RefCell<T>>` |
| `Contended` | **aliasing not provable** | **gap — emit nothing** |

`Ownership::join` is deliberately conservative and it is the safety property of the
whole design: `Owned ⊔ Owned = Owned`, but `Owned ⊔ Shared = SharedMutable` only when
mutation is witnessed, and **any join involving `Contended` is `Contended`**. Escape
analysis that cannot prove a bound yields `Contended`, and `Contended` **must** produce
a `gap::Category::Ownership` and no code. Refusing is the only correct behaviour: an
honest gap beats code that compiles and misbehaves.

This is the concrete answer to `a = b` on a list. Today the emitter has no opinion and
would emit a move or a clone by accident. Under the IR, `a = b` creates an alias edge;
if either is mutated afterwards and both are live, the join is `SharedMutable`
(`Rc<RefCell<Vec<T>>>` — faithful); if the analysis cannot see all uses (escape through
a call whose callee is unknown), it is `Contended` and it gaps.

### 2.3 Effects — `Effect` / `EffectRow`

Every IR node carries a set: `Raises(exc)`, `Mutates(place)`, `Diverges`, `Io`,
`Await`, `Unknown`. Effects propagate bottom-up to function signatures. A function whose
effect row contains `Raises` lowers to `-> Result<T, E>`; call sites of it become `?`.
This is what turns exceptions from a per-`try`-node emitter problem into an
inter-procedural one — which is what it actually is. `finally` becomes a scope-exit
obligation on the IR block (a drop-guard / `Drop` impl), not a syntactic template.

### 2.4 Obligations — `TraitObligation`

Duck typing is recorded as *usage*: a parameter used as `x.read()` accretes an
obligation `{ method: "read", arity: 0, .. }`. Trait synthesis is then a
straightforward pass — group obligations per parameter, emit a synthetic trait, bind
the parameter to it. Structural-at-runtime becomes nominal-at-compile-time by
**collecting usage across the whole function body**, which is exactly the thing a
node-local emitter cannot do.

## 3. The analyses

All run on IR, all shared by every front end and every backend. This is where the
compounding comes from: an analysis written once serves Python->Rust, Python->Mycelium,
and Rust->Mycelium.

1. **Type inference** — Hindley-Milner-ish over the `IrType` lattice, unification with
   `join`. Output: `Dynamic` where no single type exists.
2. **Escape / alias analysis** — the ownership inference. Points-to graph over IR
   places; a value that escapes its defining scope cannot be `Owned`. Output:
   `Ownership` per binding, `Contended` where unprovable.
3. **Trait synthesis** — collect `TraitObligation`s per unannotated parameter, emit
   synthetic traits.
4. **Exception-effect propagation** — fixed-point over the call graph, `Raises` upward,
   `Result`/`?` downward at lowering.

Ordering: types → effects → obligations → ownership (ownership last; it needs the call
graph effects analysis builds).

## 4. What stays MECHANICALLY IMPOSSIBLE — even with the IR

The asymptote, named rather than hidden. These are not "unimplemented". No IR, no
analysis, and no amount of effort removes them, because the information does not exist
in the source program.

1. **`eval` / `exec` / `__getattr__` / `setattr` with computed names.** The program
   text is not known until runtime. A transpiler cannot lower a program that does not
   yet exist. **Permanent refusal.**
2. **Monkey-patching and open classes.** Python types are mutable at runtime; any
   module may add a method to any class after the fact. Rust's nominal types are closed
   at compile time. Whole-program analysis narrows this only under a closed-world
   assumption that Python does not offer.
3. **Genuinely heterogeneous containers.** `[1, "a", None]` where elements are used at
   their distinct types. An enum lowering changes the type identity observable via
   `isinstance`; a `Box<dyn Any>` lowering changes performance and destroys pattern
   matching. Both are lies. `Dynamic` + gap is the honest answer.
4. **Reference cycles with deterministic finalization.** Python's GC collects cycles;
   `Rc` leaks them. `Weak` placement requires knowing programmer *intent* about which
   edge is the back-edge. Not recoverable from source.
5. **Exception identity and traceback fidelity.** `raise X from Y` chaining, and
   `finally` blocks that themselves raise while unwinding, have no Result-typed
   equivalent that preserves both the values and the ordering. Approximable; not
   faithful.
6. **Ownership under dynamic aliasing.** `globals()[name] = obj`. The alias graph is
   not statically knowable. Correctly yields `Contended`.

**Merely unimplemented** — real work, no barrier: classes/inheritance -> structs+traits,
decorators (the static subset), generators -> `Iterator` impls, `async` -> futures,
most comprehensions, f-strings, context managers -> `Drop`, module system -> the
snapshot's `symtab.rs` generalized.

The distinction is load-bearing and must survive into the gap taxonomy: a
`Mechanicallyimpossible` gap is a permanent, principled refusal and should never be
counted against coverage. An `Unimplemented` gap is a backlog item.

## 5. Migration path — no rewrite, both live, provably agreeing

The IR must land **beside** the existing emitter, not under it. Five stages; the
existing emitter is the oracle throughout and is only removed when it is provably
redundant.

**Stage 0 — types only (this commit).** `ir.rs` compiles, is unit-tested, is referenced
by nothing. Zero regression risk by construction: no existing code path changes.

**Stage 1 — shadow mode.** Add `Python AST -> IR` for the constructs `emit.rs` already
handles, plus an `IR -> Rust` backend. Both run. The IR's output is **discarded**; only
the existing emitter's output is written. A `differential` test asserts
`ir_backend(lower(ast)) == emit(ast)` over the whole fixture corpus. Divergence fails
CI. This is the proof the IR is faithful, and it costs nothing in production behaviour
because the IR path is not yet authoritative.

**Stage 2 — construct-by-construct promotion.** A per-construct switch (`IrRouting`)
flips one construct at a time to IR-authoritative, only after it has been byte-identical
in shadow mode across the corpus for that construct. The switch is the rollback.

**Stage 3 — IR-only constructs.** New work (ownership, exceptions, traits) lands
*only* on the IR path; `emit.rs` gaps them as it does today. The IR strictly dominates:
it is identical where both exist and strictly better where only it exists.

**Stage 4 — retire `emit.rs`** when every construct is promoted and the differential
corpus is empty of emitter-only paths.

**How agreement is proven.** Three tiers, in increasing strength:
- **Textual** — byte-identical output over the corpus (Stage 1 gate). Strongest
  available signal, cheap, and catches everything for constructs both paths cover.
- **Structural** — normalize both outputs (`prettyplease`/`cargo fmt`) before comparing,
  so whitespace divergence does not mask or manufacture failures.
- **Behavioural** — the snapshot's own `checked_fraction` idea, retargeted: run `rustc`
  (`check.rs` already exists) on both outputs and compare diagnostics; where a Python
  fixture has known-value semantics, run it and compare. This is the tier that catches a
  *semantically* wrong lowering that happens to be textually stable.

Additionally: the gap report is part of the contract. Shadow mode compares `.gap.json`
too — the IR path must not silently close a gap the emitter honestly reported.

## 6. Forward compatibility — the actual point

With the IR in place:

- `Python -> IR` (front end, py2rust)
- `Rust -> IR` (front end, `syn`-based — the snapshot's front half, retargeted)
- `IR -> Rust` (backend)
- `IR -> Mycelium` (backend — the snapshot's `emit/*` retargeted; `myc_map.rs` and
  `docs/PY2MYC_MAP.md` become IR-keyed rather than Python-construct-keyed)

Four components, four language pairs, and every analysis in §3 written once. Adding a
backend is additive. Adding a front end is additive. **Without** the IR, `Rust ->
Mycelium` shares exactly nothing with `Python -> Rust` — not the type lattice, not the
ownership analysis, not the gap taxonomy — and the ~97 mycelium-* repos get a second
transpiler with a second set of bugs. That asymmetry is the whole argument.

One caution, stated honestly: an IR biased toward Rust's semantics will make
`IR -> Mycelium` awkward in exactly the places Mycelium differs from Rust. The
`Ownership` lattice above is Rust-shaped. It is kept anyway, because Mycelium is itself
an ownership language and the snapshot's `type_map.rs`/`prim_map.rs` show the mapping
is close; but this is a `Declared` judgement, not a measured one, and the first real
`IR -> Mycelium` slice is what would upgrade or refute it.
