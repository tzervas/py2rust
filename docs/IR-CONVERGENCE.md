# IR Convergence: py2rust + mycelium-transpile → one universal IR

**Status:** design note, evidence-based. Every claim below cites a file I read in this
session; where I am inferring rather than citing, the sentence says **(inference)**.

**Target being served:** many languages IN → one universal IR → Rust and/or Mycelium OUT,
native and idiomatic for the target, without losing the intent of the source, with dials for
strictness / house rules / shop preferences. First ingest: Python, TypeScript, Rust. First
output: Rust.

**Sizes as measured** (`wc -l`): py2rust-core = 6,202 lines over 11 files;
mycelium-transpile = 14,370 lines over 29 files (`src/emit.rs` alone is 6,429). B is ~2.3x A.

---

## 0. The headline finding, stated before the detail

**Neither repo has an IR.** Both are *direct AST → String* emitters:

- `crates/py2rust-core/src/emit.rs:24` — `pub fn emit_function(func: &ast::StmtFunctionDef,
  source: &str) -> Emitted`. The Rust backend's input type is *rustpython's Python AST*.
- `/workspace/mycelium-transpile/src/emit.rs:2795` — `pub fn emit_expr(expr: &Expr,
  self_ty: Option<&str>, env: &TypeEnv) -> Result<String, GapReason>`. The Mycelium backend's
  input type is *syn's Rust AST*.

So the observation in the brief — "the pair already contains both halves of a Rust round-trip"
— is **true at the capability level and false at the type level**. B can parse Rust; A can
print Rust. But A's printer cannot be handed B's parse tree, because A's printer is written
against `rustpython_parser::ast`, not against any neutral node type. The round-trip is not
free. It is, however, *the cheapest possible first IR consumer*, and everything around it —
the gap ledger, the denominators, the `rustc` oracle, the vet loop — already exists. See §4a.

**Everything else in this document is more solid than that observation was.** In particular the
two independently-evolved `gap.rs` files are strong, real evidence about IR shape (§1.4, §3.6).

---

## 1. Comparative anatomy

