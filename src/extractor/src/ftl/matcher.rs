#![allow(unused_variables)]

use crate::ftl::consts;
use anyhow::{Result, bail};
use fluent::types::AnyEq;
use hashbrown::{HashMap, HashSet};
use ruff_python_ast::visitor::source_order::SourceOrderVisitor;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum FluentEntry {
    Message(fluent_syntax::ast::Message<String>),
    Term(fluent_syntax::ast::Term<String>),
    Comment(fluent_syntax::ast::Comment<String>),
    Junk(String),
}

#[derive(Clone, Debug)]
pub(crate) struct FluentKey {
    pub(crate) code_path: PathBuf,
    pub(crate) key: String,
    pub(crate) entry: FluentEntry,
    pub(crate) path: PathBuf,
    pub(crate) locale: Option<String>,
    pub(crate) position: usize,
    pub(crate) depends_on_keys: HashSet<String>,
}

impl FluentKey {
    pub(crate) fn new(
        code_path: PathBuf,
        key: String,
        entry: FluentEntry,
        path: PathBuf,
        locale: Option<String>,
        position: Option<usize>,
        depends_on_keys: HashSet<String>,
    ) -> Self {
        Self {
            code_path,
            key,
            entry,
            path,
            locale,
            position: position.unwrap_or(usize::MAX),
            depends_on_keys,
        }
    }
}

pub(crate) struct I18nMatcher<'a> {
    code_path: PathBuf,
    default_ftl_file: PathBuf,
    i18n_keys: &'a HashSet<String>,
    i18n_keys_prefix: &'a HashSet<String>,
    ignore_attributes: &'a HashSet<String>,
    ignore_kwargs: &'a HashSet<String>,
    pub(crate) fluent_keys: HashMap<String, FluentKey>,
}

