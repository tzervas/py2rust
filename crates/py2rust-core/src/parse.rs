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

    /// One row per shape the counter must handle.
    ///
    /// Table-driven so adding a Python construct is a one-line change rather
    /// than a new test function — the assertion logic is identical for every
    /// case, and duplicating it per shape is how counter tests drift apart.
    struct Case {
        name: &'static str,
        src: &'static str,
        /// Expected total statements, nested included.
        total: usize,
        /// Expected top-level statements — the L1 denominator, for contrast.
        top: usize,
    }

    const CASES: &[Case] = &[
        Case {
            name: "flat module: L1 and L2 agree",
            src: "a = 1\nb = 2\nc = 3\n",
            total: 3,
            top: 3,
        },
        Case {
            name: "function body is invisible to L1",
            src: "def f():\n    a = 1\n    b = 2\n    return a + b\n",
            total: 4,
            top: 1,
        },
        Case {
            name: "try: except/else/finally must all be walked",
            src: concat!(
                "try:\n    a = 1\n",
                "except ValueError:\n    b = 2\n",
                "except TypeError:\n    c = 3\n",
                "else:\n    d = 4\n",
                "finally:\n    e = 5\n",
            ),
            total: 6,
            top: 1,
        },
        Case {
            name: "loop else clause counts",
            src: "for i in r:\n    p = 1\nelse:\n    q = 2\n",
            total: 3,
            top: 1,
        },
        Case {
            name: "if/else both branches count",
            src: "if c:\n    w = 1\nelse:\n    y = 1\n",
            total: 3,
            top: 1,
        },
        Case {
            name: "class body and its methods count",
            src: "class C:\n    x = 1\n    def m(self):\n        return 2\n",
            total: 4,
            top: 1,
        },
        Case {
            name: "with block body counts",
            src: "with open(f) as h:\n    v = 1\n",
            total: 2,
            top: 1,
        },
        Case {
            name: "while body and else count",
            src: "while t:\n    u = 1\nelse:\n    z = 2\n",
            total: 3,
            top: 1,
        },
    ];

    #[test]
    fn statement_counter_matches_every_shape() {
        for c in CASES {
            let m = parse_source(c.src, "t.py").expect(c.name);
            assert_eq!(
                m.statement_count(),
                c.total,
                "L2 total wrong for case: {}\n--- source ---\n{}",
                c.name,
                c.src
            );
            assert_eq!(
                m.top_level_count(),
                c.top,
                "L1 top-level wrong for case: {}\n--- source ---\n{}",
                c.name,
                c.src
            );
        }
    }

    #[test]
    fn nesting_is_what_separates_l1_from_l2() {
        // The whole reason L2 exists: on anything with bodies, L1 undercounts.
        for c in CASES.iter().filter(|c| c.total != c.top) {
            let m = parse_source(c.src, "t.py").unwrap();
            assert!(
                m.statement_count() > m.top_level_count(),
                "case {:?} should have hidden statements",
                c.name
            );
        }
    }
}
