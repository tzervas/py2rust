//! Pre-planned common interface for **Python → Mycelium-native** mapping.
//!
//! This module is the **stable contract** between gap classification
//! ([`crate::gap`]), surface mapping ([`crate::myc_map`]), and coverage tests.
//! Lanes other than the interface owner must not edit this file; they request
//! changes instead.
//!
//! # Why this exists
//!
//! The operator intent is a pipeline of Python → Mycelium-native that **skips**
//! an interim Rust route. Rust remains the reference implementation to learn
//! from, not the destination. Every construct in the gap taxonomy must resolve
//! to either a Mycelium spelling or an **explicit** refusal (G2 / VR-5): never a
//! silent drop, never a fabricated coverage cell.
//!
//! # Stable vs open
//!
//! | Item | Status |
//! |------|--------|
//! | [`PyConstruct`], [`MycForm`], [`Guarantee`], [`Citation`], [`MapOutcome`] | **Stable contract** — signatures and variants are final for this PR |
//! | Concrete surface strings inside [`MycForm::surface`] | Open — owned by the surface-mapper table |
//! | Bridge from [`crate::gap::Category`] | Open — owned by the gap-closer lane |
//!
//! Vocabulary for [`Guarantee`] is taken from `lib/std/math.myc` (Exact /
//! Empirical / explicit never-silent Refusal) — not a new confidence scale.

use serde::{Deserialize, Serialize};

/// Input alphabet for the Python → Mycelium mapper.
///
/// Mirrors the existing [`crate::gap::Category`] taxonomy so the mapping layer
/// cannot fork from the never-silent gap report. Closed enum plus
/// [`PyConstruct::Other`] — a construct that fits nothing still gets recorded
/// with free text. **Never a silent drop.**
///
/// # Alignment with `gap::Category`
///
/// | [`PyConstruct`] | [`crate::gap::Category`] |
/// |-----------------|--------------------------|
/// | `Class` … `MultiStmtBody` | same name |
/// | `PartialEmit` | `FunctionBody` (signature emitted, body not fully lowered) |
/// | `Other(String)` | `Other` (reason carried on the variant / gap record) |
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PyConstruct {
    /// `class` definitions and inheritance.
    Class,
    /// `try` / `except` / `raise` / `finally`.
    Exception,
    /// Unannotated parameters, `Any`, dynamic attributes.
    DynamicTyping,
    /// Decorators that alter defs, `exec` / `eval`, metaclasses.
    Metaprogramming,
    /// `async` / `await` / `async def` / `async with` / `async for`.
    Async,
    /// Imports without a confirmed Mycelium nodule path.
    Import,
    /// `lambda` expressions.
    Lambda,
    /// List / dict / set / generator comprehensions.
    Comprehension,
    /// Multi-statement bodies not yet expressible as a single Mycelium expr form.
    MultiStmtBody,
    /// Signature (or partial surface) emitted; body not fully lowered — the
    /// partial-emit sub-gap (`Category::FunctionBody`).
    PartialEmit,
    /// Catch-all with free-text label — never a silent drop.
    Other(String),
}

impl PyConstruct {
    /// Stable PascalCase name for JSON / coverage tables.
    ///
    /// For [`PyConstruct::Other`], returns `"Other"`; the free-text payload is
    /// separate (`other_label`).
    pub fn as_str(&self) -> &str {
        match self {
            PyConstruct::Class => "Class",
            PyConstruct::Exception => "Exception",
            PyConstruct::DynamicTyping => "DynamicTyping",
            PyConstruct::Metaprogramming => "Metaprogramming",
            PyConstruct::Async => "Async",
            PyConstruct::Import => "Import",
            PyConstruct::Lambda => "Lambda",
            PyConstruct::Comprehension => "Comprehension",
            PyConstruct::MultiStmtBody => "MultiStmtBody",
            PyConstruct::PartialEmit => "PartialEmit",
            PyConstruct::Other(_) => "Other",
        }
    }

    /// Free-text payload when this is [`PyConstruct::Other`].
    pub fn other_label(&self) -> Option<&str> {
        match self {
            PyConstruct::Other(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// Closed taxonomy arms used for 100% coverage enumeration (plus one
    /// canonical [`PyConstruct::Other`] row). Free-text `Other` instances remain
    /// valid inputs; the canonical empty-label row is what coverage tables use.
    pub fn taxonomy() -> Vec<PyConstruct> {
        taxonomy_constructs()
    }
}

/// Enumerate every closed taxonomy construct exactly once (plus one canonical
/// `Other` row). This is the assessable coverage denominator:
/// `Mapped / (Mapped + Unmappable)`.
pub fn taxonomy_constructs() -> Vec<PyConstruct> {
    vec![
        PyConstruct::Class,
        PyConstruct::Exception,
        PyConstruct::DynamicTyping,
        PyConstruct::Metaprogramming,
        PyConstruct::Async,
        PyConstruct::Import,
        PyConstruct::Lambda,
        PyConstruct::Comprehension,
        PyConstruct::MultiStmtBody,
        PyConstruct::PartialEmit,
        PyConstruct::Other(String::new()),
    ]
}

impl std::fmt::Display for PyConstruct {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PyConstruct::Other(s) if !s.is_empty() => write!(f, "Other({s})"),
            other => f.write_str(other.as_str()),
        }
    }
}

/// What a Python construct becomes in **Mycelium-native** terms.
///
/// Not a Rust transliteration: `surface` is the Mycelium spelling (or a
/// schematic of it). Authority and guarantee make honesty first-class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MycForm {
    /// The Mycelium surface spelling (or a compact schematic of it).
    pub surface: String,
    /// What makes this the correct form (corpus id, proving test, or Unverified).
    pub authority: Citation,
    /// Exact | Empirical | Refusal — project vocabulary from `lib/std/math.myc`.
    pub guarantee: Guarantee,
}

