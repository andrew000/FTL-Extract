#![allow(unused_variables)]

use crate::ftl::code_extractor::kwargs_from_key;
use crate::ftl::consts;
use crate::ftl::diagnostics::{CodeLocation, ExtractionDiagnostic, ExtractionDiagnosticKind};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use common::{IgnoreMarker, LineIndex};
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use smallvec::SmallVec;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub enum FluentEntry {
    Message(fluent_syntax::ast::Message<String>),
    Term(fluent_syntax::ast::Term<String>),
    Comment(fluent_syntax::ast::Comment<String>),
    GroupComment(fluent_syntax::ast::Comment<String>),
    ResourceComment(fluent_syntax::ast::Comment<String>),
}

#[derive(Clone, Debug)]
pub struct FluentKey {
    pub code_path: Arc<PathBuf>,
    pub key: String,
    pub entry: Arc<FluentEntry>,
    pub path: Arc<PathBuf>,
    pub locale: Option<String>,
    pub position: usize,
    pub depends_on_keys: FastHashSet<String>,
    pub source_location: Option<CodeLocation>,
    /// For a key found in code: the first call site that passed `**kwargs`, if any. Such a
    /// call can pass any variable, so the key's variables cannot be verified against the
    /// stored message; `extract` and `check` skip the comparison for it.
    pub kwargs_unknown: Option<CodeLocation>,
}

impl FluentKey {
    pub(crate) fn new(
        code_path: Arc<PathBuf>,
        key: String,
        entry: FluentEntry,
        path: Arc<PathBuf>,
        locale: Option<String>,
        position: Option<usize>,
        depends_on_keys: FastHashSet<String>,
    ) -> Self {
        Self {
            code_path,
            key,
            entry: Arc::new(entry),
            path,
            locale,
            position: position.unwrap_or(usize::MAX),
            depends_on_keys,
            source_location: None,
            kwargs_unknown: None,
        }
    }

    /// Whether the call this key's variables come from passed `**kwargs`. `kwargs_unknown` is
    /// the first `**` call site among all occurrences; it is this occurrence exactly when the
    /// kept call is that site (a call without `**` always wins over one with it, see
    /// [`merge_key_occurrence`]).
    pub(crate) fn kept_call_has_double_star(&self) -> bool {
        self.kwargs_unknown.is_some() && self.kwargs_unknown == self.source_location
    }

    /// The `# ftl-extract: ignore ...` marker in the comment attached to a stored message, if any.
    /// Terms and comments carry none.
    pub(crate) fn ignore_marker(&self) -> Option<IgnoreMarker> {
        match self.entry.as_ref() {
            FluentEntry::Message(message) => IgnoreMarker::parse(message.comment.as_ref()),
            _ => None,
        }
    }
}

/// Merges `other`, another occurrence of the key that `kept` already holds, into `kept`, and
/// returns the conflict if the two cannot be reconciled. Shared by the same-file and the
/// cross-file merge so the rule exists once:
///
/// - different `_path=` values are a `KeyPathConflict`, whatever else the calls pass;
/// - only calls without `**kwargs` are compared with each other: two of them with different
///   keyword arguments are a `KeyMessageConflict`, a call with `**` never conflicts;
/// - the kept variables (placeholder, cache, check report) come from the first call without
///   `**` by location, or from the first `**` call if there is no other;
/// - `kwargs_unknown` remembers the first `**` call site among all occurrences.
pub(crate) fn merge_key_occurrence(
    kept: &mut FluentKey,
    other: FluentKey,
) -> Option<ExtractionDiagnostic> {
    if kept.path != other.path {
        return Some(conflict_diagnostic(
            ExtractionDiagnosticKind::KeyPathConflict,
            kept,
            &other,
            path_conflict_message,
        ));
    }

    let kept_star = kept.kept_call_has_double_star();
    let other_star = other.kept_call_has_double_star();

    let conflict = if kept_star || other_star {
        None
    } else {
        match (kept.entry.as_ref(), other.entry.as_ref()) {
            (FluentEntry::Message(a), FluentEntry::Message(b)) if a == b => None,
            (FluentEntry::Message(_), FluentEntry::Message(_)) => Some(conflict_diagnostic(
                ExtractionDiagnosticKind::KeyMessageConflict,
                kept,
                &other,
                kwargs_conflict_message,
            )),
            _ => Some(conflict_diagnostic(
                ExtractionDiagnosticKind::KeyTypeConflict,
                kept,
                &other,
                type_conflict_message,
            )),
        }
    };

    let first_double_star = match (kept.kwargs_unknown.take(), other.kwargs_unknown.clone()) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };

    // A call without `**` beats one with it; between equals the earlier call site is kept, so
    // the result does not depend on the order the occurrences were seen in.
    let other_wins = match (kept_star, other_star) {
        (true, false) => true,
        (false, true) => false,
        _ => other.source_location < kept.source_location,
    };
    if other_wins && conflict.is_none() {
        *kept = other;
    }
    kept.kwargs_unknown = first_double_star;

    conflict
}

