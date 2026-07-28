//! Python construct → **Mycelium-native** surface map.
//!
//! Owned exclusively by the surface-mapper lane. Reads the stable contract in
//! [`crate::interface`]; does not invent alternate outcome types.
//!
//! # Honesty (R1–R3)
//!
//! - Surfaces are the **correct Mycelium way**, never a Rust transliteration.
//! - Every taxonomy construct resolves to [`MapOutcome::Mapped`] or
//!   [`MapOutcome::Unmappable`] — no silent drops, no third case.
//! - Every mapped row carries a [`Citation`] (or explicit
//!   [`Citation::Unverified`]).
//!
//! # Guarantee tags
//!
//! Rows marked [`Guarantee::Empirical`] cite measured behaviour (grammar
//! conformance, `lib/std/*.myc` ports). Rows marked [`Guarantee::Refusal`] are
//! deliberate never-silent refusals. Where the mycelium-l1 grammar / prim set
//! was not re-run in this repo, authority is [`Citation::Unverified`] rather
//! than a fabricated corpus id (VR-5).

use crate::interface::{
    taxonomy_constructs, Citation, Guarantee, MapOutcome, MycForm, PyConstruct,
};

/// Map one Python construct to a total [`MapOutcome`].
///
/// Total by construction: every arm returns Mapped or Unmappable.
pub fn map_construct(construct: &PyConstruct) -> MapOutcome {
    match construct {
        PyConstruct::Lambda => MapOutcome::mapped(MycForm::new(
            // Mycelium spelling from grammar: lambda_expr ::= 'lambda' '(' params? ')' '=>' expr
            // (RFC-0037 D5 / M-704 / RFC-0024 §4A). Multi-param forms curry.
            "lambda(x: T) => expr",
            Citation::corpus("RFC-0037 D5 / M-704 / RFC-0024 §4A (lambda_expr)"),
            Guarantee::Empirical,
        )),

        PyConstruct::MultiStmtBody => MapOutcome::mapped(MycForm::new(
            // Nested let-in is the working desugar target; `{ a; b }` surface is
            // owned by a separate mycelium-l1 unit — do not claim block sugar here.
            "let x = a in (let y = b in y)",
            Citation::test(
                "mycelium-l1 multi_statement_body::the_desugar_target_already_works_end_to_end",
            ),
            Guarantee::Empirical,
        )),

        PyConstruct::Exception => MapOutcome::mapped(MycForm::new(
            // Result + std.recover — never panic/except transliteration.
            "Result[A, E]  /* Ok(A) | Err(E) */  +  std.recover::handle_classified / recover_classified",
            Citation::corpus("M-649 (std.result) / M-930 (std.recover) / RFC-0031 §5 D5"),
            Guarantee::Empirical,
        )),

        PyConstruct::Import => MapOutcome::mapped(MycForm::new(
            // Nodule declaration / use — not Rust `use crate::…`.
            "nodule path.to.name;   /* or same-file include; cross-nodule use when resolvable */",
            Citation::corpus("M-1001 / DN-34 §4 (import only when nodule path is confirmed)"),
            Guarantee::Empirical,
        )),

        PyConstruct::Comprehension => MapOutcome::mapped(MycForm::new(
            // Prefer explicit iteration/combinators over list-comp sugar.
            "std.iter combinators / recursive fn over list  /* no list-comp sugar claimed */",
            Citation::unverified(
                "std.iter.myc port exists in mycelium-l1; list/dict/set comprehension \
                 desugar shapes not re-verified against myc check in this py2rust tree",
            ),
            Guarantee::Empirical,
        )),

        PyConstruct::PartialEmit => MapOutcome::mapped(MycForm::new(
            // Partial emit is honest scaffolding: emit the fn spine, gap the body.
            "fn name(params) => Ret = <body-or-explicit-gap>  /* never silent TODO */",
            Citation::corpus("G2 / VR-5 (flag not guess; FunctionBody sub-gap)"),
            Guarantee::Refusal,
        )),

        PyConstruct::Class => MapOutcome::unmappable(
            PyConstruct::Class,
            "Python class/inheritance has no single Mycelium-native type+method bundle \
             equivalent; ADTs are `type T = A | B` sum types and inherent/trait `impl` \
             blocks, not OO classes with open method tables.",
            Some(
                "A documented class→(sum type | record + impl) lowering policy, plus \
                 confirmed grammar for inheritance/override if required (or an explicit \
                 forever-refusal for inheritance)."
                    .into(),
            ),
        ),

        PyConstruct::DynamicTyping => MapOutcome::unmappable(
            PyConstruct::DynamicTyping,
            "Mycelium is statically typed at the `.myc` surface; `Any` / missing \
             annotations / dynamic attributes have no total native form. Emitting a \
             fabricated type would violate VR-5.",
            Some(
                "Caller-supplied type annotations, or a future gradual-typing / \
                 dynamic-object prim set (not claimed today)."
                    .into(),
            ),
        ),

        PyConstruct::Metaprogramming => MapOutcome::unmappable(
            PyConstruct::Metaprogramming,
            "`exec` / `eval` / metaclasses / definition-altering decorators have no \
             Mycelium-native code-as-data eval surface in the self-hosted stdlib; \
             emitting them would be a silent lie.",
            Some(
                "An explicit macro/quote surface or a denied-by-policy forever-refusal \
                 with recovery via static desugars for known safe decorators only."
                    .into(),
            ),
        ),

        PyConstruct::Async => MapOutcome::unmappable(
            PyConstruct::Async,
            "Python async/await is a scheduler+Future model. Mycelium concurrency, where \
             present, is effect/spore oriented — not a drop-in `async def` spelling. No \
             confirmed async-def → spore lowering is claimed in this tree.",
            Some(
                "A documented async→std.spore / effect-surface lowering (RFC-level), \
                 with myc-check-clean examples; until then, hard gap."
                    .into(),
            ),
        ),

        PyConstruct::Other(label) => {
            let detail = if label.is_empty() {
                "construct outside the closed Python gap taxonomy".to_string()
            } else {
                format!("construct outside the closed taxonomy: {label}")
            };
            MapOutcome::unmappable(
                PyConstruct::Other(label.clone()),
                format!(
                    "{detail}; recorded explicitly so the driver never drops it (G2)"
                ),
                Some(
                    "Classify under an existing PyConstruct arm when possible, or extend \
                     the taxonomy + gap::Category together in one PR."
                        .into(),
                ),
            )
        }
    }
}

