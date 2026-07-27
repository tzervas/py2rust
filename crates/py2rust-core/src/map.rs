//! Best-effort Python type annotation → Rust type string mapping.
//! Unmapped / missing types become gaps (DynamicTyping), never guessed as silent `Any`.

use rustpython_parser::ast::{self, Ranged};

/// Map a Python annotation expression to a Rust type string when known.
pub fn map_type_expr(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Name(n) => map_name(n.id.as_str()),
        ast::Expr::Constant(c) => match &c.value {
            ast::Constant::None => Some("()".into()),
            ast::Constant::Str(s) => map_name(s.as_str()),
            _ => None,
        },
        ast::Expr::Attribute(attr) => {
            // typing.Any / typing.Optional etc. — only surface names we know.
            if let ast::Expr::Name(base) = attr.value.as_ref() {
                if base.id.as_str() == "typing" {
                    return map_typing_attr(attr.attr.as_str());
                }
            }
            None
        }
        ast::Expr::Subscript(sub) => {
            let inner = map_type_expr(&sub.slice)?;
            match sub.value.as_ref() {
                ast::Expr::Name(n) => match n.id.as_str() {
                    "list" | "List" => Some(format!("Vec<{inner}>")),
                    "Optional" => Some(format!("Option<{inner}>")),
                    "dict" | "Dict" => None, // need pair
                    _ => None,
                },
                _ => None,
            }
        }
        _ => None,
    }
}

fn map_name(name: &str) -> Option<String> {
    match name {
        "int" => Some("i64".into()),
        "float" => Some("f64".into()),
        "str" => Some("String".into()),
        "bool" => Some("bool".into()),
        "bytes" => Some("Vec<u8>".into()),
        "None" => Some("()".into()),
        // Explicit dynamic — must not emit as if known (README DynamicTyping).
        "Any" => None,
        _ => None,
    }
}

fn map_typing_attr(name: &str) -> Option<String> {
    match name {
        "Any" => None,
        _ => None,
    }
}

/// True when the annotation is explicitly `Any` / `typing.Any`.
pub fn is_any_annotation(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Name(n) => n.id.as_str() == "Any",
        ast::Expr::Attribute(attr) => {
            attr.attr.as_str() == "Any"
                && matches!(attr.value.as_ref(), ast::Expr::Name(n) if n.id.as_str() == "typing")
        }
        _ => false,
    }
}

/// Rust keywords that a raw identifier (`r#…`) cannot rescue.
///
/// `r#crate` and friends are rejected by the parser outright, so these are the
/// only names that force a rename rather than an escape.
const UNRAWABLE: [&str; 5] = ["crate", "self", "Self", "super", "_"];

/// Every Rust 2021 keyword, strict and reserved.
///
/// Reserved words are included deliberately: `become` and `yield` are not
/// keywords *yet*, and emitting them bare would produce code that compiles today
/// and stops compiling on some future edition. Escaping costs nothing.
const RUST_KEYWORDS: [&str; 51] = [
    "as", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "false", "fn",
    "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref",
    "return", "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe",
    "use", "where", "while", "async", "await", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try",
];

/// Outcome of turning a Python name into a Rust identifier.
pub enum IdentFix {
    /// Already a legal Rust identifier — emitted unchanged.
    Verbatim,
    /// Escaped as `r#name`. The name is preserved exactly; nothing is lost.
    Raw,
    /// Renamed, because no raw form of this keyword exists. The caller **must**
    /// record a gap: a silently renamed symbol is a symbol callers can no longer
    /// find, and every caller in the emitted corpus still refers to the old one.
    Renamed,
}

/// Turn a Python identifier into one `rustc` will accept.
///
/// Found by the L3 gate, not by inspection: `tg-agent-relay` defines
/// `def true(name, cond, detail)` as a test helper, which lowered to `fn true(…)`
/// and took ten modules from "emitted" to "does not parse". Every gate below L3
/// scored those modules as successfully emitted.
pub fn rust_ident(py_name: &str) -> (String, IdentFix) {
    if !RUST_KEYWORDS.contains(&py_name) {
        return (py_name.to_string(), IdentFix::Verbatim);
    }
    if UNRAWABLE.contains(&py_name) {
        (format!("{py_name}_"), IdentFix::Renamed)
    } else {
        (format!("r#{py_name}"), IdentFix::Raw)
    }
}

/// Render annotation or mark missing.
pub fn map_or_default(expr: Option<&ast::Expr>, default: &str) -> Result<String, String> {
    match expr {
        None => Err(format!(
            "missing type annotation (would default to {default} only with DynamicTyping gap)"
        )),
        Some(e) if is_any_annotation(e) => {
            Err("annotation is Any — dynamic typing not lowered (flag not guess)".into())
        }
        Some(e) => map_type_expr(e).ok_or_else(|| {
            let start = e.range().start().to_u32();
            format!("unmapped type annotation at byte offset {start}")
        }),
    }
}

#[cfg(test)]
mod ident_tests {
    use super::*;

    /// (python name, emitted rust, kind)
    const CASES: &[(&str, &str, &str)] = &[
        ("add", "add", "verbatim"),
        ("_private", "_private", "verbatim"),
        // The one the L3 gate actually found, in ten modules at once.
        ("true", "r#true", "raw"),
        ("match", "r#match", "raw"),
        ("type", "r#type", "raw"),
        ("async", "r#async", "raw"),
        // Reserved-but-not-yet-keyword: escaped now so a future edition does not
        // silently break emitted code that used to build.
        ("yield", "r#yield", "raw"),
        // No raw form exists for these; renaming is the only legal lowering.
        ("crate", "crate_", "renamed"),
        ("self", "self_", "renamed"),
        ("super", "super_", "renamed"),
    ];

    fn kind(f: &IdentFix) -> &'static str {
        match f {
            IdentFix::Verbatim => "verbatim",
            IdentFix::Raw => "raw",
            IdentFix::Renamed => "renamed",
        }
    }

    #[test]
    fn python_names_become_legal_rust_identifiers() {
        for (py, want, want_kind) in CASES {
            let (got, fix) = rust_ident(py);
            assert_eq!(&got, want, "lowering of `{py}`");
            assert_eq!(kind(&fix), *want_kind, "classification of `{py}`");
        }
    }

    #[test]
    fn only_unrawable_keywords_are_renamed() {
        // Renaming loses the caller's ability to find the symbol, so it must be
        // the exception. Everything else is escaped losslessly.
        for kw in RUST_KEYWORDS {
            let (got, fix) = rust_ident(kw);
            match fix {
                IdentFix::Renamed => assert!(
                    UNRAWABLE.contains(&kw),
                    "`{kw}` was renamed but `r#{kw}` is legal — that loses the name \
                     for no reason"
                ),
                IdentFix::Raw => assert!(got.starts_with("r#")),
                IdentFix::Verbatim => panic!("`{kw}` is a keyword and must not be emitted bare"),
            }
        }
    }
}
