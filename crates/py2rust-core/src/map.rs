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