/// Map the full closed taxonomy. Length equals [`taxonomy_constructs`].
///
/// Used by coverage tests and any future reporting CLI.
pub fn map_taxonomy() -> Vec<(PyConstruct, MapOutcome)> {
    taxonomy_constructs()
        .into_iter()
        .map(|c| {
            let outcome = map_construct(&c);
            (c, outcome)
        })
        .collect()
}

/// Coverage fraction: `Mapped / (Mapped + Unmappable)` over the closed taxonomy.
///
/// Both sides are enumerable; unaccounted constructs are a logic bug, not a
/// partial `Option`.
pub fn taxonomy_coverage() -> (usize, usize, f64) {
    let rows = map_taxonomy();
    let total = rows.len();
    let mapped = rows.iter().filter(|(_, o)| o.is_mapped()).count();
    let unmappable = total - mapped;
    let frac = if total == 0 {
        0.0
    } else {
        mapped as f64 / total as f64
    };
    debug_assert_eq!(mapped + unmappable, total);
    (mapped, unmappable, frac)
}

/// Every mapped row must carry a citation; unmappable rows carry reason+needed.
/// Returns human-readable violations (empty = honest table).
pub fn honesty_violations() -> Vec<String> {
    let mut v = Vec::new();
    for (construct, outcome) in map_taxonomy() {
        match &outcome {
            MapOutcome::Mapped(form) => {
                if form.surface.trim().is_empty() {
                    v.push(format!("{construct}: Mapped with empty surface"));
                }
                // Citation is always present by type; Unverified is allowed.
                match &form.authority {
                    Citation::Corpus(s) | Citation::Test(s) | Citation::Unverified(s) => {
                        if s.trim().is_empty() {
                            v.push(format!("{construct}: empty citation payload"));
                        }
                    }
                }
            }
            MapOutcome::Unmappable {
                reason, needed, ..
            } => {
                if reason.trim().is_empty() {
                    v.push(format!("{construct}: Unmappable with empty reason"));
                }
                let _ = needed; // optional by contract
            }
        }
    }
    v
}
