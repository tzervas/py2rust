//! Byte-offset → (line, col) helpers for gap spans.

/// 1-based line, 0-based column for `byte_offset` into `source` (UTF-8 byte index).
pub fn line_col(source: &str, byte_offset: u32) -> (usize, usize) {
    let offset = byte_offset as usize;
    let mut line = 1usize;
    let mut col = 0usize;
    let mut i = 0usize;
    for ch in source.chars() {
        if i >= offset {
            break;
        }
        let len = ch.len_utf8();
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
        i += len;
    }
    (line, col)
}

/// Extract a snippet from source using half-open byte range `[start, end)`, truncated.
pub fn snippet(source: &str, start: u32, end: u32, max_chars: usize) -> String {
    let mut start = (start as usize).min(source.len());
    let mut end = (end as usize).min(source.len()).max(start);
    while start < source.len() && !source.is_char_boundary(start) {
        start += 1;
    }
    while end < source.len() && !source.is_char_boundary(end) {
        end += 1;
    }
    if start > end {
        end = start;
    }
    let raw = &source[start..end];
    let mut out: String = raw.chars().take(max_chars).collect();
    if raw.chars().count() > max_chars {
        out.push('…');
    }
    // Collapse interior newlines for single-line reports.
    out.replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_byte() {
        assert_eq!(line_col("abc", 0), (1, 0));
    }

    #[test]
    fn after_newline() {
        assert_eq!(line_col("a\nb", 2), (2, 0));
    }
}