impl MycForm {
    /// Construct a form with explicit authority and guarantee.
    pub fn new(surface: impl Into<String>, authority: Citation, guarantee: Guarantee) -> Self {
        Self {
            surface: surface.into(),
            authority,
            guarantee,
        }
    }
}

/// Strength of a mapped result — **reuse** the project's own vocabulary.
///
/// From `lib/std/math.myc` / VR-5: results are documented as **Exact** on the
/// in-range domain, **Empirical** when backed by differential trials, or an
/// explicit never-silent **Refusal**. Do not invent a new scale.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum Guarantee {
    /// Total / exact on the declared domain (e.g. in-range `badd`).
    Exact,
    /// Backed by measured trials (e.g. L1-eval ≡ L0-interp ≡ AOT agreement).
    Empirical,
    /// Explicit never-silent refusal path (overflow, unmappable, missing prim).
    Refusal,
}

impl Guarantee {
    pub fn as_str(self) -> &'static str {
        match self {
            Guarantee::Exact => "Exact",
            Guarantee::Empirical => "Empirical",
            Guarantee::Refusal => "Refusal",
        }
    }
}

impl std::fmt::Display for Guarantee {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Honesty primitive for every mapping row.
///
/// [`Citation::Unverified`] is **first-class, not a failure**. The project
/// culture is never-claim-ahead-of-the-prims (VR-5/G2); the type system makes
/// "I could not verify" expressible instead of forcing a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "PascalCase")]
pub enum Citation {
    /// Decision-corpus identifier, e.g. `"RFC-0032 §5 D2"`, `"DN-42"`, `"M-753"`, `"VR-5"`.
    Corpus(String),
    /// A test name that proves the mapping.
    Test(String),
    /// Reason the row could not be verified — preferred over a fabricated citation.
    Unverified(String),
}

impl Citation {
    pub fn corpus(id: impl Into<String>) -> Self {
        Citation::Corpus(id.into())
    }

    pub fn test(name: impl Into<String>) -> Self {
        Citation::Test(name.into())
    }

    pub fn unverified(reason: impl Into<String>) -> Self {
        Citation::Unverified(reason.into())
    }

    /// True when authority is explicitly unverified (still a valid, honest row).
    pub fn is_unverified(&self) -> bool {
        matches!(self, Citation::Unverified(_))
    }
}

impl std::fmt::Display for Citation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Citation::Corpus(s) => write!(f, "Corpus({s})"),
            Citation::Test(s) => write!(f, "Test({s})"),
            Citation::Unverified(s) => write!(f, "Unverified({s})"),
        }
    }
}

/// Total mapping outcome — **no third case, no `Option`**.
///
/// Every construct resolves to a mapping or an explicit refusal carrying **why**
/// and **what would be needed**. Coverage is assessable:
/// `Mapped / (Mapped + Unmappable)`, both sides enumerable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "PascalCase")]
pub enum MapOutcome {
    /// Construct has a Mycelium-native form (with citation + guarantee).
    Mapped(MycForm),
    /// Construct cannot be mapped yet — never silent.
    Unmappable {
        construct: PyConstruct,
        /// Why this construct has no Mycelium-native form today.
        reason: String,
        /// What would be needed to close the gap (prim, surface syntax, policy, …).
        needed: Option<String>,
    },
}

impl MapOutcome {
    /// Convenience constructor for a mapped form.
    pub fn mapped(form: MycForm) -> Self {
        MapOutcome::Mapped(form)
    }

    /// Convenience constructor for an explicit refusal.
    pub fn unmappable(
        construct: PyConstruct,
        reason: impl Into<String>,
        needed: Option<String>,
    ) -> Self {
        MapOutcome::Unmappable {
            construct,
            reason: reason.into(),
            needed,
        }
    }

    /// True when this outcome is a successful Mycelium mapping.
    pub fn is_mapped(&self) -> bool {
        matches!(self, MapOutcome::Mapped(_))
    }

    /// True when this outcome is an explicit refusal.
    pub fn is_unmappable(&self) -> bool {
        matches!(self, MapOutcome::Unmappable { .. })
    }

    /// The construct this outcome accounts for (mapped forms carry it only
    /// indirectly; callers should retain the input construct for coverage).
    pub fn unmappable_construct(&self) -> Option<&PyConstruct> {
        match self {
            MapOutcome::Unmappable { construct, .. } => Some(construct),
            MapOutcome::Mapped(_) => None,
        }
    }

    /// Citation for mapped rows; `None` for unmappable (reason lives on the variant).
    pub fn citation(&self) -> Option<&Citation> {
        match self {
            MapOutcome::Mapped(form) => Some(&form.authority),
            MapOutcome::Unmappable { .. } => None,
        }
    }
}
