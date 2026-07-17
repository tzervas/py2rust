//! Best-effort Rust emission for expressible Python constructs (functions first).
//! Partial emission always carries sub-gaps (never silent TODO bodies).

use crate::gap::{Category, GapReason};
use crate::map::{is_any_annotation, map_type_expr};
use rustpython_parser::ast::{self, Ranged};

/// Result of attempting to emit a construct.
#[derive(Debug, Clone)]
pub struct Emitted {
    pub name: String,
    pub rust: String,
    pub sub_gaps: Vec<GapReason>,
}

/// Emit a Python `def` as a Rust `fn`.
///
/// Policy (P25):
/// - Missing / `Any` annotations → [`Category::DynamicTyping`] sub-gaps; still emit with `/* dyn */`
///   placeholders only when we also record the gap (flag not guess).
/// - Non-empty decorator list → [`Category::Metaprogramming`] (hard gap preferred by dispatch).
/// - Body not fully lowered → [`Category::FunctionBody`] sub-gap + honest comment (no silent TODO).
pub fn emit_function(func: &ast::StmtFunctionDef, source: &str) -> Emitted {
    let name = func.name.to_string();
    let mut sub_gaps = Vec::new();

    // Decorators: dispatch usually gaps the whole item; if we still get here, record sub-gap.
    if !func.decorator_list.is_empty() {
        sub_gaps.push(GapReason::new(
            Category::Metaprogramming,
            format!(
                "function `{}` has {} decorator(s) — metaprogramming not lowered (flag not guess)",
                name,
                func.decorator_list.len()
            ),
        ));
    }

    let mut args_out = Vec::new();
    for arg in func
        .args
        .posonlyargs
        .iter()
        .chain(func.args.args.iter())
        .chain(func.args.kwonlyargs.iter())
    {
        let aname = arg.def.arg.to_string();
        let ty = match arg.def.annotation.as_deref() {
            None => {
                sub_gaps.push(GapReason::new(
                    Category::DynamicTyping,
                    format!(
                        "parameter `{aname}` of `{name}` has no type annotation — dynamic typing (README)"
                    ),
                ));
                "/* dyn */ i32".to_string()
            }
            Some(ann) if is_any_annotation(ann) => {
                sub_gaps.push(GapReason::new(
                    Category::DynamicTyping,
                    format!("parameter `{aname}` of `{name}` annotated Any — dynamic typing (README)"),
                ));
                "/* Any */ i32".to_string()
            }
            Some(ann) => match map_type_expr(ann) {
                Some(t) => t,
                None => {
                    sub_gaps.push(GapReason::new(
                        Category::DynamicTyping,
                        format!(
                            "parameter `{aname}` of `{name}` has unmapped annotation — dynamic typing"
                        ),
                    ));
                    "/* unmapped */ i32".to_string()
                }
            },
        };
        args_out.push(format!("{aname}: {ty}"));
    }

    if func.args.vararg.is_some() || func.args.kwarg.is_some() {
        sub_gaps.push(GapReason::new(
            Category::Other,
            format!("function `{name}` uses *args/**kwargs — not lowered"),
        ));
    }

    let (ret_ty, ret_is_unit) = match func.returns.as_deref() {
        None => {
            // Bare `def f():` — treat as dynamic unless body is empty pass-only.
            if !is_pass_only_body(&func.body) {
                sub_gaps.push(GapReason::new(
                    Category::DynamicTyping,
                    format!(
                        "function `{name}` has no return annotation — dynamic typing (README)"
                    ),
                ));
            }
            ("i32".to_string(), false)
        }
        Some(r) if is_any_annotation(r) => {
            sub_gaps.push(GapReason::new(
                Category::DynamicTyping,
                format!("function `{name}` return annotated Any — dynamic typing (README)"),
            ));
            ("i32".to_string(), false)
        }
        Some(r) => match map_type_expr(r) {
            Some(t) if t == "()" => ("()".to_string(), true),
            Some(t) => (t, false),
            None => {
                sub_gaps.push(GapReason::new(
                    Category::DynamicTyping,
                    format!("function `{name}` has unmapped return annotation"),
                ));
                ("i32".to_string(), false)
            }
        },
    };

    // Nested honesty: scan body for exception / metaprogramming / lambda.
    scan_body_for_sub_gaps(&func.body, &name, &mut sub_gaps);

    let body_lowered = try_lower_simple_body(&func.body, ret_is_unit);
    let body_text = match body_lowered {
        Some(b) => b,
        None => {
            sub_gaps.push(GapReason::new(
                Category::FunctionBody,
                format!(
                    "function body of `{name}` not lowered — flag not guess (no silent TODO)"
                ),
            ));
            if ret_is_unit {
                "    // GAP: FunctionBody — body not lowered (flag not guess)\n".to_string()
            } else {
                format!(
                    "    // GAP: FunctionBody — body not lowered (flag not guess)\n    todo!(\"py2rust: body of `{name}` not lowered\")\n"
                )
            }
        }
    };

    let sig = if ret_is_unit {
        format!("fn {name}({}) {{", args_out.join(", "))
    } else {
        format!("fn {name}({}) -> {ret_ty} {{", args_out.join(", "))
    };

    let mut rust = String::new();
    rust.push_str(&sig);
    rust.push('\n');
    rust.push_str(&body_text);
    if !body_text.ends_with('\n') {
        rust.push('\n');
    }
    rust.push_str("}\n");

    // Snippet context unused for now but keeps source available for future fidelity notes.
    let _ = source;

    Emitted {
        name,
        rust,
        sub_gaps,
    }
}