impl<'a> SourceOrderVisitor<'a> for I18nMatcher<'a> {
    #[inline]
    fn visit_expr(&mut self, expr: &'a ruff_python_ast::Expr) {
        if expr.is_call_expr() {
            let expr = expr.as_call_expr().unwrap();
            if expr.func.is_attribute_expr() {
                self.process_attribute_call(&expr);
            } else if expr.func.is_name_expr()
                && self
                    .i18n_keys
                    .contains(&expr.func.as_name_expr().unwrap().id.to_string())
            {
                self.process_name_call(&expr);
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
        i18n_keys: &'a HashSet<String>,
        i18n_keys_prefix: &'a HashSet<String>,
        ignore_attributes: &'a HashSet<String>,
        ignore_kwargs: &'a HashSet<String>,
    ) -> Self {
        Self {
            code_path,
            default_ftl_file,
            i18n_keys,
            i18n_keys_prefix,
            ignore_attributes,
            ignore_kwargs,
            fluent_keys: HashMap::<String, FluentKey>::new(),
        }
    }
    #[inline]
    fn process_attribute_call(&mut self, expr: &ruff_python_ast::ExprCall) {
        let mut attr = expr.func.clone();
        let mut attrs: Vec<String> = vec![];
        while attr.is_attribute_expr() {
            let attribute_expr = attr.as_attribute_expr().unwrap();
            attrs.push(attribute_expr.attr.to_string());
            attr = attribute_expr.value.clone();
        }

        if attr.is_name_expr() {
            self.process_attribute_name_call(expr, attr.as_name_expr_mut().unwrap(), attrs);
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
        attr: &mut ruff_python_ast::ExprName,
        mut attrs: Vec<String>,
    ) {
        if self.i18n_keys.contains(attr.id.as_str()) {
            self.process_i18n_key_call(expr, attrs);
        } else if self.i18n_keys_prefix.contains(&attr.id.to_string())
            && !attrs.is_empty()
            && self.i18n_keys.contains(&attrs.last().unwrap().to_string())
        {
            // Remove the last attribute to handle cases where the prefix key is followed by a
            // valid i18n key.
            attrs.pop();
            self.process_i18n_key_call(expr, attrs);
        }
    }
    #[inline]
    fn process_i18n_key_call(&mut self, expr: &ruff_python_ast::ExprCall, attrs: Vec<String>) {
        if attrs.len() == 1 && attrs.first().unwrap() == consts::GET_LITERAL {
            self.process_i18n_key_call_get_literal(expr, attrs);
        } else {
            self.process_i18n_key_call_attrs(expr, attrs);
        }
    }
    #[inline]
    fn process_i18n_key_call_get_literal(
        &mut self,
        expr: &ruff_python_ast::ExprCall,
        mut attrs: Vec<String>,
    ) {
        if expr.arguments.is_empty() {
            return;
        }

        attrs.clear();
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
        attrs: Vec<String>,
    ) {
        if self
            .ignore_attributes
            .contains(&attrs.last().unwrap().to_string())
        {
            return;
        }

        self.add_fluent_key(
            expr,
            attrs
                .iter()
                .rev()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join("-"),
        )
        .unwrap();
    }
    #[inline]
    fn create_fluent_key(&self, expr: &ruff_python_ast::ExprCall, key: String) -> FluentKey {
        let mut fluent_key = FluentKey::new(
            self.code_path.clone(),
            key.clone(),
            FluentEntry::Message(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier { name: key.clone() },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                        value: key.clone(),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }),
            self.default_ftl_file.clone(),
            None,
            None,
            HashSet::new(),
        );

        let keywords = expr
            .arguments
            .keywords
            .iter()
            .filter_map(|keyword| keyword.arg.as_ref().map(|arg| keyword.to_owned()))
            .collect::<Vec<_>>();

        let fluent_key_message_elements = match &mut fluent_key.entry {
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

            let arg = kw.arg.clone().unwrap();

            if arg.as_str() == consts::PATH_LITERAL {
                if kw.value.is_string_literal_expr() {
                    let path = &kw.value.as_string_literal_expr().as_ref().unwrap().value;

                    if !path.is_empty() {
                        fluent_key.path = PathBuf::from(path.clone().to_str());
                    }
                }
            } else {
                if self.ignore_kwargs.contains(&arg.to_string()) {
                    continue;
                }

                fluent_key_message_elements.push(fluent_syntax::ast::PatternElement::Placeable {
                    expression: fluent_syntax::ast::Expression::Inline(
                        fluent_syntax::ast::InlineExpression::VariableReference {
                            id: fluent_syntax::ast::Identifier {
                                name: kw.arg.as_ref().unwrap().to_string(),
                            },
                        },
                    ),
                })
            }
        }

        fluent_key
    }
    #[inline]
    fn add_fluent_key(&mut self, expr: &ruff_python_ast::ExprCall, key: String) -> Result<()> {
        let new_fluent_key = self.create_fluent_key(expr, key);

        if self.fluent_keys.contains_key(&new_fluent_key.key) {
            if self.fluent_keys[&new_fluent_key.key].path != new_fluent_key.path {
                bail!(
                    "Fluent key {} has different paths: {} and {}",
                    new_fluent_key.key,
                    new_fluent_key.path.display(),
                    self.fluent_keys[&new_fluent_key.key].path.display()
                )
            }
            if let (FluentEntry::Message(existing_message), FluentEntry::Message(new_message)) = (
                &self.fluent_keys[&new_fluent_key.key].entry,
                &new_fluent_key.entry,
            ) {
                if !existing_message.clone().equals(new_message) {
                    bail!(
                        "Fluent key {} has different translations:\n{:?}\nand\n{:?}",
                        new_fluent_key.key,
                        new_message,
                        existing_message
                    );
                }
            } else {
                bail!(
                    "Fluent key {} is not a Message in one of the entries.",
                    new_fluent_key.key
                );
            }
        } else {
            self.fluent_keys
                .insert(new_fluent_key.key.clone(), new_fluent_key);
        }

        Ok(())
    }
}