/// The placeholder message written for a key found in code: the key as text followed by one
/// `{ $kwarg }` per keyword argument, sorted by name.
///
/// Sorting makes two calls with the same keyword arguments in a different order, or in
/// different files, produce equal messages, and keeps the generated placeholder and the cache
/// content reproducible: files are extracted in parallel, so call order would not be stable.
pub(crate) fn code_message(
    key: String,
    mut kwargs: Vec<String>,
) -> fluent_syntax::ast::Message<String> {
    kwargs.sort_unstable();

    let mut elements = Vec::with_capacity(kwargs.len() + 1);
    elements.push(fluent_syntax::ast::PatternElement::TextElement { value: key.clone() });
    elements.extend(
        kwargs
            .into_iter()
            .map(|name| fluent_syntax::ast::PatternElement::Placeable {
                expression: fluent_syntax::ast::Expression::Inline(
                    fluent_syntax::ast::InlineExpression::VariableReference {
                        id: fluent_syntax::ast::Identifier { name },
                    },
                ),
            }),
    );

    fluent_syntax::ast::Message {
        id: fluent_syntax::ast::Identifier { name: key },
        value: Some(fluent_syntax::ast::Pattern { elements }),
        attributes: vec![],
        comment: None,
    }
}

/// The two occurrences of a conflicting key, ordered by call site (path, line, column), so
/// the message and the `locations` of the diagnostic do not depend on which file the parallel
/// extraction reduced first. Within one file this is source order.
pub(crate) fn order_conflict_sides<'k>(
    a: &'k FluentKey,
    b: &'k FluentKey,
) -> (&'k FluentKey, &'k FluentKey) {
    if b.source_location < a.source_location {
        (b, a)
    } else {
        (a, b)
    }
}

/// The call sites of the two ordered occurrences, for the diagnostic's `locations`.
pub(crate) fn conflict_locations(first: &FluentKey, second: &FluentKey) -> Vec<CodeLocation> {
    [first, second]
        .into_iter()
        .filter_map(|key| key.source_location.clone())
        .collect()
}

/// Builds a conflict diagnostic for `existing` and `new_fluent_key`. `message` receives the
/// two occurrences ordered by call site, matching the order of `locations`.
pub(crate) fn conflict_diagnostic(
    kind: ExtractionDiagnosticKind,
    existing: &FluentKey,
    new_fluent_key: &FluentKey,
    message: impl FnOnce(&FluentKey, &FluentKey) -> String,
) -> ExtractionDiagnostic {
    let (first, second) = order_conflict_sides(existing, new_fluent_key);
    ExtractionDiagnostic {
        kind,
        key: Some(new_fluent_key.key.clone()),
        message: message(first, second),
        locations: conflict_locations(first, second),
    }
}

/// Message text of a `KeyMessageConflict`: the two keyword-argument sets, in the same order as
/// the diagnostic's `locations`. The call sites themselves are only in `locations`.
pub(crate) fn kwargs_conflict_message(first: &FluentKey, second: &FluentKey) -> String {
    fn describe(key: &FluentKey) -> String {
        let kwargs = kwargs_from_key(key);
        if kwargs.is_empty() {
            "no keyword arguments".to_string()
        } else {
            kwargs.join(", ")
        }
    }

    format!(
        "Fluent key {} is used with different keyword arguments: {} and {}",
        first.key,
        describe(first),
        describe(second)
    )
}

/// Message text of a `KeyPathConflict`, sides ordered like `locations`.
pub(crate) fn path_conflict_message(first: &FluentKey, second: &FluentKey) -> String {
    format!(
        "Fluent key {} has different paths: {} and {}",
        first.key,
        first.path.display(),
        second.path.display()
    )
}

