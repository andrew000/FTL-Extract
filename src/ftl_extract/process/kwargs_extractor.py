from __future__ import annotations

from typing import TYPE_CHECKING, cast

from fluent.syntax import ast

from ftl_extract.exceptions import FTLExtractorCantFindReferenceError, FTLExtractorCantFindTermError

if TYPE_CHECKING:
    from ftl_extract.matcher import FluentKey


def _extract_kwargs_from_variable_reference(
    *,
    variable_reference: ast.VariableReference,
    kwargs: set[str],
) -> None:
    kwargs.add(variable_reference.id.name)


def _extract_kwargs_from_selector_expression(
    *,
    key: FluentKey,
    selector_expression: ast.SelectExpression,
    kwargs: set[str],
    terms: dict[str, FluentKey],
    all_fluent_keys: dict[str, FluentKey],
    depend_keys: set[str] | None = None,
) -> None:
    if isinstance(selector_expression.selector, ast.VariableReference):
        _extract_kwargs_from_variable_reference(
            variable_reference=selector_expression.selector,
            kwargs=kwargs,
        )

    for variant in selector_expression.variants:
        for element in variant.value.elements:
            if isinstance(element, ast.Placeable):
                _extract_kwargs_from_placeable(
                    key=key,
                    placeable=element,
                    kwargs=kwargs,
                    terms=terms,
                    all_fluent_keys=all_fluent_keys,
                    depend_keys=depend_keys,
                )


def _extract_kwargs_from_message_reference(
    *,
    key: FluentKey,
    message_reference: ast.MessageReference,
    kwargs: set[str],
    terms: dict[str, FluentKey],
    all_fluent_keys: dict[str, FluentKey],
    depend_keys: set[str] | None = None,
) -> None:
    reference_key = all_fluent_keys.get(message_reference.id.name, None)

    if not reference_key:
        raise FTLExtractorCantFindReferenceError(
            key=key.key,
            key_path=key.path,
            reference_key=message_reference.id.name,
        )

    kwargs.update(
        extract_kwargs(
            key=reference_key,
            terms=terms,
            all_fluent_keys=all_fluent_keys,
            depend_keys=depend_keys,
        ),
    )


def _extract_kwargs_from_term_reference(
    *,
    key: FluentKey,
    term_expression: ast.TermReference,
    kwargs: set[str],
    terms: dict[str, FluentKey],
    all_fluent_keys: dict[str, FluentKey],
) -> None:
    term = terms.get(term_expression.id.name, None)

    if not term:
        raise FTLExtractorCantFindTermError(
            key=key.key,
            locale=cast(str, key.locale),
            key_path=key.path,
            term_key=term_expression.id.name,
        )

    kwargs.update(extract_kwargs(key=term, terms=terms, all_fluent_keys=all_fluent_keys))


def _extract_kwargs_from_placeable(
    *,
    key: FluentKey,
    placeable: ast.Placeable,
    kwargs: set[str],
    terms: dict[str, FluentKey],
    all_fluent_keys: dict[str, FluentKey],
    depend_keys: set[str] | None = None,
) -> None:
    expression = placeable.expression

    if isinstance(expression, ast.VariableReference):
        _extract_kwargs_from_variable_reference(variable_reference=expression, kwargs=kwargs)

    elif isinstance(expression, ast.SelectExpression):
        _extract_kwargs_from_selector_expression(
            key=key,
            selector_expression=expression,
            kwargs=kwargs,
            terms=terms,
            all_fluent_keys=all_fluent_keys,
            depend_keys=depend_keys,
        )

    elif isinstance(expression, ast.MessageReference):
        # Add `ast.MessageReference.id.name` to depends_on_keys
        # to avoid key to be removed
        key.depends_on_keys.add(expression.id.name)
        if depend_keys is not None:
            depend_keys.add(expression.id.name)

        # Extract kwargs
        _extract_kwargs_from_message_reference(
            key=key,
            message_reference=expression,
            kwargs=kwargs,
            terms=terms,
            all_fluent_keys=all_fluent_keys,
            depend_keys=depend_keys,
        )

    elif isinstance(expression, ast.TermReference):
        _extract_kwargs_from_term_reference(
            key=key,
            term_expression=expression,
            kwargs=kwargs,
            terms=terms,
            all_fluent_keys=all_fluent_keys,
        )


def extract_kwargs(
    *,
    key: FluentKey,
    terms: dict[str, FluentKey] | None = None,
    all_fluent_keys: dict[str, FluentKey] | None = None,
    depend_keys: set[str] | None = None,
) -> set[str]:
    kwargs: set[str] = set()
    terms = terms or {}
    all_fluent_keys = all_fluent_keys or {}
    depend_keys = depend_keys if depend_keys is not None else set()

    if not isinstance(key.translation, (ast.Message, ast.Term)):
        return kwargs

    if not key.translation.value:
        return kwargs

    for element in key.translation.value.elements:
        if isinstance(element, ast.Placeable):
            _extract_kwargs_from_placeable(
                key=key,
                placeable=element,
                kwargs=kwargs,
                terms=terms,
                all_fluent_keys=all_fluent_keys,
                depend_keys=depend_keys,
            )

    return kwargs
