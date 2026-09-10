#![allow(unused_variables)]

use crate::ftl::code_extractor::kwargs_from_key;
use crate::ftl::consts;
use crate::ftl::diagnostics::{CodeLocation, ExtractionDiagnostic, ExtractionDiagnosticKind};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use common::LineIndex;
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
    Junk(String),
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

    /// Records that `other`, another occurrence of the same key, passed `**kwargs`. The first
    /// such call site is kept.
    pub(crate) fn absorb_kwargs_unknown(&mut self, other: &FluentKey) {
        if self.kwargs_unknown.is_none() {
            self.kwargs_unknown = other.kwargs_unknown.clone();
        }
    }
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

/// Message text of a `KeyMessageConflict`: the two keyword-argument sets with their locations,
/// in the same order as the diagnostic's `locations` (existing first).
pub(crate) fn kwargs_conflict_message(existing: &FluentKey, new_fluent_key: &FluentKey) -> String {
    fn describe(key: &FluentKey) -> String {
        let kwargs = kwargs_from_key(key);
        let kwargs = if kwargs.is_empty() {
            "no keyword arguments".to_string()
        } else {
            kwargs.join(", ")
        };
        match &key.source_location {
            Some(location) => format!("{kwargs} ({location})"),
            None => kwargs,
        }
    }

    format!(
        "Fluent key {} is used with different keyword arguments: {} and {}",
        new_fluent_key.key,
        describe(existing),
        describe(new_fluent_key)
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

    fn add_conflict_diagnostic(
        &mut self,
        kind: ExtractionDiagnosticKind,
        existing: &FluentKey,
        new_fluent_key: &FluentKey,
        message: String,
    ) {
        let mut locations = Vec::new();
        if let Some(location) = existing.source_location.clone() {
            locations.push(location);
        }
        if let Some(location) = new_fluent_key.source_location.clone() {
            locations.push(location);
        }

        self.diagnostics.push(ExtractionDiagnostic {
            kind,
            key: Some(new_fluent_key.key.clone()),
            message,
            locations,
        });
    }

    #[inline]
    fn add_fluent_key(&mut self, expr: &ruff_python_ast::ExprCall, key: String) {
        let new_fluent_key = self.create_fluent_key(expr, key);

        let Some(existing) = self.fluent_keys.get_mut(&new_fluent_key.key) else {
            self.fluent_keys
                .insert(new_fluent_key.key.clone(), new_fluent_key);
            return;
        };
        // The kept occurrence carries the `**kwargs` marker of every occurrence; explicit
        // keyword arguments are still compared below, independently of it.
        existing.absorb_kwargs_unknown(&new_fluent_key);
        let existing = existing.clone();

        if existing.path != new_fluent_key.path {
            let message = format!(
                "Fluent key {} has different paths: {} and {}",
                new_fluent_key.key,
                new_fluent_key.path.display(),
                existing.path.display()
            );
            self.add_conflict_diagnostic(
                ExtractionDiagnosticKind::KeyPathConflict,
                &existing,
                &new_fluent_key,
                message,
            );
            return;
        }

        match (existing.entry.as_ref(), new_fluent_key.entry.as_ref()) {
            (FluentEntry::Message(existing_message), FluentEntry::Message(new_message)) => {
                if existing_message != new_message {
                    let message = kwargs_conflict_message(&existing, &new_fluent_key);
                    self.add_conflict_diagnostic(
                        ExtractionDiagnosticKind::KeyMessageConflict,
                        &existing,
                        &new_fluent_key,
                        message,
                    );
                }
            }
            _ => {
                let message = format!(
                    "Fluent key {} is not a Message in one of the entries.",
                    new_fluent_key.key
                );
                self.add_conflict_diagnostic(
                    ExtractionDiagnosticKind::KeyTypeConflict,
                    &existing,
                    &new_fluent_key,
                    message,
                );
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
            "Fluent key order is used with different keyword arguments: a, b (app.py:2:5) and a, c (app.py:3:5)"
        );
        assert_eq!(
            diagnostics[0].to_string(),
            "[key-message-conflict] Fluent key order is used with different keyword arguments: a, b (app.py:2:5) and a, c (app.py:3:5) (app.py:2:5, app.py:3:5)"
        );
    }

    #[test]
    fn test_conflict_message_names_a_call_without_kwargs() {
        let (_, diagnostics) = run("i18n.get(\"k\")\ni18n.get(\"k\", a=1)\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].message,
            "Fluent key k is used with different keyword arguments: no keyword arguments (app.py:1:1) and a (app.py:2:1)"
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
            "Fluent key k is used with different keyword arguments: a, b (app.py:1:1) and a, c (app.py:3:1)"
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
}