fn is_pass_only_body(body: &[ast::Stmt]) -> bool {
    body.iter().all(|s| matches!(s, ast::Stmt::Pass(_)))
}

fn try_lower_simple_body(body: &[ast::Stmt], ret_is_unit: bool) -> Option<String> {
    if body.is_empty() {
        return Some(String::new());
    }
    // Single pass
    if body.len() == 1 && matches!(&body[0], ast::Stmt::Pass(_)) {
        return Some(String::new());
    }
    // Single return of constant / name / simple binop
    if body.len() == 1 {
        if let ast::Stmt::Return(r) = &body[0] {
            match r.value.as_deref() {
                None => return Some(String::new()),
                Some(expr) => {
                    let lit = lower_simple_expr(expr)?;
                    return Some(format!("    {lit}\n"));
                }
            }
        }
    }
    // Unit body of only pass / bare returns
    if ret_is_unit
        && body.iter().all(|s| {
            matches!(s, ast::Stmt::Pass(_))
                || matches!(s, ast::Stmt::Return(r) if r.value.is_none())
        })
    {
        return Some(String::new());
    }
    None
}

fn lower_simple_expr(expr: &ast::Expr) -> Option<String> {
    match expr {
        ast::Expr::Constant(c) => constant_to_rust(&c.value),
        ast::Expr::Name(n) => Some(n.id.to_string()),
        ast::Expr::BinOp(b) => {
            let left = lower_simple_expr(&b.left)?;
            let right = lower_simple_expr(&b.right)?;
            let op = match b.op {
                ast::Operator::Add => "+",
                ast::Operator::Sub => "-",
                ast::Operator::Mult => "*",
                ast::Operator::Div => "/",
                ast::Operator::Mod => "%",
                ast::Operator::BitOr => "|",
                ast::Operator::BitXor => "^",
                ast::Operator::BitAnd => "&",
                ast::Operator::LShift => "<<",
                ast::Operator::RShift => ">>",
                _ => return None,
            };
            Some(format!("({left} {op} {right})"))
        }
        ast::Expr::UnaryOp(u) => {
            let operand = lower_simple_expr(&u.operand)?;
            match u.op {
                ast::UnaryOp::UAdd => Some(operand),
                ast::UnaryOp::USub => Some(format!("(-{operand})")),
                ast::UnaryOp::Not => Some(format!("(!{operand})")),
                ast::UnaryOp::Invert => Some(format!("(!{operand})")),
            }
        }
        _ => None,
    }
}

