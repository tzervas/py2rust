//! Robust AST walking helpers to recursively traverse Python AST nodes (G2 / VR-5).
//! This avoids duplicate walk logic and guarantees never-silent drops inside complex
//! constructs.

use rustpython_parser::ast;

/// Walk an expression recursively, calling `f` on every sub-expression.
pub fn walk_expr<F>(expr: &ast::Expr, f: &mut F)
where
    F: FnMut(&ast::Expr),
{
    f(expr);
    match expr {
        ast::Expr::BoolOp(node) => {
            for val in &node.values {
                walk_expr(val, f);
            }
        }
        ast::Expr::NamedExpr(node) => {
            walk_expr(&node.target, f);
            walk_expr(&node.value, f);
        }
        ast::Expr::BinOp(node) => {
            walk_expr(&node.left, f);
            walk_expr(&node.right, f);
        }
        ast::Expr::UnaryOp(node) => {
            walk_expr(&node.operand, f);
        }
        ast::Expr::Lambda(node) => {
            for def in node.args.defaults() {
                walk_expr(def, f);
            }
            for kw_arg in &node.args.kwonlyargs {
                if let Some(def) = &kw_arg.default {
                    walk_expr(def, f);
                }
            }
            walk_expr(&node.body, f);
        }
        ast::Expr::IfExp(node) => {
            walk_expr(&node.test, f);
            walk_expr(&node.body, f);
            walk_expr(&node.orelse, f);
        }
        ast::Expr::Dict(node) => {
            for key in node.keys.iter().flatten() {
                walk_expr(key, f);
            }
            for val in &node.values {
                walk_expr(val, f);
            }
        }
        ast::Expr::Set(node) => {
            for elt in &node.elts {
                walk_expr(elt, f);
            }
        }
        ast::Expr::ListComp(node) => {
            walk_expr(&node.elt, f);
            for gen in &node.generators {
                walk_expr(&gen.target, f);
                walk_expr(&gen.iter, f);
                for ifs in &gen.ifs {
                    walk_expr(ifs, f);
                }
            }
        }
        ast::Expr::SetComp(node) => {
            walk_expr(&node.elt, f);
            for gen in &node.generators {
                walk_expr(&gen.target, f);
                walk_expr(&gen.iter, f);
                for ifs in &gen.ifs {
                    walk_expr(ifs, f);
                }
            }
        }
        ast::Expr::DictComp(node) => {
            walk_expr(&node.key, f);
            walk_expr(&node.value, f);
            for gen in &node.generators {
                walk_expr(&gen.target, f);
                walk_expr(&gen.iter, f);
                for ifs in &gen.ifs {
                    walk_expr(ifs, f);
                }
            }
        }
        ast::Expr::GeneratorExp(node) => {
            walk_expr(&node.elt, f);
            for gen in &node.generators {
                walk_expr(&gen.target, f);
                walk_expr(&gen.iter, f);
                for ifs in &gen.ifs {
                    walk_expr(ifs, f);
                }
            }
        }
        ast::Expr::Await(node) => {
            walk_expr(&node.value, f);
        }
        ast::Expr::Yield(node) => {
            if let Some(val) = &node.value {
                walk_expr(val, f);
            }
        }
        ast::Expr::YieldFrom(node) => {
            walk_expr(&node.value, f);
        }
        ast::Expr::Compare(node) => {
            walk_expr(&node.left, f);
            for comparator in &node.comparators {
                walk_expr(comparator, f);
            }
        }
        ast::Expr::Call(node) => {
            walk_expr(&node.func, f);
            for arg in &node.args {
                walk_expr(arg, f);
            }
            for kw in &node.keywords {
                walk_expr(&kw.value, f);
            }
        }
        ast::Expr::FormattedValue(node) => {
            walk_expr(&node.value, f);
            if let Some(spec) = &node.format_spec {
                walk_expr(spec, f);
            }
        }
        ast::Expr::JoinedStr(node) => {
            for val in &node.values {
                walk_expr(val, f);
            }
        }
        ast::Expr::Constant(_) => {}
        ast::Expr::Attribute(node) => {
            walk_expr(&node.value, f);
        }
        ast::Expr::Subscript(node) => {
            walk_expr(&node.value, f);
            walk_expr(&node.slice, f);
        }
        ast::Expr::Starred(node) => {
            walk_expr(&node.value, f);
        }
        ast::Expr::Name(_) => {}
        ast::Expr::List(node) => {
            for elt in &node.elts {
                walk_expr(elt, f);
            }
        }
        ast::Expr::Tuple(node) => {
            for elt in &node.elts {
                walk_expr(elt, f);
            }
        }
        ast::Expr::Slice(node) => {
            if let Some(lower) = &node.lower {
                walk_expr(lower, f);
            }
            if let Some(upper) = &node.upper {
                walk_expr(upper, f);
            }
            if let Some(step) = &node.step {
                walk_expr(step, f);
            }
        }
    }
}

