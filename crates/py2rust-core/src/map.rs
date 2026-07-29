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
        ast::Expr::Attribute(attr) => map_attribute(attr),
        ast::Expr::Subscript(sub) => map_subscript(sub),
        // PEP 604: `X | Y` → `Option` when one arm is None, else refuse rather than invent enums.
        ast::Expr::BinOp(b) if matches!(b.op, ast::Operator::BitOr) => map_union_binop(b),
        // `tuple[int, str]` already handled via Subscript; bare `()` is Constant/None above.
        _ => None,
    }
}

fn map_attribute(attr: &ast::ExprAttribute) -> Option<String> {
    let base = match attr.value.as_ref() {
        ast::Expr::Name(n) => n.id.as_str(),
        _ => return None,
    };
    let leaf = attr.attr.as_str();
    match (base, leaf) {
        ("typing", name) => map_typing_attr(name),
        // pathlib.Path and os.PathLike are the free table wins from the architecture
        // doc §6 — Path appears constantly and is a single-name mapping.
        ("pathlib", "Path") | ("os", "PathLike") => Some("std::path::PathBuf".into()),
        // collections.abc containers map the same as builtins when bare (no params).
        ("collections", "abc") => None,
        _ => None,

    }
}

fn map_subscript(sub: &ast::ExprSubscript) -> Option<String> {
    let head = type_head(sub.value.as_ref())?;
    match head.as_str() {
        "list" | "List" | "Sequence" | "MutableSequence" => {
            let inner = map_type_expr(&sub.slice)?;
            Some(format!("Vec<{inner}>"))
        }
        "set" | "Set" | "FrozenSet" | "frozenset" => {
            let inner = map_type_expr(&sub.slice)?;
            Some(format!("std::collections::HashSet<{inner}>"))
        }
        "Optional" => {
            let inner = map_type_expr(&sub.slice)?;
            Some(format!("Option<{inner}>"))
        }
        "dict" | "Dict" | "Mapping" | "MutableMapping" => {
            let (k, v) = pair_from_slice(&sub.slice)?;
            let k = map_type_expr(&k)?;
            let v = map_type_expr(&v)?;
            Some(format!("std::collections::HashMap<{k}, {v}>"))
        }
        "tuple" | "Tuple" => map_tuple_args(&sub.slice),
        "Union" => map_union_args(&sub.slice),
        // Callable / Iterable / Iterator left unmapped — need arg structure we
        // do not yet lower honestly. Flag, don't invent `fn()` placeholders.
        _ => None,
    }
}

/// `X | Y | None` style unions via BitOr.
fn map_union_binop(b: &ast::ExprBinOp) -> Option<String> {
    let mut arms = Vec::new();
    collect_bitor_arms(&ast::Expr::BinOp(b.clone()), &mut arms);
    map_union_list(&arms)
}

fn collect_bitor_arms(expr: &ast::Expr, out: &mut Vec<ast::Expr>) {
    match expr {
        ast::Expr::BinOp(b) if matches!(b.op, ast::Operator::BitOr) => {
            collect_bitor_arms(&b.left, out);
            collect_bitor_arms(&b.right, out);
        }
        other => out.push(other.clone()),
    }
}

fn map_union_args(slice: &ast::Expr) -> Option<String> {
    let arms = match slice {
        ast::Expr::Tuple(t) => t.elts.clone(),
        other => vec![other.clone()],
    };
    map_union_list(&arms)
}

/// Only lower unions that are exactly `T | None` / `Optional[T]` shape.
/// Anything broader is a real enum decision and must stay a gap.
fn map_union_list(arms: &[ast::Expr]) -> Option<String> {
    if arms.is_empty() {
        return None;
    }
    let mut non_none = Vec::new();
    let mut saw_none = false;
    for a in arms {
        if is_none_type(a) {
            saw_none = true;
        } else {
            non_none.push(a);
        }
    }
    if saw_none && non_none.len() == 1 {
        let inner = map_type_expr(non_none[0])?;
        return Some(format!("Option<{inner}>"));
    }
    // Single non-None after stripping nothing — just map it.
    if !saw_none && non_none.len() == 1 {
        return map_type_expr(non_none[0]);
    }
    None
}

fn is_none_type(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Constant(c) => matches!(c.value, ast::Constant::None),
        ast::Expr::Name(n) => n.id.as_str() == "None",
        _ => false,
    }
}

