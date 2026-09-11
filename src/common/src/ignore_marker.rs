//! The `# ftl-extract: ignore ...` marker a message carries in its attached comment.
//!
//! `ftl check` and `ftl extract` read the same marker, so the grammar lives here once. Only the
//! comment the Fluent parser attaches to the message counts: a `#` comment directly above it,
//! with no blank line in between. Every line of that comment is parsed and the results are
//! merged. Per line, after trimming and ASCII lowercasing:
//!
//! - a line that is exactly `ignore` is a legacy alias of `ignore untranslated`;
//! - otherwise the line must contain `ftl-extract:`; what follows it is the directive, so
//!   `Note: ftl-extract: ignore stale` is a marker too;
//! - the directive `ignore-untranslated` is a legacy alias of `ignore untranslated`;
//! - the directive must be `ignore`, alone or followed by whitespace and names separated by
//!   commas or whitespace (`ignored`, `ignores` are not markers);
//! - `all` ignores every check; any other word is a check name, kept as written (lowercased),
//!   so each command decides which names mean something to it and ignores the rest;
//! - `ignore` with no names at all means `all`.

use crate::FastHashSet;
use fluent_syntax::ast::Comment;

/// Which checks a message opts out of.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IgnoreMarker {
    all: bool,
    names: FastHashSet<String>,
}

impl IgnoreMarker {
    /// The marker in `comment`, or `None` when the comment is absent or is not a marker.
    pub fn parse(comment: Option<&Comment<String>>) -> Option<Self> {
        let mut marker = Self::default();
        for line in &comment?.content {
            if let Some(line_marker) = parse_line(line) {
                marker.merge(line_marker);
            }
        }
        (marker != Self::default()).then_some(marker)
    }

    /// Whether the marker opts out of `check` (`stale`, `kwargs`, `untranslated`, ...).
    pub fn ignores(&self, check: &str) -> bool {
        self.all || self.names.contains(check)
    }

    fn merge(&mut self, other: Self) {
        self.all |= other.all;
        self.names.extend(other.names);
    }

    fn single(name: &str) -> Self {
        Self {
            all: false,
            names: FastHashSet::from_iter([name.to_string()]),
        }
    }
}

/// Reads a check-comment line as an [`IgnoreMarker`] when it is one, or an empty marker when
/// the line does not opt out of anything (an ordinary comment, or `ignore` followed only by
/// nothing that names a check).
fn parse_line(line: &str) -> Option<IgnoreMarker> {
    const PREFIX: &str = "ftl-extract:";

    let normalized = line.trim().to_ascii_lowercase();
    if normalized == "ignore" {
        return Some(IgnoreMarker::single("untranslated"));
    }

    let position = normalized.find(PREFIX)?;
    let directive = normalized[position + PREFIX.len()..].trim();

    if directive == "ignore-untranslated" {
        return Some(IgnoreMarker::single("untranslated"));
    }

    let names = directive.strip_prefix("ignore")?;
    if !names.is_empty() && !names.starts_with(char::is_whitespace) {
        return None;
    }

    let mut marker = IgnoreMarker::default();
    let mut any_name = false;
    for name in names.split(|c: char| c == ',' || c.is_whitespace()) {
        if name.is_empty() {
            continue;
        }
        any_name = true;
        if name == "all" {
            marker.all = true;
        } else {
            marker.names.insert(name.to_string());
        }
    }

    if !any_name {
        marker.all = true;
    }
    Some(marker)
}

#[cfg(test)]
mod tests {
    use super::IgnoreMarker;
    use fluent_syntax::ast::Comment;

    fn parse(lines: &[&str]) -> Option<IgnoreMarker> {
        let comment = Comment {
            content: lines.iter().map(|line| line.to_string()).collect(),
        };
        IgnoreMarker::parse(Some(&comment))
    }

    /// `(stale, untranslated, kwargs)` for a quick look at a marker.
    fn flags(marker: &Option<IgnoreMarker>) -> (bool, bool, bool) {
        match marker {
            Some(marker) => (
                marker.ignores("stale"),
                marker.ignores("untranslated"),
                marker.ignores("kwargs"),
            ),
            None => (false, false, false),
        }
    }

    #[test]
    fn no_comment_or_unrelated_comment_is_not_a_marker() {
        assert_eq!(IgnoreMarker::parse(None), None);
        assert_eq!(parse(&["Brand name, do not translate"]), None);
        assert_eq!(parse(&["ftl-extract: ignored"]), None);
        assert_eq!(parse(&["ftl-extract: skip stale"]), None);
        assert_eq!(parse(&[]), None);
    }

    #[test]
    fn legacy_spellings_ignore_untranslated_only() {
        for line in [
            "ftl-extract: ignore-untranslated",
            "FTL-Extract: Ignore-Untranslated",
            "ignore",
            "Note: ftl-extract: ignore-untranslated",
        ] {
            assert_eq!(flags(&parse(&[line])), (false, true, false), "{line}");
        }
    }

    #[test]
    fn named_checks_and_all() {
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore stale"])),
            (true, false, false)
        );
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore untranslated"])),
            (false, true, false)
        );
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore kwargs"])),
            (false, false, true)
        );
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore stale, untranslated"])),
            (true, true, false)
        );
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore stale kwargs"])),
            (true, false, true)
        );
        assert_eq!(
            flags(&parse(&["FTL-EXTRACT: IGNORE STALE"])),
            (true, false, false)
        );
        for line in [
            "ftl-extract: ignore all",
            "ftl-extract: ignore",
            "ftl-extract:ignore",
            "ftl-extract: ignore stale, all",
        ] {
            assert_eq!(flags(&parse(&[line])), (true, true, true), "{line}");
        }
    }

    #[test]
    fn unknown_names_are_kept_but_ignore_nothing_known() {
        // `missing` is a valid name for some future consumer; here it opts out of nothing.
        let marker = parse(&["ftl-extract: ignore missing"]).unwrap();
        assert!(marker.ignores("missing"));
        assert_eq!(flags(&Some(marker)), (false, false, false));
        assert_eq!(
            flags(&parse(&["ftl-extract: ignore missing, stale"])),
            (true, false, false)
        );
    }

    #[test]
    fn lines_of_one_comment_merge() {
        assert_eq!(
            flags(&parse(&[
                "Brand name",
                "ftl-extract: ignore stale",
                "ftl-extract: ignore untranslated"
            ])),
            (true, true, false)
        );
    }
}