/// Walk a statement recursively, calling `fe` on every sub-expression and `fs` on every sub-statement.
pub fn walk_stmt<FE, FS>(stmt: &ast::Stmt, fe: &mut FE, fs: &mut FS)
where
    FE: FnMut(&ast::Expr),
    FS: FnMut(&ast::Stmt),
{
    fs(stmt);
    match stmt {
        ast::Stmt::FunctionDef(node) => {
            for decorator in &node.decorator_list {
                walk_expr(decorator, fe);
            }
            for def in node.args.defaults() {
                walk_expr(def, fe);
            }
            for kw_arg in &node.args.kwonlyargs {
                if let Some(def) = &kw_arg.default {
                    walk_expr(def, fe);
                }
            }
            if let Some(ret) = &node.returns {
                walk_expr(ret, fe);
            }
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::AsyncFunctionDef(node) => {
            for decorator in &node.decorator_list {
                walk_expr(decorator, fe);
            }
            for def in node.args.defaults() {
                walk_expr(def, fe);
            }
            for kw_arg in &node.args.kwonlyargs {
                if let Some(def) = &kw_arg.default {
                    walk_expr(def, fe);
                }
            }
            if let Some(ret) = &node.returns {
                walk_expr(ret, fe);
            }
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::ClassDef(node) => {
            for decorator in &node.decorator_list {
                walk_expr(decorator, fe);
            }
            for base in &node.bases {
                walk_expr(base, fe);
            }
            for kw in &node.keywords {
                walk_expr(&kw.value, fe);
            }
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::Return(node) => {
            if let Some(val) = &node.value {
                walk_expr(val, fe);
            }
        }
        ast::Stmt::Delete(node) => {
            for target in &node.targets {
                walk_expr(target, fe);
            }
        }
        ast::Stmt::Assign(node) => {
            for target in &node.targets {
                walk_expr(target, fe);
            }
            walk_expr(&node.value, fe);
        }
        ast::Stmt::AugAssign(node) => {
            walk_expr(&node.target, fe);
            walk_expr(&node.value, fe);
        }
        ast::Stmt::AnnAssign(node) => {
            walk_expr(&node.target, fe);
            walk_expr(&node.annotation, fe);
            if let Some(val) = &node.value {
                walk_expr(val, fe);
            }
        }
        ast::Stmt::TypeAlias(node) => {
            walk_expr(&node.name, fe);
            walk_expr(&node.value, fe);
        }
        ast::Stmt::For(node) => {
            walk_expr(&node.target, fe);
            walk_expr(&node.iter, fe);
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::AsyncFor(node) => {
            walk_expr(&node.target, fe);
            walk_expr(&node.iter, fe);
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::While(node) => {
            walk_expr(&node.test, fe);
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::If(node) => {
            walk_expr(&node.test, fe);
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::With(node) => {
            for item in &node.items {
                walk_expr(&item.context_expr, fe);
                if let Some(target) = &item.optional_vars {
                    walk_expr(target, fe);
                }
            }
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::AsyncWith(node) => {
            for item in &node.items {
                walk_expr(&item.context_expr, fe);
                if let Some(target) = &item.optional_vars {
                    walk_expr(target, fe);
                }
            }
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::Match(node) => {
            walk_expr(&node.subject, fe);
            for case in &node.cases {
                if let Some(guard) = &case.guard {
                    walk_expr(guard, fe);
                }
                for s in &case.body {
                    walk_stmt(s, fe, fs);
                }
            }
        }
        ast::Stmt::Raise(node) => {
            if let Some(exc) = &node.exc {
                walk_expr(exc, fe);
            }
            if let Some(cause) = &node.cause {
                walk_expr(cause, fe);
            }
        }
        ast::Stmt::Try(node) => {
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for handler in &node.handlers {
                let ast::ExceptHandler::ExceptHandler(handler_node) = handler;
                if let Some(t) = &handler_node.type_ {
                    walk_expr(t, fe);
                }
                for s in &handler_node.body {
                    walk_stmt(s, fe, fs);
                }
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
            for s in &node.finalbody {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::TryStar(node) => {
            for s in &node.body {
                walk_stmt(s, fe, fs);
            }
            for handler in &node.handlers {
                let ast::ExceptHandler::ExceptHandler(handler_node) = handler;
                if let Some(t) = &handler_node.type_ {
                    walk_expr(t, fe);
                }
                for s in &handler_node.body {
                    walk_stmt(s, fe, fs);
                }
            }
            for s in &node.orelse {
                walk_stmt(s, fe, fs);
            }
            for s in &node.finalbody {
                walk_stmt(s, fe, fs);
            }
        }
        ast::Stmt::Assert(node) => {
            walk_expr(&node.test, fe);
            if let Some(msg) = &node.msg {
                walk_expr(msg, fe);
            }
        }
        ast::Stmt::Import(_) | ast::Stmt::ImportFrom(_) => {}
        ast::Stmt::Global(_) | ast::Stmt::Nonlocal(_) => {}
        ast::Stmt::Expr(node) => {
            walk_expr(&node.value, fe);
        }
        ast::Stmt::Pass(_) | ast::Stmt::Break(_) | ast::Stmt::Continue(_) => {}
    }
}

/// Helper to check if an expression contains a lambda expression anywhere.
pub fn contains_lambda(expr: &ast::Expr) -> bool {
    let mut found = false;
    walk_expr(expr, &mut |e| {
        if matches!(e, ast::Expr::Lambda(_)) {
            found = true;
        }
    });
    found
}

/// Helper to check if an expression contains an `exec` or `eval` call anywhere.
pub fn contains_exec_eval(expr: &ast::Expr) -> bool {
    let mut found = false;
    walk_expr(expr, &mut |e| {
        if let ast::Expr::Call(c) = e {
            if let ast::Expr::Name(n) = c.func.as_ref() {
                if matches!(n.id.as_str(), "exec" | "eval") {
                    found = true;
                }
            }
        }
    });
    found
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustpython_parser::ast::{self, Constant, Expr, Identifier};

    #[test]
    fn test_contains_lambda_nested() {
        // Construct standard lambda: lambda x: x
        let lambda_expr = Expr::Lambda(ast::ExprLambda {
            range: Default::default(),
            args: Box::new(ast::Arguments {
                range: Default::default(),
                posonlyargs: vec![],
                args: vec![ast::ArgWithDefault {
                    range: Default::default(),
                    def: ast::Arg {
                        range: Default::default(),
                        arg: Identifier::new("x"),
                        annotation: None,
                        type_comment: None,
                    },
                    default: None,
                }],
                vararg: None,
                kwonlyargs: vec![],
                kwarg: None,
            }),
            body: Box::new(Expr::Name(ast::ExprName {
                range: Default::default(),
                id: Identifier::new("x"),
                ctx: ast::ExprContext::Load,
            })),
        });

        // Nest it inside a Call: foo(lambda x: x)
        let call_expr = Expr::Call(ast::ExprCall {
            range: Default::default(),
            func: Box::new(Expr::Name(ast::ExprName {
                range: Default::default(),
                id: Identifier::new("foo"),
                ctx: ast::ExprContext::Load,
            })),
            args: vec![lambda_expr],
            keywords: vec![],
        });

        assert!(contains_lambda(&call_expr));
        assert!(!contains_exec_eval(&call_expr));
    }

    #[test]
    fn test_contains_exec_eval_nested() {
        // Construct eval call: eval("1")
        let eval_call = Expr::Call(ast::ExprCall {
            range: Default::default(),
            func: Box::new(Expr::Name(ast::ExprName {
                range: Default::default(),
                id: Identifier::new("eval"),
                ctx: ast::ExprContext::Load,
            })),
            args: vec![Expr::Constant(ast::ExprConstant {
                range: Default::default(),
                value: Constant::Str("1".into()),
                kind: None,
            })],
            keywords: vec![],
        });

        assert!(contains_exec_eval(&eval_call));
        assert!(!contains_lambda(&eval_call));
    }
}