/// Message text of a `KeyTypeConflict`.
pub(crate) fn type_conflict_message(first: &FluentKey, _second: &FluentKey) -> String {
    format!(
        "Fluent key {} is not a Message in one of the entries.",
        first.key
    )
}

pub(crate) struct I18nMatcher<'a> {
    code_path: Arc<PathBuf>,
    default_ftl_file: Arc<PathBuf>,
    i18n_keys: &'a FastHashSet<String>,
    i18n_keys_prefix: &'a FastHashSet<String>,
    ignore_attributes: &'a FastHashSet<String>,
    ignore_kwargs: &'a FastHashSet<String>,
    source: &'a str,
    line_index: LineIndex,
    pub(crate) diagnostics: Vec<ExtractionDiagnostic>,
    pub(crate) fluent_keys: FastHashMap<String, FluentKey>,
}

impl<'a> SourceOrderVisitor<'a> for I18nMatcher<'a> {
    #[inline]
    fn visit_expr(&mut self, expr: &'a ruff_python_ast::Expr) {
        if expr.is_call_expr() {
            let expr = expr.as_call_expr().unwrap();
            if expr.func.is_attribute_expr() {
                self.process_attribute_call(expr);
            } else if expr.func.is_name_expr()
                && self
                    .i18n_keys
                    .contains(&expr.func.as_name_expr().unwrap().id.to_string())
            {
                self.process_name_call(expr);
            } else {
                // println!(
                //     "Ignoring {:#?}, {}, {}",
                //     expr.func,
                //     expr.func.is_name_expr(),
                //     &expr.func.as_name_expr().unwrap().id.as_str()
                // );
            }
        }

        ruff_python_ast::visitor::source_order::walk_expr(self, expr);
    }
}