fn constant_to_rust(c: &ast::Constant) -> Option<String> {
    match c {
        ast::Constant::Int(i) => Some(i.to_string()),
        ast::Constant::Float(f) => Some(format!("{f}")),
        ast::Constant::Bool(b) => Some(b.to_string()),
        ast::Constant::Str(s) => Some(format!("{:?}", s.as_str())),
        ast::Constant::None => Some("()".into()),
        _ => None,
    }
}

fn scan_body_for_sub_gaps(body: &[ast::Stmt], fname: &str, out: &mut Vec<GapReason>) {
    for stmt in body {
        match stmt {
            ast::Stmt::Try(_) | ast::Stmt::TryStar(_) | ast::Stmt::Raise(_) => {
                out.push(GapReason::new(
                    Category::Exception,
                    format!(
                        "exception handling inside `{fname}` not lowered (README Exception)"
                    ),
                ));
            }
            ast::Stmt::Expr(e) => {
                if contains_lambda(&e.value) {
                    out.push(GapReason::new(
                        Category::Lambda,
                        format!("lambda inside `{fname}` not lowered"),
                    ));
                }
                if contains_exec_eval(&e.value) {
                    out.push(GapReason::new(
                        Category::Metaprogramming,
                        format!("exec/eval inside `{fname}` not lowered (README Metaprogramming)"),
                    ));
                }
            }
            ast::Stmt::FunctionDef(_) | ast::Stmt::AsyncFunctionDef(_) => {
                out.push(GapReason::new(
                    Category::Other,
                    format!("nested function inside `{fname}` not lowered in this phase"),
                ));
            }
            ast::Stmt::ClassDef(c) => {
                out.push(GapReason::new(
                    Category::Class,
                    format!(
                        "nested class `{}` inside `{fname}` not lowered (README Class)",
                        c.name
                    ),
                ));
            }
            _ => {}
        }
    }
}

fn contains_lambda(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Lambda(_) => true,
        ast::Expr::Call(c) => {
            contains_lambda(&c.func)
                || c.args.iter().any(contains_lambda)
                || c.keywords.iter().any(|k| contains_lambda(&k.value))
        }
        ast::Expr::BinOp(b) => contains_lambda(&b.left) || contains_lambda(&b.right),
        ast::Expr::UnaryOp(u) => contains_lambda(&u.operand),
        ast::Expr::IfExp(i) => {
            contains_lambda(&i.test) || contains_lambda(&i.body) || contains_lambda(&i.orelse)
        }
        ast::Expr::List(l) => l.elts.iter().any(contains_lambda),
        ast::Expr::Tuple(t) => t.elts.iter().any(contains_lambda),
        _ => false,
    }
}

fn contains_exec_eval(expr: &ast::Expr) -> bool {
    match expr {
        ast::Expr::Call(c) => {
            if let ast::Expr::Name(n) = c.func.as_ref() {
                if n.id.as_str() == "exec" || n.id.as_str() == "eval" {
                    return true;
                }
            }
            c.args.iter().any(contains_exec_eval)
                || c.keywords.iter().any(|k| contains_exec_eval(&k.value))
        }
        _ => false,
    }
}

/// Placeholder for class emission — always a hard gap at dispatch layer.
pub fn class_gap_reason(class: &ast::StmtClassDef) -> GapReason {
    let bases = if class.bases.is_empty() {
        "no bases".to_string()
    } else {
        format!("{} base(s)", class.bases.len())
    };
    let meta = class.keywords.iter().any(|k| {
        k.arg
            .as_ref()
            .map(|a| a.as_str() == "metaclass")
            .unwrap_or(false)
    });
    let mut reason = format!(
        "class `{}` ({bases}) not lowered to Rust struct/impl — classes and inheritance (README)"
        ,
        class.name
    );
    if meta {
        reason.push_str("; metaclass present (Metaprogramming also applies)");
    }
    if !class.decorator_list.is_empty() {
        reason.push_str("; class decorators present (Metaprogramming also applies)");
    }
    GapReason::new(Category::Class, reason)
}

// Silence unused import if Ranged only needed for future span work in this module.
#[allow(dead_code)]
fn _range_start(stmt: &ast::Stmt) -> u32 {
    stmt.range().start().to_u32()
}