| Concern | A — py2rust (Python→Rust) | B — mycelium-transpile (Rust→Mycelium) | Better, and why |
|---|---|---|---|
| **Parse** | `parse.rs:1-60`. `rustpython-parser` → `ParsedModule { file_label, source, body: Suite }`. Carries the *source text* alongside the AST — that is what makes byte-offset provenance possible downstream. Also computes `count_statements` (`parse.rs:56+`), the **L2 denominator**. | `transpile.rs:26/56` — `syn::parse_file`. No separate parse module; parse is inlined at the driver. No statement-level denominator; `GapReport.total_top_level_items` is `syn::File::items.len()` (`gap.rs`, `GapReport` doc). | **A**, decisively, for one reason that is not about parsing at all: A measured its own denominator honestly and B did not. `parse.rs:48-55` records the measurement — on `tg-agent-relay`, 1,987 top-level statements vs 10,955 total, i.e. **81.9% of code lives inside bodies**; reporting against top-level "flatters the transpiler by roughly 5x." B still reports against top-level items only. |
| **Symbol table** | **None.** No cross-file name resolution anywhere in the 11 files. | `symtab.rs` (678 lines). Batch-scoped Rust-module-path → Mycelium-nodule map plus the *emitted* names of each sibling (`symtab.rs:1-8`). Handles `crate::`/`self::`/`super::` peeling, bare-head uniform-path ambiguity with **root-file-only lexical shadowing** (`symtab.rs:20-40`), cross-crate ("cross-phylum") resolution, and an explicitly-scoped-out set (renames, globs). | **B**, no contest. This is the single largest thing B bought with its extra 8k lines and it is *not* language-pair-specific — it is name resolution, which every frontend needs and no target can fake. Note the discipline in `symtab.rs:1-6`: a resolved name must have been **emitted** by the sibling's own pass, not merely *present* in the sibling's source. That rule survives into the IR verbatim. |
| **Type mapping** | `map.rs:1-60`, `map_type_expr(&ast::Expr) -> Option<String>`. Returns a **Rust type string**. Handles `list`/`set`/`dict`/`Optional`/PEP-604 `X \| Y` → `Option` when one arm is `None`, and refuses to invent enums for real unions (`map.rs:18`). `pathlib.Path` → `std::path::PathBuf`. | `map.rs` (496) + `type_map.rs` (246) + `prim_map.rs` (495). `type_map.rs:44-55` factors the fixed-name arms into a `&[TypeMapRow]` data table with `rust_name`, a `fn`-pointer mapping, and a **citation** field per row. Structural arms (generic application, passthrough) deliberately stay in the visitor (`type_map.rs:18-25`). | **B** for structure, **A** for one specific instinct. B's row table with a per-row citation is the right shape for a multi-frontend IR (add a language, add rows, never edit a shared body). But A's refusal at `map.rs:18` — *do not synthesize an enum for a real union* — is the correct default and is precisely the decision TypeScript will force (§3.4). Both map to **strings**, which is the coupling the IR must break. |
| **Gap reporting** | `gap.rs`. 11 categories (`gap.rs:28-52`), all Python-surface. `Gap { file, line, col, category, python_construct, snippet, reason, item_name }` (`gap.rs:188-198`). Two-level denominator (L1 top-level, L2 all statements) with `statement_fraction() -> Option<f64>` returning **`None` when unmeasured** so "not measured" can never render as 0% (`gap.rs:~290`). Bridges 1:1 to `interface::PyConstruct`. | `gap.rs`. **22 categories** (`gap.rs:17-110`) with paragraph-length rationales each. Adds `excluded_from_denominator()` (`TestItem`, `ModuleDecl`) and `is_non_gap_advisory()` (`DeriveSatisfied`) — and the doc at `gap.rs:~265` explains at length why `NamedFieldDrop` is deliberately *not* advisory (it names a real, non-recoverable loss: field names). Plus a `thread_local` `RecursionBudget` with `guarded()` (`gap.rs:~300-363`) turning stack-overflow `SIGABRT` into a `Category::RecursionBudget` gap. | **Split, and the split is the interesting part.** B's *taxonomy machinery* is better: the denominator-exclusion and advisory-vs-real-loss distinctions are genuine epistemics, and A imported the shape but left both predicates returning `false` (`gap.rs:70-79`) — i.e. A has the API and no live policy. A's *numerator* is better: A measures statements, B measures items. The IR needs **both** — B's category algebra over A's denominator. |
| **Emission** | `emit.rs` (1,361), one file. `Emitted` at `:11`. Real lowering: `try_lower_body:335`, `lower_if:521`, `lower_while:560`, `lower_for:585`, plus a mutability inference pass `names_needing_mut:248` / `mark_loop_rebinds:291` and numeric-width promotion `forced_ty:678` / `promote:705`. | `emit.rs` (6,429) + `emit/{patterns,derives,calls,macros}/` (~1,900 more). `emit_expr:2795`, `emit_block_as_expr:2029`, and thread-local `EmitCtx` plumbing (`with_emit_ctx:397`, `cross_nodule_resolve:946`, `claim_bare_fn_name:1188`). | **B** structurally, **A** for a capability B does not need and therefore does not have. B's decomposition is the mature answer (see next row). But `names_needing_mut`/`mark_loop_rebinds` is **the only mutability/ownership inference in either repo**, and it exists in A because Rust is a target with `mut` and Python has no such notion. That pass is a prototype of the IR's ownership-annotation synthesis (§3.3). |
| **Pattern / derive / call handling** | Absent as separate concerns. Python has no `match` surface being lowered here, and derives don't exist. | `emit/patterns/mod.rs:1-27` and `emit/calls/mod.rs:1-20`: static per-axis handler tables pairing a **pure recognizer** (`fn(&syn::Pat) -> bool`, no ctx, no emission) with the lowering that owns it; the driver consults the table first and falls through to its own explicit gap on a miss (never a silent drop). `emit/derives/` has 5 real derive lowerings (`eq.rs` 426, `show.rs` 306, `ord.rs` 264, `hash.rs` 211, `init.rs` 107). | **B**, and this is the second big thing the extra lines bought. The recognizer/lowering split with a *pure, context-free recognizer* is exactly the raise-then-lower architecture in miniature (§4) — B built the pattern-matching half of intent recognition and just didn't call it that. **Caveat:** `emit/derives/*` is a Rust→Mycelium-specific asset (Mycelium's value semantics make `Clone`/`Copy` a no-op — `gap.rs`'s `DeriveSatisfied`). The *table shape* generalizes; the *derive bodies* do not. |
| **Batch driving** | `batch.rs` (1,449 — A's largest file). `discover_py_files`, `transpile_batch_with(BatchOptions)`, `UnionGapReport`, plus `render_priority_report` / `render_ranked_report` — it **ranks** what to fix next. | `batch.rs` (442). `discover_rs_files`, `transpile_batch`, `UnionGapReport`, `summarize`. Crucially a **two-pass** driver: pass 1 collects each file's emitted names into the symtab, pass 2 emits with cross-file resolution (`symtab.rs:4-6`). | **Split.** B's two-pass architecture is required and A cannot resolve imports without adopting it. A's ranked/priority reporting is the operator-facing half B lacks — it turns a gap ledger into a work queue. Both survive; they are orthogonal. |
| **Verification / vetting** | `check.rs` (501). **L3: run the real `rustc`** (`--emit=metadata`, `--error-format=json`, per file, no cargo — `check.rs:33-40`). And the sharpest idea in either repo: it reports *two* numbers, because "compiles" is trivially satisfiable by emitting only signatures with `todo!()` bodies (`check.rs:10-26`). `L3Status::NotRun` is explicitly **not** failure (`check.rs:28-32`). | `vet.rs` (728). Runs the real `myc check` oracle over emitted `.myc`, folds into `VetRecord`, reports `checked_fraction` **alongside** (never in place of) the emission-only `expressible_fraction`. Numerator is **file-gated**: a file contributes 0 unless its *entire* emission is clean — "we never guess which item broke a failing file". `VetClass::ToolUnavailable` is never counted clean. Adds a phylum-mode dual-report for cross-nodule imports. | **Dead tie, and both must survive.** These are the same idea invented twice: *run the real downstream toolchain, and never let an unmeasured run render as a good number.* A's contribution is the `todo!()`-counting insight (a compiling file full of stubs is a header file, not a port). B's is the file-gated conservative numerator and the tool-unavailable class. **Together they are the IR's acceptance harness** (§4a). |

### 1.1 What B bought with 2.3x the lines, itemized

1. **Cross-file name resolution** (`symtab.rs`, 678) — a capability, not polish.
2. **Emit decomposition into recognizer tables** (`emit/{patterns,derives,calls,macros}/`, ~1,900) — turns "edit the shared 6k-line match" into "append a row + a file."
3. **A DRY dispatch layer** (`visit.rs`, 202) whose module doc quantifies the tax it removed: *five* independently hand-rolled matches over `syn::Expr`/`syn::Type`, one of which (`field_type_user_deps`) was a self-admitted drifting mirror of another (`visit.rs:5-18`).
4. **A real downstream oracle loop** (`vet.rs`, 728) with a conservative numerator.
5. **A provenance ledger** (`remap.rs`, 282) — see §4.
6. **A 22-category gap taxonomy with denominator epistemics** vs A's 11 with the predicates stubbed off.
7. **Reserved-word collision handling** (`reserved.rs`, 363) — target-specific, largely non-transferable, but the *principle* (a target identifier that would fail to parse is a gap, never an auto-rename) is universal.

Roughly: ~2,600 lines of it is genuinely transferable architecture, ~1,900 is Mycelium-specific
lowering (derives, reserved words, prim_map), and the rest is `emit.rs` bulk. **(inference,
from file sizes and the module docs — I did not read all 6,429 lines of `emit.rs`.)**

---

## 2. What each proved — the transferable lessons

### 2.1 Survives into the shared design (from B)

- **`visit.rs`'s visitor-over-one-canonical-match.** The `ExprVisitor`/`TypeVisitor` traits with
  one method per shape, all defaulting to `fallback` (`visit.rs:44-60`), mean a consumer that
  cares about 3 shapes overrides 3 and *inherits never-silence for the rest*. With N frontends
  and M backends this stops being a nicety and becomes the only way to keep them in sync. **Take
  it whole.**
- **`symtab.rs`'s resolution discipline**, specifically the two rules: (a) a name resolves only
  if the defining unit actually *emitted* it, and (b) a resolution that cannot be confirmed is a
  gap, never a guess (`symtab.rs:44-52` on `use`). Rule (a) is the one that will keep a
  multi-file IR honest.
- **Recognizer/lowering separation with pure recognizers** (`emit/patterns/mod.rs:4-8`). The
  recognizer must not inspect context or mutate state. This is the constraint that makes intent
  recognition (§4) safely reorderable.
- **Data-table-with-citation** (`type_map.rs:44-55`). Every mapping row carries its own
  justification. In a universal IR, every lowering row should cite *which target's* spec or
  corpus justifies it.
- **`gap.rs`'s three-way classification of a gap**: real coverage loss / denominator-excluded
  non-surface / non-gap advisory. And the `NamedFieldDrop`-vs-`DeriveSatisfied` argument at
  `gap.rs:~265` is the template for reasoning about every future borderline case.
- **`remap.rs`'s `IdiomClass { Mechanical, Heuristic, Judgment }`** — see §4/§5.

### 2.2 Survives into the shared design (from A)

- **`check.rs`'s two-number rule.** *Compiles* and *has no unlowered bodies* are different
  claims and reporting only the first is a lie by omission (`check.rs:10-26`). Generalize:
  every backend must report **accepted-by-target-toolchain** AND **stub-free**.
- **`L3Status::NotRun` / `VetClass::ToolUnavailable`.** Both repos independently insisted an
  unrun oracle is not a failing oracle. Bake it into the IR's report type as a *three*-valued
  status, never `bool`.
- **The L2 denominator** (`parse.rs:48-55`, `gap.rs:~275-295`). Coverage must be measured
  against *all* statements/expressions, and `statement_fraction() -> Option<f64>` returning
  `None` when unmeasured is the right type. Port this to B, which currently over-reports.
- **`source_loc.rs`** (60 lines, the smallest file in either repo, and load-bearing).
  `line_col(source, byte_offset)` + `snippet(source, start, end, max)` with UTF-8 boundary
  correction and newline collapsing. Provenance is *cheap* and both repos already prove a
  gap record with `{file, line, col, snippet}` is what makes a port auditable.
- **`interface.rs`'s pre-planned mapping alphabet** with `Guarantee { Exact, Empirical,
  Refusal }` and `Citation { Corpus, Test, Unverified }` (`interface.rs:1-26`). This is a
  *contract module* explicitly declared off-limits to other lanes (`interface.rs:8-10`) and it
  forces total coverage: every construct in the alphabet is Mapped or explicitly Unmappable.
  This is the right way to make an IR's node set assessable rather than aspirational.
- **The mutability inference pass** (`emit.rs:248-334`) — the seed of §3.3.

### 2.3 Accidents of the language pair — do NOT bake into the IR

- **B:** `reserved.rs` entirely (Mycelium keyword collisions); `emit/derives/*` bodies (Mycelium
  value semantics make `Clone`/`Copy` vacuous); `Category::{NamedFieldDrop, ModuleDecl,
  DeriveSatisfied, Conversion}` (all four are artifacts of Mycelium's positional-only
  `constructor` grammar, nodule-per-file model, ADR-003 value semantics, and DN-41 width casts);
  `prim_map.rs`; the nodule-path derivation in `transpile.rs:1100-1135`.
- **A:** `Category::{Class, Lambda, Comprehension, Metaprogramming, DynamicTyping}` as
  *categories* — these are Python surface names. The IR must not have a "comprehension" node;
  it must have a **map/filter/reduce intent** node that a comprehension raises into (§4).
  `myc_map.rs` (a Python→Mycelium shortcut table) is a point solution.
- **Both:** `Category::MultiStmtBody` is the most revealing shared category — it exists in *both*
  taxonomies (A `gap.rs:47`, B `gap.rs:22`) and in both cases means "the target's expression
  grammar is narrower than the source's statement grammar." That is a **backend capability
  fact**, not an IR node. In the IR it becomes a lowering-time refusal, not a category.

### 2.4 The convergent-evolution evidence, read carefully

Categories present in **both** taxonomies, independently derived: `Import`, `MultiStmtBody`,
`Other`, and the *structure* around them — `Gap { file, line, col, category, snippet, reason,
item_name }` is field-for-field identical in both (A `gap.rs:188-198`, B `gap.rs:180-196`), as
is `GapReason { category, reason }` as a pre-materialization carrier, as is
`GapReport { source, emitted_items, gaps, total_top_level_items }`.

A's `gap.rs:1-5` admits it: *"Shape ported from `research/mycelium-transpile-snapshot/src/gap.rs`."*
So the *struct* convergence is descent-with-modification, not independent invention — I must not
over-claim it. **What is genuinely independent** is that A, having copied the shape, then
*diverged the taxonomy completely* (11 Python categories, zero overlap with B's 22 except
`Import`/`MultiStmtBody`/`Other`) and *added* a dimension B lacks (L2 statement denominator),
while B added two dimensions A lacks (denominator exclusion, advisory-vs-loss).

**Therefore, the honest reading:**
- **Intrinsic** (in both, and mechanism-forced): the gap *record shape* — category + free-text
  reason + `{file, line, col}` + snippet + optional item name; the *never-silent invariant*
  (every unit is emitted, gapped, or both); the *catch-all `Other` with free text*; the
  *two-tier* structure of item-level gaps carrying sub-gaps.
- **Language-pair-specific** (must not be baked in): every named category on both sides except
  `Import` and `Other`. `Import` survives because *name resolution across compilation units* is
  universal. Everything else is surface vocabulary.
- **The real universal**: a gap is `(provenance, severity-class, reason, recoverability)` where
  severity-class is B's three-way split. The *vocabulary* of what got gapped is per-frontend and
  per-backend and must live in extensible registries, not a closed enum in the IR crate.

---

## 3. The common IR spec

Working name: **`transpile-ir`**. Crate-level goal: no frontend type and no backend type appears
in its public API.

### 3.1 Node layer (structure)

Three strata, deliberately separated:

```
Module ─ Item* ─ Body(Region) ─ Op*
                                 ├─ Core ops    (universal, total)
                                 ├─ Intent ops  (raised; §4)
                                 └─ Opaque op   (never-silent escape hatch)
```

- **`Item`** — `Func`, `TypeDef { Record | Sum | Alias | Newtype }`, `Const`, `Impl`/`Conform`
  (a set of methods conforming a type to an interface), `Import`, `Module`.
- **`Region`** — an ordered list of `Op`s with explicit block params and terminators. *Not* a
  statement list. Reason: A's `Category::MultiStmtBody` and B's `Category::MultiStmtBody` both
  exist because a statement-shaped IR cannot lower into an expression-shaped target. A region
  with explicit terminators is neutral to that distinction and lets the backend decide.
- **`Op`** — `Let`, `Assign`, `Call`, `MethodCall`, `FieldGet/Set`, `Index`, `If`, `Loop`,
  `Match`, `Return`, `Break/Continue`, `Construct`, `Literal`, `Closure`, `Raise`, `Try`,
  `Await`, `Yield`.
- **`Op::Opaque { text, frontend, reason }`** — the never-silent escape. Carries the original
  source text. A backend that meets one **must** gap it (borrowed from the discipline both
  `dispatch.rs:42-49` and `transpile.rs:630`'s `dispatch_item` already enforce).

**Frontend-local; must be lowered away, never represented in the IR:**

| Source construct | Lowered to |
|---|---|
| Python `class` + MRO/inheritance | `TypeDef::Record` + `Conform` blocks; MRO resolved at raise time into concrete method targets. The IR has no inheritance. |
| Python comprehension / generator expr | `Intent::Map`/`Filter`/`Collect` pipeline (§4). A's `lower_list_comp:895` already does the one-generator case ad hoc. |
| Python decorators | Either an `Attribute` on the item (recognized set) or `Opaque`. Never an IR node. |
| Python `with` | `Try` region + explicit acquire/release ops. |
| Python duck-typed call | `Call` against a **synthesized structural interface** (§3.4). |
| TS `interface` / object type literal | `TypeDef` with `StructuralShape` (§3.4) — the *same* node the duck-typing path produces. |
| TS `enum` | `TypeDef::Sum` with all-unit variants + const table. |
| TS `namespace` / declaration merging | Flattened at raise time into `Module`. |
| TS `async`/`Promise<T>` | `Effect::Async` on the signature + `Await` op. Not a type. |
| TS `any` | `Type::Dynamic` **with a mandatory provenance record** — see §3.2. |
| TS decorators, JSX | `Opaque`. |
| Rust lifetimes `'a` | `Region`-scoped `Ownership::Borrow { mutable, region_id }`. Named lifetimes are a *Rust surface encoding* of a constraint graph; the IR carries the graph. |
| Rust `impl Trait` / generic bounds | `Type::Var` + `Constraint` list. |
| Rust macros | `Opaque` unless in a recognized set (`format!`, `vec!`, `write!` — B already has `emit/macros/write_format.rs`, 298 lines). |
| Rust `mod foo;` | Module-graph edge, not an item. (B learned this the hard way — `Category::ModuleDecl`, `gap.rs:~75-88`.) |
| Rust `#[derive(..)]` | `Conform` request with a named protocol. Whether the backend must *generate* it (Rust: yes) or it is vacuous (Mycelium: `DeriveSatisfied`) is a **backend** fact. |

### 3.2 Type layer

```
Type ::= Prim(Bool|Int{signed,width}|Float{width}|Char|Str|Unit|Never)
       | Nominal { id, args }              -- declared, identity by name
       | Structural { shape: StructuralShape }   -- §3.4
       | Var { id, constraints: [Constraint] }
       | Fn { params, ret, effects }
       | Sum { variants }  | Tuple | Seq | Map | Set
       | Opt(Type)                          -- distinct from Sum, backends differ
       | Result(Ok, Err)                    -- distinct, see effects
       | Dynamic { provenance: DynProvenance }
```

`Dynamic` is the load-bearing one. Python unannotated params, TS `any`, and TS `unknown` all
land here, and **they are not the same thing**, so `DynProvenance` records which:
`Unannotated | ExplicitAny | Unknown | InferenceFailed { reason }`. A's `map.rs` header already
insists unmapped types become gaps "never guessed as silent `Any`"; the IR keeps that by making
`Dynamic` *carry why*, so a strictness dial (§5) can reject `Unannotated` while permitting
`ExplicitAny` — a distinction you cannot make if you collapse both to one node.

`Opt` and `Result` are separate from `Sum` because they are the two places where every backend
has a native idiom and getting them wrong destroys idiomaticity. A already special-cases
PEP-604 `X | None` → `Option` (`map.rs:18`).

### 3.3 Effects, ownership, aliasing

**Effects** are on `Fn` signatures and on `Op`s, as a lattice, not a bool:

```
Effect ::= Pure | Alloc | Read(Resource) | Write(Resource)
         | Throws(TypeSet)   -- Python raise, TS throw, Rust panic
         | Fallible(ErrType) -- Rust Result, TS union-with-error
         | Async | Diverges | Unsafe
```

`Throws` vs `Fallible` must be distinct. Python's `except ValueError` and Rust's
`Result<_, E>` are the same *intent* (this can fail, here is how) with opposite *mechanics*
(unwinding vs value). A's `Category::Exception` (`gap.rs:35`) is currently a hard refusal
precisely because it has nowhere to put this. With `Throws` in the IR, a Rust backend can lower
`Throws(E)` → `Result<T, E>` + `?` at call sites under an idiom dial, or → `panic!` under a
strict-fidelity dial. That single node is the largest coverage unlock available to A.

**Ownership/aliasing** — an *annotation lattice*, computed by an IR pass, never required of a
frontend:

```
Ownership ::= Unknown | Owned | Borrow { mutable: bool, region: RegionId }
            | Shared { thread_safe: bool }   -- Rc vs Arc
Aliasing  ::= Unique | MayAlias | MustAlias
Mutation  ::= Never | Local | Escaping
```

`Unknown` is the honest default for Python/TS frontends. The Rust frontend fills it in from
syntax (`&`, `&mut`, `Rc`, `Arc`) — it is the only frontend that *knows*. An IR pass
(`ownership_infer`) upgrades `Unknown` toward `Owned`/`Borrow` and is the natural home for
A's `names_needing_mut` / `mark_loop_rebinds` / `collect_assign_targets`
(`emit.rs:248-334`), which today compute `Mutation` under a different name and throw the
result straight into a string. **Aggressiveness of this pass is a dial (§5)** — it is the
single most likely source of silently-wrong output, so its default must be conservative and its
output must be recorded as an `IdiomChoice` (§4).

### 3.4 Structural types — the TypeScript stress test

TS sits between Python's duck typing and Rust's nominal traits, so it is the right forcing
function. The IR carries:

```
StructuralShape {
  members: [ { name, ty, optional: bool, readonly: bool } ],
  call_sigs: [FnType],       -- callable objects
  index_sigs: [(KeyTy, ValTy)],
  origin: ShapeOrigin,       -- Declared{ts_iface} | Inferred{from_use} | DuckSite{py}
}
```

The claim to test: **all three languages produce the same node.**
- TS `interface Reader { read(n: number): string }` → `Declared` shape.
- Python `def f(r): return r.read(n)` → `DuckSite` shape inferred from member access at the
  call site. Same members, weaker provenance.
- Rust `trait Reader { fn read(&self, n: usize) -> String }` → *`Nominal`* with a
  `StructuralShape` attached as its **witness**.

Lowering to Rust then has exactly one algorithm — **trait synthesis**:
1. If a `Nominal` in scope has a witness shape that is a supertype of the required shape, use it
   (this is how a Python duck-typed function ports to `impl std::io::Read`).
2. Else synthesize `trait __Shape_N` + blanket `impl`s for the concrete types observed.
3. Else, if the shape has exactly one concrete inhabitant in the module, monomorphize to it.
4. Else gap it, with the shape printed.

Steps 1–3 are ranked by an **idiom-aggressiveness dial**; step 4 is the never-silent floor.
TS union types `A | B` lower to: `Option` if one arm is `null`/`undefined` (A's `map.rs:18` rule,
generalized), `Result` if one arm is an error-shaped type, else a synthesized `enum` **only if
the idiom dial permits** — otherwise a gap, because A's instinct to refuse to invent enums is
correct as a *default*, not as a law.

If the IR handles this, it handles Python and Rust as degenerate cases (Python = all shapes
`DuckSite`, Rust = all shapes attached to `Nominal`s). **That is the test to run first.**

### 3.5 Provenance — non-negotiable, and it must reach the output

Every `Item`, `Op`, and `Type` carries:

```
Provenance { unit: UnitId, span: ByteSpan, frontend: FrontendId,
             derived_from: Option<NodeId>,   -- set by every raise/lower pass
             pass_chain: SmallVec<PassId> }
```

`source_loc.rs:3-23`'s `line_col` is the byte-offset→(line,col) resolver, and
`source_loc.rs:26-45`'s `snippet` is the excerpt renderer — both already correct including UTF-8
boundary walking. Lift that file into the IR crate essentially unchanged.

**"Must survive to the output"** means concretely, and this is the auditability requirement:
1. Every emitted target item gets a comment/attribute naming source file + line range.
2. Every gap record already has it (both repos).
3. Every applied idiom transformation records `(source_span, target_span, decision, reason)` —
   which is *exactly* B's `IdiomChoice` (`remap.rs:96-105`).
4. A reviewer can go from any line of output to the source line that caused it. Without this,
   a port is unreviewable and therefore untrustworthy at scale.

### 3.6 The IR's own gap model

```
GapRecord { provenance, class: GapClass, reason: String,
            source_construct: ConstructId,   -- registry, per-frontend
            target: Option<BackendId>, recoverability: Recoverability }

GapClass ::= Refusal          -- nothing emitted; counts against coverage
           | PartialEmit      -- some emitted; sub-gaps attached
           | FidelityLoss     -- emitted, information lost (B's NamedFieldDrop)
           | Advisory         -- emitted, nothing lost (B's DeriveSatisfied)
           | OutOfScope       -- not translatable surface (B's TestItem/ModuleDecl)

Recoverability ::= Mechanical | NeedsDial(DialId) | NeedsHuman | Impossible
```

`GapClass` is B's three predicates promoted to a first-class enum (its `excluded_from_denominator`
and `is_non_gap_advisory` become derived: `OutOfScope` excludes from denominator, `Advisory` is
excluded from headline totals — the exact rules at B's `gap.rs:~250-270`). `ConstructId` is a
**registry key, not an enum variant** — that is how you avoid baking Python's `Comprehension` or
Mycelium's `Conversion` into a universal crate. `Recoverability` is new to both and is what makes
the gap ledger a work queue (A's `render_priority_report` gets a real sort key).

Coverage reporting keeps A's two denominators (L1 items, L2 ops) with `Option<f64>` for
unmeasured, plus B's exclusion arithmetic, plus both oracles' three-valued status.

---

## 4. Raise-then-lower

**Where intent recognition sits:**

```
source ─frontend─> Surface IR ─RAISE─> Intent IR ─LOWER─> Target IR ─backend─> text
                    (faithful,           (semantic,        (target-native
                     1:1, lossless)       idiom-free)        idioms chosen)
```

- **Raise** runs on Surface IR and is *target-independent*. It recognizes
  `Intent::{Reduce, Map, Filter, Scan, MatMul, Broadcast, Transpose, Gather, Scatter, ElemWise,
  Reshape, Contract}` plus the plumbing intents `{Iterate, Accumulate, EarlyExit, ResourceScope,
  Retry}`. A `for` loop that accumulates a sum becomes `Intent::Reduce { op: Add, init, over }`.
  For the stated domain corpus (AI, ternary, quantum, VSA, dense embeddings) this is the whole
  ballgame: a hand-written triple loop in Python and a `ndarray` call are the *same intent*, and
  only the intent form ports to an idiomatic Rust `ndarray`/`faer`/SIMD backend.
- **Lower** is target-specific and picks the idiom: `Intent::Reduce` → Rust
  `.iter().fold(..)` or `.sum()` or a `rayon` par-reduce or a raw loop, chosen by dials.
- **Raising is where intent is preserved; lowering is where nativeness is achieved.** Neither is
  optional, and doing them in one step is what makes both current repos target-locked.

**What already exists:**

| Capability | Status |
|---|---|
| Pure, context-free **recognizers** separated from lowerings | **Exists in B**, `emit/patterns/mod.rs:4-8` and `emit/calls/mod.rs:1-20`. This is the raise/lower split's mechanism, applied to syntax rather than semantics. Directly reusable as the pass framework. |
| **Recording** which idiom was chosen, with alternatives + reason | **Exists in B as a schema**, `remap.rs:96-105` (`IdiomChoice { target_span, rust_span, decision, class, chose, alternatives, reason }`) with `IdiomClass { Mechanical, Heuristic, Judgment }` at `remap.rs:64-69`. But `remap.rs:19-25` says plainly the vec starts **honestly empty** — the schema landed, the instrumentation did not. So: the ledger for raise-then-lower exists and is unpopulated. |
| A single, narrow **intent recognition** | **Exists in A**, `emit.rs:876` `is_simple_list_comp_shape` (recognizer) + `emit.rs:895` `lower_list_comp` (lowering) — one generator, `Name` target, non-async, → an iterator chain. This is literally raise-then-lower for exactly one intent, and it is A's newest commit (`09e47e2 feat(emit): simple list comprehensions → iterator chains`). |
| A second, unnamed one | **Exists in A**, `emit.rs:248-334` — the mutability inference pass. It raises "which bindings are rebound" out of syntax before emission decides `mut`. |
| Loop/reduction/matmul/broadcast recognition | **Net-new. Neither repo does any of it.** No `fold`, no reduction detection, no array-shape reasoning anywhere in either codebase. |
| Any shape/dtype/layout analysis for the AI corpus | **Net-new.** |

**Verdict on §4:** the *architecture* for raise-then-lower is ~60% present (B's recognizer
tables + B's `IdiomChoice` ledger + A's two prototype passes), and the *content* — the actual
intent vocabulary and its recognizers — is ~100% net-new. That is a good position: the hard
structural decisions are already validated in production code, and the new work is additive
passes with an existing ledger to record into.

### 4a. The Rust round-trip as acceptance harness — assessment

**Is it real?** The *idea* is real and it is the right harness. The *"for free"* part is not.

What exists:
- B has a complete, exercised Rust **frontend**: `syn::parse_file` at `transpile.rs:26/56`, an
  exhaustive `dispatch_item` at `transpile.rs:630` with a gapping fallback, `visit.rs`'s
  canonical walks over `syn::Expr`/`syn::Type`, `map.rs`+`type_map.rs`+`prim_map.rs` for types,
  `symtab.rs` for cross-file names, and pattern/derive/call/macro decomposition. This is a
  genuinely deep Rust reader — far deeper than "it calls syn."
- A has a Rust **emitter**: `emit.rs`'s `try_lower_body`/`lower_if`/`lower_while`/`lower_for`
  plus mut-inference and numeric promotion, and — critically — `check.rs`, a **working `rustc`
  oracle** with per-file JSON diagnostics and stub counting.

What does not exist:
- Any type they could exchange. B's walks terminate in `Result<String, GapReason>`
  (`emit.rs:2795`); A's emitter starts from `&ast::StmtFunctionDef` (`emit.rs:24`).

So the honest cost of the round-trip harness is: **retarget B's `visit.rs`-driven walks to build
IR nodes instead of strings** (mechanical, because `visit.rs` already centralized the dispatch
into one place per node kind — this is exactly the refactor `visit.rs` was built to enable), and
**retarget A's emitter from rustpython AST to IR** (a rewrite of `emit.rs`'s lowering functions,
but they are only ~1,000 lines and their logic transfers). Estimate: the frontend retarget is
the smaller half. **(inference — I have not attempted either.)**

**Why it is still the right first move, and why it is sharper than any other test:**

1. **Identity is the only test with a free oracle.** For Python→Rust there is no ground truth to
   diff against; a human must judge. For Rust→IR→Rust the ground truth *is the input file*.
   Every byte of divergence is a question the IR must answer.
2. **Three graded acceptance levels, all mechanizable:**
   - **L-A: `rustc` accepts the output.** Already implemented — `check.rs`, unchanged, is the
     gate. Plus A's stub counter, so a round-trip that emits `todo!()` bodies fails.
   - **L-B: token-stream / AST-shape equivalence** after normalizing formatting
     (`prettyplease` or a `syn`-level structural compare). This is where the real signal is: any
     node the IR cannot carry shows up as a structural diff at a known source span.
   - **L-C: behavioral equivalence** — the input crate's own test suite run against the output.
     Strictly stronger, and available for any input crate that has tests.
3. **The diff is self-localizing.** Because every IR node carries `Provenance` (§3.5), a
   structural diff at output line N names the source span that produced it. The failure report
   is a gap record, in the format both repos already emit and A already ranks
   (`render_priority_report`).
4. **It generalizes to the other two frontends immediately.** Once Rust→IR→Rust is at identity,
   TS→IR→Rust and Python→IR→Rust reuse *the same backend*, so any backend bug is already
   excluded. That turns a two-variable debugging problem into a one-variable one — which is the
   real reason to build this first, more than the free oracle.
5. **It measures IR expressiveness directly, which nothing else does.** Coverage fractions
   measure the *transpiler*. Round-trip diff measures the *IR*.

**The trap to name explicitly:** round-trip identity is achievable by a degenerate IR that
carries source text through in `Op::Opaque`. The harness must therefore report **opaque-node
count and opaque-covered byte fraction** alongside the diff, exactly as `check.rs:10-26` reports
`todo!()` counts alongside "compiles" — same failure mode, same fix, and A already learned it.
An IR round-trip at 100% identity with 40% opaque bytes is a copier, not an IR.

**Corpus for it:** B's own input (the mycelium monorepo Rust it was extracted from) and A's
`research/mycelium-transpile-snapshot/src/` — a ~12k-line real Rust codebase already sitting in
A's tree, with a known-good gap profile from B's own runs to compare against.

---

## 5. The dials

One `Policy` struct, resolved from defaults → shop config → per-crate config → per-file
attribute → per-item annotation (later wins). Each dial names the **pipeline stage** it acts on.

```
Policy {
  target:       TargetSelection,
  strictness:   Strictness,
  ownership:    OwnershipPolicy,
  idiom:        IdiomPolicy,
  house:        HouseRules,
  reporting:    ReportingPolicy,
}
```

| Dial | Values | Applies at | Effect |
|---|---|---|---|
| `target.backends` | `[Rust]`, `[Mycelium]`, `[Rust, Mycelium]` | **Lower + backend** | Which lowering pipelines run. Raise is shared and runs once. |
| `target.rust_edition` / `min_version` | edition, msrv | **Backend** | Gates `let-else`, GATs, etc. A construct needing a newer feature becomes a gap, not a silent downgrade. |
| `strictness.level` | `Pedantic \| Standard \| Permissive` | **Whole pipeline** (gap classification) | `Pedantic`: any `Dynamic`, any `Advisory`, any `Heuristic` idiom is a hard failure. `Standard`: `Refusal`+`FidelityLoss` fail, `Advisory` reports. `Permissive`: emit best-effort, report all. |
| `strictness.on_dynamic` | `Reject \| StubTrait \| Erase \| Passthrough` | **Raise** (type inference) | What `Type::Dynamic` does. Keyed on `DynProvenance` (§3.2), so `Unannotated` and `ExplicitAny` can differ. |
| `strictness.unlowered_body` | `Fail \| Todo \| Unimplemented \| Comment` | **Backend** | Governs `check.rs`'s stub markers. `Fail` makes A's two-number problem structurally impossible. |
| `strictness.oracle` | `Required \| Advisory \| Skip` | **Verify** | `Required` makes `L3Status::NotRun` / `VetClass::ToolUnavailable` a build failure rather than a "not measured." |
| `ownership.aggressiveness` | `0 Unknown-only \| 1 SyntacticMut \| 2 LocalBorrow \| 3 EscapeAnalysis \| 4 Full` | **IR pass** `ownership_infer` | Level 1 is A's current `names_needing_mut` behavior. Levels ≥3 are where wrong-but-compiling output becomes possible; every inference at ≥2 emits an `IdiomChoice`. |
| `ownership.shared_default` | `Rc \| Arc \| Box \| Clone` | **Lower** | What `Shared` becomes when thread-safety is unproven. |
| `ownership.clone_budget` | `Forbid \| Warn \| Free` | **Lower** | Governs whether the lowerer may insert `.clone()` to escape a borrow it cannot prove. `Forbid` turns those into gaps — the honest setting. |
| `idiom.aggressiveness` | `Literal \| Conservative \| Idiomatic \| Aggressive` | **Lower** | `Literal`: a source loop stays a loop. `Idiomatic`: `Intent::Reduce` → `.fold()`. `Aggressive`: permits `rayon`, SIMD, trait synthesis step 2, union→enum synthesis. |
| `idiom.max_class` | `Mechanical \| Heuristic \| Judgment` | **Lower** | **Reuses B's `IdiomClass` verbatim** (`remap.rs:64-69`). Ceiling on how speculative an automatic transformation may be. `Mechanical` is B's shipped v0 (`remap.rs:19-22`). |
| `idiom.error_model` | `Preserve \| ResultQuestion \| PanicOnErr` | **Lower** | How `Effect::Throws` lands in Rust. The single highest-leverage dial for Python input. |
| `idiom.allowed_crates` | allowlist | **Lower** | Prevents an idiom pass reaching for a dependency the shop does not permit. |
| `house.naming` | case conventions per item kind | **Backend** | |
| `house.reserved_idents` | list + `Gap \| Prefix \| Suffix` | **Backend** | Generalizes B's `reserved.rs` (363 lines) from a hardcoded Mycelium keyword set to policy. B's *default* — gap, never auto-rename — stays the default. |
| `house.lints` / `attributes` | injected `#![..]`, `#[..]` | **Backend** | |
| `house.provenance_comments` | `None \| Item \| Op` | **Backend** | §3.5 requirement 1. Default `Item`. |
| `house.module_layout` | `Mirror \| Flatten \| Custom` | **Backend** | Generalizes B's `RemapOperation { Keep, Consolidate, Split, Relocate, CrateToPhylum }` (`remap.rs:44-50`) — already a schema, only `Keep` implemented. |
| `reporting.denominator` | `TopLevel \| AllOps \| Both` | **Report** | A measures ops, B measures top-level. `Both` is the honest default. |
| `reporting.fail_under` | fraction per level | **Report** | CI gate. |

**Invariant across all dials:** no dial may turn a gap into silence. A dial changes what is
*attempted* and what is *fatal*; the ledger records everything either way. This is the one rule
both repos already enforce (A `lib.rs:11-14`, B `lib.rs:5-8`) and it must be the IR's rule too.

---

## 6. Migration — converge without a rewrite, without regressing

**Host:** a **new standalone crate `transpile-ir`**, not inside either repo. Reasons: A is a
Cargo workspace (`crates/py2rust-core`) and can take a path/git dependency trivially; B is a
single crate and its README says it was extracted from the `tzervas/mycelium` monorepo, so it
already knows how to live as an extracted unit. More importantly, if the IR lives in A, then B
depending on it makes B look downstream of a Python tool, which it is not — and A already
carries `research/mycelium-transpile-snapshot/` as a vendored copy of B, so a dependency edge in
that direction would create a confusing double relationship. Neutral crate, both depend on it.
**(inference on the workspace-mechanics claim; I confirmed A's layout and B's single-crate
layout, not their manifests.)**

**Phasing — each phase is independently shippable and independently revertible:**

- **P0 — Extract, no behavior change.** Move into `transpile-ir`: `source_loc.rs` (A, verbatim),
  the `Gap`/`GapReason`/`GapReport` record shapes (identical in both, so this is a genuine
  de-duplication), and `visit.rs`'s visitor *pattern* (generic over node type). Both repos
  re-export from their old paths. **Proof of no regression:** both test suites unchanged and
  green; A's `.gap.json` sidecars and B's `summary.json` byte-identical to before. B's
  `visit.rs:33-38` already documents the precedent — that refactor was verified byte-identical
  across 65 tests.
- **P1 — Unify the ledger.** `GapClass` + `ConstructId` registry land in `transpile-ir`. A
  registers its 11 Python constructs, B its 22 Rust/Mycelium ones. A's stubbed
  `excluded_from_denominator`/`is_non_gap_advisory` (`gap.rs:70-79`) become real via `GapClass`;
  B's become derived. A gains B's exclusion arithmetic, B gains A's L2 denominator. **Proof:**
  per-category counts must match the pre-P1 counts exactly for every category on both sides;
  where a number *changes* (B's denominator now includes nested ops) the change is asserted in a
  test with the reason recorded.
- **P2 — Unify verification.** One `Oracle` trait in `transpile-ir` with the three-valued status.
  A's `check.rs` becomes `RustcOracle`; B's `vet.rs` becomes `MycCheckOracle`. Both keep their
  own logic. A gains B's file-gated conservative numerator; B gains A's stub-marker counting.
  **Proof:** L3/vet fractions unchanged on both fixture corpora.
- **P3 — The IR nodes + the Rust round-trip harness (§4a).** Add `transpile-ir`'s node/type
  layer. Build **B's frontend → IR** by retargeting the `visit.rs` walks, keeping B's existing
  string emitter live and unchanged. Build **IR → Rust** as a *new* backend crate seeded from
  A's `emit.rs` lowering logic, again keeping A's existing path live. Run the identity harness
  at L-A/L-B/L-C over `research/mycelium-transpile-snapshot/src/`. **Nothing in either repo has
  changed behavior at this point** — the IR path is additive and off by default.
- **P4 — Cut over one path at a time, behind a flag.** `--via-ir` on A first (its corpus has the
  `rustc` oracle, so regressions are mechanically detectable). Differential gate: for every
  fixture, IR path vs legacy path must produce equal-or-better L3 pass, equal-or-better
  stub-free count, and a gap ledger that is a *superset-or-equal* in coverage terms. Legacy path
  stays until the IR path wins on every fixture; then it is deleted, not before.
- **P5 — New frontends.** TypeScript frontend (`swc` or `oxc`) targets the IR directly and never
  touches either legacy path. Python frontend moves to IR. Raise passes (§4) land here, gated by
  `idiom.max_class = Mechanical` initially — B's own shipped default (`remap.rs:19-22`).

**What proves nothing broke, in one line:** every phase's gate is a *differential against the
repo's own prior output* — gap JSON, coverage fractions, oracle verdicts — and both repos already
emit exactly those artifacts as committed sidecars, which is the property that makes this
migration checkable at all.

**Guarantee tags must not be upgraded by migration.** Both repos tag emitted output `Declared`
and oracle verdicts `Empirical` (A `lib.rs:16-21`, B `lib.rs:33-44`, B `vet.rs:16-24`). Moving
code into a shared crate proves nothing new; the tags travel unchanged. B's README honesty note
about tags staying Declared/Empirical until differential upgrades is exactly right, and the
round-trip harness in P3 is the first thing in either project that could justify an upgrade —
and only for the specific claim it measures.

---

## 7. Honest limits

**Mechanically impossible even with this IR:**

1. **Dynamic dispatch on runtime-constructed names.** Python `getattr(obj, name)`, `eval`,
   `exec`, `__getattr__`, metaclasses; TS `obj[k]` with computed `k`; `Proxy`. The target set is
   not knowable statically. Permanently `Opaque` + gap. A's `Category::Metaprogramming` is
   correct to be a refusal and always will be.
2. **Deciding whether a mutation is observable.** Full escape analysis is undecidable in
   general. `ownership.aggressiveness ≥ 3` will be wrong sometimes, and the wrongness will
   compile. This is the dial that can produce silently-incorrect output.
3. **Reconstructing intent that the source destroyed.** If a human hand-unrolled a matmul into
   scalar arithmetic with strength reduction, no recognizer reliably raises it back. Raising is
   pattern matching and pattern matching has a recall ceiling.
4. **Exception→Result at scale.** Mechanical per-function, but the *call graph* must agree: one
   function becoming fallible forces every caller to change. Cross-module, with dynamic dispatch
   in the middle, this is not mechanical.
5. **Semantic equality of numerics.** Python ints are arbitrary-precision; Rust's are not. Every
   `int` → `i64` is a silent semantic narrowing. A's `forced_ty`/`promote` (`emit.rs:678-720`)
   heuristics cannot fix this; only a `BigInt` default (slow, un-idiomatic) or a gap can.
6. **Structural-to-nominal is not injective.** Two TS interfaces with identical members are the
   same type; two Rust traits are not. Round-tripping TS→Rust→TS cannot recover which name the
   author meant. Provenance mitigates (record the name) but does not solve.
7. **Non-determinism and platform behavior.** Iteration order, float reassociation under
   `Aggressive` idioms, GC-timing-dependent code.

**The biggest risk to the plan, named singly:** *the IR becomes a union of three languages
rather than an abstraction over them.* Every hard case will present as "just add a node for it,"
and each addition makes the IR carry one frontend's concept, which every backend must then
handle or gap. The failure mode is an IR with `PythonClass`, `TSNamespace`, and `RustLifetime`
nodes — at which point it is a serialization format, not an IR, and the round-trip harness will
still read 100% because `Op::Opaque` and one-language nodes both round-trip perfectly.

The two defenses, both cheap and both mechanizable, are: **(a)** the §4a opaque-fraction metric,
reported always, treated exactly as `check.rs` treats `todo!()` counts; and **(b)** a hard rule
that **no IR node may be produced by exactly one frontend** — every node must have at least two
independent source constructs that lower into it, or it belongs in a frontend, not the IR. That
rule alone would have kept every category in §2.3 out.

A close second risk: A and B are both live (other agents are working in A right now), so a
long-lived divergent IR branch will rot. P0–P2 are deliberately behavior-preserving and small
for exactly this reason — they are mergeable in days, not weeks, and they front-load the shared
value (one ledger, one oracle contract) before any of the speculative work begins.
