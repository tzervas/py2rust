//! Python source → AST via `rustpython-parser`.

use rustpython_parser::ast::{self, Ranged, Suite};
use rustpython_parser::{parse, Mode, ParseError};
use thiserror::Error;

/// Parse failures (hard errors — distinct from per-construct gaps).
#[derive(Debug, Error)]
pub enum ParseFail {
    #[error("failed to read {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse {path}: {message}")]
    Syntax { path: String, message: String },
}

impl From<ParseError> for ParseFail {
    fn from(e: ParseError) -> Self {
        ParseFail::Syntax {
            path: e.source_path.clone(),
            message: e.to_string(),
        }
    }
}

/// Parsed Python module body (top-level statements).
#[derive(Debug, Clone)]
pub struct ParsedModule {
    pub file_label: String,
    pub source: String,
    pub body: Suite,
}

impl ParsedModule {
    pub fn top_level_count(&self) -> usize {
        self.body.len()
    }

    /// Every statement in the module, nested ones included.
    pub fn statement_count(&self) -> usize {
        count_statements(&self.body)
    }
}

/// Count every `Stmt` in a suite, recursing into every statement-bearing field.
///
/// This is the **L2 denominator**, and it exists because top-level count is a
/// misleading thing to measure against. Measured on tg-agent-relay: 1,987
/// top-level statements but 10,955 in total, so **81.9% of the code lives inside
/// bodies** that top-level accounting never looks at. Reporting progress against
/// `module.body.len()` flatters the transpiler by roughly 5x.
///
/// Every nested field is walked — `orelse` and `finalbody` included — because a
/// denominator that quietly omits `else` branches is the same class of error this
/// measurement exists to remove.
pub fn count_statements(body: &[ast::Stmt]) -> usize {
    fn handlers(hs: &[ast::ExceptHandler]) -> usize {
        hs.iter()
            .map(|h| {
                let ast::ExceptHandler::ExceptHandler(h) = h;
                count_statements(&h.body)
            })
            .sum()
    }
    let mut n = 0usize;
    for stmt in body {
        n += 1;
        n += match stmt {
            ast::Stmt::FunctionDef(s) => count_statements(&s.body),
            ast::Stmt::AsyncFunctionDef(s) => count_statements(&s.body),
            ast::Stmt::ClassDef(s) => count_statements(&s.body),
            ast::Stmt::For(s) => count_statements(&s.body) + count_statements(&s.orelse),
            ast::Stmt::AsyncFor(s) => count_statements(&s.body) + count_statements(&s.orelse),
            ast::Stmt::While(s) => count_statements(&s.body) + count_statements(&s.orelse),
            ast::Stmt::If(s) => count_statements(&s.body) + count_statements(&s.orelse),
            ast::Stmt::With(s) => count_statements(&s.body),
            ast::Stmt::AsyncWith(s) => count_statements(&s.body),
            ast::Stmt::Match(s) => s.cases.iter().map(|c| count_statements(&c.body)).sum(),
            ast::Stmt::Try(s) => {
                count_statements(&s.body)
                    + count_statements(&s.orelse)
                    + count_statements(&s.finalbody)
                    + handlers(&s.handlers)
            }
            ast::Stmt::TryStar(s) => {
                count_statements(&s.body)
                    + count_statements(&s.orelse)
                    + count_statements(&s.finalbody)
                    + handlers(&s.handlers)
            }
            _ => 0,
        };
    }
    n
}

/// Parse Python source text into a module suite.
pub fn parse_source(source: &str, file_label: &str) -> Result<ParsedModule, ParseFail> {
    let mod_ = parse(source, Mode::Module, file_label).map_err(|e| ParseFail::Syntax {
        path: file_label.to_string(),
        message: e.to_string(),
    })?;
    let body = match mod_ {
        ast::Mod::Module(m) => m.body,
        other => {
            return Err(ParseFail::Syntax {
                path: file_label.to_string(),
                message: format!("expected Module, got {other:?}"),
            });
        }
    };
    Ok(ParsedModule {
        file_label: file_label.to_string(),
        source: source.to_string(),
        body,
    })
}