impl<'a> I18nMatcher<'a> {
    pub(crate) fn new(
        code_path: PathBuf,
        source: &'a str,
        default_ftl_file: PathBuf,
        i18n_keys: &'a FastHashSet<String>,
        i18n_keys_prefix: &'a FastHashSet<String>,
        ignore_attributes: &'a FastHashSet<String>,
        ignore_kwargs: &'a FastHashSet<String>,
    ) -> Self {
        Self {
            code_path: Arc::new(code_path),
            default_ftl_file: Arc::new(default_ftl_file),
            i18n_keys,
            i18n_keys_prefix,
            ignore_attributes,
            ignore_kwargs,
            source,
            line_index: LineIndex::new(source),
            diagnostics: Vec::new(),
            fluent_keys: FastHashMap::default(),
        }
    }
    #[inline]
    fn process_attribute_call(&mut self, expr: &'a ruff_python_ast::ExprCall) {
        let mut attrs: SmallVec<&str, 8> = SmallVec::new();
        let mut current_expr = expr.func.as_ref();

        while let Some(attribute_expr) = current_expr.as_attribute_expr() {
            attrs.push(attribute_expr.attr.as_str());
            current_expr = &attribute_expr.value;
        }

        if let Some(name_expr) = current_expr.as_name_expr() {
            self.process_attribute_name_call(expr, name_expr, attrs);
        }
    }
    #[inline]
    fn process_name_call(&mut self, expr: &ruff_python_ast::ExprCall) {
        // `find_positional` is `None` for `i18n(*args)`, `i18n(**kwargs)` and `i18n(key="x")`:
        // there is no key to extract, and `is_empty()` alone does not rule those out.
        let Some(arg) = expr.arguments.find_positional(0) else {
            return;
        };
        let Some(literal) = arg.as_string_literal_expr() else {
            return;
        };

        self.add_fluent_key(expr, literal.value.to_string());
    }
    #[inline]
    fn process_attribute_name_call(
        &mut self,
        expr: &ruff_python_ast::ExprCall,
        attr: &ruff_python_ast::ExprName,
        mut attrs: SmallVec<&str, 8>,
    ) {
        if self.i18n_keys.contains(attr.id.as_str()) {
            self.process_i18n_key_call(expr, attrs);
        } else if self.i18n_keys_prefix.contains(attr.id.as_str())
            && attrs
                .last()
                .is_some_and(|last| self.i18n_keys.contains(*last))
        {
            // Remove the last attribute to handle cases where the prefix key is followed by a
            // valid i18n key.
            attrs.pop();
            self.process_i18n_key_call(expr, attrs);
        }
    }
    #[inline]
    fn process_i18n_key_call(
        &mut self,
        expr: &ruff_python_ast::ExprCall,
        attrs: SmallVec<&str, 8>,
    ) {
        match attrs.as_slice() {
            // `self.i18n("key")` with `self` as a prefix: the prefix step removed the only
            // attribute, so treat it like `i18n("key")`.
            [] => self.process_name_call(expr),
            [attr] if *attr == consts::GET_LITERAL => self.process_i18n_key_call_get_literal(expr),
            _ => self.process_i18n_key_call_attrs(expr, attrs),
        }
    }
    #[inline]
    fn process_i18n_key_call_get_literal(&mut self, expr: &ruff_python_ast::ExprCall) {
        // Same as `process_name_call`: `i18n.get(*args)` and `i18n.get(key="x")` have no
        // positional argument even though `arguments` is not empty.
        let Some(arg) = expr.arguments.find_positional(0) else {
            return;
        };
        let Some(literal) = arg.as_string_literal_expr() else {
            return;
        };

        self.add_fluent_key(expr, literal.value.to_string());
    }
    #[inline]
    fn process_i18n_key_call_attrs(
        &mut self,
        expr: &ruff_python_ast::ExprCall,
        attrs: SmallVec<&str, 8>,
    ) {
        let Some(last) = attrs.last() else {
            return;
        };
        if self.ignore_attributes.contains(*last) {
            return;
        }

        // Calculate capacity for the new key string to avoid reallocations
        // Sum of lengths + number of separators
        let capacity = attrs.iter().map(|s| s.len()).sum::<usize>() + attrs.len().saturating_sub(1);
        let mut key = String::with_capacity(capacity);

        // Join in reverse order
        for (i, s) in attrs.iter().rev().enumerate() {
            if i > 0 {
                key.push('-');
            }
            key.push_str(s);
        }

        self.add_fluent_key(expr, key);
    }
    #[inline]
    fn create_fluent_key(&self, expr: &ruff_python_ast::ExprCall, key: String) -> FluentKey {
        let mut path = self.default_ftl_file.clone();
        let mut kwargs: Vec<String> = Vec::new();
        let mut kwargs_unknown = None;

        for kw in &expr.arguments.keywords {
            let Some(arg) = kw.arg.as_ref() else {
                // `**something`: any variable may be passed, so the set is unknowable.
                kwargs_unknown.get_or_insert_with(|| self.code_location(expr));
                continue;
            };

            if arg.as_str() == consts::PATH_LITERAL {
                if let Some(literal) = kw.value.as_string_literal_expr() {
                    let raw_path = literal.value.to_str();

                    if !raw_path.is_empty() {
                        let mut p = PathBuf::from(raw_path);
                        if p.extension().is_none() {
                            p.push(self.default_ftl_file.as_ref());
                        }
                        path = Arc::new(p);
                    }
                }
            } else if !self.ignore_kwargs.contains(arg.as_str()) {
                kwargs.push(arg.to_string());
            }
        }

        let mut fluent_key = FluentKey::new(
            self.code_path.clone(),
            key.clone(),
            FluentEntry::Message(code_message(key, kwargs)),
            path,
            None,
            None,
            FastHashSet::default(),
        );
        fluent_key.source_location = Some(self.code_location(expr));
        fluent_key.kwargs_unknown = kwargs_unknown;

        fluent_key
    }

    fn code_location(&self, expr: &ruff_python_ast::ExprCall) -> CodeLocation {
        let (line, column) = self
            .line_index
            .line_column(self.source, expr.range_start.to_usize());

        CodeLocation {
            path: self.code_path.as_ref().clone(),
            line,
            column,
        }
    }

