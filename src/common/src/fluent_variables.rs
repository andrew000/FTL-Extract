//! Which `$variables` a Fluent entry needs from the code that formats it.
//!
//! The rules follow the `fluent-bundle` resolver, so `ftl extract` and `ftl check` agree on the
//! same answer:
//!
//! - `{ $name }` in the pattern of the entry being formatted, in a selector, in a nested
//!   placeable or as a function argument (`{ NUMBER($count) }`) is a caller variable.
//! - `{ other }` pulls in the *value* of `other`; `{ other.attr }` pulls in only that attribute.
//! - `{ -term }` and `{ -term(case: "gen") }` pull in the term (or `{ -term.attr }` that
//!   attribute), but variables inside a term are resolved against the term's own call arguments
//!   only. They never come from the caller, whether the reference binds them, so nothing
//!   inside a term (including messages the term references) counts as a caller variable. The
//!   argument values of the reference itself are evaluated in the caller's scope and do count.
//! - Attributes of the entry being formatted are not visited unless
//!   [`VariableOptions::include_own_attributes`] is set: formatting `msg` renders `msg`'s value,
//!   not `msg.title`.
//!
//! Every (entry, attribute, scope) triple is visited at most once, so reference cycles terminate.
//! Unknown references are reported in [`CollectedVariables::unknown_references`] instead of
//! failing, so each caller decides whether they are an error.

use crate::FastHashSet;
use fluent_syntax::ast::{
    CallArguments, Expression, Identifier, InlineExpression, Message, Pattern, PatternElement, Term,
};

/// Lookup of the messages and terms an entry may reference.
pub trait FluentEntries {
    fn message(&self, id: &str) -> Option<&Message<String>>;
    fn term(&self, id: &str) -> Option<&Term<String>>;
}

/// How [`message_variables`] treats the message it starts from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct VariableOptions {
    /// Also count the variables used in the message's own attributes.
    pub include_own_attributes: bool,
}

/// A reference to a message or term that does not exist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnknownReference {
    Message(String),
    Term(String),
}

/// The result of a traversal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollectedVariables {
    /// Variables the caller has to pass.
    pub variables: FastHashSet<String>,
    /// Every message reached through a message reference, directly or transitively, including
    /// messages referenced from inside terms and messages that do not exist.
    pub referenced_messages: FastHashSet<String>,
    /// References that could not be resolved, in traversal order.
    pub unknown_references: Vec<UnknownReference>,
}

/// Collects the caller variables of `message`.
pub fn message_variables<E: FluentEntries>(
    entries: &E,
    message: &Message<String>,
    options: VariableOptions,
) -> CollectedVariables {
    let mut collector = Collector::new(entries);
    collector.visit_message(message, None, Scope::Caller);
    if options.include_own_attributes {
        for attribute in &message.attributes {
            collector.visit_message(message, Some(&attribute.id), Scope::Caller);
        }
    }
    collector.result
}

/// Collects the parameters of `term`: the variables its own value uses, which a reference has to
/// bind with call arguments. Terms the term references contribute nothing, as for messages.
pub fn term_variables<E: FluentEntries>(entries: &E, term: &Term<String>) -> CollectedVariables {
    let mut collector = Collector::new(entries);
    collector.visit_term(term, None, Scope::Caller);
    collector.result
}

/// Whose arguments a `$variable` resolves against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Scope {
    /// The arguments passed by the code that formats the entry.
    Caller,
    /// The call arguments of an enclosing term reference; the caller's arguments are invisible.
    Term,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum EntryKind {
    Message,
    Term,
}

struct Collector<'a, E> {
    entries: &'a E,
    result: CollectedVariables,
    /// (kind, entry id, attribute id, scope) already visited.
    visited: FastHashSet<(EntryKind, &'a str, Option<&'a str>, Scope)>,
}

impl<'a, E: FluentEntries> Collector<'a, E> {
    fn new(entries: &'a E) -> Self {
        Self {
            entries,
            result: CollectedVariables::default(),
            visited: FastHashSet::default(),
        }
    }

    fn visit_message(
        &mut self,
        message: &'a Message<String>,
        attribute: Option<&'a Identifier<String>>,
        scope: Scope,
    ) {
        let Some(pattern) = Self::message_pattern(message, attribute) else {
            return;
        };
        let attribute = attribute.map(|attribute| attribute.name.as_str());
        if !self.visited.insert((
            EntryKind::Message,
            message.id.name.as_str(),
            attribute,
            scope,
        )) {
            return;
        }
        self.visit_pattern(pattern, scope);
    }

