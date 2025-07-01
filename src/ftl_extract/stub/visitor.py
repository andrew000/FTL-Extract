from __future__ import annotations

from dataclasses import dataclass, field

from fluent.syntax import ast
from fluent.syntax.visitor import Visitor


class FTLStubCantFindMessageReferenceError(Exception): ...


class FTLStubCantFindTermError(Exception): ...


@dataclass
class Message:
    fluent_message: ast.Message
    kwargs: list[str] = field(default_factory=list)


@dataclass
class Term:
    fluent_term: ast.Term
    kwargs: list[str] = field(default_factory=list)


class FluentVisitor(Visitor):
    def __init__(self) -> None:
        self.messages: dict[str, Message] = {}  # { key: Message }
        self.terms: dict[str, Term] = {}

    @staticmethod
    def _extract_kwargs_from_variable_reference(
        *,
        obj: Message | Term,
        variable_reference: ast.VariableReference,
    ) -> None:
        obj.kwargs.append(variable_reference.id.name)

    def _extract_kwargs_from_selector_expression(
        self,
        *,
        obj: Message | Term,
        selector_expression: ast.SelectExpression,
    ) -> None:
        if isinstance(selector_expression.selector, ast.VariableReference):
            self._extract_kwargs_from_variable_reference(
                obj=obj,
                variable_reference=selector_expression.selector,
            )

        for variant in selector_expression.variants:
            for element in variant.value.elements:
                if isinstance(element, ast.Placeable):
                    self._extract_kwargs_from_placeable(
                        obj=obj,
                        placeable=element,
                    )

    def _extract_kwargs_from_message_reference(
        self,
        *,
        obj: Message | Term,
        message_reference: ast.MessageReference,
    ) -> None:
        reference_message = self.messages.get(message_reference.id.name, None)

        if not reference_message:
            raise FTLStubCantFindMessageReferenceError

        obj.kwargs.extend(self._extract_kwargs_from_message(message=reference_message))

    def _extract_kwargs_from_term_reference(
        self,
        *,
        obj: Message | Term,
        term_reference: ast.TermReference,
    ) -> None:
        term = self.terms.get(term_reference.id.name, None)

        if not term:
            raise FTLStubCantFindTermError

        obj.kwargs.extend(self._extract_kwargs_from_term(term=term))

    def _extract_kwargs_from_placeable(
        self,
        *,
        obj: Message | Term,
        placeable: ast.Placeable,
    ) -> None:
        expression = placeable.expression

        if isinstance(expression, ast.VariableReference):
            self._extract_kwargs_from_variable_reference(
                obj=obj,
                variable_reference=expression,
            )

        elif isinstance(expression, ast.SelectExpression):
            self._extract_kwargs_from_selector_expression(
                obj=obj,
                selector_expression=expression,
            )

        elif isinstance(expression, ast.MessageReference):
            self._extract_kwargs_from_message_reference(
                obj=obj,
                message_reference=expression,
            )

        elif isinstance(expression, ast.TermReference):
            self._extract_kwargs_from_term_reference(
                obj=obj,
                term_reference=expression,
            )

    def _extract_kwargs_from_message(self, message: Message) -> list[str]:
        if not message.fluent_message.value:
            return []

        for element in message.fluent_message.value.elements:
            if isinstance(element, ast.Placeable):
                self._extract_kwargs_from_placeable(
                    obj=message,
                    placeable=element,
                )

        return message.kwargs

    def _extract_kwargs_from_term(self, term: Term) -> list[str]:
        if not term.fluent_term.value:
            return []

        for element in term.fluent_term.value.elements:
            if isinstance(element, ast.Placeable):
                self._extract_kwargs_from_placeable(
                    obj=term,
                    placeable=element,
                )

        return term.kwargs

    def visit_Message(self, fluent_message: ast.Message) -> None:  # noqa: N802
        message = self.messages.setdefault(
            fluent_message.id.name,
            Message(fluent_message=fluent_message),
        )

        if not fluent_message.value:
            return self.generic_visit(fluent_message)

        self._extract_kwargs_from_message(message)

        return self.generic_visit(fluent_message)

    def visit_Term(self, fluent_term: ast.Term) -> None:  # noqa: N802
        term = self.terms.setdefault(fluent_term.id.name, Term(fluent_term=fluent_term))

        if not fluent_term.value:
            return self.generic_visit(fluent_term)

        self._extract_kwargs_from_term(term)

        return self.generic_visit(fluent_term)
