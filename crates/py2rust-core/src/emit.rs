//! Best-effort Rust emission for expressible Python constructs (functions first).
//! Partial emission always carries sub-gaps (never silent TODO bodies).

use crate::gap::{Category, GapReason};
use crate::map::{is_any_annotation, map_type_expr, rust_ident, IdentFix};
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

    // Rust-legal name. `name` stays the Python one — it is what the gap report
    // and every human-facing message refer to.
    let (rs_name, fix) = rust_ident(&name);
    if matches!(fix, IdentFix::Renamed) {
        sub_gaps.push(GapReason::new(
            Category::Other,
            format!(
                "function `{name}` is a Rust keyword with no raw form; emitted as `{rs_name}`. \
                 Callers still say `{name}` — renaming is the only legal lowering, so this is \
                 flagged rather than fixed."
            ),
        ));
    }

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
    let mut arg_types: Vec<(String, String)> = Vec::new();
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
                    format!(
                        "parameter `{aname}` of `{name}` annotated Any — dynamic typing (README)"
                    ),
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
        // Parameters need the same escape as the function name: `def f(type)`
        // and `def f(match)` are ordinary Python.
        let (rs_aname, afix) = rust_ident(&aname);
        if matches!(afix, IdentFix::Renamed) {
            sub_gaps.push(GapReason::new(
                Category::Other,
                format!(
                    "parameter `{aname}` of `{name}` is a Rust keyword with no raw form; \
                     emitted as `{rs_aname}`"
                ),
            ));
        }
        // Keyed on the Python name, because that is what the body refers to.
        arg_types.push((aname.clone(), ty.clone()));
        args_out.push(format!("{rs_aname}: {ty}"));
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
                    format!("function `{name}` has no return annotation — dynamic typing (README)"),
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

    // Only known types enter the environment. Placeholders (`/* dyn */ i32`)
    // stay out so inference declines instead of reasoning from a guess.
    let env: TypeEnv = arg_types
        .into_iter()
        .filter(|(_, t)| is_known_type(t))
        .collect();
    let ret_known = is_known_type(&ret_ty).then(|| ret_ty.clone());
    let body_lowered = try_lower_body(&func.body, ret_is_unit, &env, ret_known.as_deref(), None);
    let body_text = match body_lowered {
        Some(b) => b,
        None => {
            sub_gaps.push(GapReason::new(
                Category::FunctionBody,
                format!("function body of `{name}` not lowered — flag not guess (no silent TODO)"),
            ));
            // `todo!()` regardless of return type. A unit-returning function
            // used to get an empty body instead, which compiles *and returns
            // normally* — so an unlowered body silently did nothing at runtime,
            // which is the one thing this transpiler promises never to do. It
            // also made the body indistinguishable from a lowered one to any
            // check that scans the emitted text, which is how the L3 gate
            // initially credited 11 modules of pure scaffolding as real ports.
            format!(
                "    // GAP: FunctionBody — body not lowered (flag not guess)\n    todo!(\"py2rust: body of `{name}` not lowered\")\n"
            )
        }
    };

    let sig = if ret_is_unit {
        format!("fn {rs_name}({}) {{", args_out.join(", "))
    } else {
        format!("fn {rs_name}({}) -> {ret_ty} {{", args_out.join(", "))
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

/// Types we will put in the TypeEnv / treat as known for inference.
fn is_known_type(t: &str) -> bool {
    matches!(
        t,
        "i64" | "f64" | "bool" | "String" | "()" | "Vec<u8>" | "!"
    ) || t.starts_with("Vec<")
        || t.starts_with("Option<")
        || t.starts_with("std::collections::HashMap<")
        || t.starts_with("std::collections::HashSet<")
        || t.starts_with("std::path::")
        || (t.starts_with('(') && t.ends_with(')'))
}

/// What is known about the names in scope: parameter/local name → emitted Rust type.
///
/// Only unambiguous known types are recorded. A parameter whose type was a guess
/// (`/* dyn */ i32`) is deliberately absent, so inference declines rather than
/// reasoning from a placeholder.
pub type TypeEnv = std::collections::HashMap<String, String>;

/// Lower a function body. Multi-statement: assign / ann-assign / if / while /
/// for / pass / return / break / continue. Anything else declines the whole body.
fn try_lower_body(
    body: &[ast::Stmt],
    ret_is_unit: bool,
    env: &TypeEnv,
    ret_ty: Option<&str>,
    // Names bound before entering a loop. Reassignment of these inside the
    // loop would need `mut` (we only emit `let` shadows, which are wrong across
    // iterations). Decline rather than emit L3-green incorrect code.
    rebind_forbidden: Option<&TypeEnv>,
) -> Option<String> {
    if body.is_empty() {
        return Some(String::new());
    }
    // Single pass
    if body.len() == 1 && matches!(&body[0], ast::Stmt::Pass(_)) {
        return Some(String::new());
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

    let mut local_env = env.clone();
    let mut lines: Vec<String> = Vec::new();
    let last = body.len() - 1;

    for (i, stmt) in body.iter().enumerate() {
        let is_tail = i == last;
        match stmt {
            ast::Stmt::Pass(_) => continue,
            ast::Stmt::Return(r) => {
                match r.value.as_deref() {
                    None => {
                        if ret_is_unit {
                            if !is_tail {
                                lines.push("    return;".into());
                            }
                        } else {
                            return None;
                        }
                    }
                    Some(expr) => {
                        let lit = lower_simple_expr_in(expr, &local_env, ret_ty)?;
                        if is_tail {
                            // Tail return becomes the block value (no `return` keyword).
                            lines.push(format!("    {lit}"));
                        } else {
                            lines.push(format!("    return {lit};"));
                        }
                    }
                }
            }
            ast::Stmt::Assign(a) => {
                if a.targets.len() != 1 {
                    return None;
                }
                let name = match &a.targets[0] {
                    ast::Expr::Name(n) => n.id.to_string(),
                    _ => return None,
                };
                let (rs_name, fix) = rust_ident(&name);
                if matches!(fix, IdentFix::Renamed) {
                    return None;
                }
                // Loop rebind of an outer name → would need mut; decline (honesty).
                if rebind_forbidden.is_some_and(|e| e.contains_key(&name)) {
                    return None;
                }
                // Prefer type forced by RHS; fall back to existing env entry.
                let want = forced_ty(&a.value, &local_env)
                    .or_else(|| local_env.get(&name).cloned());
                let rhs = lower_simple_expr_in(&a.value, &local_env, want.as_deref())?;
                // Always record the binding so loop rebind detection sees untyped
                // literals (`total = 0`) as well as annotated names.
                local_env.insert(name, want.unwrap_or_else(|| "_".into()));
                lines.push(format!("    let {rs_name} = {rhs};"));
            }
            ast::Stmt::AnnAssign(a) => {
                let name = match a.target.as_ref() {
                    ast::Expr::Name(n) => n.id.to_string(),
                    _ => return None,
                };
                let (rs_name, fix) = rust_ident(&name);
                if matches!(fix, IdentFix::Renamed) {
                    return None;
                }
                let ann_ty = map_type_expr(a.annotation.as_ref())?;
                let value = a.value.as_deref()?;
                let rhs = lower_simple_expr_in(value, &local_env, Some(&ann_ty))?;
                local_env.insert(name, ann_ty);
                lines.push(format!("    let {rs_name} = {rhs};"));
            }
            ast::Stmt::AugAssign(a) => {
                let name = match a.target.as_ref() {
                    ast::Expr::Name(n) => n.id.to_string(),
                    _ => return None,
                };
                let (rs_name, fix) = rust_ident(&name);
                if matches!(fix, IdentFix::Renamed) {
                    return None;
                }
                if rebind_forbidden.is_some_and(|e| e.contains_key(&name)) {
                    return None;
                }
                let want = local_env.get(&name).cloned();
                let rhs = lower_simple_expr_in(&a.value, &local_env, want.as_deref())?;
                // No mut tracking yet: rewrite `x += rhs` as shadow `let x = (x + rhs)`,
                // which is always legal Rust. Decline unsupported ops rather than invent mut.
                let bin = match a.op {
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
                lines.push(format!("    let {rs_name} = ({rs_name} {bin} {rhs});"));
            }
            ast::Stmt::If(i) => {
                let block = lower_if(i, &local_env, ret_ty, is_tail, ret_is_unit, rebind_forbidden)?;
                lines.push(block);
            }
            ast::Stmt::While(w) => {
                let block = lower_while(w, &local_env, ret_ty, rebind_forbidden)?;
                lines.push(block);
            }
            ast::Stmt::For(f) => {
                let block = lower_for(f, &local_env, ret_ty, rebind_forbidden)?;
                lines.push(block);
            }
            ast::Stmt::Break(_) => {
                lines.push("    break;".into());
            }
            ast::Stmt::Continue(_) => {
                lines.push("    continue;".into());
            }
            _ => return None,
        }
    }

    // Non-unit functions must end in a value-producing tail.
    if !ret_is_unit {
        let has_value_tail = body
            .last()
            .map(|s| matches!(s, ast::Stmt::Return(r) if r.value.is_some()) || matches!(s, ast::Stmt::If(_)))
            .unwrap_or(false);
        if !has_value_tail {
            return None;
        }
    }

    Some(lines.join("\n") + "\n")
}

fn lower_if(
    i: &ast::StmtIf,
    env: &TypeEnv,
    ret_ty: Option<&str>,
    is_tail: bool,
    ret_is_unit: bool,
    rebind_forbidden: Option<&TypeEnv>,
) -> Option<String> {
    let test = lower_simple_expr_in(&i.test, env, Some("bool"))?;
    let body = try_lower_body(&i.body, ret_is_unit, env, ret_ty, rebind_forbidden)?;
    let body_inner = indent_block(&body);
    if i.orelse.is_empty() {
        // Bare if without else cannot be a value-producing tail unless unit.
        if is_tail && !ret_is_unit {
            return None;
        }
        return Some(format!("    if {test} {{\n{body_inner}    }}"));
    }
    // `elif` chains arrive as a single Stmt::If in orelse.
    let else_inner = if i.orelse.len() == 1 {
        if let ast::Stmt::If(nested) = &i.orelse[0] {
            let nested_block =
                lower_if(nested, env, ret_ty, is_tail, ret_is_unit, rebind_forbidden)?;
            // nested_block already has leading indent; strip one level for else arm.
            return Some(format!(
                "    if {test} {{\n{body_inner}    }} else {{\n{}\n    }}",
                strip_outer_indent(&nested_block)
            ));
        }
        try_lower_body(&i.orelse, ret_is_unit, env, ret_ty, rebind_forbidden)?
    } else {
        try_lower_body(&i.orelse, ret_is_unit, env, ret_ty, rebind_forbidden)?
    };
    let else_inner = indent_block(&else_inner);
    Some(format!(
        "    if {test} {{\n{body_inner}    }} else {{\n{else_inner}    }}"
    ))
}

fn lower_while(
    w: &ast::StmtWhile,
    env: &TypeEnv,
    ret_ty: Option<&str>,
    outer_rebind: Option<&TypeEnv>,
) -> Option<String> {
    if !w.orelse.is_empty() {
        // while/else is Python-specific; decline rather than drop the else.
        return None;
    }
    let test = lower_simple_expr_in(&w.test, env, Some("bool"))?;
    // Forbid rebinding anything visible at loop entry (outer_rebind ∪ env).
    // `env` alone is enough: it already contains outer names.
    let _ = outer_rebind; // retained for call-site symmetry / future union
    let body = try_lower_body(&w.body, true, env, ret_ty, Some(env))?;
    let body_inner = indent_block(&body);
    Some(format!("    while {test} {{\n{body_inner}    }}"))
}

/// `for x in iterable:` — only shapes we can lower honestly:
/// - `range(n)` → `0..n`
/// - `range(a, b)` → `a..b` (step ≠ 1 declined)
/// - bare name / simple expr that already lowers → `for x in <expr>`
///
/// for/else declined (Python-only). Unpacking targets declined.
/// Rebinding names bound before the loop is declined (needs `mut`).
fn lower_for(
    f: &ast::StmtFor,
    env: &TypeEnv,
    ret_ty: Option<&str>,
    outer_rebind: Option<&TypeEnv>,
) -> Option<String> {
    if !f.orelse.is_empty() {
        return None;
    }
    let target = match f.target.as_ref() {
        ast::Expr::Name(n) => n.id.to_string(),
        _ => return None,
    };
    let (rs_target, fix) = rust_ident(&target);
    if matches!(fix, IdentFix::Renamed) {
        return None;
    }
    let iter = lower_for_iter(f.iter.as_ref(), env)?;
    let _ = outer_rebind;
    // Env at loop entry: reassignment of these names needs mut — decline.
    let body = try_lower_body(&f.body, true, env, ret_ty, Some(env))?;
    let body_inner = indent_block(&body);
    Some(format!(
        "    for {rs_target} in {iter} {{\n{body_inner}    }}"
    ))
}

fn lower_for_iter(expr: &ast::Expr, env: &TypeEnv) -> Option<String> {
    // range(...) special-case — the free win on every numeric loop in the corpus.
    if let ast::Expr::Call(c) = expr {
        if let ast::Expr::Name(n) = c.func.as_ref() {
            if n.id.as_str() == "range" && c.keywords.is_empty() {
                return match c.args.len() {
                    1 => {
                        let end = lower_simple_expr_in(&c.args[0], env, Some("i64"))?;
                        Some(format!("0..{end}"))
                    }
                    2 => {
                        let start = lower_simple_expr_in(&c.args[0], env, Some("i64"))?;
                        let end = lower_simple_expr_in(&c.args[1], env, Some("i64"))?;
                        Some(format!("{start}..{end}"))
                    }
                    // step requires step_by(usize) and sign handling — decline for honesty
                    _ => None,
                };
            }
        }
    }
    lower_simple_expr_in(expr, env, None)
}

fn indent_block(block: &str) -> String {
    block
        .lines()
        .map(|l| {
            if l.is_empty() {
                String::new()
            } else {
                format!("    {l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + if block.ends_with('\n') || block.is_empty() {
            "\n"
        } else {
            "\n"
        }
}

fn strip_outer_indent(block: &str) -> String {
    block
        .lines()
        .map(|l| l.strip_prefix("    ").unwrap_or(l))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The type an expression *forces*, ignoring anything a literal could adapt to.
///
/// The distinction is the whole trick. A bound name has a type it cannot change;
/// a numeric literal has whichever type its context needs — in Python `48` and
/// `48.0` are interchangeable, and in Rust the literal can be written either
/// way. So literals return `None` here, meaning "no opinion", and the expected
/// type from context is free to decide.
///
/// Getting this backwards is not hypothetical: inferring `1 if b else 2` as
/// `i64` from its two literals made that inference beat the `-> f64` return type
/// and emitted `if b { 1 } else { 2 }` into a float function.
///
/// Exists to answer one question — is this context floating-point? Python
/// happily writes `window_hours <= 48` where `window_hours` is a float; Rust
/// requires `48.0`. Found by the L3 gate on `lib/metrics_agg.py`.
fn forced_ty(expr: &ast::Expr, env: &TypeEnv) -> Option<String> {
    match expr {
        ast::Expr::Name(n) => env.get(n.id.as_str()).cloned(),
        // Literals adapt. A string literal does not, but nothing here would
        // coerce one, so treating it as adaptable costs nothing.
        ast::Expr::Constant(_) => None,
        // Numeric promotion, one level: if either operand forces float, so does
        // the result.
        ast::Expr::BinOp(b) => promote(
            forced_ty(&b.left, env).as_deref(),
            forced_ty(&b.right, env).as_deref(),
        ),
        // These genuinely produce bool whatever their operands are.
        ast::Expr::Compare(_) | ast::Expr::BoolOp(_) => Some("bool".into()),
        ast::Expr::UnaryOp(u) => match u.op {
            ast::UnaryOp::Not => Some("bool".into()),
            _ => forced_ty(&u.operand, env),
        },
        ast::Expr::IfExp(i) => promote(
            forced_ty(&i.body, env).as_deref(),
            forced_ty(&i.orelse, env).as_deref(),
        ),
        _ => None,
    }
}

/// Combine two operand types. Float wins; otherwise they must agree.
fn promote(l: Option<&str>, r: Option<&str>) -> Option<String> {
    match (l, r) {
        (Some("f64"), _) | (_, Some("f64")) => Some("f64".into()),
        (Some(a), Some(b)) if a == b => Some(a.into()),
        // One side unknown: an unknown operand makes the result unknown. Guessing
        // here would be exactly the silent inference this project refuses.
        _ => None,
    }
}

/// Lower an expression, adjusting *literals* to the expected type.
///
/// Literals only. Where a named value's type does not match its context, this
/// leaves the mismatch alone for `rustc` to reject: inserting a cast would
/// change what the program does, silently, on a guess. Adjusting a literal
/// changes nothing — Python's `48` in a float context already *is* `48.0`.
fn lower_simple_expr_in(expr: &ast::Expr, env: &TypeEnv, expected: Option<&str>) -> Option<String> {
    match expr {
        ast::Expr::Constant(c) => constant_to_rust_as(&c.value, expected),
        ast::Expr::Name(n) => {
            let (rs, fix) = rust_ident(n.id.as_str());
            if matches!(fix, IdentFix::Renamed) {
                None
            } else {
                Some(rs)
            }
        }
        ast::Expr::BinOp(b) => {
            // Both operands share a type in Rust, so settle it once and lower
            // each side against it.
            let want = promote(
                forced_ty(&b.left, env).as_deref(),
                forced_ty(&b.right, env).as_deref(),
            )
            .or_else(|| expected.map(str::to_string));
            let left = lower_simple_expr_in(&b.left, env, want.as_deref())?;
            let right = lower_simple_expr_in(&b.right, env, want.as_deref())?;
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
            let operand = lower_simple_expr_in(&u.operand, env, expected)?;
            match u.op {
                ast::UnaryOp::UAdd => Some(operand),
                ast::UnaryOp::USub => Some(format!("(-{operand})")),
                ast::UnaryOp::Not => Some(format!("(!{operand})")),
                ast::UnaryOp::Invert => Some(format!("(!{operand})")),
            }
        }
        ast::Expr::Compare(c) => {
            if c.ops.len() == 1 && c.comparators.len() == 1 {
                // The comparison yields bool, but its two operands must agree
                // with each other — not with the surrounding expected type.
                let want = promote(
                    forced_ty(&c.left, env).as_deref(),
                    forced_ty(&c.comparators[0], env).as_deref(),
                );
                let left = lower_simple_expr_in(&c.left, env, want.as_deref())?;
                let right = lower_simple_expr_in(&c.comparators[0], env, want.as_deref())?;
                let op = match c.ops[0] {
                    ast::CmpOp::Eq => "==",
                    ast::CmpOp::NotEq => "!=",
                    ast::CmpOp::Lt => "<",
                    ast::CmpOp::LtE => "<=",
                    ast::CmpOp::Gt => ">",
                    ast::CmpOp::GtE => ">=",
                    _ => return None,
                };
                Some(format!("({left} {op} {right})"))
            } else {
                None
            }
        }
        ast::Expr::BoolOp(b) => {
            let op = match b.op {
                ast::BoolOp::And => "&&",
                ast::BoolOp::Or => "||",
            };
            let mut parts = Vec::new();
            for val in &b.values {
                parts.push(lower_simple_expr_in(val, env, Some("bool"))?);
            }
            if parts.is_empty() {
                None
            } else {
                Some(format!("({})", parts.join(&format!(" {op} "))))
            }
        }
        ast::Expr::IfExp(i) => {
            // Both arms are one Rust type. `_bucket_size_seconds` in
            // tg-agent-relay is exactly this shape: `3600 if h <= 48 else 86400`
            // returned as a float.
            let want = promote(
                forced_ty(&i.body, env).as_deref(),
                forced_ty(&i.orelse, env).as_deref(),
            )
            .or_else(|| expected.map(str::to_string));
            let test = lower_simple_expr_in(&i.test, env, Some("bool"))?;
            let body = lower_simple_expr_in(&i.body, env, want.as_deref())?;
            let orelse = lower_simple_expr_in(&i.orelse, env, want.as_deref())?;
            Some(format!("(if {test} {{ {body} }} else {{ {orelse} }})"))
        }
        ast::Expr::Tuple(t) => {
            let mut parts = Vec::new();
            for e in &t.elts {
                parts.push(lower_simple_expr_in(e, env, None)?);
            }
            Some(format!("({})", parts.join(", ")))
        }
        ast::Expr::List(l) => {
            let mut parts = Vec::new();
            for e in &l.elts {
                parts.push(lower_simple_expr_in(e, env, None)?);
            }
            Some(format!("vec![{}]", parts.join(", ")))
        }
        _ => None,
    }
}

/// Render a Python constant as Rust, honouring an expected type for numerics.
fn constant_to_rust_as(c: &ast::Constant, expected: Option<&str>) -> Option<String> {
    match c {
        // An int literal in a float context. Python already treats these as
        // interchangeable; Rust does not, and `48` against an `f64` is E0308.
        ast::Constant::Int(i) if expected == Some("f64") => Some(format!("{i}.0")),
        ast::Constant::Int(i) => Some(i.to_string()),
        // `{}` on an f64 drops a whole `.0`: `format!("{}", 1.0f64)` is `"1"`,
        // which is an *integer* literal in Rust. So `return 1.0` from a `-> f64`
        // emitted `1` and did not compile. `{:?}` keeps the point.
        ast::Constant::Float(f) => Some(format!("{f:?}")),
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
                format!(
                    "nested function `{}` inside `{fname}` not lowered in this phase",
                    f.name
                ),
            ));
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncFunctionDef(f) => {
            out.push(GapReason::new(
                Category::Other,
                format!(
                    "nested async function `{}` inside `{fname}` not lowered in this phase",
                    f.name
                ),
            ));
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::ClassDef(c) => {
            out.push(GapReason::new(
                Category::Class,
                format!(
                    "nested class `{}` inside `{fname}` not lowered (README Class)",
                    c.name
                ),
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
            for t in &d.targets {
                walk_expr(t, fname, out);
            }
        }
        ast::Stmt::Assign(a) => {
            for t in &a.targets {
                walk_expr(t, fname, out);
            }
            walk_expr(&a.value, fname, out);
        }
        ast::Stmt::AugAssign(a) => {
            walk_expr(&a.target, fname, out);
            walk_expr(&a.value, fname, out);
        }
        ast::Stmt::AnnAssign(a) => {
            walk_expr(&a.target, fname, out);
            if let Some(v) = &a.value {
                walk_expr(v, fname, out);
            }
        }
        ast::Stmt::For(f) => {
            walk_expr(&f.iter, fname, out);
            for s in &f.body {
                walk_stmt(s, fname, out);
            }
            for s in &f.orelse {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncFor(f) => {
            out.push(GapReason::new(
                Category::Async,
                format!("async for inside `{fname}`"),
            ));
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
            }
            for s in &w.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::AsyncWith(w) => {
            out.push(GapReason::new(
                Category::Async,
                format!("async with inside `{fname}`"),
            ));
            for item in &w.items {
                walk_expr(&item.context_expr, fname, out);
            }
            for s in &w.body {
                walk_stmt(s, fname, out);
            }
        }
        ast::Stmt::Match(m) => {
            walk_expr(&m.subject, fname, out);
            for case in &m.cases {
                for s in &case.body {
                    walk_stmt(s, fname, out);
                }
            }
        }
        ast::Stmt::Raise(r) => {
            out.push(GapReason::new(
                Category::Exception,
                format!("raise inside `{fname}`"),
            ));
            if let Some(e) = &r.exc {
                walk_expr(e, fname, out);
            }
        }
        ast::Stmt::Try(t) => {
            out.push(GapReason::new(
                Category::Exception,
                format!("try/except inside `{fname}`"),
            ));
            for s in &t.body {
                walk_stmt(s, fname, out);
            }
            for h in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = h;
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
                format!("try/except* inside `{fname}`"),
            ));
            for s in &t.body {
                walk_stmt(s, fname, out);
            }
            for h in &t.handlers {
                let ast::ExceptHandler::ExceptHandler(h) = h;
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
            if let Some(m) = &a.msg {
                walk_expr(m, fname, out);
            }
        }
        ast::Stmt::Import(i) => {
            for a in &i.names {
                out.push(GapReason::new(
                    Category::Import,
                    format!("import `{}` inside `{fname}`", a.name),
                ));
            }
        }
        ast::Stmt::ImportFrom(i) => {
            let mod_name = i.module.as_deref().unwrap_or("");
            out.push(GapReason::new(
                Category::Import,
                format!("from {mod_name} import … inside `{fname}`"),
            ));
        }
        ast::Stmt::Global(_)
        | ast::Stmt::Nonlocal(_)
        | ast::Stmt::Pass(_)
        | ast::Stmt::Break(_)
        | ast::Stmt::Continue(_) => {}
        ast::Stmt::Expr(e) => {
            walk_expr(&e.value, fname, out);
        }
        ast::Stmt::TypeAlias(t) => {
            walk_expr(&t.name, fname, out);
            walk_expr(&t.value, fname, out);
        }
    }
}

fn walk_expr(expr: &ast::Expr, fname: &str, out: &mut Vec<GapReason>) {
    if let ast::Expr::Lambda(l) = expr {
        out.push(GapReason::new(
            Category::Lambda,
            format!("lambda inside `{fname}`"),
        ));
        walk_expr(&l.body, fname, out);
        return;
    }
    if let ast::Expr::Call(c) = expr {
        if let ast::Expr::Name(n) = c.func.as_ref() {
            if matches!(n.id.as_str(), "exec" | "eval") {
                out.push(GapReason::new(
                    Category::Metaprogramming,
                    format!("`{}` call inside `{fname}`", n.id),
                ));
            }
        }
    }
    match expr {
        ast::Expr::BoolOp(b) => {
            for v in &b.values {
                walk_expr(v, fname, out);
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
        ast::Expr::UnaryOp(u) => walk_expr(&u.operand, fname, out),
        ast::Expr::Lambda(_) => unreachable!(),
        ast::Expr::IfExp(i) => {
            walk_expr(&i.test, fname, out);
            walk_expr(&i.body, fname, out);
            walk_expr(&i.orelse, fname, out);
        }
        ast::Expr::Dict(d) => {
            for k in d.keys.iter().flatten() {
                walk_expr(k, fname, out);
            }
            for v in &d.values {
                walk_expr(v, fname, out);
            }
        }
        ast::Expr::Set(s) => {
            for e in &s.elts {
                walk_expr(e, fname, out);
            }
        }
        ast::Expr::ListComp(lc) => {
            out.push(comp_gap(fname, "list"));
            walk_expr(&lc.elt, fname, out);
        }
        ast::Expr::SetComp(sc) => {
            out.push(comp_gap(fname, "set"));
            walk_expr(&sc.elt, fname, out);
        }
        ast::Expr::DictComp(dc) => {
            out.push(comp_gap(fname, "dict"));
            walk_expr(&dc.key, fname, out);
            walk_expr(&dc.value, fname, out);
        }
        ast::Expr::GeneratorExp(ge) => {
            out.push(comp_gap(fname, "generator"));
            walk_expr(&ge.elt, fname, out);
        }
        ast::Expr::Await(a) => {
            out.push(GapReason::new(Category::Async, format!("await in `{fname}`")));
            walk_expr(&a.value, fname, out);
        }
        ast::Expr::Yield(y) => {
            if let Some(v) = &y.value {
                walk_expr(v, fname, out);
            }
        }
        ast::Expr::YieldFrom(y) => walk_expr(&y.value, fname, out),
        ast::Expr::Compare(c) => {
            walk_expr(&c.left, fname, out);
            for r in &c.comparators {
                walk_expr(r, fname, out);
            }
        }
        ast::Expr::Call(c) => {
            walk_expr(&c.func, fname, out);
            for a in &c.args {
                walk_expr(a, fname, out);
            }
            for k in &c.keywords {
                walk_expr(&k.value, fname, out);
            }
        }
        ast::Expr::FormattedValue(f) => {
            walk_expr(&f.value, fname, out);
            if let Some(fmt) = &f.format_spec {
                walk_expr(fmt, fname, out);
            }
        }
        ast::Expr::JoinedStr(j) => {
            for v in &j.values {
                walk_expr(v, fname, out);
            }
        }
        ast::Expr::Constant(_) => {}
        ast::Expr::Attribute(a) => walk_expr(&a.value, fname, out),
        ast::Expr::Subscript(s) => {
            walk_expr(&s.value, fname, out);
            walk_expr(&s.slice, fname, out);
        }
        ast::Expr::Starred(s) => walk_expr(&s.value, fname, out),
        ast::Expr::Name(_) => {}
        ast::Expr::List(l) => {
            for e in &l.elts {
                walk_expr(e, fname, out);
            }
        }
        ast::Expr::Tuple(t) => {
            for e in &t.elts {
                walk_expr(e, fname, out);
            }
        }
        ast::Expr::Slice(s) => {
            if let Some(l) = &s.lower {
                walk_expr(l, fname, out);
            }
            if let Some(u) = &s.upper {
                walk_expr(u, fname, out);
            }
            if let Some(st) = &s.step {
                walk_expr(st, fname, out);
            }
        }
    }
}

fn comp_gap(fname: &str, kind: &str) -> GapReason {
    GapReason::new(
        Category::Comprehension,
        format!("{kind} comprehension inside `{fname}`"),
    )
}

/// Emit a class as a hard Class gap (dispatch handles structure).
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
        "class `{}` ({bases}) not lowered to Rust struct/impl — classes and inheritance (README)",
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