    fn visit_term(
        &mut self,
        term: &'a Term<String>,
        attribute: Option<&'a Identifier<String>>,
        scope: Scope,
    ) {
        let Some(pattern) = Self::term_pattern(term, attribute) else {
            return;
        };
        let attribute = attribute.map(|attribute| attribute.name.as_str());
        if !self
            .visited
            .insert((EntryKind::Term, term.id.name.as_str(), attribute, scope))
        {
            return;
        }
        self.visit_pattern(pattern, scope);
    }

    /// The value of `message`, or the requested attribute. `None` for a message without a value
    /// or an attribute it does not have (the `references` check reports the latter).
    fn message_pattern<'m>(
        message: &'m Message<String>,
        attribute: Option<&Identifier<String>>,
    ) -> Option<&'m Pattern<String>> {
        match attribute {
            None => message.value.as_ref(),
            Some(attribute) => message
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute.name)
                .map(|attribute| &attribute.value),
        }
    }

    fn term_pattern<'t>(
        term: &'t Term<String>,
        attribute: Option<&Identifier<String>>,
    ) -> Option<&'t Pattern<String>> {
        match attribute {
            None => Some(&term.value),
            Some(attribute) => term
                .attributes
                .iter()
                .find(|candidate| candidate.id.name == attribute.name)
                .map(|attribute| &attribute.value),
        }
    }

    fn visit_pattern(&mut self, pattern: &'a Pattern<String>, scope: Scope) {
        for element in &pattern.elements {
            if let PatternElement::Placeable { expression } = element {
                self.visit_expression(expression, scope);
            }
        }
    }

    fn visit_expression(&mut self, expression: &'a Expression<String>, scope: Scope) {
        match expression {
            Expression::Inline(inline) => self.visit_inline(inline, scope),
            Expression::Select { selector, variants } => {
                self.visit_inline(selector, scope);
                for variant in variants {
                    self.visit_pattern(&variant.value, scope);
                }
            }
        }
    }

    fn visit_inline(&mut self, inline: &'a InlineExpression<String>, scope: Scope) {
        match inline {
            InlineExpression::VariableReference { id } => {
                if scope == Scope::Caller {
                    self.result.variables.insert(id.name.clone());
                }
            }
            InlineExpression::MessageReference { id, attribute } => {
                self.result.referenced_messages.insert(id.name.clone());
                match self.entries.message(&id.name) {
                    Some(message) => self.visit_message(message, attribute.as_ref(), scope),
                    None => self
                        .result
                        .unknown_references
                        .push(UnknownReference::Message(id.name.clone())),
                }
            }
            InlineExpression::TermReference {
                id,
                attribute,
                arguments,
            } => {
                if let Some(arguments) = arguments {
                    self.visit_call_arguments(arguments, scope);
                }
                match self.entries.term(&id.name) {
                    Some(term) => self.visit_term(term, attribute.as_ref(), Scope::Term),
                    None => self
                        .result
                        .unknown_references
                        .push(UnknownReference::Term(id.name.clone())),
                }
            }
            InlineExpression::FunctionReference { arguments, .. } => {
                self.visit_call_arguments(arguments, scope);
            }
            InlineExpression::Placeable { expression } => self.visit_expression(expression, scope),
            InlineExpression::StringLiteral { .. } | InlineExpression::NumberLiteral { .. } => {}
        }
    }

    /// Call arguments are evaluated where the call appears, so a `$variable` among them belongs
    /// to the current scope. Named values are literals by grammar; visiting them is harmless.
    fn visit_call_arguments(&mut self, arguments: &'a CallArguments<String>, scope: Scope) {
        for positional in &arguments.positional {
            self.visit_inline(positional, scope);
        }
        for named in &arguments.named {
            self.visit_inline(&named.value, scope);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CollectedVariables, FluentEntries, UnknownReference, VariableOptions, message_variables,
        term_variables,
    };
    use crate::FastHashMap;
    use fluent_syntax::ast::{Entry, Message, Resource, Term};
    use fluent_syntax::parser::parse;

    #[derive(Default)]
    struct Fixture {
        messages: FastHashMap<String, Message<String>>,
        terms: FastHashMap<String, Term<String>>,
    }

    impl Fixture {
        fn parse(source: &str) -> Self {
            let resource: Resource<String> = match parse(source.to_string()) {
                Ok(resource) => resource,
                Err((_, errors)) => panic!("fixture does not parse: {errors:?}"),
            };
            let mut fixture = Self::default();
            for entry in resource.body {
                match entry {
                    Entry::Message(message) => {
                        fixture.messages.insert(message.id.name.clone(), message);
                    }
                    Entry::Term(term) => {
                        fixture.terms.insert(term.id.name.clone(), term);
                    }
                    Entry::Junk { content } => panic!("junk in fixture: {content}"),
                    _ => {}
                }
            }
            fixture
        }

        fn message(&self, id: &str, include_own_attributes: bool) -> CollectedVariables {
            message_variables(
                self,
                &self.messages[id],
                VariableOptions {
                    include_own_attributes,
                },
            )
        }

        fn term(&self, id: &str) -> CollectedVariables {
            term_variables(self, &self.terms[id])
        }
    }

    impl FluentEntries for Fixture {
        fn message(&self, id: &str) -> Option<&Message<String>> {
            self.messages.get(id)
        }

        fn term(&self, id: &str) -> Option<&Term<String>> {
            self.terms.get(id)
        }
    }

    fn sorted(set: &crate::FastHashSet<String>) -> Vec<&str> {
        let mut names = set.iter().map(String::as_str).collect::<Vec<_>>();
        names.sort_unstable();
        names
    }

    #[test]
    fn plain_variables() {
        let fixture = Fixture::parse("msg = Hello { $username }, you have { $count } items\n");
        let collected = fixture.message("msg", false);
        assert_eq!(sorted(&collected.variables), ["count", "username"]);
        assert!(collected.referenced_messages.is_empty());
        assert!(collected.unknown_references.is_empty());
    }

    #[test]
    fn positional_function_argument() {
        let fixture = Fixture::parse("msg = You have { NUMBER($count) } items\n");
        assert_eq!(sorted(&fixture.message("msg", false).variables), ["count"]);
    }

    #[test]
    fn named_function_argument() {
        // Named values are literals by grammar, so the only variable is the positional one.
        let fixture =
            Fixture::parse("msg = { NUMBER($amount, minimumFractionDigits: 2) } { $unit }\n");
        assert_eq!(
            sorted(&fixture.message("msg", false).variables),
            ["amount", "unit"]
        );
    }

    #[test]
    fn function_in_selector() {
        let fixture = Fixture::parse(
            "msg =\n    { NUMBER($n) ->\n        [one] One item\n       *[other] Many items\n    }\n",
        );
        assert_eq!(sorted(&fixture.message("msg", false).variables), ["n"]);
    }

    #[test]
    fn nested_functions_and_placeables() {
        let fixture = Fixture::parse(
            "msg = { NUMBER(NUMBER($inner)) } {{ $nested }} { $sel ->\n    [a] { DATETIME($when) }\n   *[b] { { $deep } }\n}\n",
        );
        assert_eq!(
            sorted(&fixture.message("msg", false).variables),
            ["deep", "inner", "nested", "sel", "when"]
        );
    }

    #[test]
    fn term_with_parameters_binds_its_own_variables() {
        let fixture = Fixture::parse(
            "-brand =\n    { $case ->\n        [gen] Brand's\n       *[nom] Brand\n    }\nabout = About { -brand(case: \"gen\") }\n",
        );
        let collected = fixture.message("about", false);
        assert!(collected.variables.is_empty());
        assert!(collected.referenced_messages.is_empty());
        assert!(collected.unknown_references.is_empty());
    }

    #[test]
    fn term_without_arguments_needs_nothing_from_the_caller() {
        let fixture = Fixture::parse(
            "-brand =\n    { $case ->\n        [gen] Brand's\n       *[nom] Brand\n    }\nabout = About { -brand }\n",
        );
        assert!(fixture.message("about", false).variables.is_empty());
    }

    #[test]
    fn unbound_term_variable_is_not_a_caller_variable() {
        let fixture =
            Fixture::parse("-brand = Bot { $suffix }\nabout = About { -brand(case: \"gen\") }\n");
        assert!(fixture.message("about", false).variables.is_empty());
    }

    #[test]
    fn term_call_arguments_are_evaluated_in_the_caller_scope() {
        // Positional term arguments are not defined by the spec but parse; the resolver still
        // evaluates them where the reference appears.
        let fixture =
            Fixture::parse("-brand = Brand\nabout = About { -brand($who, case: \"gen\") }\n");
        assert_eq!(sorted(&fixture.message("about", false).variables), ["who"]);
    }

    #[test]
    fn message_referenced_from_a_term_is_in_term_scope_but_still_a_dependency() {
        let fixture = Fixture::parse(
            "-brand = { subtitle }\nsubtitle = Brand { $case }\nwrapper = { subtitle }\ntitle = { -brand(case: \"gen\") } { wrapper }\nonly-term = { -brand(case: \"gen\") }\n",
        );
        // Reached in term scope first, then again through `wrapper` in caller scope.
        let collected = fixture.message("title", false);
        assert_eq!(sorted(&collected.variables), ["case"]);
        assert_eq!(
            sorted(&collected.referenced_messages),
            ["subtitle", "wrapper"]
        );

        let collected = fixture.message("only-term", false);
        assert!(collected.variables.is_empty());
        assert_eq!(sorted(&collected.referenced_messages), ["subtitle"]);
    }

    #[test]
    fn term_attribute_reference() {
        let fixture = Fixture::parse(
            "-brand = Brand\n    .gender = { $g }\nmsg = { -brand.gender ->\n    [f] She\n   *[m] He\n}\n",
        );
        assert!(fixture.message("msg", false).variables.is_empty());
    }

    #[test]
    fn term_parameters() {
        let fixture =
            Fixture::parse("-inner = { $unit }\n-outer = { $case } { -inner(unit: \"x\") }\n");
        assert_eq!(sorted(&fixture.term("outer").variables), ["case"]);
        assert_eq!(sorted(&fixture.term("inner").variables), ["unit"]);
    }

    #[test]
    fn message_attribute_reference_follows_only_that_attribute() {
        let fixture = Fixture::parse(
            "b = B { $value }\n    .title = Title { $x }\n    .other = { $y }\na = See { b.title }\n",
        );
        let collected = fixture.message("a", false);
        assert_eq!(sorted(&collected.variables), ["x"]);
        assert_eq!(sorted(&collected.referenced_messages), ["b"]);
    }

    #[test]
    fn message_reference_follows_only_the_value() {
        let fixture =
            Fixture::parse("b = B { $value }\n    .title = Title { $x }\na = See { b }\n");
        assert_eq!(sorted(&fixture.message("a", false).variables), ["value"]);
    }

    #[test]
    fn unknown_attribute_of_a_known_message_contributes_nothing() {
        let fixture = Fixture::parse("b = B { $value }\na = See { b.missing }\n");
        let collected = fixture.message("a", false);
        assert!(collected.variables.is_empty());
        assert_eq!(sorted(&collected.referenced_messages), ["b"]);
        assert!(collected.unknown_references.is_empty());
    }

    #[test]
    fn own_attributes_are_an_option() {
        let fixture = Fixture::parse("b = B { $value }\n    .title = Title { $x }\n");
        assert_eq!(sorted(&fixture.message("b", false).variables), ["value"]);
        assert_eq!(
            sorted(&fixture.message("b", true).variables),
            ["value", "x"]
        );
    }

    #[test]
    fn own_attributes_of_a_valueless_message() {
        let fixture = Fixture::parse("b =\n    .title = Title { $x }\n");
        assert!(fixture.message("b", false).variables.is_empty());
        assert_eq!(sorted(&fixture.message("b", true).variables), ["x"]);
    }

    #[test]
    fn message_to_message_references() {
        let fixture = Fixture::parse("c = C { $z }\nb = B { $y } { c }\na = A { $x } { b }\n");
        let collected = fixture.message("a", false);
        assert_eq!(sorted(&collected.variables), ["x", "y", "z"]);
        assert_eq!(sorted(&collected.referenced_messages), ["b", "c"]);
    }

    #[test]
    fn reference_cycles_terminate() {
        let fixture = Fixture::parse(
            "a = { $x } { b }\nb = { $y } { a }\nself-ref = { self-ref } { $z }\n-t1 = { -t2 }\n-t2 = { -t1 } { $w }\nuses-terms = { -t1 }\n",
        );
        let collected = fixture.message("a", false);
        assert_eq!(sorted(&collected.variables), ["x", "y"]);
        assert_eq!(sorted(&collected.referenced_messages), ["a", "b"]);

        assert_eq!(sorted(&fixture.message("self-ref", false).variables), ["z"]);
        assert!(fixture.message("uses-terms", false).variables.is_empty());
        assert!(fixture.term("t1").variables.is_empty());
        assert_eq!(sorted(&fixture.term("t2").variables), ["w"]);
    }

    #[test]
    fn unknown_references_are_reported_in_order() {
        let fixture = Fixture::parse("a = { missing } { -gone } { $x }\n");
        let collected = fixture.message("a", false);
        assert_eq!(sorted(&collected.variables), ["x"]);
        assert_eq!(sorted(&collected.referenced_messages), ["missing"]);
        assert_eq!(
            collected.unknown_references,
            vec![
                UnknownReference::Message("missing".to_string()),
                UnknownReference::Term("gone".to_string()),
            ]
        );
    }

    #[test]
    fn valueless_message_without_attributes_has_no_variables() {
        let fixture = Fixture::parse("b =\n    .title = T\na = { b }\n");
        let collected = fixture.message("a", false);
        assert!(collected.variables.is_empty());
        assert_eq!(sorted(&collected.referenced_messages), ["b"]);
    }
}
