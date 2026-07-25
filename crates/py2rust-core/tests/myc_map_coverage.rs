//! Verifier lane — coverage and honesty for Python → Mycelium-native mapping.
//!
//! No production code. Asserts S1–S3:
//! - S1: every taxonomy construct is Mapped or Unmappable-with-reason
//! - S2: existing tests still pass (this file only adds checks)
//! - S3: every mapped row is cited or explicitly Unverified

use py2rust_core::{
    honesty_violations, map_construct, map_taxonomy, taxonomy_constructs, taxonomy_coverage,
    Category, Citation, Guarantee, MapOutcome, PyConstruct,
};
use std::collections::BTreeSet;

#[test]
fn s1_taxonomy_fully_accounted() {
    let rows = map_taxonomy();
    assert_eq!(
        rows.len(),
        taxonomy_constructs().len(),
        "taxonomy enumerator and map table length must match"
    );
    for (construct, outcome) in &rows {
        match outcome {
            MapOutcome::Mapped(form) => {
                assert!(
                    !form.surface.trim().is_empty(),
                    "{construct}: Mapped requires non-empty Mycelium surface"
                );
            }
            MapOutcome::Unmappable {
                construct: c,
                reason,
                needed: _,
            } => {
                assert_eq!(
                    c.as_str(),
                    construct.as_str(),
                    "Unmappable.construct must echo the input construct"
                );
                assert!(
                    !reason.trim().is_empty(),
                    "{construct}: Unmappable requires a reason"
                );
            }
        }
    }
    let (mapped, unmappable, frac) = taxonomy_coverage();
    assert_eq!(mapped + unmappable, rows.len());
    assert!(
        (0.0..=1.0).contains(&frac),
        "coverage fraction in [0,1], got {frac}"
    );
    // Snapshot the assessable split so regressions are visible.
    assert!(
        mapped >= 1,
        "expected at least one Mapped Mycelium-native form; mapped={mapped}"
    );
    assert!(
        unmappable >= 1,
        "expected at least one explicit Unmappable; unmappable={unmappable}"
    );
}

#[test]
fn s3_every_mapped_row_cited_or_unverified() {
    for (construct, outcome) in map_taxonomy() {
        if let MapOutcome::Mapped(form) = outcome {
            match form.authority {
                Citation::Corpus(ref s) | Citation::Test(ref s) | Citation::Unverified(ref s) => {
                    assert!(
                        !s.trim().is_empty(),
                        "{construct}: citation payload must be non-empty"
                    );
                }
            }
            // Guarantee is one of the project vocabulary arms.
            assert!(
                matches!(
                    form.guarantee,
                    Guarantee::Exact | Guarantee::Empirical | Guarantee::Refusal
                ),
                "{construct}: unexpected guarantee {:?}",
                form.guarantee
            );
        }
    }
    let violations = honesty_violations();
    assert!(
        violations.is_empty(),
        "honesty violations: {violations:?}"
    );
}

#[test]
fn category_pyconstruct_roundtrip_closed_arms() {
    // PartialEmit ↔ FunctionBody is the only name mismatch; others share names.
    let pairs = [
        (Category::Class, PyConstruct::Class),
        (Category::Exception, PyConstruct::Exception),
        (Category::DynamicTyping, PyConstruct::DynamicTyping),
        (Category::Metaprogramming, PyConstruct::Metaprogramming),
        (Category::Async, PyConstruct::Async),
        (Category::Import, PyConstruct::Import),
        (Category::Lambda, PyConstruct::Lambda),
        (Category::Comprehension, PyConstruct::Comprehension),
        (Category::MultiStmtBody, PyConstruct::MultiStmtBody),
        (Category::FunctionBody, PyConstruct::PartialEmit),
        (Category::Other, PyConstruct::Other(String::new())),
    ];
    for (cat, expected) in pairs {
        let pc = PyConstruct::from(cat);
        assert_eq!(pc.as_str(), expected.as_str());
        let back = Category::from(&pc);
        assert_eq!(back, cat);
        // Bridge through gap::Category::myc_map_outcome is total.
        let outcome = cat.myc_map_outcome();
        assert!(outcome.is_mapped() || outcome.is_unmappable());
    }
}

#[test]
fn lambda_maps_to_mycelium_lambda_not_rust_closure() {
    let outcome = map_construct(&PyConstruct::Lambda);
    match outcome {
        MapOutcome::Mapped(form) => {
            assert!(
                form.surface.contains("lambda"),
                "Lambda must use Mycelium lambda surface, got {}",
                form.surface
            );
            assert!(
                !form.surface.contains("|x|") && !form.surface.contains("Fn("),
                "must not be a Rust closure transliteration: {}",
                form.surface
            );
            assert!(matches!(
                form.authority,
                Citation::Corpus(_) | Citation::Test(_) | Citation::Unverified(_)
            ));
        }
        MapOutcome::Unmappable { reason, .. } => {
            panic!("Lambda should be Mapped (grammar has lambda_expr); got Unmappable: {reason}");
        }
    }
}

#[test]
fn multistmt_maps_to_nested_let_in_not_block_claim() {
    let outcome = map_construct(&PyConstruct::MultiStmtBody);
    match outcome {
        MapOutcome::Mapped(form) => {
            assert!(
                form.surface.contains("let") && form.surface.contains("in"),
                "MultiStmtBody native form is nested let-in, got {}",
                form.surface
            );
            // Do not claim `{ a; b }` sugar from this crate.
            assert!(
                !form.surface.trim_start().starts_with('{'),
                "must not claim block surface owned by mycelium-l1: {}",
                form.surface
            );
        }
        other => panic!("expected Mapped nested let-in, got {other:?}"),
    }
}

#[test]
fn class_and_async_are_explicit_unmappable() {
    for c in [PyConstruct::Class, PyConstruct::Async, PyConstruct::DynamicTyping] {
        match map_construct(&c) {
            MapOutcome::Unmappable {
                reason, needed, ..
            } => {
                assert!(!reason.is_empty());
                assert!(
                    needed.as_ref().map(|s| !s.is_empty()).unwrap_or(false),
                    "{c}: needed should say what would close the gap"
                );
            }
            MapOutcome::Mapped(form) => {
                panic!("{c} must not be silently Mapped without a real native class/async/dyn surface; got {}", form.surface);
            }
        }
    }
}

#[test]
fn no_third_outcome_and_no_option_holes() {
    // Every taxonomy arm produces exactly one MapOutcome variant.
    let kinds: BTreeSet<_> = map_taxonomy()
        .into_iter()
        .map(|(_, o)| match o {
            MapOutcome::Mapped(_) => "Mapped",
            MapOutcome::Unmappable { .. } => "Unmappable",
        })
        .collect();
    assert!(kinds.contains("Mapped"));
    assert!(kinds.contains("Unmappable"));
    assert_eq!(kinds.len(), 2, "only Mapped|Unmappable — no third case");
}

#[test]
fn gap_record_py_construct_bridge() {
    let g = py2rust_core::Gap::new(
        "x.py",
        1,
        1,
        Category::FunctionBody,
        "def f(): ...",
        "body not lowered",
        Some("f".into()),
    );
    assert_eq!(g.py_construct(), PyConstruct::PartialEmit);
    assert!(g.myc_map_outcome().is_mapped() || g.myc_map_outcome().is_unmappable());
}