/// Read `path` and parse as a Python module.
pub fn parse_file(path: &std::path::Path) -> Result<ParsedModule, ParseFail> {
    let label = path.display().to_string();
    let source = std::fs::read_to_string(path).map_err(|source| ParseFail::Io {
        path: label.clone(),
        source,
    })?;
    parse_source(&source, &label)
}

/// Byte offset → 1-based (line, col) using the original source text.
pub fn line_col_at(source: &str, offset: u32) -> (usize, usize) {
    let target = offset as usize;
    let mut line = 1usize;
    let mut col = 1usize;
    for (i, ch) in source.char_indices() {
        if i >= target {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Span helpers for a ranged AST node.
pub fn stmt_line_col(source: &str, stmt: &ast::Stmt) -> (usize, usize) {
    line_col_at(source, stmt.range().start().to_u32())
}

/// Best-effort single-line snippet from source for a statement span.
pub fn snippet_for(source: &str, stmt: &ast::Stmt) -> String {
    let mut start = stmt.range().start().to_u32() as usize;
    let mut end = stmt.range().end().to_u32() as usize;
    end = end.min(source.len());
    start = start.min(end);
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    if start > end {
        end = start;
    }
    let slice = &source[start..end];
    let first = slice.lines().next().unwrap_or(slice).trim();
    if first.len() > 120 {
        format!("{}…", &first[..117])
    } else {
        first.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_def() {
        let p = parse_source("def foo(x: int) -> int:\n    return x\n", "t.py").unwrap();
        assert_eq!(p.top_level_count(), 1);
        assert!(matches!(p.body[0], ast::Stmt::FunctionDef(_)));
    }

    #[test]
    fn rejects_syntax_error() {
        let err = parse_source("def (\n", "bad.py").unwrap_err();
        assert!(matches!(err, ParseFail::Syntax { .. }));
    }

    #[test]
    fn line_col_basic() {
        let src = "a\nb\nc";
        assert_eq!(line_col_at(src, 0), (1, 1));
        assert_eq!(line_col_at(src, 2), (2, 1)); // 'b'
    }
}

#[cfg(test)]
mod l2_tests {
    use super::*;

    fn n(src: &str) -> usize {
        parse_source(src, "t.py").expect("parse").statement_count()
    }

    #[test]
    fn counts_nested_not_just_top_level() {
        // 1 def + 3 body statements = 4, where top_level_count() would say 1.
        let src = "def f():\n    a = 1\n    b = 2\n    return a + b\n";
        assert_eq!(n(src), 4);
        let m = parse_source(src, "t.py").unwrap();
        assert_eq!(m.top_level_count(), 1, "L1 sees one item; L2 must see four");
    }

    #[test]
    fn walks_orelse_and_finally_and_handlers() {
        // A denominator that skips else/finally/except would undercount, which
        // is the same class of error L2 exists to remove.
        let src = concat!(
            "try:\n    a = 1\n",
            "except ValueError:\n    b = 2\n",
            "except TypeError:\n    c = 3\n",
            "else:\n    d = 4\n",
            "finally:\n    e = 5\n",
        );
        assert_eq!(n(src), 6, "try + 5 nested statements");
    }

    #[test]
    fn walks_loop_and_if_else_and_with_and_class() {
        let src = concat!(
            "class C:\n    x = 1\n    def m(self):\n        return 2\n",
            "for i in r:\n    p = 1\nelse:\n    q = 2\n",
            "while t:\n    u = 1\n",
            "with open(f) as h:\n    v = 1\n",
            "if c:\n    w = 1\nelse:\n    y = 1\n",
        );
        // class(1)+x(1)+def(1)+return(1) + for(1)+p(1)+q(1) + while(1)+u(1)
        // + with(1)+v(1) + if(1)+w(1)+y(1) = 14
        assert_eq!(n(src), 14);
    }

    #[test]
    fn flat_module_matches_top_level() {
        let src = "a = 1\nb = 2\nc = 3\n";
        let m = parse_source(src, "t.py").unwrap();
        assert_eq!(m.statement_count(), m.top_level_count());
    }
}
