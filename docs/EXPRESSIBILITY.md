# Expressibility Map

Measures how much of the Python language surface `py2rust` can lower today, from a corpus run rather than from vibes. Regenerate with:

```
python3 scripts/expressibility_report.py
```

**Staleness warning:** at the time this file was generated, seven other agents were concurrently closing gaps in Lambda, dict/set comprehensions, MultiStmtBody, Import, Class, Exception, and DynamicTyping on separate branches. The numbers below reflect only what had landed on *this* branch's base commit at generation time — they will be an undercount the moment any of those land. Re-run the script above after merging to refresh.

## Headline

**9 of 32** corpus constructs are mechanically expressible end-to-end today (clean lower, zero gaps, of files that emitted something). See per-family table below.

## L3 (rustc) gate on emitted output

- files: 32
- emitted items: 33 / 47 top-level statements
- L3 checked: 25, passed: 22, failed: 3
- passed with zero unlowered (`todo!`) bodies: 10

## By construct family

| Family | Files | Clean | Gapped | Gap categories seen |
|---|---|---|---|---|
| async_await | 1 | 0 | 1 | Async |
| classes | 3 | 0 | 3 | Class |
| comprehensions | 4 | 0 | 4 | Comprehension, DynamicTyping, FunctionBody |
| control_flow | 3 | 3 | 0 | — |
| exceptions | 2 | 0 | 2 | Class, Exception, FunctionBody |
| fstrings | 1 | 0 | 1 | FunctionBody |
| functions | 3 | 1 | 2 | DynamicTyping, Metaprogramming, Other |
| generators | 1 | 0 | 1 | DynamicTyping, FunctionBody |
| imports | 2 | 1 | 1 | Import |
| lambdas | 1 | 0 | 1 | Lambda |
| literals | 3 | 2 | 1 | Other |
| match | 1 | 0 | 1 | FunctionBody |
| operators | 3 | 2 | 1 | FunctionBody |
| slicing | 1 | 0 | 1 | DynamicTyping, FunctionBody |
| typing | 1 | 0 | 1 | FunctionBody |
| unpacking | 1 | 0 | 1 | DynamicTyping, FunctionBody |
| with_stmt | 1 | 0 | 1 | FunctionBody |

## Per-file detail

| File | Status | Categories |
|---|---|---|
| `async_await/async_basic.py` | GAPPED | Async |
| `classes/inheritance.py` | GAPPED | Class |
| `classes/init_methods.py` | GAPPED | Class |
| `classes/properties_dunders.py` | GAPPED | Class |
| `comprehensions/dict_set_gen.py` | GAPPED | Comprehension, DynamicTyping, FunctionBody |
| `comprehensions/filtered_comp.py` | GAPPED | DynamicTyping |
| `comprehensions/list_comp.py` | GAPPED | DynamicTyping |
| `comprehensions/nested_comp.py` | GAPPED | Comprehension, DynamicTyping, FunctionBody |
| `control_flow/for_range.py` | CLEAN | — |
| `control_flow/if_elif_else.py` | CLEAN | — |
| `control_flow/while_break_continue.py` | CLEAN | — |
| `exceptions/raise_custom.py` | GAPPED | Class, Exception, FunctionBody |
| `exceptions/try_except_else_finally.py` | GAPPED | Exception, FunctionBody |
| `fstrings/basic_fstring.py` | GAPPED | FunctionBody |
| `functions/args_kwargs.py` | GAPPED | DynamicTyping, Other |
| `functions/decorators.py` | GAPPED | DynamicTyping, Metaprogramming |
| `functions/defaults.py` | CLEAN | — |
| `generators/yield_basic.py` | GAPPED | DynamicTyping, FunctionBody |
| `imports/relative_import.py` | GAPPED | Import |
| `imports/stdlib_import.py` | CLEAN | — |
| `lambdas/basic_lambda.py` | GAPPED | Lambda |
| `literals/collections.py` | GAPPED | Other |
| `literals/int_float.py` | CLEAN | — |
| `literals/str_bool_none.py` | CLEAN | — |
| `match/basic_match.py` | GAPPED | FunctionBody |
| `operators/arithmetic.py` | GAPPED | FunctionBody |
| `operators/bitwise.py` | CLEAN | — |
| `operators/comparison_boolean.py` | CLEAN | — |
| `slicing/basic_slice.py` | GAPPED | DynamicTyping, FunctionBody |
| `typing/generics_optional.py` | GAPPED | FunctionBody |
| `unpacking/tuple_unpack.py` | GAPPED | DynamicTyping, FunctionBody |
| `with_stmt/single_with.py` | GAPPED | FunctionBody |

