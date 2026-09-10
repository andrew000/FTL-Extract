use memchr::memchr_iter;

/// Byte offsets of every line start in a source file, for O(log n) offset-to-line lookups.
#[derive(Clone, Debug)]
pub struct LineIndex {
    line_starts: Vec<usize>,
}

impl LineIndex {
    pub fn new(content: &str) -> Self {
        let mut line_starts = Vec::with_capacity(content.len() / 40 + 1);
        line_starts.push(0);
        line_starts.extend(memchr_iter(b'\n', content.as_bytes()).map(|offset| offset + 1));
        Self { line_starts }
    }

    /// 1-based line and column of `byte_index` in `content`. The column counts characters
    /// from the start of the line, matching [`line_column`].
    pub fn line_column(&self, content: &str, byte_index: usize) -> (usize, usize) {
        let target = byte_index.min(content.len());
        let line = self
            .line_starts
            .partition_point(|&start| start <= target)
            .max(1);
        let line_start = self.line_starts[line - 1];
        let column = content
            .get(line_start..target)
            .map_or(target - line_start, |prefix| prefix.chars().count())
            + 1;

        (line, column)
    }
}

/// 1-based line and column of `byte_index` in `content`, scanning from the start.
/// Use [`LineIndex`] when looking up many offsets in the same content.
pub fn line_column(content: &str, byte_index: usize) -> (usize, usize) {
    let target = byte_index.min(content.len());
    let mut line = 1;
    let mut column = 1;

    for (offset, ch) in content.char_indices() {
        if offset >= target {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

#[cfg(test)]
mod tests {
    use super::{LineIndex, line_column};

    #[test]
    fn test_line_index_matches_linear_scan() {
        let content = "first\nsecond ünï\r\n\nfourth";
        let index = LineIndex::new(content);

        for byte_index in 0..=content.len() + 3 {
            if byte_index < content.len() && !content.is_char_boundary(byte_index) {
                continue;
            }
            assert_eq!(
                index.line_column(content, byte_index),
                line_column(content, byte_index),
                "byte index {byte_index}"
            );
        }
    }

    #[test]
    fn test_line_column_positions() {
        let content = "ab\ncd\n";

        assert_eq!(line_column(content, 0), (1, 1));
        assert_eq!(line_column(content, 2), (1, 3));
        assert_eq!(line_column(content, 3), (2, 1));
        assert_eq!(line_column(content, 4), (2, 2));
        assert_eq!(line_column(content, 6), (3, 1));
        assert_eq!(line_column("", 5), (1, 1));
    }
}
