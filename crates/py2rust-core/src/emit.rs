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
        walk_stmt(stmt, fname, out);
    }
}

fn walk_stmt(stmt: &ast::Stmt, fname: &str, out: &mut Vec<GapReason>) {
    match stmt {
        ast::Stmt::FunctionDef(f) => {
            out.push(GapReason::new(
                Category::Other,
                format!("nested function `{}` inside `{fname}` not lowered in this phase", f.name),
            ));
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncFunctionDef(f) => {
            out.push(GapReason::new(
                Category::Other,
                format!("nested async function `{}` inside `{fname}` not lowered in this phase", f.name),
            ));
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::ClassDef(c) => {
            out.push(GapReason::new(
                Category::Class,
                format!("nested class `{}` inside `{fname}` not lowered (README Class)", c.name),
            ));
            for s in &c.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::Return(r) => {
            if let Some(value) = &r.value {
                walk_expr(value, fname, out);
            }
        }
        ast::Stmt::Delete(d) => {
            for target in &d.targets {
                walk_expr(target, fname, out);
            }
        }
        ast::Stmt::Assign(a) => {
            for target in &a.targets {
                walk_expr(target, fname, out);
            }
            walk_expr(&a.value, fname, out);
        }
        ast::Stmt::AugAssign(a) => {
            walk_expr(&a.target, fname, out);
            walk_expr(&a.value, fname, out);
        }
        ast::Stmt::AnnAssign(a) => {
            walk_expr(&a.target, fname, out);
            walk_expr(&a.annotation, fname, out);
            if let Some(value) = &a.value {
                walk_expr(value, fname, out);
            }
        }
        ast::Stmt::For(f) => {
            walk_expr(&f.target, fname, out);
            walk_expr(&f.iter, fname, out);
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
            for s in &f.orelse {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncFor(f) => {
            walk_expr(&f.target, fname, out);
            walk_expr(&f.iter, fname, out);
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
            for s in &f.orelse {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::While(w) => {
            walk_expr(&w.test, fname, out);
            for s in &w.body {
                walk_stmt(s, fname, out);
            }
            for s in &w.orelse {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::If(i) => {
            walk_expr(&i.test, fname, out);
            for s in &i.body {
                walk_stmt(s, fname, out);
            }
            for s in &i.orelse {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::With(w) => {
            for item in &w.items {
                walk_expr(&item.context_expr, fname, out);
                if let Some(optional_vars) = &item.optional_vars {
                    walk_expr(optional_vars, fname, out);
                }
            }
            for s in &w.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncWith(w) => {
            for item in &w.items {
                walk_expr(&item.context_expr, fname, out);
                if let Some(optional_vars) = &item.optional_vars {
                    walk_expr(optional_vars, fname, out);
                }
            }
            for s in &w.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::Match(m) => {
            walk_expr(&m.subject, fname, out);
            for case in &m.cases {
                if let Some(guard) = &case.guard {
                    walk_expr(guard, fname, out);
                }
                for s in &case.body {
                    walk_stmt(s, fname, out);
                }
            }
        }
        ast::Stmt::Raise(r) => {
            out.push(GapReason::new(
                Category::Exception,
                format!("exception handling inside `{fname}` not lowered (README Exception)"),
            ));
            if let Some(exc) = &r.exc {
                walk_expr(exc, fname, out);
            }
            if let Some(cause) = &r.cause {
                walk_expr(cause, fname, out);
            }
        }
        ast::Stmt::Try(t) => {
            out.push(GapReason::new(
                Category::Exception,
                format!("exception handling inside `{fname}` not lowered (README Exception)"),
            ));
            for s in &t.body {
                walk_stmt(s, fname, out);
            }
            for handler in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = handler;
                if let Some(type_) = &h.type_ {
                    walk_expr(type_, fname, out);
                }
                for s in &h.body {
                    walk_stmt(s, fname, out);
                }
            }
            for s in &t.orelse {
                walk_stmt(s, fname, out);
            }
            for s in &t.finalbody {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::TryStar(t) => {
            out.push(GapReason::new(
                Category::Exception,
                format!("exception handling inside `{fname}` not lowered (README Exception)"),
            ));
            for s in &t.body {
                walk_stmt(s, fname, out);
            }
            for handler in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = handler;
                if let Some(type_) = &h.type_ {
                    walk_expr(type_, fname, out);
                }
                for s in &h.body {
                    walk_stmt(s, fname, out);
                }
            }
            for s in &t.orelse {
                walk_stmt(s, fname, out);
            }
            for s in &t.finalbody {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::Assert(a) => {
            walk_expr(&a.test, fname, out);
            if let Some(msg) = &a.msg {
                walk_expr(msg, fname, out);
            }
        }
        ast::Stmt::Import(i) => {
            let names: Vec<_> = i.names.iter().map(|a| a.name.to_string()).collect();
            out.push(GapReason::new(
                Category::Import,
                format!(
                    "nested import {} inside `{fname}` not lowered — unresolved / unmapped import (flag not guess)",
                    names.join(", ")
                ),
            ));
        }
        ast::Stmt::ImportFrom(i) => {
            let mod_name = i
                .module
                .as_ref()
                .map(|m| m.to_string())
                .unwrap_or_else(|| ".".into());
            out.push(GapReason::new(
                Category::Import,
                format!(
                    "nested from {mod_name} import … inside `{fname}` not lowered — unresolved / unmapped import"
                ),
            ));
        }
        ast::Stmt::Global(_) | ast::Stmt::Nonlocal(_) | ast::Stmt::Pass(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_) => {}
        ast::Stmt::Expr(e) => {
            walk_expr(&e.value, fname, out);
        }
        ast::Stmt::TypeAlias(t) => {
            walk_expr(&t.value, fname, out);
        }
    }
}

fn walk_expr(expr: &ast::Expr, fname: &str, out: &mut Vec<GapReason>) {
    if let ast::Expr::Lambda(l) = expr {
        out.push(GapReason::new(
            Category::Lambda,
            format!("lambda inside `{fname}` not lowered"),
        ));
        walk_expr(&l.body, fname, out);
        return;
    }

    if let ast::Expr::Call(c) = expr {
        if let ast::Expr::Name(n) = c.func.as_ref() {
            if n.id.as_str() == "exec" || n.id.as_str() == "eval" {
                out.push(GapReason::new(
                    Category::Metaprogramming,
                    format!("exec/eval inside `{fname}` not lowered (README Metaprogramming)"),
                ));
            }
        }
    }

    match expr {
        ast::Expr::BoolOp(b) => {
            for val in &b.values {
                walk_expr(val, fname, out);
            }
        }
        ast::Expr::NamedExpr(n) => {
            walk_expr(&n.target, fname, out);
            walk_expr(&n.value, fname, out);
        }
        ast::Expr::BinOp(b) => {
            walk_expr(&b.left, fname, out);
            walk_expr(&b.right, fname, out);
        }
        ast::Expr::UnaryOp(u) => {
            walk_expr(&u.operand, fname, out);
        }
        ast::Expr::Lambda(_) => unreachable!(),
        ast::Expr::IfExp(i) => {
            walk_expr(&i.test, fname, out);
            walk_expr(&i.body, fname, out);
            walk_expr(&i.orelse, fname, out);
        }
        ast::Expr::Dict(d) => {
            for key in &d.keys {
                if let Some(k) = key {
                    walk_expr(k, fname, out);
                }
            }
            for val in &d.values {
                walk_expr(val, fname, out);
            }
        }
        ast::Expr::Set(s) => {
            for elt in &s.elts {
                walk_expr(elt, fname, out);
            }
        }
        ast::Expr::ListComp(lc) => {
            walk_expr(&lc.elt, fname, out);
            for gen in &lc.generators {
                walk_comprehension(gen, fname, out);
            }
        }
        ast::Expr::SetComp(sc) => {
            walk_expr(&sc.elt, fname, out);
            for gen in &sc.generators {
                walk_comprehension(gen, fname, out);
            }
        }
        ast::Expr::DictComp(dc) => {
            walk_expr(&dc.key, fname, out);
            walk_expr(&dc.value, fname, out);
            for gen in &dc.generators {
                walk_comprehension(gen, fname, out);
            }
        }
        ast::Expr::GeneratorExp(ge) => {
            walk_expr(&ge.elt, fname, out);
            for gen in &ge.generators {
                walk_comprehension(gen, fname, out);
            }
        }
        ast::Expr::Await(a) => {
            walk_expr(&a.value, fname, out);
        }
        ast::Expr::Yield(y) => {
            if let Some(val) = &y.value {
                walk_expr(val, fname, out);
            }
        }
        ast::Expr::YieldFrom(y) => {
            walk_expr(&y.value, fname, out);
        }
        ast::Expr::Compare(c) => {
            walk_expr(&c.left, fname, out);
            for comparator in &c.comparators {
                walk_expr(comparator, fname, out);
            }
        }
        ast::Expr::Call(c) => {
            walk_expr(&c.func, fname, out);
            for arg in &c.args {
                walk_expr(arg, fname, out);
            }
            for kw in &c.keywords {
                walk_expr(&kw.value, fname, out);
            }
        }
        ast::Expr::FormattedValue(f) => {
            walk_expr(&f.value, fname, out);
            if let Some(spec) = &f.format_spec {
                walk_expr(spec, fname, out);
            }
        }
        ast::Expr::JoinedStr(j) => {
            for val in &j.values {
                walk_expr(val, fname, out);
            }
        }
        ast::Expr::Constant(_) => {}
        ast::Expr::Attribute(a) => {
            walk_expr(&a.value, fname, out);
        }
        ast::Expr::Subscript(s) => {
            walk_expr(&s.value, fname, out);
            walk_expr(&s.slice, fname, out);
        }
        ast::Expr::Starred(s) => {
            walk_expr(&s.value, fname, out);
        }
        ast::Expr::Name(_) => {}
        ast::Expr::List(l) => {
            for elt in &l.elts {
                walk_expr(elt, fname, out);
            }
        }
        ast::Expr::Tuple(t) => {
            for elt in &t.elts {
                walk_expr(elt, fname, out);
            }
        }
        ast::Expr::Slice(s) => {
            if let Some(lower) = &s.lower {
                walk_expr(lower, fname, out);
            }
            if let Some(upper) = &s.upper {
                walk_expr(upper, fname, out);
            }
            if let Some(step) = &s.step {
                walk_expr(step, fname, out);
            }
        }
    }
}

fn walk_comprehension(gen: &ast::Comprehension, fname: &str, out: &mut Vec<GapReason>) {
    walk_expr(&gen.target, fname, out);
    walk_expr(&gen.iter, fname, out);
    for cond in &gen.ifs {
        walk_expr(cond, fname, out);
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