## Mechanically-impossible vs merely-unimplemented

This is the distinction that makes the roadmap meaningful: only MERELY-UNIMPLEMENTED gaps are closable by writing more transpiler. MECHANICALLY-IMPOSSIBLE gaps require either a semantic-changing workaround (an enum/Any wrapper, a runtime interpreter embedded in the output) or must stay a permanent, honestly-reported gap.

| Gap category | Classification | Why |
|---|---|---|
| Class | MERELY-UNIMPLEMENTED | structs + impl blocks exist in Rust; single inheritance maps to composition or trait objects. Lowering is unwritten, not impossible. |
| Exception | MERELY-UNIMPLEMENTED | Result<T, E> + ? and panic/catch_unwind cover try/except/else/finally faithfully enough to lower mechanically. |
| DynamicTyping | MOSTLY MERELY-UNIMPLEMENTED | Missing annotations just need type inference (closable). Genuine runtime type polymorphism (a variable that holds different concrete types across a function's life) has no 1:1 static-Rust rendering without an enum/Any wrapper that changes semantics — that residual is MECHANICALLY-IMPOSSIBLE to render *silently*; it must gap or require a manual enum. |
| Metaprogramming | SPLIT | A decorator with a known, statically analyzable body (memoization, simple wrapping) is MERELY-UNIMPLEMENTED. `eval`/`exec` of a dynamically constructed string and metaclasses that rewrite the class at runtime are MECHANICALLY-IMPOSSIBLE — Rust has no runtime code-gen. |
| Async | MERELY-UNIMPLEMENTED | Rust has async/await natively (tokio/async-std); lowering is unwritten, not blocked by any semantic gap. |
| Import | MOSTLY MERELY-UNIMPLEMENTED | Known stdlib modules map to `use` lines (already partially done). `importlib`/`__import__` with a computed module name is MECHANICALLY-IMPOSSIBLE — Rust has no dynamic module loading. |
| Lambda | MERELY-UNIMPLEMENTED | Rust closures (`|x| ...`) are a direct target; lowering is unwritten. |
| Comprehension | MERELY-UNIMPLEMENTED | List comprehensions already lower to iterator chains (see git 09e47e2); dict/set/generator comprehensions are the same shape, just not yet wired up. |
| MultiStmtBody | MERELY-UNIMPLEMENTED | Sequencing multiple statements in one Rust block is mechanical; the lowering pass is simply incomplete. |
| FunctionBody | MERELY-UNIMPLEMENTED | Sub-gap on a signature that emitted but whose body did not; closable per-construct as each body-statement kind is lowered. |
| Other | NEEDS PER-CASE JUDGEMENT | Catch-all category; `*args`/`**kwargs` (MERELY-UNIMPLEMENTED — Vec/HashMap can express variadics) and module-level literal collections (MERELY-UNIMPLEMENTED — same const/static lowering as scalars) are the cases seen in this corpus. |

## Corpus

`tests/corpus/` holds 32 small, single-purpose Python snippets across 17 construct families (literals, operators, control flow, functions incl. defaults/*args/**kwargs/decorators, comprehensions incl. nested/filtered/dict/set/generator, classes incl. inheritance/properties/dunders, exceptions, imports incl. relative, lambdas, with-statements, f-strings, slicing, unpacking incl. star-unpack, match, generators/yield, async/await, and typing generics). Add more snippets to `tests/corpus/<family>/` and re-run the script to widen coverage.

