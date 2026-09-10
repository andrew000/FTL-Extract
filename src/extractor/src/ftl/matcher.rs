#![allow(unused_variables)]

use crate::ftl::consts;
use crate::ftl::diagnostics::{CodeLocation, ExtractionDiagnostic, ExtractionDiagnosticKind};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use memchr::memchr_iter;
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
        }
    }
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
        if expr.arguments.is_empty()
            || !expr
                .arguments
                .find_positional(0)
                .unwrap()
                .is_string_literal_expr()
        {
            return;
        }

        let key = expr
            .arguments
            .find_positional(0)
            .unwrap()
            .as_string_literal_expr()
            .unwrap()
            .value
            .clone()
            .to_string();

        self.add_fluent_key(expr, key);
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
            && !attrs.is_empty()
            && self.i18n_keys.contains(*attrs.last().unwrap())
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
        if attrs.len() == 1 && *attrs.first().unwrap() == consts::GET_LITERAL {
            self.process_i18n_key_call_get_literal(expr);
        } else {
            self.process_i18n_key_call_attrs(expr, attrs);
        }
    }
    #[inline]
    fn process_i18n_key_call_get_literal(&mut self, expr: &ruff_python_ast::ExprCall) {
        if expr.arguments.is_empty() {
            return;
        }

        let arg = expr.arguments.find_positional(0).unwrap();
        if arg.is_string_literal_expr() {
            let key = arg
                .as_string_literal_expr()
                .unwrap()
                .value
                .to_string()
                .clone();

            self.add_fluent_key(expr, key);
        }
    }
    #[inline]
    fn process_i18n_key_call_attrs(
        &mut self,
        expr: &ruff_python_ast::ExprCall,
        attrs: SmallVec<&str, 8>,
    ) {
        if self.ignore_attributes.contains(*attrs.last().unwrap()) {
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
        let mut elements =
            vec![fluent_syntax::ast::PatternElement::TextElement { value: key.clone() }];

        for kw in &expr.arguments.keywords {
            let Some(arg) = kw.arg.as_ref() else {
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
                elements.push(fluent_syntax::ast::PatternElement::Placeable {
                    expression: fluent_syntax::ast::Expression::Inline(
                        fluent_syntax::ast::InlineExpression::VariableReference {
                            id: fluent_syntax::ast::Identifier {
                                name: arg.to_string(),
                            },
                        },
                    ),
                });
            }
        }

        let mut fluent_key = FluentKey::new(
            self.code_path.clone(),
            key.clone(),
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier { name: key },
                value: Some(fluent_syntax::ast::Pattern { elements }),
                attributes: vec![],
                comment: None,
            }),
            path,
            None,
            None,
            FastHashSet::default(),
        );
        fluent_key.source_location = Some(self.code_location(expr));

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

        let Some(existing) = self.fluent_keys.get(&new_fluent_key.key).cloned() else {
            self.fluent_keys
                .insert(new_fluent_key.key.clone(), new_fluent_key);
            return;
        };

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
                    let message = format!(
                        "Fluent key {} has different translations:\n{:?}\nand\n{:?}",
                        new_fluent_key.key, new_message, existing_message
                    );
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