    #[inline]
    fn add_fluent_key(&mut self, expr: &ruff_python_ast::ExprCall, key: String) {
        let new_fluent_key = self.create_fluent_key(expr, key);

        match self.fluent_keys.get_mut(&new_fluent_key.key) {
            Some(kept) => {
                if let Some(conflict) = merge_key_occurrence(kept, new_fluent_key) {
                    self.diagnostics.push(conflict);
                }
            }
            None => {
                self.fluent_keys
                    .insert(new_fluent_key.key.clone(), new_fluent_key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{FluentEntry, FluentKey, I18nMatcher, code_message};
    use crate::ftl::code_extractor::kwargs_from_key;
    use crate::ftl::diagnostics::{ExtractionDiagnostic, ExtractionDiagnosticKind};
    use crate::ftl::utils::{FastHashMap, FastHashSet};
    use pretty_assertions::assert_eq;
    use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
    use std::path::PathBuf;
    use std::sync::{Arc, LazyLock};

    static I18N: LazyLock<FastHashSet<String>> =
        LazyLock::new(|| FastHashSet::from_iter(["i18n".to_string()]));
    static EMPTY: LazyLock<FastHashSet<String>> = LazyLock::new(FastHashSet::default);

    fn run(source: &str) -> (FastHashMap<String, FluentKey>, Vec<ExtractionDiagnostic>) {
        let module = ruff_python_parser::parse_module(source).unwrap();
        let mut matcher = I18nMatcher::new(
            PathBuf::from("app.py"),
            source,
            PathBuf::from("_default.ftl"),
            &I18N,
            &EMPTY,
            &EMPTY,
            &EMPTY,
        );
        matcher.visit_body(module.suite());
        (matcher.fluent_keys, matcher.diagnostics)
    }

    #[test]
    fn test_code_message_sorts_kwargs_by_name() {
        let message = code_message(
            "k".to_string(),
            vec!["zeta".to_string(), "alpha".to_string(), "mid".to_string()],
        );
        let key = FluentKey::new(
            Arc::new(PathBuf::from("app.py")),
            "k".to_string(),
            FluentEntry::Message(message),
            Arc::new(PathBuf::from("_default.ftl")),
            None,
            None,
            FastHashSet::default(),
        );
        assert_eq!(kwargs_from_key(&key), vec!["alpha", "mid", "zeta"]);
    }

    #[test]
    fn test_same_kwargs_in_a_different_order_are_one_key() {
        let (keys, diagnostics) = run(
            "def f(i18n):\n    i18n.get(\"order\", a=1, b=2)\n    i18n.get(\"order\", b=2, a=1)\n",
        );

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(keys.len(), 1);
        assert_eq!(kwargs_from_key(&keys["order"]), vec!["a", "b"]);
    }

    #[test]
    fn test_different_kwargs_conflict_with_a_readable_message() {
        let (keys, diagnostics) = run(
            "def f(i18n):\n    i18n.get(\"order\", a=1, b=2)\n    i18n.get(\"order\", a=1, c=3)\n",
        );

        assert_eq!(keys.len(), 1);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyMessageConflict
        );
        assert_eq!(diagnostics[0].key.as_deref(), Some("order"));
        assert_eq!(diagnostics[0].locations.len(), 2);
        assert_eq!(
            diagnostics[0].message,
            "Fluent key order is used with different keyword arguments: a, b and a, c"
        );
        assert_eq!(
            diagnostics[0].to_string(),
            "[key-message-conflict] Fluent key order is used with different keyword arguments: a, b and a, c (app.py:2:5, app.py:3:5)"
        );
    }

    #[test]
    fn test_conflict_message_names_a_call_without_kwargs() {
        let (_, diagnostics) = run("i18n.get(\"k\")\ni18n.get(\"k\", a=1)\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "Fluent key k is used with different keyword arguments: no keyword arguments and a"
        );
    }

    #[test]
    fn test_different_paths_still_conflict() {
        let (_, diagnostics) =
            run("i18n.get(\"k\", _path=\"one.ftl\")\ni18n.get(\"k\", _path=\"two.ftl\")\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyPathConflict
        );
    }

    fn location(line: usize, column: usize) -> crate::ftl::diagnostics::CodeLocation {
        crate::ftl::diagnostics::CodeLocation {
            path: PathBuf::from("app.py"),
            line,
            column,
        }
    }

    #[test]
    fn test_double_star_kwargs_mark_the_key_as_unverifiable() {
        let (keys, diagnostics) = run("i18n.get(\"welcome\", **data)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(keys.len(), 1);
        assert!(kwargs_from_key(&keys["welcome"]).is_empty());
        assert_eq!(keys["welcome"].kwargs_unknown, Some(location(1, 1)));
    }

    #[test]
    fn test_explicit_kwargs_next_to_double_star_are_kept() {
        let (keys, _) = run("i18n.get(\"k\", a=1, **data)\n");

        assert_eq!(kwargs_from_key(&keys["k"]), vec!["a"]);
        assert_eq!(keys["k"].kwargs_unknown, Some(location(1, 1)));
    }

    #[test]
    fn test_unverifiable_marker_is_carried_by_the_kept_occurrence() {
        // The first call is kept as the key; the marker comes from the second call.
        let (keys, diagnostics) = run("i18n.get(\"k\", a=1)\ni18n.get(\"k\", a=1, **data)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(keys["k"].source_location, Some(location(1, 1)));
        assert_eq!(keys["k"].kwargs_unknown, Some(location(2, 1)));
    }

    #[test]
    fn test_explicit_conflicts_are_still_reported_next_to_double_star() {
        let (keys, diagnostics) = run(
            "i18n.get(\"k\", a=1, b=2)\ni18n.get(\"k\", a=1, b=2, **data)\ni18n.get(\"k\", a=1, c=3)\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyMessageConflict
        );
        assert_eq!(
            diagnostics[0].message,
            "Fluent key k is used with different keyword arguments: a, b and a, c"
        );
        // The `**` call is neither compared nor named.
        assert_eq!(
            diagnostics[0].locations,
            vec![location(1, 1), location(3, 1)]
        );
        assert_eq!(keys["k"].kwargs_unknown, Some(location(2, 1)));
    }

    #[test]
    fn test_calls_without_a_key_are_still_skipped() {
        let (keys, diagnostics) = run("i18n.get(*args)\ni18n.get(**kw)\ni18n.get(\"k\", *rest)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(keys.len(), 1);
        assert_eq!(keys["k"].kwargs_unknown, None);
    }

    #[test]
    fn test_explicit_call_and_double_star_call_are_one_key() {
        let (keys, diagnostics) =
            run("i18n.get(\"welcome\", name=x)\ni18n.get(\"welcome\", **data)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(keys.len(), 1);
        assert_eq!(kwargs_from_key(&keys["welcome"]), vec!["name"]);
        assert_eq!(keys["welcome"].source_location, Some(location(1, 1)));
        assert_eq!(keys["welcome"].kwargs_unknown, Some(location(2, 1)));
        assert!(!keys["welcome"].kept_call_has_double_star());
    }

    #[test]
    fn test_explicit_call_wins_even_when_the_double_star_call_comes_first() {
        let (keys, diagnostics) =
            run("i18n.get(\"welcome\", **data)\ni18n.get(\"welcome\", name=x)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(kwargs_from_key(&keys["welcome"]), vec!["name"]);
        assert_eq!(keys["welcome"].source_location, Some(location(2, 1)));
        assert_eq!(keys["welcome"].kwargs_unknown, Some(location(1, 1)));
        assert!(!keys["welcome"].kept_call_has_double_star());
    }

    #[test]
    fn test_two_partial_double_star_calls_keep_the_first_by_location() {
        let (keys, diagnostics) =
            run("i18n.get(\"k\", b=2, **data)\ni18n.get(\"k\", a=1, **data)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert_eq!(kwargs_from_key(&keys["k"]), vec!["b"]);
        assert_eq!(keys["k"].source_location, Some(location(1, 1)));
        assert_eq!(keys["k"].kwargs_unknown, Some(location(1, 1)));
        assert!(keys["k"].kept_call_has_double_star());
    }

    #[test]
    fn test_two_double_star_only_calls_have_no_variables() {
        let (keys, diagnostics) = run("i18n.get(\"k\", **data)\ni18n.get(\"k\", **other)\n");

        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        assert!(kwargs_from_key(&keys["k"]).is_empty());
        assert_eq!(keys["k"].kwargs_unknown, Some(location(1, 1)));
        assert!(keys["k"].kept_call_has_double_star());
    }

    #[test]
    fn test_path_conflicts_ignore_double_star() {
        let (_, diagnostics) =
            run("i18n.get(\"k\", _path=\"one.ftl\", **data)\ni18n.get(\"k\", _path=\"two.ftl\")\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].kind,
            ExtractionDiagnosticKind::KeyPathConflict
        );
    }
}
