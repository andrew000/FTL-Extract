#![allow(unused_variables)]

use crate::ftl::consts;
use crate::ftl::diagnostics::{CodeLocation, ExtractionDiagnostic, ExtractionDiagnosticKind};
use crate::ftl::utils::{FastHashMap, FastHashSet};
use anyhow::{Result, bail};
use fluent::types::AnyEq;
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
    source: Option<&'a str>,
    collect_diagnostics: bool,
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
            source: None,
            collect_diagnostics: false,
            diagnostics: Vec::new(),
            fluent_keys: FastHashMap::default(),
        }
    }

    pub(crate) fn collecting(
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
            source: Some(source),
            collect_diagnostics: true,
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

        self.add_fluent_key(expr, key).unwrap();
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

            self.add_fluent_key(expr, key).unwrap();
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

        self.add_fluent_key(expr, key).unwrap();
    }
    #[inline]
    fn create_fluent_key(&self, expr: &ruff_python_ast::ExprCall, key: String) -> FluentKey {
        let path = self.default_ftl_file.clone();

        let mut fluent_key = FluentKey::new(
            self.code_path.clone(),
            key.clone(),
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier { name: key.clone() },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::TextElement { value: key }],
                }),
                attributes: vec![],
                comment: None,
            }),
            path,
            None,
            None,
            FastHashSet::default(),
        );
        fluent_key.source_location = self.code_location(expr);

        let keywords = expr
            .arguments
            .keywords
            .iter()
            .filter_map(|keyword| keyword.arg.as_ref().map(|arg| keyword.to_owned()))
            .collect::<Vec<_>>();

        let fluent_key_message_elements = match Arc::get_mut(&mut fluent_key.entry).unwrap() {
            FluentEntry::Message(msg) => match msg.value.as_mut() {
                Some(pattern) => &mut pattern.elements,
                None => panic!("Message has no value pattern"),
            },
            _ => panic!("Expected FluentEntry::Message"),
        };

        for kw in keywords {
            if kw.arg.is_none() {
                continue;
            }

            let arg = kw.arg.as_ref().unwrap();

            if arg.as_str() == consts::PATH_LITERAL {
                if kw.value.is_string_literal_expr() {
                    let raw_path = &kw.value.as_string_literal_expr().as_ref().unwrap().value;

                    if !raw_path.is_empty() {
                        let p = PathBuf::from(raw_path.to_str());
                        if p.extension().is_none() {
                            let mut new_p = p.clone();
                            new_p.push(self.default_ftl_file.as_ref());
                            fluent_key.path = Arc::new(new_p);
                        } else {
                            fluent_key.path = Arc::new(p);
                        }
                    }
                }
            } else {
                if self.ignore_kwargs.contains(arg.as_str()) {
                    continue;
                }

                fluent_key_message_elements.push(fluent_syntax::ast::PatternElement::Placeable {
                    expression: fluent_syntax::ast::Expression::Inline(
                        fluent_syntax::ast::InlineExpression::VariableReference {
                            id: fluent_syntax::ast::Identifier {
                                name: arg.to_string(),
                            },
                        },
                    ),
                })
            }
        }

        fluent_key
    }

    fn code_location(&self, expr: &ruff_python_ast::ExprCall) -> Option<CodeLocation> {
        let source = self.source?;
        let (line, column) = line_column(source, expr.range.start().to_u32() as usize);

        Some(CodeLocation {
            path: self.code_path.as_ref().clone(),
            line,
            column,
        })
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
            key: new_fluent_key.key.clone(),
            message,
            locations,
        });
    }

    #[inline]
    fn add_fluent_key(&mut self, expr: &ruff_python_ast::ExprCall, key: String) -> Result<()> {
        let new_fluent_key = self.create_fluent_key(expr, key);

        if let Some(existing) = self.fluent_keys.get(&new_fluent_key.key).cloned() {
            if existing.path != new_fluent_key.path {
                let message = format!(
                    "Fluent key {} has different paths: {} and {}",
                    new_fluent_key.key,
                    new_fluent_key.path.display(),
                    existing.path.display()
                );
                if self.collect_diagnostics {
                    self.add_conflict_diagnostic(
                        ExtractionDiagnosticKind::KeyPathConflict,
                        &existing,
                        &new_fluent_key,
                        message,
                    );
                    return Ok(());
                }
                bail!(message)
            }
            if let (FluentEntry::Message(existing_message), FluentEntry::Message(new_message)) =
                (existing.entry.as_ref(), new_fluent_key.entry.as_ref())
            {
                if !existing_message.clone().equals(new_message) {
                    let message = format!(
                        "Fluent key {} has different translations:\n{:?}\nand\n{:?}",
                        new_fluent_key.key, new_message, existing_message
                    );
                    if self.collect_diagnostics {
                        self.add_conflict_diagnostic(
                            ExtractionDiagnosticKind::KeyMessageConflict,
                            &existing,
                            &new_fluent_key,
                            message,
                        );
                        return Ok(());
                    }
                    bail!(message);
                }
            } else {
                let message = format!(
                    "Fluent key {} is not a Message in one of the entries.",
                    new_fluent_key.key
                );
                if self.collect_diagnostics {
                    self.add_conflict_diagnostic(
                        ExtractionDiagnosticKind::KeyTypeConflict,
                        &existing,
                        &new_fluent_key,
                        message,
                    );
                    return Ok(());
                }
                bail!(message);
            }
        } else {
            self.fluent_keys
                .insert(new_fluent_key.key.clone(), new_fluent_key);
        }

        Ok(())
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
