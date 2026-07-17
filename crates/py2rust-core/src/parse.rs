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
    let start = stmt.range().start().to_u32() as usize;
    let end = stmt.range().end().to_u32() as usize;
    let end = end.min(source.len());
    let start = start.min(end);
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