fn map_tuple_args(slice: &ast::Expr) -> Option<String> {
    match slice {
        // tuple[()] empty
        ast::Expr::Tuple(t) if t.elts.is_empty() => Some("()".into()),
        ast::Expr::Tuple(t) => {
            let mut parts = Vec::new();
            for e in &t.elts {
                // `tuple[int, ...]` variable-length — decline rather than guess Vec.
                if matches!(e, ast::Expr::Constant(c) if matches!(c.value, ast::Constant::Ellipsis))
                {
                    return None;
                }
                parts.push(map_type_expr(e)?);
            }
            Some(format!("({})", parts.join(", ")))
        }
        // Single-element subscript `tuple[int]` is a 1-tuple in typing.
        other => {
            let inner = map_type_expr(other)?;
            Some(format!("({inner},)"))
        }
    }
}

fn pair_from_slice(slice: &ast::Expr) -> Option<(ast::Expr, ast::Expr)> {
    match slice {
        ast::Expr::Tuple(t) if t.elts.len() == 2 => Some((t.elts[0].clone(), t.elts[1].clone())),
        _ => None,
    }
}

/// Head name of a type constructor: `list`, `typing.List`, `pathlib.Path`, …
fn type_head(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Name(n) => Some(n.id.to_string()),
        ast::Expr::Attribute(attr) => Some(attr.attr.to_string()),
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
        "bytearray" => Some("Vec<u8>".into()),
        "None" => Some("()".into()),
        // Builtin containers without type args — decline; empty containers need
        // element types and inventing `Vec<()>` would lie.
        "list" | "List" | "dict" | "Dict" | "set" | "Set" | "tuple" | "Tuple" => None,
        // Path is so common it earns a bare-name alias (from `from pathlib import Path`).
        "Path" => Some("std::path::PathBuf".into()),
        // Explicit dynamic — must not emit as if known (README DynamicTyping).
        "Any" => None,
        _ => None,
    }
}

fn map_typing_attr(name: &str) -> Option<String> {
    match name {
        "Any" => None,
        // Bare typing.Optional / List without args — unmapped.
        "List" | "Dict" | "Set" | "Tuple" | "Optional" | "Union" | "Sequence" | "Mapping" => None,
        "NoReturn" | "Never" => Some("!".into()),
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

/// Modules whose import is a pure no-op in Rust emission (erase, do not gap).
///
/// `__future__` annotations and `typing` names are compile-time Python surface
/// that leave no runtime residue once annotations are resolved. Recording them
/// as Import gaps inflated DynamicTyping-adjacent noise on every annotated file.
pub fn is_erasable_import_module(module: &str) -> bool {
    matches!(module, "__future__" | "typing" | "typing_extensions")
}

#[cfg(test)]
mod type_map_tests {
    use super::*;
    use rustpython_parser::{parse, Mode};

    fn ann(src: &str) -> ast::Expr {
        // Parse `x: <src> = None` and pull the annotation.
        let mod_src = format!("x: {src} = None\n");
        let m = parse(&mod_src, Mode::Module, "<test>").expect("parse");
        let ast::Mod::Module(module) = m else {
            panic!("not a module");
        };
        let ast::Stmt::AnnAssign(a) = &module.body[0] else {
            panic!("not annassign");
        };
        a.annotation.as_ref().clone()
    }

    #[test]
    fn scalars_and_containers() {
        assert_eq!(map_type_expr(&ann("int")).as_deref(), Some("i64"));
        assert_eq!(map_type_expr(&ann("str")).as_deref(), Some("String"));
        assert_eq!(map_type_expr(&ann("list[int]")).as_deref(), Some("Vec<i64>"));
        assert_eq!(
            map_type_expr(&ann("dict[str, int]")).as_deref(),
            Some("std::collections::HashMap<String, i64>")
        );
        assert_eq!(
            map_type_expr(&ann("Optional[str]")).as_deref(),
            Some("Option<String>")
        );
        assert_eq!(
            map_type_expr(&ann("str | None")).as_deref(),
            Some("Option<String>")
        );
        assert_eq!(
            map_type_expr(&ann("tuple[int, str]")).as_deref(),
            Some("(i64, String)")
        );
        assert_eq!(
            map_type_expr(&ann("Path")).as_deref(),
            Some("std::path::PathBuf")
        );
        // Broad unions stay gaps — enum decision is not ours to invent.
        assert_eq!(map_type_expr(&ann("int | str")), None);
        assert_eq!(map_type_expr(&ann("Any")), None);
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
