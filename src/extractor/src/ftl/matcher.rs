#![allow(unused_variables)]

use crate::ftl::consts;
use anyhow::{Result, bail};
use fluent::types::AnyEq;
use hashbrown::{HashMap, HashSet};
use rustpython_ast::{self as py_ast, Keyword, MatchCase};
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) struct FluentKey {
    pub(crate) code_path: PathBuf,
    pub(crate) key: String,
    pub(crate) message: Option<fluent_syntax::ast::Message<String>>,
    pub(crate) term: Option<fluent_syntax::ast::Term<String>>,
    pub(crate) comment: Option<fluent_syntax::ast::Comment<String>>,
    pub(crate) junk: Option<String>,
    pub(crate) path: PathBuf,
    pub(crate) locale: Option<String>,
    pub(crate) position: usize,
    pub(crate) depends_on_keys: HashSet<String>,
}

impl FluentKey {
    pub(crate) fn new(
        code_path: PathBuf,
        key: String,
        message: Option<fluent_syntax::ast::Message<String>>,
        term: Option<fluent_syntax::ast::Term<String>>,
        comment: Option<fluent_syntax::ast::Comment<String>>,
        junk: Option<String>,
        path: PathBuf,
        locale: Option<String>,
        position: Option<usize>,
        depends_on_keys: HashSet<String>,
    ) -> Self {
        Self {
            code_path,
            key,
            message,
            term,
            comment,
            junk,
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

impl<'a> py_ast::Visitor for I18nMatcher<'a> {
    fn visit_expr_call(&mut self, node: py_ast::ExprCall<py_ast::text_size::TextRange>) {
        if node.func.is_attribute_expr() {
            self.process_attribute_call(&node);
        } else if node.func.is_name_expr()
            && self
                .i18n_keys
                .contains(&node.func.as_name_expr().unwrap().id.to_string())
        {
            self.process_name_call(&node);
        } else {
            // println!(
            //     "Ignoring {:#?}, {}, {}",
            //     node.func,
            //     node.func.is_name_expr(),
            //     &node.func.as_name_expr().unwrap().id.as_str()
            // );
        }

        self.generic_visit_expr_call(node)
    }

    fn visit_keyword(&mut self, node: Keyword<py_ast::text_size::TextRange>) {
        self.generic_visit_keyword(node)
    }
    fn generic_visit_keyword(&mut self, node: Keyword<py_ast::text_size::TextRange>) {
        self.visit_expr(node.value)
    }

    fn visit_match_case(&mut self, node: MatchCase<py_ast::text_size::TextRange>) {
        self.generic_visit_match_case(node)
    }
    fn generic_visit_match_case(&mut self, node: MatchCase<py_ast::text_size::TextRange>) {
        for value in node.body {
            self.visit_stmt(value);
        }
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
    fn process_attribute_call(&mut self, node: &py_ast::ExprCall<py_ast::text_size::TextRange>) {
        let mut attr = node.func.clone();
        let mut attrs: Vec<String> = vec![];
        while attr.is_attribute_expr() {
            let attribute_expr = attr.as_attribute_expr().unwrap();
            attrs.push(attribute_expr.attr.to_string());
            attr = attribute_expr.value.clone();
        }

        if attr.is_name_expr() {
            self.process_attribute_name_call(node, attr.as_mut_name_expr().unwrap(), attrs);
        }
    }

    fn process_name_call(&mut self, node: &py_ast::ExprCall<py_ast::text_size::TextRange>) {
        if node.args.is_empty() || !node.args.first().unwrap().is_constant_expr() {
            return;
        }

        let key = node
            .args
            .first()
            .unwrap()
            .as_constant_expr()
            .unwrap()
            .value
            .clone()
            .str()
            .unwrap();

        self.add_fluent_key(node, key).unwrap();
    }

    fn process_attribute_name_call(
        &mut self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        attr: &mut py_ast::ExprName,
        mut attrs: Vec<String>,
    ) {
        if self.i18n_keys.contains(attr.id.as_str()) {
            self.process_i18n_key_call(node, attrs);
        } else if self.i18n_keys_prefix.contains(&attr.id.to_string())
            && !attrs.is_empty()
            && self.i18n_keys.contains(&attrs.last().unwrap().to_string())
        {
            // Remove the last attribute to handle cases where the prefix key is followed by a
            // valid i18n key.
            attrs.pop();
            self.process_i18n_key_call(node, attrs);
        }
    }

    fn process_i18n_key_call(
        &mut self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        attrs: Vec<String>,
    ) {
        if attrs.len() == 1 && attrs.first().unwrap() == consts::GET_LITERAL {
            self.process_i18n_key_call_get_literal(node, attrs);
        } else {
            self.process_i18n_key_call_attrs(node, attrs);
        }
    }
    fn process_i18n_key_call_get_literal(
        &mut self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        mut attrs: Vec<String>,
    ) {
        if node.args.is_empty() {
            return;
        }

        attrs.clear();
        let arg = node.args.first().unwrap();
        if arg.is_constant_expr() {
            let key = arg
                .as_constant_expr()
                .unwrap()
                .value
                .as_str()
                .unwrap()
                .clone();

            self.add_fluent_key(node, key).unwrap();
        }
    }

    fn process_i18n_key_call_attrs(
        &mut self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        attrs: Vec<String>,
    ) {
        if self
            .ignore_attributes
            .contains(&attrs.last().unwrap().to_string())
        {
            return;
        }

        self.add_fluent_key(
            node,
            attrs
                .iter()
                .rev()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
                .join("-"),
        )
        .unwrap();
    }

    fn create_fluent_key(
        &self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        key: String,
    ) -> FluentKey {
        let mut fluent_key = FluentKey::new(
            self.code_path.clone(),
            key.clone(),
            Some(fluent_syntax::ast::Message {
                id: fluent_syntax::ast::Identifier { name: key.clone() },
                value: Some(fluent_syntax::ast::Pattern {
                    elements: vec![fluent_syntax::ast::PatternElement::TextElement {
                        value: key.clone(),
                    }],
                }),
                attributes: vec![],
                comment: None,
            }),
            None,
            None,
            None,
            self.default_ftl_file.clone(),
            None,
            None,
            HashSet::new(),
        );

        let keywords = node
            .keywords
            .iter()
            .filter_map(|keyword| keyword.arg.as_ref().map(|arg| keyword.to_owned()))
            .collect::<Vec<_>>();

        let fluent_key_message_elements = &mut fluent_key
            .message
            .as_mut()
            .unwrap()
            .value
            .as_mut()
            .unwrap()
            .elements;

        for kw in keywords {
            if kw.arg.is_none() {
                continue;
            }

            let arg = kw.arg.clone().unwrap();

            if arg.as_str() == consts::PATH_LITERAL {
                if kw.value.is_constant_expr() {
                    let path = &kw.value.as_constant_expr().as_ref().unwrap().value;

                    if path.is_str() {
                        fluent_key.path = PathBuf::from(path.clone().expect_str());
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

    fn add_fluent_key(
        &mut self,
        node: &py_ast::ExprCall<py_ast::text_size::TextRange>,
        key: String,
    ) -> Result<()> {
        let new_fluent_key = self.create_fluent_key(node, key);

        if self.fluent_keys.contains_key(&new_fluent_key.key) {
            if self.fluent_keys[&new_fluent_key.key].path != new_fluent_key.path {
                bail!(
                    "Fluent key {} has different paths: {} and {}",
                    new_fluent_key.key,
                    new_fluent_key.path.display(),
                    self.fluent_keys[&new_fluent_key.key].path.display()
                )
            }
            if !self.fluent_keys[&new_fluent_key.key]
                .message
                .clone()
                .unwrap()
                .equals(new_fluent_key.message.as_ref().unwrap())
            {
                bail!(
                    "Fluent key {} has different translations:\n{:?}\nand\n{:?}",
                    new_fluent_key.key,
                    &new_fluent_key.message.as_ref().unwrap(),
                    self.fluent_keys[&new_fluent_key.key]
                        .message
                        .clone()
                        .unwrap()
                );
            }
        } else {
            self.fluent_keys
                .insert(new_fluent_key.key.clone(), new_fluent_key);
        }

        Ok(())
    }
}
